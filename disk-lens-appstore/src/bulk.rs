//! macOS metadata batching. Unsupported or malformed batches fail as a whole so
//! callers can retry the directory using the standard metadata traversal.
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const ATTR_CMN_ERROR: u32 = 0x2000_0000;
const COMMON: u32 = libc::ATTR_CMN_RETURNED_ATTRS
    | ATTR_CMN_ERROR
    | libc::ATTR_CMN_NAME
    | libc::ATTR_CMN_DEVID
    | libc::ATTR_CMN_OBJTYPE
    | libc::ATTR_CMN_FLAGS
    | libc::ATTR_CMN_FILEID;
const DIRECTORY: u32 =
    libc::ATTR_DIR_LINKCOUNT | libc::ATTR_DIR_MOUNTSTATUS | libc::ATTR_DIR_ALLOCSIZE;
const FILE: u32 =
    libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_ALLOCSIZE | libc::ATTR_FILE_DATALENGTH;
const VREG: u32 = 1;
const VDIR: u32 = 2;
const SF_FIRMLINK: u32 = 0x0080_0000;
const BUFFER_BYTES: usize = 128 * 1024;

/// Reused across directories to avoid repeatedly allocating the syscall buffer.
pub struct Buffer {
    storage: Vec<u64>,
    initial_bytes: usize,
    adaptive: bool,
    pub bulk_calls: u64,
    pub stat_calls: u64,
    pub record_bytes: u64,
}
impl Default for Buffer {
    fn default() -> Self {
        let fixed = std::env::var("DISK_LENS_BULK_BYTES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| (4096..=BUFFER_BYTES).contains(v));
        Self {
            initial_bytes: fixed.unwrap_or(4096),
            adaptive: fixed.is_none(),
            storage: vec![0; BUFFER_BYTES / 8],
            bulk_calls: 0,
            stat_calls: 0,
            record_bytes: 0,
        }
    }
}

#[derive(Debug)]
pub struct Entry {
    pub name: OsString,
    pub is_dir: bool,
    pub dev: u64,
    pub ino: u64,
    /// Directory counts omit historical entries; only file counts are used for deduplication.
    pub nlink: u64,
    pub logical: u64,
    pub allocated: u64,
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid getattrlistbulk record")
}
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Required bulk metadata is unavailable",
    )
}
fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Scan cancelled"))
    } else {
        Ok(())
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Cursor<'_> {
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let end = self.offset.checked_add(N).ok_or_else(invalid)?;
        let value = self.bytes.get(self.offset..end).ok_or_else(invalid)?;
        self.offset = end;
        value.try_into().map_err(|_| invalid())
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn size(&mut self) -> io::Result<u64> {
        let value = i64::from_le_bytes(self.take()?);
        u64::try_from(value).map_err(|_| invalid())
    }
}

struct Parsed {
    entry: Entry,
    needs_stat: bool,
}

fn parse_record(record: &[u8]) -> io::Result<Parsed> {
    let mut cursor = Cursor {
        bytes: record,
        offset: 0,
    };
    if cursor.u32()? as usize != record.len() {
        return Err(invalid());
    }
    let common = cursor.u32()?;
    let volume = cursor.u32()?;
    let directory = cursor.u32()?;
    let file = cursor.u32()?;
    let fork = cursor.u32()?;
    if common & !COMMON != 0
        || directory & !DIRECTORY != 0
        || file & !FILE != 0
        || volume != 0
        || fork != 0
    {
        return Err(unsupported());
    }
    // RETURNED_ATTRS is first, ERROR second, regardless of their mask values.
    if common & ATTR_CMN_ERROR != 0 {
        let error = cursor.u32()?;
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
    }
    let required =
        libc::ATTR_CMN_NAME | libc::ATTR_CMN_DEVID | libc::ATTR_CMN_OBJTYPE | libc::ATTR_CMN_FILEID;
    if common & required != required {
        return Err(unsupported());
    }
    let reference = cursor.offset;
    let name_offset = i32::from_le_bytes(cursor.take()?) as i64;
    let name_length = cursor.u32()? as usize;
    let dev = cursor.u32()? as i32 as u64;
    let object_type = cursor.u32()?;
    let flags = if common & libc::ATTR_CMN_FLAGS != 0 {
        cursor.u32()?
    } else {
        0
    };
    let ino = cursor.u64()?;
    let mut nlink = 0;
    let mut allocated = 0;
    let mut logical = 0;
    let mut mount = false;
    if directory & libc::ATTR_DIR_LINKCOUNT != 0 {
        nlink = cursor.u32()? as u64;
    }
    if directory & libc::ATTR_DIR_MOUNTSTATUS != 0 {
        mount = cursor.u32()? & 1 != 0;
    }
    if directory & libc::ATTR_DIR_ALLOCSIZE != 0 {
        allocated = cursor.size()?;
    }
    if file & libc::ATTR_FILE_LINKCOUNT != 0 {
        nlink = cursor.u32()? as u64;
    }
    if file & libc::ATTR_FILE_ALLOCSIZE != 0 {
        allocated = cursor.size()?;
    }
    if file & libc::ATTR_FILE_DATALENGTH != 0 {
        logical = cursor.size()?;
    }
    let start = usize::try_from(reference as i64 + name_offset).map_err(|_| invalid())?;
    let end = start.checked_add(name_length).ok_or_else(invalid)?;
    if start < cursor.offset || name_length < 2 {
        return Err(invalid());
    }
    let name = record.get(start..end).ok_or_else(invalid)?;
    if name.last() != Some(&0) {
        return Err(invalid());
    }
    let name = &name[..name.len() - 1];
    if name.contains(&0) || name.contains(&b'/') || name == b"." || name == b".." {
        return Err(invalid());
    }
    let is_dir = object_type == VDIR;
    let missing = if is_dir {
        directory & DIRECTORY != DIRECTORY
    } else {
        file & FILE != FILE
    };
    // Mount-point and firmlink bulk attributes describe the underlying entry,
    // not the mounted target. lstat also preserves exact symlink/special sizes.
    if missing && matches!(object_type, VREG | VDIR) {
        return Err(unsupported());
    }
    let needs_stat = missing
        || !matches!(object_type, VREG | VDIR)
        || mount
        || flags & SF_FIRMLINK != 0
        || common & libc::ATTR_CMN_FLAGS == 0;
    Ok(Parsed {
        entry: Entry {
            name: OsString::from_vec(name.to_vec()),
            is_dir,
            dev,
            ino,
            nlink,
            logical: if is_dir { 0 } else { logical },
            allocated,
        },
        needs_stat,
    })
}

/// This setting affects only the calling scan worker, never global system policy.
pub fn prevent_file_hydration() {
    unsafe extern "C" {
        fn setiopolicy_np(
            kind: libc::c_int,
            scope: libc::c_int,
            policy: libc::c_int,
        ) -> libc::c_int;
    }
    // Public macOS SDK: MATERIALIZE_DATALESS_FILES=3, THREAD=1, OFF=1.
    // All scanning remains metadata-only even on systems rejecting this hint.
    unsafe {
        setiopolicy_np(3, 1, 1);
    }
}

pub fn read_dir(path: &Path, cancel: &AtomicBool, buffer: &mut Buffer) -> io::Result<Vec<Entry>> {
    check_cancel(cancel)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)?;
    let mut attrs = libc::attrlist {
        bitmapcount: 5,
        reserved: 0,
        commonattr: COMMON,
        volattr: 0,
        dirattr: DIRECTORY,
        fileattr: FILE,
        forkattr: 0,
    };
    // u64 backing storage meets the ABI's eight-byte buffer alignment.
    let mut request_bytes = buffer.initial_bytes;
    let storage = &mut buffer.storage;
    let mut entries = Vec::new();
    loop {
        check_cancel(cancel)?;
        // The owned descriptor and buffers remain valid for this synchronous call.
        buffer.bulk_calls += 1;
        let count = unsafe {
            libc::getattrlistbulk(
                directory.as_raw_fd(),
                (&mut attrs as *mut libc::attrlist).cast(),
                storage.as_mut_ptr().cast(),
                request_bytes,
                libc::FSOPT_NOFOLLOW as u64,
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        if count == 0 {
            return Ok(entries);
        }
        // Initialized u64 storage is readable as bytes, within its allocation.
        let bytes =
            unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), request_bytes) };
        let mut offset = 0usize;
        for _ in 0..count {
            check_cancel(cancel)?;
            let mut cursor = Cursor { bytes, offset };
            let length = cursor.u32()? as usize;
            if length < 24 || !length.is_multiple_of(8) {
                return Err(invalid());
            }
            let end = offset.checked_add(length).ok_or_else(invalid)?;
            buffer.record_bytes += length as u64;
            let mut parsed = parse_record(bytes.get(offset..end).ok_or_else(invalid)?)?;
            if parsed.needs_stat {
                buffer.stat_calls += 1;
                let metadata = fs::symlink_metadata(path.join(&parsed.entry.name))?;
                parsed.entry.is_dir = metadata.is_dir();
                parsed.entry.dev = metadata.dev();
                parsed.entry.ino = metadata.ino();
                parsed.entry.nlink = metadata.nlink();
                parsed.entry.logical = if metadata.is_dir() { 0 } else { metadata.len() };
                parsed.entry.allocated = metadata.blocks().saturating_mul(512);
            }
            entries.push(parsed.entry);
            offset = end;
        }
        // Small directories avoid oversized kernel buffers; dense directories
        // grow subsequent batches to amortize syscall overhead.
        if buffer.adaptive && offset > request_bytes * 3 / 4 {
            request_bytes = (request_bytes * 4).min(BUFFER_BYTES);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "disk-lens-bulk-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn metadata_matches_lstat_for_regular_sparse_hardlink_symlink_and_unicode_entries() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("regular"), [7u8; 8192]).unwrap();
        fs::hard_link(fixture.0.join("regular"), fixture.0.join("hardlink")).unwrap();
        fs::File::create(fixture.0.join("sparse"))
            .unwrap()
            .set_len(32 * 1024 * 1024)
            .unwrap();
        fs::create_dir(fixture.0.join("directory")).unwrap();
        symlink("directory", fixture.0.join("link")).unwrap();
        symlink("missing", fixture.0.join("broken-link")).unwrap();
        fs::write(fixture.0.join("한글"), b"unicode").unwrap();
        fs::write(fixture.0.join("x".repeat(250)), b"long").unwrap();
        let forked = fixture.0.join("forked");
        fs::write(&forked, b"data-fork").unwrap();
        fs::write(forked.join("..namedfork/rsrc"), [4u8; 8192]).unwrap();
        use std::os::unix::ffi::OsStrExt;
        let cpath = std::ffi::CString::new(forked.as_os_str().as_bytes()).unwrap();
        let attribute = std::ffi::CString::new("com.disklens.test").unwrap();
        let value = [9u8; 8192];
        // Both C strings and the value allocation live throughout setxattr.
        assert_eq!(
            unsafe {
                libc::setxattr(
                    cpath.as_ptr(),
                    attribute.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                    0,
                )
            },
            0
        );
        let bulk: HashMap<_, _> =
            read_dir(&fixture.0, &AtomicBool::new(false), &mut Buffer::default())
                .unwrap()
                .into_iter()
                .map(|e| (e.name.clone(), e))
                .collect();
        let standard: Vec<_> = fs::read_dir(&fixture.0).unwrap().collect();
        assert_eq!(bulk.len(), standard.len());
        for item in standard {
            let item = item.unwrap();
            let metadata = fs::symlink_metadata(item.path()).unwrap();
            let entry = &bulk[&item.file_name()];
            assert_eq!(entry.is_dir, metadata.is_dir(), "{:?}", entry.name);
            assert_eq!(entry.dev, metadata.dev(), "{:?}", entry.name);
            assert_eq!(entry.ino, metadata.ino(), "{:?}", entry.name);
            if !entry.is_dir {
                assert_eq!(entry.nlink, metadata.nlink(), "{:?}", entry.name);
            }
            assert_eq!(
                entry.logical,
                if metadata.is_dir() { 0 } else { metadata.len() },
                "{:?}",
                entry.name
            );
            assert_eq!(entry.allocated, metadata.blocks() * 512, "{:?}", entry.name);
        }
    }

    #[test]
    fn empty_directory_cancellation_and_missing_path() {
        let fixture = Fixture::new();
        assert!(
            read_dir(&fixture.0, &AtomicBool::new(false), &mut Buffer::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            read_dir(&fixture.0, &AtomicBool::new(true), &mut Buffer::default())
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(
            read_dir(
                &fixture.0.join("missing"),
                &AtomicBool::new(false),
                &mut Buffer::default()
            )
            .is_err()
        );
    }

    #[test]
    fn reads_multiple_batches_without_truncation() {
        let fixture = Fixture::new();
        for i in 0..1800 {
            fs::File::create(fixture.0.join(format!("{i:04}-{}", "x".repeat(80)))).unwrap();
        }
        let entries =
            read_dir(&fixture.0, &AtomicBool::new(false), &mut Buffer::default()).unwrap();
        let names: std::collections::HashSet<_> = entries.iter().map(|e| &e.name).collect();
        assert_eq!(entries.len(), 1800);
        assert_eq!(names.len(), 1800);
    }

    #[test]
    fn validates_name_references_and_signed_sizes() {
        let mut record = vec![0u8; 80];
        for (offset, value) in [
            (0, 80u32),
            (4, COMMON),
            (16, FILE),
            (28, 48),
            (32, 3),
            (36, 1),
            (40, VREG),
            (56, 1),
        ] {
            record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        record[48..56].copy_from_slice(&1u64.to_le_bytes());
        record[60..68].copy_from_slice(&4096u64.to_le_bytes());
        record[68..76].copy_from_slice(&2u64.to_le_bytes());
        record[76..79].copy_from_slice(b"ok\0");
        let parsed = parse_record(&record).unwrap();
        assert_eq!(parsed.entry.name, "ok");
        assert_eq!(parsed.entry.logical, 2);
        assert_eq!(parsed.entry.allocated, 4096);
        for (offset, replacement) in [
            (28, i32::MAX as u32),
            (28, 0),
            (32, 500),
            (24, libc::EACCES as u32),
        ] {
            let mut bad = record.clone();
            bad[offset..offset + 4].copy_from_slice(&replacement.to_le_bytes());
            assert!(parse_record(&bad).is_err());
        }
        let mut bad = record.clone();
        bad[77] = b'/';
        assert!(parse_record(&bad).is_err());
        let mut bad = record.clone();
        bad[77] = 0;
        assert!(parse_record(&bad).is_err());
        let mut bad = record.clone();
        bad[68..76].copy_from_slice(&(-1i64).to_le_bytes());
        assert!(parse_record(&bad).is_err());
    }

    #[test]
    fn rejects_truncated_or_missing_required_attributes() {
        assert!(parse_record(&[]).is_err());
        let mut record = vec![0u8; 24];
        record[..4].copy_from_slice(&24u32.to_le_bytes());
        assert!(parse_record(&record).is_err());
        record[..4].copy_from_slice(&128u32.to_le_bytes());
        assert!(parse_record(&record).is_err());
    }
}
