use anyhow::Result;
use shared::{
    communication::{self, HooksMessage, LogLevel, RuntimeMessage},
    event::{self, ManualResetEvent},
    pipe, process, thread,
};
use std::{io::Read, thread::JoinHandle, time::Duration};
use thiserror::Error;

pub struct Runtime {
    process: process::Process,
    #[allow(clippy::type_complexity)]
    stdout_callback: Option<Box<dyn Fn(&[u8]) + Send>>,
    stdout_pipe_reader: pipe::Reader,
    message_sender: communication::Sender<RuntimeMessage>,
    message_self_sender: communication::Sender<HooksMessage>,
    idle: ManualResetEvent,
    receive_messages_thread: Option<JoinHandle<()>>,
}

impl Runtime {
    pub fn new<F>(
        executable_path: impl AsRef<str>,
        stdout_callback: Option<F>,
    ) -> Result<Self, NewError>
    where
        F: Fn(&[u8]) + Send + 'static,
    {
        let (stdout_pipe_writer, stdout_pipe_reader) = pipe::new()?;

        // sender/receiver pairs must be created before the process, so that their
        // handles can be inherited
        let (runtime_sender, hooks_receiver) = communication::new_sender_and_receiver()?;
        let (hooks_sender, mut runtime_receiver) = communication::new_sender_and_receiver()?;
        let runtime_self_sender = hooks_sender.try_clone()?;

        let subprocess = process::Process::create(
            executable_path.as_ref(),
            true,
            None,
            Some(stdout_pipe_writer),
            None,
        )?;
        subprocess.inject_dll("hooks32.dll")?;

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
                subprocess.get_export_address("hooks32.dll", "initialize")?,
                false,
                Some(serialized_hooks_sender_and_receiver_pointer as _),
            )
            .map_err(NewError::ThreadCreate)?;

        match runtime_receiver.receive_blocking()? {
            HooksMessage::HooksInitialized => (),
            message => return Err(UnexpectedMessageError::new(message).into()),
        }

        let idle = ManualResetEvent::new()?;
        let receive_messages_thread = {
            let mut idle = idle.try_clone().unwrap();
            std::thread::spawn(move || loop {
                match runtime_receiver.receive_blocking().unwrap() {
                    HooksMessage::HooksInitialized => todo!(),
                    HooksMessage::Idle => idle.set().unwrap(),
                    HooksMessage::Stop => break,
                    HooksMessage::Log { level, message } => {
                        match level {
                            LogLevel::Trace => tracing::trace!(target: "hooks", message),
                            LogLevel::Debug => tracing::debug!(target: "hooks", message),
                            LogLevel::Info => tracing::info!(target: "hooks", message),
                            LogLevel::Warning => tracing::warn!(target: "hooks", message),
                            LogLevel::Error => tracing::error!(target: "hooks", message),
                        };
                    }
                    message => unimplemented!("handle message {message:?}"),
                }
            })
        };

        Ok(Self {
            process: subprocess,
            stdout_callback: match stdout_callback {
                Some(stdout_callback) => Some(Box::new(*Box::new(stdout_callback))),
                None => todo!(),
            },
            stdout_pipe_reader,
            message_sender: runtime_sender,
            message_self_sender: runtime_self_sender,
            idle,
            receive_messages_thread: Some(receive_messages_thread),
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

    pub fn set_key_state(&mut self, id: u8, state: bool) -> Result<(), SetKeyStateError> {
        self.message_sender
            .send(&RuntimeMessage::SetKeyState { id, state })?;
        Ok(())
    }

    pub fn advance_time(&mut self, time: Duration) -> Result<(), AdvanceTimeError> {
        self.message_sender
            .send(&RuntimeMessage::AdvanceTime(time))?;
        Ok(())
    }

    pub fn wait_until_idle(&mut self) -> Result<(), WaitUntilIdleError> {
        self.idle.reset()?;
        self.message_sender.send(&RuntimeMessage::IdleRequest)?;
        self.idle.wait()?;
        self.check_stdout();
        Ok(())
    }

    pub fn wait_until_exit(&mut self) -> Result<(), WaitUntilExitError> {
        self.process.join()?;
        self.check_stdout();
        self.message_self_sender.send(&HooksMessage::Stop)?;
        if let Some(thread) = self.receive_messages_thread.take() {
            thread.join().map_err(|_| WaitUntilExitError::ThreadJoin)?;
        }
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
    MessageSenderClone(#[from] communication::SenderCloneError),
    ProcessCreate(#[from] process::CreateError),
    InjectDll(#[from] process::InjectDllError),
    ProcessAllocate(#[source] std::io::Error),
    ProcessWrite(#[source] std::io::Error),
    GetExportAddress(#[from] process::GetExportAddressError),
    NewEvent(#[from] event::NewError),
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
#[error("error occurred while setting key state")]
pub enum SetKeyStateError {
    TransceiverSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while advancing time")]
pub enum AdvanceTimeError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while waiting for the subprocess to become idle")]
pub enum WaitUntilIdleError {
    EventGet(#[from] event::GetError),
    EventReset(#[from] event::ResetError),
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while waiting for the subprocess to exit")]
pub enum WaitUntilExitError {
    ProcessJoin(#[from] process::JoinError),
    MessageSend(#[from] communication::SendError),
    ThreadJoin,
}
