use crate::Result;
use std::path::Path;

#[cfg(unix)]
mod platform {
    use super::*;
    use crate::MAX_PAGE_BYTES;
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::ptr;

    #[link(name = "archive")]
    unsafe extern "C" {
        fn archive_read_new() -> *mut c_void;
        fn archive_read_support_filter_all(a: *mut c_void) -> c_int;
        fn archive_read_support_format_all(a: *mut c_void) -> c_int;
        fn archive_read_open_filename(a: *mut c_void, path: *const c_char, block: usize) -> c_int;
        fn archive_read_next_header(a: *mut c_void, entry: *mut *mut c_void) -> c_int;
        fn archive_read_data_skip(a: *mut c_void) -> c_int;
        fn archive_read_data(a: *mut c_void, buffer: *mut c_void, length: usize) -> isize;
        fn archive_read_free(a: *mut c_void) -> c_int;
        fn archive_entry_pathname_utf8(entry: *mut c_void) -> *const c_char;
        fn archive_entry_pathname(entry: *mut c_void) -> *const c_char;
        fn archive_entry_filetype(entry: *mut c_void) -> u32;
        fn archive_entry_size(entry: *mut c_void) -> i64;
        fn archive_error_string(a: *mut c_void) -> *const c_char;
    }

    struct Reader(*mut c_void);

    // libarchive's handle is isolated to one serial IO worker. Moving ownership
    // between threads is allowed; simultaneous access is never exposed.
    unsafe impl Send for Reader {}

    impl Reader {
        fn open(path: &Path) -> Result<Self> {
            let path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| "Archive path contains NUL")?;
            let reader = Self(unsafe { archive_read_new() });
            if reader.0.is_null() {
                return Err("Cannot allocate archive reader.".into());
            }
            unsafe {
                archive_read_support_filter_all(reader.0);
                archive_read_support_format_all(reader.0);
                if archive_read_open_filename(reader.0, path.as_ptr(), 64 * 1024) != 0 {
                    return Err(reader.error());
                }
            }
            Ok(reader)
        }

        fn error(&self) -> String {
            let message = unsafe { archive_error_string(self.0) };
            if message.is_null() {
                "Cannot read this archive.".into()
            } else {
                format!(
                    "Cannot read archive: {}",
                    unsafe { CStr::from_ptr(message) }.to_string_lossy()
                )
            }
        }

        fn next(&mut self) -> Result<Option<*mut c_void>> {
            let mut entry = ptr::null_mut();
            let status = unsafe { archive_read_next_header(self.0, &mut entry) };
            if status == 1 {
                return Ok(None);
            }
            if status < -20 || entry.is_null() {
                return Err(self.error());
            }
            Ok(Some(entry))
        }

        fn skip(&mut self) -> Result<()> {
            if unsafe { archive_read_data_skip(self.0) } < -20 {
                return Err(self.error());
            }
            Ok(())
        }

        fn bytes(&mut self, entry: *mut c_void) -> Result<Vec<u8>> {
            let declared = unsafe { archive_entry_size(entry) };
            if declared > MAX_PAGE_BYTES as i64 {
                return Err("Image exceeds the 256 MiB per-page limit.".into());
            }
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 64 * 1024];
            loop {
                let read =
                    unsafe { archive_read_data(self.0, chunk.as_mut_ptr().cast(), chunk.len()) };
                if read < 0 {
                    return Err(self.error());
                }
                if read == 0 {
                    break;
                }
                if bytes.len() as u64 + read as u64 > MAX_PAGE_BYTES {
                    return Err("Image exceeds the 256 MiB per-page limit.".into());
                }
                bytes
                    .try_reserve(read as usize)
                    .map_err(|_| "Not enough memory to read this page")?;
                bytes.extend_from_slice(&chunk[..read as usize]);
            }
            Ok(bytes)
        }
    }

    impl Drop for Reader {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    archive_read_free(self.0);
                }
            }
        }
    }

    pub(crate) struct ArchiveSource {
        path: PathBuf,
        reader: Option<Reader>,
        cursor: usize,
    }

    impl ArchiveSource {
        pub(crate) fn open(path: &Path, limit: usize) -> Result<(Self, Vec<(usize, String)>)> {
            let mut reader = Reader::open(path)?;
            let mut entries = Vec::new();
            let mut index = 0;
            while let Some(entry) = reader.next()? {
                if index >= limit {
                    return Err("Archive contains too many entries.".into());
                }
                if unsafe { archive_entry_filetype(entry) } & 0o170000 == 0o100000 {
                    let utf8 = unsafe { archive_entry_pathname_utf8(entry) };
                    let name = if utf8.is_null() {
                        unsafe { archive_entry_pathname(entry) }
                    } else {
                        utf8
                    };
                    if !name.is_null() {
                        entries.push((
                            index,
                            unsafe { CStr::from_ptr(name) }
                                .to_string_lossy()
                                .replace('\\', "/"),
                        ));
                    }
                }
                reader.skip()?;
                index += 1;
            }
            Ok((
                Self {
                    path: path.to_owned(),
                    reader: None,
                    cursor: 0,
                },
                entries,
            ))
        }

        pub(crate) fn read(&mut self, index: usize) -> Result<Vec<u8>> {
            if self.reader.is_none() || self.cursor > index {
                self.reader = Some(Reader::open(&self.path)?);
                self.cursor = 0;
            }
            let reader = self
                .reader
                .as_mut()
                .ok_or("Archive reader is unavailable")?;
            while let Some(entry) = reader.next()? {
                let current = self.cursor;
                self.cursor += 1;
                if current == index {
                    return reader.bytes(entry);
                }
                reader.skip()?;
            }
            Err("Archive changed or this page is missing.".into())
        }
    }
}

#[cfg(not(unix))]
mod platform {
    use super::*;
    pub(crate) struct ArchiveSource;
    impl ArchiveSource {
        pub(crate) fn open(_: &Path, _: usize) -> Result<(Self, Vec<(usize, String)>)> {
            Err("RAR and 7z require libarchive on this platform. ZIP/CBZ is supported.".into())
        }
        pub(crate) fn read(&mut self, _: usize) -> Result<Vec<u8>> {
            Err("Archive reader is unavailable.".into())
        }
    }
}

pub(crate) use platform::ArchiveSource;
