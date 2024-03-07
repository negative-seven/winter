use anyhow::Result;
use std::{
    collections::VecDeque,
    io::{BufRead, Read},
};
use thiserror::Error;
use windows::{pipe, process, thread};

pub struct Runtime {
    process: process::Process,
    stdout: Stdout,
}

impl Runtime {
    pub fn new(
        executable_path: impl AsRef<str>,
        injected_dll_path: impl AsRef<str>,
    ) -> Result<Self, NewError> {
        let injected_dll_name = std::path::Path::new(injected_dll_path.as_ref())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(); // TODO: handle errors

        let (stdout_pipe_writer, stdout_pipe_reader) = pipe::new()?;

        let process = process::Process::create(
            executable_path.as_ref(),
            true,
            None,
            Some(&stdout_pipe_writer),
            None,
        )?;
        process.inject_dll(injected_dll_path.as_ref())?;

        let initialize_function = process.get_export_address(injected_dll_name, "initialize")?;
        process
            .create_thread(initialize_function, false, None)
            .map_err(NewError::ThreadCreate)?
            .join()?;

        Ok(Self {
            process,
            stdout: Stdout::new(stdout_pipe_reader),
        })
    }

    #[must_use]
    pub fn stdout_mut(&mut self) -> &mut Stdout {
        &mut self.stdout
    }

    pub fn resume(&self) -> Result<(), RuntimeError> {
        for thread in self
            .process
            .iter_thread_ids()?
            .map(windows::thread::Thread::from_id)
            .collect::<Result<Vec<_>, _>>()?
        {
            thread.resume()?;
        }

        Ok(())
    }

    pub fn wait_until_exit(&self) -> Result<(), WaitUntilExitError> {
        self.process.join()?;
        Ok(())
    }
}

pub struct Stdout {
    reader: pipe::Reader,
    buffer: VecDeque<u8>,
}

impl Stdout {
    pub(crate) fn new(reader: pipe::Reader) -> Self {
        Self {
            reader,
            buffer: VecDeque::new(),
        }
    }
}

impl Read for Stdout {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

impl BufRead for Stdout {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let mut new_bytes = Vec::new();
        self.reader.read_to_end(&mut new_bytes)?;
        self.buffer.extend(new_bytes.iter());
        Ok(self.buffer.as_slices().0)
    }

    fn consume(&mut self, amt: usize) {
        self.buffer = self.buffer.split_off(amt);
    }
}

#[derive(Debug, Error)]
#[error("failed to create winter runtime")]
pub enum NewError {
    NewPipe(#[from] pipe::NewError),
    ProcessCreate(#[from] process::CreateError),
    InjectDll(#[from] process::InjectDllError),
    GetExportAddress(#[from] process::GetExportAddressError),
    ThreadCreate(#[source] std::io::Error),
    ThreadJoin(#[from] thread::JoinError),
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
