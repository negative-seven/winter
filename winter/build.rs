use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn main() {
    build_hooks();
}

fn build_hooks() {
    let out_directory = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let hooks_package_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../hooks");
    let hooks_package_manifest_path = hooks_package_path.join("Cargo.toml");

    println!(
        "cargo:rerun-if-changed={}",
        hooks_package_path.to_str().unwrap()
    );

    let hooks_target_directory = &out_directory;
    for (target, dll_filename) in [
        ("i686-pc-windows-msvc", "hooks32.dll"),
        ("x86_64-pc-windows-msvc", "hooks64.dll"),
    ] {
        // paths inferred from observed behavior of cargo
        let hooks_dll_build_path = hooks_target_directory
            .join(target)
            .join("release/hooks.dll");
        let hooks_dll_destination_path = out_directory.join("../../..").join(dll_filename);

        assert!(Command::new("cargo")
            .arg("build")
            .arg("--release")
            .args([
                "--manifest-path",
                hooks_package_manifest_path.to_str().unwrap()
            ])
            .args(["--target", target])
            .args(["--target-dir", hooks_target_directory.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success());
        std::fs::copy(&hooks_dll_build_path, &hooks_dll_destination_path).unwrap();

        println!(
            "cargo:rerun-if-changed={}",
            hooks_dll_destination_path.to_str().unwrap()
        );
    }
}
