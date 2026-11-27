use std::{
    io,
    path::{Path, PathBuf},
};

pub use std::fs::{Metadata, symlink_metadata as metadata};

pub fn is_loop(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ELOOP)
}

/// 사용자가 디스크 맨 위("/")를 고르면 실제 자료가 있는 볼륨을 스캔한다.
/// "/" 는 읽기 전용 시스템 볼륨이라 그대로 스캔하면 사용자 자료가 빠진다.
pub fn scan_root(chosen: PathBuf) -> PathBuf {
    if chosen == Path::new("/") {
        data_volume()
    } else {
        chosen
    }
}
pub fn data_volume() -> PathBuf {
    PathBuf::from("/System/Volumes/Data")
}
pub fn disk_label() -> &'static str {
    "Macintosh HD"
}
/// 샌드박스 안에서는 HOME 이 앱 컨테이너를 가리키므로 계정 정보에서 진짜 홈 폴더를 얻는다.
pub fn home_folder() -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    // getpwuid 는 프로세스 수명 동안 유효한 정적 버퍼를 돌려준다. 바로 복사해 둔다.
    let entry = unsafe { libc::getpwuid(libc::getuid()) };
    if entry.is_null() {
        return None;
    }
    let directory = unsafe { std::ffi::CStr::from_ptr((*entry).pw_dir) };
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(
        directory.to_bytes(),
    )))
}
pub fn font_path() -> PathBuf {
    PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc")
}
pub fn reveal(path: &Path) -> io::Result<()> {
    std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map(|_| ())
}
pub fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::rename(source, destination)
}
