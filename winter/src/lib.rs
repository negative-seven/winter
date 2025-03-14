mod saved_state;

use anyhow::Result;
use saved_state::SavedState;
use shared::{
    input::MouseButton,
    ipc::{self, message::Message, Sender},
    windows::{
        event, module, pipe,
        process::{self, CheckIs64BitError},
        thread,
    },
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
    message_sender: Sender<ipc::message::FromConductor>,
    receive_log_messages_task: Option<tokio::task::JoinHandle<()>>,
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

        let process = process::Process::get_current();
        let (conductor_sender, hooks_receiver) =
            ipc::new_sender_and_receiver(&process, &subprocess)?;
        let (hooks_initialized_sender, mut conductor_initialized_receiver) =
            ipc::new_sender_and_receiver(&subprocess, &process)?;
        let (hooks_log_sender, mut conductor_log_receiver) =
            ipc::new_sender_and_receiver(&subprocess, &process)?;

        let initial_message = ipc::message::Initial {
            main_thread_id: main_thread.get_id()?,
            initialized_message_sender: hooks_initialized_sender,
            log_message_sender: hooks_log_sender,
            message_receiver: hooks_receiver,
        };
        let initial_message_serialized = unsafe { initial_message.serialize() }.unwrap();
        let initial_message_pointer = subprocess.allocate_memory(
            size_of::<u32>() + initial_message_serialized.len(),
            process::MemoryPermissions {
                rwe: process::MemoryPermissionsRwe::ReadWrite,
                is_guard: false,
            },
        )?;
        subprocess.write(
            initial_message_pointer.cast(),
            &u32::try_from(initial_message_serialized.len())
                .unwrap()
                .to_ne_bytes(),
        )?;
        subprocess.write(
            unsafe { initial_message_pointer.byte_add(size_of::<u32>()).cast() },
            &initial_message_serialized,
        )?;
        unsafe {
            subprocess.create_thread(
                subprocess
                    .get_module(OsStr::new(hooks_library))?
                    .unwrap()
                    .get_export_address("initialize")?
                    .unwrap(),
                false,
                Some(initial_message_pointer.cast()),
            )?;
        }

        let receive_log_messages_task = {
            tokio::spawn(async move {
                loop {
                    let ipc::message::Log { level, message } =
                        conductor_log_receiver.receive().await.unwrap();
                    match level {
                        ipc::message::LogLevel::Trace => tracing::trace!(target: "hooks", message),
                        ipc::message::LogLevel::Debug => tracing::debug!(target: "hooks", message),
                        ipc::message::LogLevel::Info => tracing::info!(target: "hooks", message),
                        ipc::message::LogLevel::Warning => tracing::warn!(target: "hooks", message),
                        ipc::message::LogLevel::Error => tracing::error!(target: "hooks", message),
                    };
                }
            })
        };

        conductor_initialized_receiver.receive().await?;

        Ok(Self {
            process: subprocess,
            stdout_callback: match stdout_callback {
                Some(stdout_callback) => Some(Box::new(*Box::new(stdout_callback))),
                None => todo!(),
            },
            stdout_pipe_reader,
            message_sender: conductor_sender,
            receive_log_messages_task: Some(receive_log_messages_task),
            saved_state: None,
        })
    }

    pub async fn resume(&mut self) -> Result<(), ResumeError> {
        self.message_sender
            .send(ipc::message::FromConductor::Resume)
            .await?;
        Ok(())
    }

    pub async fn set_key_state(&mut self, id: u8, state: bool) -> Result<(), SetKeyStateError> {
        self.message_sender
            .send(ipc::message::FromConductor::SetKeyState { id, state })
            .await?;
        Ok(())
    }

    pub async fn set_mouse_position(
        &mut self,
        x: u16,
        y: u16,
    ) -> Result<(), SetMousePositionError> {
        self.message_sender
            .send(ipc::message::FromConductor::SetMousePosition { x, y })
            .await?;
        Ok(())
    }

    pub async fn set_mouse_button_state(
        &mut self,
        button: MouseButton,
        state: bool,
    ) -> Result<(), SetMouseButtonStateError> {
        self.message_sender
            .send(ipc::message::FromConductor::SetMouseButtonState { button, state })
            .await?;
        Ok(())
    }

    pub async fn advance_time(&mut self, time: Duration) -> Result<(), AdvanceTimeError> {
        self.message_sender
            .send(ipc::message::FromConductor::AdvanceTime(time))
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
        let mut stdout = Vec::new();
        let state = select! {
            result = async {
                let (response_sender, mut response_receiver) =
                    ipc::new_sender_and_receiver(&self.process, &process::Process::get_current())?;
                self.message_sender
                    .send(ipc::message::FromConductor::IdleRequest { response_sender })
                    .await?;
                response_receiver.receive().await?;
                Ok::<_, WaitUntilInactiveError>(())
            } => {
                result?;
                InactiveState::Idle
            }
            result = self.process.join() => {
                let exit_code = result?;
                if let Some(task) = self.receive_log_messages_task.take() {
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
#[error("failed to create conductor")]
pub enum NewError {
    NewPipe(#[from] pipe::NewError),
    NewSenderAndReceiver(#[from] ipc::NewSenderAndReceiverError),
    MessageSenderClone(#[from] ipc::SenderCloneError),
    ProcessCreate(#[from] process::CreateError),
    ThreadFromId(#[from] thread::FromIdError),
    ThreadGetId(#[from] thread::GetIdError),
    CheckIs64Bit(#[from] CheckIs64BitError),
    KillOnCurrentProcessExit(#[from] process::KillOnCurrentProcessExitError),
    InjectDll(#[from] process::InjectDllError),
    ProcessAllocateMemory(#[from] process::AllocateMemoryError),
    ProcessWriteMemory(#[from] process::WriteMemoryError),
    GetModules(#[from] process::GetModulesError),
    ModuleGetExportAddress(#[from] module::GetExportAddressError),
    NewEvent(#[from] event::NewError),
    ProcessCreateThread(#[from] process::CreateThreadError),
    MessageReceive(#[from] ipc::ReceiveError),
    Bincode(#[from] bincode::Error),
    IterThreadIds(#[from] process::IterThreadIdsError),
}

#[derive(Debug, Error)]
#[error("error occurred while resuming")]
pub enum ResumeError {
    MessageSend(#[from] ipc::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while setting key state")]
pub enum SetKeyStateError {
    MessageSend(#[from] ipc::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while setting key state")]
pub enum SetMousePositionError {
    MessageSend(#[from] ipc::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while setting key state")]
pub enum SetMouseButtonStateError {
    MessageSend(#[from] ipc::SendError),
}

#[derive(Debug, Error)]
#[error("error occurred while advancing time")]
pub enum AdvanceTimeError {
    MessageSend(#[from] ipc::SendError),
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
    ProcessJoin(#[from] process::JoinError),
    NewSenderAndReceiver(#[from] ipc::NewSenderAndReceiverError),
    MessageSend(#[from] ipc::SendError),
    MessageReceive(#[from] ipc::ReceiveError),
    Os(#[from] io::Error),
}
