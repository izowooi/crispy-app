//! Windows uses the standard bounded directory workers with native metadata.
use std::{ffi::OsString, io, path::Path, sync::atomic::AtomicBool};

#[derive(Default)]
pub struct Buffer {
    _private: (),
}

pub struct Entry {
    pub name: OsString,
    pub is_dir: bool,
    pub dev: u64,
    pub ino: u64,
    pub nlink: u64,
    pub logical: u64,
    pub allocated: u64,
}

pub fn prevent_file_hydration() {}

pub fn read_dir(_: &Path, _: &AtomicBool, _: &mut Buffer) -> io::Result<Vec<Entry>> {
    Err(io::ErrorKind::Unsupported.into())
}
