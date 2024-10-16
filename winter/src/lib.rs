mod saved_state;

use saved_state::SavedState;
pub use shared::communication::MouseButton;

use anyhow::Result;
use shared::{
    communication::{self, ConductorInitialMessage, ConductorMessage, HooksMessage, LogLevel},
    event::{self, ManualResetEvent},
    pipe,
    process::{self, CheckIs64BitError},
    thread,
};
use std::{
    ffi::OsStr,
    io::{self, Read},
    path::Path,
    time::Duration,
};
use thiserror::Error;
use tokio::select;

pub struct Conductor {
    process: process::Process,
    #[expect(clippy::type_complexity)]
    stdout_callback: Option<Box<dyn Fn(&[u8]) + Send>>,
    stdout_pipe_reader: pipe::Reader,
    message_sender: communication::Sender<ConductorMessage>,
    idle: ManualResetEvent,
    receive_messages_task: Option<tokio::task::JoinHandle<()>>,
    saved_state: Option<SavedState>,
}

impl Conductor {
    pub async fn new<F>(
        executable_path: impl AsRef<Path>,
        command_line_string: impl AsRef<OsStr>,
        stdout_callback: Option<F>,
    ) -> Result<Self, NewError>
    where
        F: Fn(&[u8]) + Send + 'static,
    {
        let (stdout_pipe_writer, stdout_pipe_reader) = pipe::new()?;

        // sender/receiver pairs must be created before the process, so that their
        // handles can be inherited
        let (conductor_sender, hooks_receiver) = communication::new_sender_and_receiver()?;
        let (hooks_sender, mut conductor_receiver) = communication::new_sender_and_receiver()?;

        let subprocess = process::Process::create(
            executable_path.as_ref(),
            command_line_string,
            true,
            None,
            Some(stdout_pipe_writer),
            None,
        )?;
        subprocess.kill_on_current_process_exit()?;
        let main_thread = thread::Thread::from_id(
            subprocess
                .iter_thread_ids()?
                .next()
                .expect("no threads in subprocess"),
        )?;

        let hooks_library = if subprocess.is_64_bit()? {
            "hooks64.dll"
        } else {
            "hooks32.dll"
        };
        subprocess.inject_dll(hooks_library).await?;

        let initial_message = bincode::serialize(&ConductorInitialMessage {
            main_thread_id: main_thread.get_id()?,
            serialized_message_sender: unsafe { hooks_sender.leak_to_bytes() },
            serialized_message_receiver: unsafe { hooks_receiver.leak_to_bytes() },
        })?;
        let initial_message_pointer = subprocess.allocate_memory(
            initial_message.len(),
            process::MemoryPermissions {
                rwe: process::MemoryPermissionsRwe::ReadWrite,
                is_guard: false,
            },
        )?;
        subprocess.write(initial_message_pointer, &initial_message)?;
        subprocess.create_thread(
            subprocess.get_export_address(hooks_library, "initialize")?,
            false,
            Some(initial_message_pointer as _),
        )?;

        match conductor_receiver.receive().await? {
            HooksMessage::HooksInitialized => (),
            message => return Err(UnexpectedMessageError::new(message).into()),
        }

        let idle = ManualResetEvent::new()?;
        let receive_messages_task = {
            let mut idle = idle.try_clone().unwrap();
            tokio::spawn(async move {
                loop {
                    match conductor_receiver.receive().await.unwrap() {
                        HooksMessage::Idle => idle.set().unwrap(),
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
            message_sender: conductor_sender,
            idle,
            receive_messages_task: Some(receive_messages_task),
            saved_state: None,
        })
    }

    pub async fn resume(&mut self) -> Result<(), ResumeError> {
        self.message_sender.send(&ConductorMessage::Resume).await?;
        Ok(())
    }

    pub async fn set_key_state(&mut self, id: u8, state: bool) -> Result<(), SetKeyStateError> {
        self.message_sender
            .send(&ConductorMessage::SetKeyState { id, state })
            .await?;
        Ok(())
    }

    pub async fn set_mouse_position(
        &mut self,
        x: u16,
        y: u16,
    ) -> Result<(), SetMousePositionError> {
        self.message_sender
            .send(&ConductorMessage::SetMousePosition { x, y })
            .await?;
        Ok(())
    }

    pub async fn set_mouse_button_state(
        &mut self,
        button: MouseButton,
        state: bool,
    ) -> Result<(), SetMouseButtonStateError> {
        self.message_sender
            .send(&ConductorMessage::SetMouseButtonState { button, state })
            .await?;
        Ok(())
    }

    pub async fn advance_time(&mut self, time: Duration) -> Result<(), AdvanceTimeError> {
        self.message_sender
            .send(&ConductorMessage::AdvanceTime(time))
            .await?;
        Ok(())
    }

    pub async fn save_state(&mut self) -> Result<(), SaveStateError> {
        self.wait_until_inactive().await?;
        self.saved_state = Some(SavedState::new(&self.process)?);
        Ok(())
    }

    pub async fn load_state(&mut self) -> Result<(), LoadStateError> {
        self.wait_until_inactive().await?;
        if let Some(state) = &self.saved_state {
            state.load(&self.process)?;
        } else {
            panic!("no damn state");
        }
        Ok(())
    }

    pub async fn wait_until_inactive(&mut self) -> Result<InactiveState, WaitUntilInactiveError> {
        self.idle.reset()?;
        let mut stdout = Vec::new();
        let state = select! {
            result = async {
                self.message_sender
                    .send(&ConductorMessage::IdleRequest)
                    .await?;
                self.idle.wait().await?;
                Ok::<_, WaitUntilInactiveError>(())
            } => {
                result?;
                InactiveState::Idle
            }
            result = self.process.join() => {
                let exit_code = result?;
                if let Some(task) = self.receive_messages_task.take() {
                    task.abort();
                }
                InactiveState::Terminated { exit_code }
            }
            error = async {
                loop {
                    // stdout is read in a loop with a sleep, as there appears to be no way
                    // to await a signal indicating that stdout has just been written to
                    if let Err(err) = self.stdout_pipe_reader.read_to_end(&mut stdout) {
                        return err;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            } => {
                return Err(error.into());
            }
        };

        self.stdout_pipe_reader.read_to_end(&mut stdout).unwrap();
        if !stdout.is_empty() {
            self.stdout_callback.as_ref().inspect(|f| f(&stdout));
        }

        Ok(state)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum InactiveState {
    Idle,
    Terminated { exit_code: u32 },
}

#[derive(Debug, Error)]
#[error("unexpected message received: {message:?}")]
pub struct UnexpectedMessageError {
    message: HooksMessage,
}

#[derive(Debug, Error)]
#[error("failed to create conductor")]
pub enum NewError {
    NewPipe(#[from] pipe::NewError),
    NewSenderAndReceiver(#[from] communication::NewSenderAndReceiverError),
    MessageSenderClone(#[from] communication::SenderCloneError),
    ProcessCreate(#[from] process::CreateError),
    ThreadFromId(#[from] thread::FromIdError),
    ThreadGetId(#[from] thread::GetIdError),
    CheckIs64Bit(#[from] CheckIs64BitError),
    KillOnCurrentProcessExit(#[from] process::KillOnCurrentProcessExitError),
    InjectDll(#[from] process::InjectDllError),
    ProcessAllocateMemory(#[from] process::AllocateMemoryError),
    ProcessWriteMemory(#[from] process::WriteMemoryError),
    GetExportAddress(#[from] process::GetExportAddressError),
    NewEvent(#[from] event::NewError),
    ProcessCreateTHread(#[from] process::CreateThreadError),
    MessageReceive(#[from] communication::ReceiveError),
    UnexpectedMessage(#[from] UnexpectedMessageError),
    Bincode(#[from] bincode::Error),
    IterThreadIds(#[from] process::IterThreadIdsError),
}

impl UnexpectedMessageError {
    #[must_use]
    pub fn new(message: HooksMessage) -> Self {
        Self { message }
    }
}

#[derive(Debug, Error)]
#[error("error occurred while resuming")]
pub enum ResumeError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while setting key state")]
pub enum SetKeyStateError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while setting key state")]
pub enum SetMousePositionError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while setting key state")]
pub enum SetMouseButtonStateError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while advancing time")]
pub enum AdvanceTimeError {
    MessageSend(#[from] communication::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while saving state")]
pub enum SaveStateError {
    WaitUntilInactive(#[from] WaitUntilInactiveError),
    ThreadFromId(#[from] thread::FromIdError),
    NewSavedState(#[from] saved_state::NewError),
}

#[derive(Debug, Error)]
#[error("error occurred while loading state")]
pub enum LoadStateError {
    WaitUntilInactive(#[from] WaitUntilInactiveError),
    SavedStateLoad(#[from] saved_state::LoadError),
}

#[derive(Debug, Error)]
#[error("error occurred while waiting for the subprocess to become inactive")]
pub enum WaitUntilInactiveError {
    EventWait(#[from] event::WaitError),
    EventReset(#[from] event::ResetError),
    ProcessJoin(#[from] process::JoinError),
    MessageSend(#[from] communication::SendError),
    Os(#[from] io::Error),
}
