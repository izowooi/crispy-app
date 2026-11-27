fn main() {
    // 빌드하는 컴퓨터가 아니라 대상이 Windows 인지 본다. macOS 에서 교차 빌드할 수 있어야 한다.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    // ico 는 `swift scripts/make-icon.swift --assets` 로 미리 만들어 저장소에 둔다.
    println!("cargo:rerun-if-changed=assets/disk-lens.ico");
    winresource::WindowsResource::new()
        .set_icon("assets/disk-lens.ico")
        .set("ProductName", "DiskLens")
        .set("FileDescription", "DiskLens disk space visualizer")
        .compile()
        .expect("Compile Windows resources");
}
