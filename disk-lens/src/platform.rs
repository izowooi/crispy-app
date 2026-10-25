use std::{
    io,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
pub use std::fs::{Metadata, symlink_metadata as metadata};

#[cfg(target_os = "macos")]
pub fn is_loop(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ELOOP)
}
#[cfg(windows)]
pub fn is_loop(error: &io::Error) -> bool {
    error.raw_os_error() == Some(1921)
}

pub fn system_disk() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!(
            "{}\\",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
        ))
    } else {
        PathBuf::from("/System/Volumes/Data")
    }
}
pub fn disk_label() -> &'static str {
    if cfg!(windows) {
        "Windows"
    } else {
        "Macintosh HD"
    }
}
pub fn font_path() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
            .join("Fonts/malgun.ttf")
    } else {
        PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc")
    }
}
pub fn reveal(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("explorer.exe");
        let mut argument = std::ffi::OsString::from("/select,");
        argument.push(path);
        command.arg(argument);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg("-R").arg(path);
        command
    };
    command.spawn().map(|_| ())
}
#[cfg(target_os = "macos")]
pub fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(windows)]
pub use windows::*;
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        fs::OpenOptions,
        os::windows::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
    };
    use windows_sys::Win32::{Globalization::GetUserDefaultLocaleName, Storage::FileSystem::*};

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub struct Metadata {
        inner: std::fs::Metadata,
        identity: BY_HANDLE_FILE_INFORMATION,
        standard: FILE_STANDARD_INFO,
    }
    impl Metadata {
        // Never descend into reparse points, including junctions and cloud placeholders.
        pub fn is_dir(&self) -> bool {
            self.inner.is_dir() && self.inner.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
        }
        pub fn dev(&self) -> u64 {
            self.identity.dwVolumeSerialNumber as u64
        }
        pub fn ino(&self) -> u64 {
            (self.identity.nFileIndexHigh as u64) << 32 | self.identity.nFileIndexLow as u64
        }
        pub fn nlink(&self) -> u64 {
            self.identity.nNumberOfLinks as u64
        }
        pub fn len(&self) -> u64 {
            if self.inner.is_dir() {
                0
            } else {
                self.inner.file_size()
            }
        }
        pub fn blocks(&self) -> u64 {
            self.standard.AllocationSize.max(0) as u64 / 512
        }
    }
    pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
        let path = path.as_ref();
        // Zero desired access queries metadata without reading data or recalling cloud files.
        // OPEN_REPARSE_POINT opens the link itself, BACKUP_SEMANTICS allows directories.
        let file = OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)?;
        let inner = file.metadata()?;
        let mut identity = BY_HANDLE_FILE_INFORMATION::default();
        let mut standard = FILE_STANDARD_INFO::default();
        // Both output buffers have the exact ABI types required by these APIs.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut identity) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileStandardInfo,
                (&mut standard as *mut FILE_STANDARD_INFO).cast(),
                std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if inner.file_attributes() & (FILE_ATTRIBUTE_SPARSE_FILE | FILE_ATTRIBUTE_COMPRESSED) != 0 {
            let mut compression = FILE_COMPRESSION_INFO::default();
            if unsafe {
                GetFileInformationByHandleEx(
                    file.as_raw_handle(),
                    FileCompressionInfo,
                    (&mut compression as *mut FILE_COMPRESSION_INFO).cast(),
                    std::mem::size_of::<FILE_COMPRESSION_INFO>() as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            standard.AllocationSize = compression.CompressedFileSize;
        }
        Ok(Metadata {
            inner,
            identity,
            standard,
        })
    }
    pub fn disk_space(path: &Path) -> io::Result<crate::scanner::DiskSpace> {
        let absolute = std::path::absolute(path)?;
        let path = wide(&absolute);
        let (mut available, mut total, mut free) = (0, 0, 0);
        if unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut available, &mut total, &mut free) } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(crate::scanner::DiskSpace {
            total,
            available,
            free,
        })
    }
    pub fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
        let source = wide(source);
        let destination = wide(destination);
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn locale() -> String {
        let mut buffer = [0u16; 85];
        let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
        if length > 0 {
            String::from_utf16_lossy(&buffer[..length as usize - 1])
        } else {
            String::new()
        }
    }
}
