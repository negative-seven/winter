use anyhow::Result;
use thiserror::Error;
use windows::{process, thread};

pub struct Runtime {
    executable_path: String,
    injected_dll_path: String,
}

impl Runtime {
    pub fn new(executable_path: impl AsRef<str>, injected_dll_path: impl AsRef<str>) -> Self {
        Self {
            executable_path: executable_path.as_ref().to_string(),
            injected_dll_path: injected_dll_path.as_ref().to_string(),
        }
    }

    pub fn start(&self) -> Result<(), RuntimeError> {
        let injected_dll_name = std::path::Path::new(&self.injected_dll_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(); // TODO: handle errors

        let process = process::Process::create(&self.executable_path, true)?;
        process.inject_dll(&self.injected_dll_path)?;

        let initialize_function = process.get_export_address(injected_dll_name, "initialize")?;
        process
            .create_thread(initialize_function, false, None)
            .map_err(RuntimeError::ThreadCreate)?
            .join()?;

        for thread in process
            .iter_thread_ids()?
            .map(windows::thread::Thread::from_id)
            .collect::<Result<Vec<_>, _>>()?
        {
            thread.resume()?;
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("error in winter runtime")]
pub enum RuntimeError {
    ProcessCreate(#[from] process::CreateError),
    InjectDll(#[from] process::InjectDllError),
    GetExportAddress(#[from] process::GetExportAddressError),
    ThreadCreate(#[source] std::io::Error),
    IterThreadIds(#[from] process::IterThreadIdsError),
    ThreadFromId(#[from] thread::FromIdError),
    ThreadResume(#[from] thread::ResumeError),
    ThreadJoin(#[from] thread::JoinError),
}
