fn main() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        println!("cargo:rerun-if-changed=assets/disk-lens.svg");
        println!("cargo:rerun-if-changed=scripts/make-icon.ps1");
        let icon =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("disk-lens.ico");
        let status = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "scripts/make-icon.ps1",
                "-Output",
            ])
            .arg(&icon)
            .creation_flags(0x08000000)
            .status()
            .expect("Run Windows icon renderer");
        assert!(status.success(), "Windows icon rendering failed");
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().expect("UTF-8 build path"))
            .set("ProductName", "Disk Lens")
            .set("FileDescription", "Disk Lens disk space visualizer")
            .compile()
            .expect("Compile Windows resources");
    }
}
