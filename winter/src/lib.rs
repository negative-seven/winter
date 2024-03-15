use anyhow::Result;
use shared::{
    communication::{self, HooksMessage, RuntimeMessage},
    pipe, process, thread,
};
use std::{io::Read, time::Duration};
use thiserror::Error;

pub struct Runtime {
    process: process::Process,
    #[allow(clippy::type_complexity)]
    stdout_callback: Option<Box<dyn Fn(&[u8]) + Send>>,
    stdout_pipe_reader: pipe::Reader,
    message_sender: communication::Sender<RuntimeMessage>,
    message_receiver: communication::Receiver<HooksMessage>,
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

        let (stdout_pipe_writer, stdout_pipe_reader) = pipe::new()?;

        // sender/receiver pairs must be created before the process, so that their
        // handles can be inherited
        let (runtime_sender, hooks_receiver) = communication::new_sender_and_receiver()?;
        let (hooks_sender, mut runtime_receiver) = communication::new_sender_and_receiver()?;

        let subprocess = process::Process::create(
            executable_path.as_ref(),
            true,
            None,
            Some(stdout_pipe_writer),
            None,
        )?;
        subprocess.inject_dll(injected_dll_path.as_ref())?;

        let serialized_hooks_sender_and_receiver =
            unsafe { [hooks_sender.leak_to_bytes(), hooks_receiver.leak_to_bytes()].concat() };
        let serialized_hooks_sender_and_receiver_pointer = subprocess
            .allocate_read_write_memory(serialized_hooks_sender_and_receiver.len())
            .map_err(NewError::ProcessAllocate)?;
        subprocess
            .write(
                serialized_hooks_sender_and_receiver_pointer,
                &serialized_hooks_sender_and_receiver,
            )
            .map_err(NewError::ProcessWrite)?;
        subprocess
            .create_thread(
                subprocess.get_export_address(injected_dll_name, "initialize")?,
                false,
                Some(serialized_hooks_sender_and_receiver_pointer as _),
            )
            .map_err(NewError::ThreadCreate)?;

        match runtime_receiver.receive_blocking()? {
            HooksMessage::HooksInitialized => (),
            message => return Err(UnexpectedMessageError::new(message).into()),
        }

        Ok(Self {
            process: subprocess,
            stdout_callback: match stdout_callback {
                Some(stdout_callback) => Some(Box::new(*Box::new(stdout_callback))),
                None => todo!(),
            },
            stdout_pipe_reader,
            message_sender: runtime_sender,
            message_receiver: runtime_receiver,
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

    pub fn advance_time(&mut self, time: Duration) -> Result<(), AdvanceTimeError> {
        self.message_sender
            .send(&RuntimeMessage::AdvanceTime(time))?;
        Ok(())
    }

    pub fn wait_until_idle(&mut self) -> Result<(), WaitUntilIdleError> {
        match self.message_receiver.receive_blocking()? {
            HooksMessage::Idle => (),
            message => return Err(UnexpectedMessageError::new(message).into()),
        }
        self.check_stdout();
        Ok(())
    }

    pub fn wait_until_exit(&mut self) -> Result<(), WaitUntilExitError> {
        self.process.join()?;
        self.check_stdout();
        Ok(())
    }

    fn check_stdout(&mut self) {
        let mut stdout = Vec::new();
        self.stdout_pipe_reader.read_to_end(&mut stdout).unwrap();
        if !stdout.is_empty() {
            self.stdout_callback.as_ref().inspect(|f| f(&stdout));
        }
    }
}

#[derive(Debug, Error)]
#[error("unexpected message received: {message:?}")]
pub struct UnexpectedMessageError {
    message: HooksMessage,
}

#[derive(Debug, Error)]
#[error("failed to create winter runtime")]
pub enum NewError {
    NewPipe(#[from] pipe::NewError),
    NewSenderAndReceiver(#[from] communication::NewSenderAndReceiverError),
    ProcessCreate(#[from] process::CreateError),
    InjectDll(#[from] process::InjectDllError),
    ProcessAllocate(#[source] std::io::Error),
    ProcessWrite(#[source] std::io::Error),
    GetExportAddress(#[from] process::GetExportAddressError),
    ThreadCreate(#[source] std::io::Error),
    MessageReceive(#[from] communication::ReceiveError),
    UnexpectedMessage(#[from] UnexpectedMessageError),
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
pub enum AdvanceTimeError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while waiting for the winter runtime to exit")]
pub enum WaitUntilIdleError {
    MessageReceive(#[from] communication::ReceiveError),
    UnexpectedMessage(#[from] UnexpectedMessageError),
}

#[derive(Debug, Error)]
#[error("error occurred while waiting for the winter runtime to exit")]
pub enum WaitUntilExitError {
    ProcessJoinError(#[from] process::JoinError),
}
