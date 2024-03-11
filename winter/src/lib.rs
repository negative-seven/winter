use anyhow::Result;
use shared::{
    communication::{self, new_transceiver_pair, HooksMessage, RuntimeTransceiver},
    pipe, process, thread,
};
use std::{
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};
use thiserror::Error;

pub struct Runtime {
    process: process::Process,
    running: Arc<AtomicBool>,
    stdout_thread: Option<JoinHandle<()>>,
    transceiver: RuntimeTransceiver,
}

impl Runtime {
    pub fn new<F>(
        executable_path: impl AsRef<str>,
        injected_dll_path: impl AsRef<str>,
        stdout_callback: Option<F>,
    ) -> Result<Self, NewError>
    where
        F: Fn(&[u8]) + Send + 'static,
    {
        let injected_dll_name = std::path::Path::new(injected_dll_path.as_ref())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(); // TODO: handle errors
        let subprocess_is_running = Arc::new(AtomicBool::new(true));

        let (stdout_pipe_writer, mut stdout_pipe_reader) = pipe::new()?;
        let stdout_thread = {
            let stdout_callback = stdout_callback.map(|f| Box::new(f));
            let subprocess_is_running = Arc::clone(&subprocess_is_running);
            std::thread::spawn(move || {
                let mut check_stdout = || {
                    let mut stdout = Vec::new();
                    stdout_pipe_reader.read_to_end(&mut stdout).unwrap();
                    if !stdout.is_empty() {
                        stdout_callback.as_ref().inspect(|f| f(&stdout));
                    }
                };

                while subprocess_is_running.load(Ordering::Relaxed) {
                    check_stdout();
                    std::thread::sleep(Duration::from_millis(1));
                }
                check_stdout();
            })
        };

        // hooks_transceiver must be created before the process, so that its handles can
        // be inherited
        let (mut transceiver, hooks_transceiver) = new_transceiver_pair()?;

        let subprocess = process::Process::create(
            executable_path.as_ref(),
            true,
            None,
            Some(stdout_pipe_writer),
            None,
        )?;
        subprocess.inject_dll(injected_dll_path.as_ref())?;

        let serialized_hooks_transceiver = unsafe { hooks_transceiver.leak_to_bytes() };
        let serialized_hooks_transceiver_pointer = subprocess
            .allocate_read_write_memory(serialized_hooks_transceiver.len())
            .map_err(NewError::ProcessAllocate)?;
        subprocess
            .write(
                serialized_hooks_transceiver_pointer,
                &serialized_hooks_transceiver,
            )
            .map_err(NewError::ProcessWrite)?;
        subprocess
            .create_thread(
                subprocess.get_export_address(injected_dll_name, "initialize")?,
                false,
                Some(serialized_hooks_transceiver_pointer as _),
            )
            .map_err(NewError::ThreadCreate)?;

        match transceiver.receive_blocking()? {
            HooksMessage::HooksInitialized => (),
            message => return Err(UnexpectedMessageError::new(message).into()),
        }

        Ok(Self {
            process: subprocess,
            running: subprocess_is_running,
            stdout_thread: Some(stdout_thread),
            transceiver,
        })
    }

    pub fn resume(&self) -> Result<(), RuntimeError> {
        for thread in self
            .process
            .iter_thread_ids()?
            .map(thread::Thread::from_id)
            .collect::<Result<Vec<_>, _>>()?
        {
            thread.resume()?;
        }

        Ok(())
    }

    pub fn wait_until_exit(&mut self) -> Result<(), WaitUntilExitError> {
        self.process.join()?;
        self.running.store(false, Ordering::Relaxed);
        if let Some(stdout_thread) = self.stdout_thread.take() {
            if let Err(e) = stdout_thread.join() {
                std::panic::resume_unwind(e);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("failed to create winter runtime")]
pub enum NewError {
    NewPipe(#[from] pipe::NewError),
    NewTransceiverPair(#[from] communication::NewTransceiverPairError),
    ProcessCreate(#[from] process::CreateError),
    InjectDll(#[from] process::InjectDllError),
    ProcessAllocate(#[source] std::io::Error),
    ProcessWrite(#[source] std::io::Error),
    GetExportAddress(#[from] process::GetExportAddressError),
    ThreadCreate(#[source] std::io::Error),
    TransceiverRead(#[from] communication::ReceiveError),
    UnexpectedMessage(#[from] UnexpectedMessageError),
}

#[derive(Debug, Error)]
#[error("unexpected message received: {message:?}")]
pub struct UnexpectedMessageError {
    message: HooksMessage,
}

impl UnexpectedMessageError {
    #[must_use]
    pub fn new(message: HooksMessage) -> Self {
        Self { message }
    }
}

#[derive(Debug, Error)]
#[error("error in winter runtime")]
pub enum RuntimeError {
    IterThreadIds(#[from] process::IterThreadIdsError),
    ThreadFromId(#[from] thread::FromIdError),
    ThreadResume(#[from] thread::ResumeError),
}

#[derive(Debug, Error)]
#[error("error occurred while waiting for the winter runtime to exit")]
pub enum WaitUntilExitError {
    ProcessJoinError(#[from] process::JoinError),
}
