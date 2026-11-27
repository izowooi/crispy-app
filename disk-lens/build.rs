fn main() {
    // 빌드하는 컴퓨터가 아니라 대상이 Windows 인지 본다. macOS 에서 교차 빌드할 수 있어야 한다.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    println!("cargo:rerun-if-changed=assets/disk-lens.svg");
    println!("cargo:rerun-if-changed=scripts/make-icon.ps1");
    println!("cargo:rerun-if-changed=scripts/make-icon.swift");
    let icon = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("disk-lens.ico");
    let status = if cfg!(windows) {
        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            "scripts/make-icon.ps1",
            "-Output",
        ]);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x08000000);
        command.arg(&icon).status()
    } else {
        std::process::Command::new("swift")
            .args(["scripts/make-icon.swift", "--ico"])
            .arg(&icon)
            .status()
    }
    .expect("Run icon renderer");
    assert!(status.success(), "Windows icon rendering failed");
    winresource::WindowsResource::new()
        .set_icon(icon.to_str().expect("UTF-8 build path"))
        .set("ProductName", "DiskLens")
        .set("FileDescription", "DiskLens disk space visualizer")
        .compile()
        .expect("Compile Windows resources");
}
