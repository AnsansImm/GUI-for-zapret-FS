use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets/app.ico");
    let resource = output.join("app.rc");
    let compiled = output.join("app.res");
    fs::write(
        &resource,
        format!(
            "1 ICON \"{}\"\n",
            icon.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .unwrap();

    let sdk = PathBuf::from(
        env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:\\Program Files (x86)".into()),
    )
    .join("Windows Kits/10/bin");
    let host = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let mut compilers: Vec<_> = fs::read_dir(sdk)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(host).join("rc.exe"))
        .filter(|path| path.is_file())
        .collect();
    compilers.sort();
    let compiler = compilers.pop().unwrap_or_else(|| PathBuf::from("rc.exe"));
    let status = Command::new(compiler)
        .arg("/nologo")
        .arg("/fo")
        .arg(&compiled)
        .arg(&resource)
        .status()
        .expect("Windows SDK resource compiler is required");
    assert!(status.success(), "Failed to compile application icon");
    println!(
        "cargo:rustc-link-arg-bin=gui-for-zapret-fs={}",
        compiled.display()
    );
}
