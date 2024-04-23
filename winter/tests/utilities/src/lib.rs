use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::OnceLock,
};

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

pub fn build(program_name: impl AsRef<str>, architecture: Architecture) -> PathBuf {
    static ENVIRONMENT_VARIABLES_X86: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();
    static ENVIRONMENT_VARIABLES_X64: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();

    let source_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../programs/src/{}.c", program_name.as_ref(),));
    let binary_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../programs/bin/{}/{}.exe",
        architecture.name(),
        program_name.as_ref(),
    ));
    if !should_build(&source_file_path, &binary_file_path) {
        return binary_file_path;
    }

    let environment_variables = match architecture {
        Architecture::X86 => {
            ENVIRONMENT_VARIABLES_X86.get_or_init(|| get_environment_variables(architecture))
        }
        Architecture::X64 => {
            ENVIRONMENT_VARIABLES_X64.get_or_init(|| get_environment_variables(architecture))
        }
    };

    std::fs::create_dir_all(format!("tests/programs/obj/{}", architecture.name())).unwrap();
    std::fs::create_dir_all(format!("tests/programs/bin/{}", architecture.name())).unwrap();
    let command_output = Command::new("cl")
        .envs(environment_variables.clone())
        .arg(source_file_path)
        .arg("user32.lib")
        .arg("winmm.lib")
        .arg("/DYNAMICBASE:NO")
        .arg("/Fo:")
        .arg(format!("tests/programs/obj/{}/", architecture.name()))
        .arg("/Fe:")
        .arg(format!("tests/programs/bin/{}/", architecture.name()))
        .output()
        .unwrap();
    print!("{}", String::from_utf8_lossy(&command_output.stdout));
    eprint!("{}", String::from_utf8_lossy(&command_output.stderr));
    assert!(command_output.status.success());

    binary_file_path
}

fn should_build(source_file_path: impl AsRef<Path>, binary_file_path: impl AsRef<Path>) -> bool {
    let Ok(source_file_modified_time) = source_file_path
        .as_ref()
        .metadata()
        .map(|m| m.modified().unwrap())
    else {
        return true;
    };

    let Ok(binary_file_modified_time) = binary_file_path
        .as_ref()
        .metadata()
        .map(|m| m.modified().unwrap())
    else {
        return true;
    };

    binary_file_modified_time <= source_file_modified_time
}

fn get_environment_variables(architecture: Architecture) -> Vec<(OsString, OsString)> {
    const VCVARS_DIR_ERROR: &str = "the environment variable VCVARS_DIR must be set to a \
    directory containing vcvars scripts to successfully build tests";

    let vcvars_script_path =
        Path::new(&std::env::var("VCVARS_DIR").expect(VCVARS_DIR_ERROR)).join("vcvarsall.bat");
    assert!(vcvars_script_path.exists(), "{}", VCVARS_DIR_ERROR);

    let command = Command::new("cmd")
        .arg("/C")
        .arg(vcvars_script_path)
        .arg(architecture.name())
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
