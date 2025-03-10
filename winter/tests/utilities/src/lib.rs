#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use anyhow::Result;
use shared::input::MouseButton;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::{Arc, Mutex, Once, OnceLock},
    time::Duration,
};
use tracing::info;

pub fn init_test() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    });
}

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
    unicode_flag: bool,
    events: Vec<Event>,
}

impl<'a> Instance<'a> {
    #[must_use]
    pub fn new(program_name: &'a str, architecture: Architecture) -> Self {
        Self {
            program_name,
            architecture,
            command_line_string: OsString::new(),
            unicode_flag: false,
            events: Vec::new(),
        }
    }

    pub fn with_command_line_string(&mut self, string: OsString) -> &mut Self {
        self.command_line_string = string;
        self
    }

    pub fn with_unicode_flag(&mut self, flag: bool) -> &mut Self {
        self.unicode_flag = flag;
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

    fn object_file_path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../programs/obj/{}/{}/{}.obj",
            self.architecture.name(),
            if self.unicode_flag { "unicode" } else { "ansi" },
            self.program_name,
        ))
    }

    fn binary_file_path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../programs/bin/{}/{}/{}.exe",
            self.architecture.name(),
            if self.unicode_flag { "unicode" } else { "ansi" },
            self.program_name,
        ))
    }

    pub async fn stdout(&self) -> Result<Vec<u8>> {
        Ok(self
            .stdout_by_instant()
            .await?
            .into_iter()
            .flatten()
            .collect())
    }

    pub async fn stdout_by_instant(&self) -> Result<Vec<Vec<u8>>> {
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
            match *event {
                Event::AdvanceTime(duration) => {
                    assert_eq!(
                        conductor.wait_until_inactive().await?,
                        winter::InactiveState::Idle
                    );
                    stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
                    conductor.advance_time(duration).await?;
                }
                Event::SetKeyState { id, state } => {
                    conductor.set_key_state(id, state).await?;
                }
                Event::SetMousePosition { x, y } => {
                    conductor.set_mouse_position(x, y).await?;
                }
                Event::SetMouseButtonState { button, state } => {
                    conductor.set_mouse_button_state(button, state).await?;
                }
                Event::SaveState => {
                    conductor.save_state().await?;
                }
                Event::LoadState => {
                    conductor.load_state().await?;
                }
            }
        }
        if let winter::InactiveState::Terminated { exit_code } =
            conductor.wait_until_inactive().await?
        {
            assert_eq!(exit_code, 0);
        } else {
            panic!("the final checked inactive state is not the terminated state")
        }
        stdout_by_instant.push(std::mem::take(&mut *stdout.lock().unwrap()));
        Ok(stdout_by_instant)
    }

    pub async fn stdout_from_utf8_lossy(&self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.stdout().await?).to_string())
    }

    pub async fn stdout_by_instant_from_utf8_lossy(&self) -> Result<Vec<String>> {
        Ok(self
            .stdout_by_instant()
            .await?
            .iter()
            .map(|b| String::from_utf8_lossy(b).to_string())
            .collect::<Vec<_>>())
    }

    fn build(&self) {
        static ENVIRONMENT_VARIABLES_X86: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();
        static ENVIRONMENT_VARIABLES_X64: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();
        static BINARY_FILE_MUTEXES: Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>> =
            Mutex::new(BTreeMap::new());

        // avoid building the same program multiple times at once, preventing needless
        // recompilation and attempts to access .obj and .exe files while locked
        let binary_file_mutex = {
            let mut binary_file_mutexes = BINARY_FILE_MUTEXES.lock().unwrap();
            Arc::clone(
                binary_file_mutexes
                    .entry(self.binary_file_path())
                    .or_insert_with(|| Arc::new(Mutex::new(()))),
            )
        };
        let _binary_file_lock = binary_file_mutex.lock().unwrap();

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

        std::fs::create_dir_all(self.object_file_path().parent().unwrap()).unwrap();
        std::fs::create_dir_all(self.binary_file_path().parent().unwrap()).unwrap();
        let command_output = Command::new("cl")
            .envs(environment_variables.clone())
            .arg(self.source_file_path())
            .arg("user32.lib")
            .arg("winmm.lib")
            .args(["/I", "tests/programs/include"])
            .args([if self.unicode_flag { "/D" } else { "/U" }, "UNICODE"])
            .args([if self.unicode_flag { "/D" } else { "/U" }, "_UNICODE"])
            .arg("/DYNAMICBASE:NO")
            .arg("/W3")
            .arg("/WX")
            .arg("/Fo:")
            .arg(self.object_file_path())
            .arg("/Fe:")
            .arg(self.binary_file_path())
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
    SetMousePosition { x: u16, y: u16 },
    SetMouseButtonState { button: MouseButton, state: bool },
    SaveState,
    LoadState,
}
