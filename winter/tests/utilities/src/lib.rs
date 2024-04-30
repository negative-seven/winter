#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use anyhow::Result;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tracing::info;

#[derive(Clone, Copy)]
pub enum Architecture {
    X86,
    X64,
}

impl Architecture {
    fn name(self) -> &'static str {
        match self {
            Architecture::X86 => "x86",
            Architecture::X64 => "x64",
        }
    }
}

pub struct Instance<'a> {
    program_name: &'a str,
    architecture: Architecture,
    command_line_string: OsString,
    events: Vec<Event>,
}

impl<'a> Instance<'a> {
    #[must_use]
    pub fn new(program_name: &'a str, architecture: Architecture) -> Self {
        Self {
            program_name,
            architecture,
            command_line_string: OsString::new(),
            events: Vec::new(),
        }
    }

    pub fn with_command_line_string(&mut self, string: OsString) -> &mut Self {
        self.command_line_string = string;
        self
    }

    pub fn with_events(&mut self, events: impl IntoIterator<Item = Event>) -> &mut Self {
        self.events.extend(events);
        self
    }

    fn source_file_path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../programs/src/{}.c", self.program_name))
    }
    fn binary_file_path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../programs/bin/{}/{}.exe",
            self.architecture.name(),
            self.program_name,
        ))
    }

    pub async fn stdout(&self) -> Result<Vec<Vec<u8>>> {
        self.build();

        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stdout_callback = {
            let stdout = Arc::clone(&stdout);
            move |bytes: &_| {
                for line in String::from_utf8_lossy(bytes).lines() {
                    const LINE_LENGTH_LIMIT: usize = 256;
                    if line.len() <= LINE_LENGTH_LIMIT {
                        info!("stdout: {}", line);
                    } else {
                        info!("stdout: {} (...)", &line[..LINE_LENGTH_LIMIT]);
                    }
                }

                stdout.lock().unwrap().extend_from_slice(bytes);
            }
        };
        let mut stdout_by_instant = Vec::new();
        let mut conductor = winter::Conductor::new(
            &self.binary_file_path(),
            &self.command_line_string,
            Some(stdout_callback),
        )
        .await?;
        conductor.resume().await?;
        for event in &self.events {
            match event {
                Event::AdvanceTime(duration) => {
                    assert!(conductor.wait_until_inactive().await? == winter::InactiveState::Idle);
                    stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
                    conductor.advance_time(*duration).await?;
                }
                Event::SetKeyState { id, state } => {
                    conductor.set_key_state(*id, *state).await?;
                }
            }
        }
        assert!(conductor.wait_until_inactive().await? == winter::InactiveState::Terminated);
        stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
        Ok(stdout_by_instant)
    }

    pub async fn stdout_from_utf8_lossy(&self) -> Result<Vec<String>> {
        Ok(self
            .stdout()
            .await?
            .iter()
            .map(|b| String::from_utf8_lossy(b).to_string())
            .collect::<Vec<_>>())
    }

    fn build(&self) {
        static ENVIRONMENT_VARIABLES_X86: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();
        static ENVIRONMENT_VARIABLES_X64: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();

        if !self.should_build() {
            return;
        }

        let environment_variables = match self.architecture {
            Architecture::X86 => {
                ENVIRONMENT_VARIABLES_X86.get_or_init(|| self.get_build_environment_variables())
            }
            Architecture::X64 => {
                ENVIRONMENT_VARIABLES_X64.get_or_init(|| self.get_build_environment_variables())
            }
        };

        std::fs::create_dir_all(format!("tests/programs/obj/{}", self.architecture.name()))
            .unwrap();
        std::fs::create_dir_all(format!("tests/programs/bin/{}", self.architecture.name()))
            .unwrap();
        let command_output = Command::new("cl")
            .envs(environment_variables.clone())
            .arg(self.source_file_path())
            .arg("user32.lib")
            .arg("winmm.lib")
            .args(["/I", "tests/programs/include"])
            .arg("/DYNAMICBASE:NO")
            .arg("/Fo:")
            .arg(format!("tests/programs/obj/{}/", self.architecture.name()))
            .arg("/Fe:")
            .arg(format!("tests/programs/bin/{}/", self.architecture.name()))
            .output()
            .unwrap();
        print!("{}", String::from_utf8_lossy(&command_output.stdout));
        eprint!("{}", String::from_utf8_lossy(&command_output.stderr));
        assert!(command_output.status.success());
    }

    fn should_build(&self) -> bool {
        let Ok(source_file_modified_time) = self
            .source_file_path()
            .metadata()
            .map(|m| m.modified().unwrap())
        else {
            return true;
        };

        let Ok(binary_file_modified_time) = self
            .binary_file_path()
            .metadata()
            .map(|m| m.modified().unwrap())
        else {
            return true;
        };

        binary_file_modified_time <= source_file_modified_time
    }

    fn get_build_environment_variables(&self) -> Vec<(OsString, OsString)> {
        const VCVARS_DIR_ERROR: &str = "the environment variable VCVARS_DIR must be set to a \
    directory containing vcvars scripts to successfully build tests";

        let vcvars_script_path =
            Path::new(&std::env::var("VCVARS_DIR").expect(VCVARS_DIR_ERROR)).join("vcvarsall.bat");
        assert!(vcvars_script_path.exists(), "{}", VCVARS_DIR_ERROR);

        let command = Command::new("cmd")
            .arg("/C")
            .arg(vcvars_script_path)
            .arg(self.architecture.name())
            .args([">NUL", "&&", "set"])
            .output()
            .unwrap();
        eprint!("{}", String::from_utf8_lossy(&command.stderr));
        assert!(command.status.success());
        let stdout = String::from_utf8(command.stdout).unwrap();
        stdout
            .lines()
            .map(|line| {
                let (key, value) = line.split_once('=').unwrap();
                let key = OsString::from_str(key).unwrap();
                let value = OsString::from_str(value).unwrap();
                (key, value)
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Clone)]
pub enum Event {
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
}
