use std::{
    io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct Workload {
    pub name: &'static str,
    pub block: usize,
    pub queue: usize,
    pub random: bool,
}
pub const WORKLOADS: [Workload; 4] = [
    Workload {
        name: "SEQ1M Q8T1",
        block: 1048576,
        queue: 8,
        random: false,
    },
    Workload {
        name: "SEQ1M Q1T1",
        block: 1048576,
        queue: 1,
        random: false,
    },
    Workload {
        name: "RND4K Q32T1",
        block: 4096,
        queue: 32,
        random: true,
    },
    Workload {
        name: "RND4K Q1T1",
        block: 4096,
        queue: 1,
        random: true,
    },
];
#[derive(Clone, Copy, Debug, Default)]
pub struct Score {
    pub mbps: f64,
    pub iops: f64,
    pub latency_us: f64,
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        alloc::{Layout, alloc_zeroed, dealloc},
        fs::{File, OpenOptions},
        os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_IO_PENDING, HANDLE, WAIT_FAILED, WAIT_TIMEOUT},
        Storage::FileSystem::{
            FILE_FLAG_DELETE_ON_CLOSE, FILE_FLAG_NO_BUFFERING, FILE_FLAG_OVERLAPPED,
            FILE_FLAG_WRITE_THROUGH, GetDiskFreeSpaceExW, ReadFile, WriteFile,
        },
        System::{
            IO::{GetOverlappedResult, OVERLAPPED},
            Threading::{CreateEventW, WaitForMultipleObjects},
        },
    };

    struct Slot {
        buffer: *mut u8,
        layout: Layout,
        ov: Box<OVERLAPPED>,
        pending: bool,
        started: Instant,
    }
    impl Slot {
        fn new(size: usize) -> io::Result<Self> {
            let layout = Layout::from_size_align(size, 65536).unwrap();
            let buffer = unsafe { alloc_zeroed(layout) };
            if buffer.is_null() {
                return Err(io::Error::other("Unable to allocate aligned I/O buffer"));
            }
            let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
            if event.is_null() {
                unsafe { dealloc(buffer, layout) };
                return Err(io::Error::last_os_error());
            }
            let mut ov: Box<OVERLAPPED> = Box::default();
            ov.hEvent = event;
            // Incompressible deterministic data, generated outside measurement.
            let mut seed = 0xa73c59e18731u64;
            for byte in unsafe { std::slice::from_raw_parts_mut(buffer, size) } {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *byte = seed as u8;
            }
            Ok(Self {
                buffer,
                layout,
                ov,
                pending: false,
                started: Instant::now(),
            })
        }
        fn submit(&mut self, handle: HANDLE, offset: u64, write: bool) -> io::Result<()> {
            self.ov.Anonymous.Anonymous.Offset = offset as u32;
            self.ov.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;
            self.started = Instant::now();
            let ok = unsafe {
                if write {
                    WriteFile(
                        handle,
                        self.buffer,
                        self.layout.size() as u32,
                        ptr::null_mut(),
                        &mut *self.ov,
                    )
                } else {
                    ReadFile(
                        handle,
                        self.buffer,
                        self.layout.size() as u32,
                        ptr::null_mut(),
                        &mut *self.ov,
                    )
                }
            };
            if ok == 0 && io::Error::last_os_error().raw_os_error() != Some(ERROR_IO_PENDING as i32)
            {
                return Err(io::Error::last_os_error());
            }
            self.pending = true;
            Ok(())
        }
        fn finish(&mut self, handle: HANDLE) -> io::Result<()> {
            if !self.pending {
                return Ok(());
            }
            let mut transferred = 0;
            let ok = unsafe { GetOverlappedResult(handle, &*self.ov, &mut transferred, 1) };
            self.pending = false;
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            if transferred as usize != self.layout.size() {
                return Err(io::Error::other("Short disk I/O"));
            }
            Ok(())
        }
    }
    impl Drop for Slot {
        fn drop(&mut self) {
            debug_assert!(!self.pending);
            unsafe {
                CloseHandle(self.ov.hEvent);
                dealloc(self.buffer, self.layout);
            }
        }
    }

    pub struct TestFile {
        // FILE_FLAG_DELETE_ON_CLOSE ties cleanup to this file handle, even on process exit.
        file: File,
        size: u64,
    }
    impl TestFile {
        pub fn create(directory: &Path, size: u64, cancel: &AtomicBool) -> io::Result<Self> {
            if size < 1048576 || !size.is_multiple_of(1048576) {
                return Err(io::Error::other(
                    "Test size must be a positive multiple of 1 MiB",
                ));
            }
            let directory = directory.canonicalize()?;
            let wide: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut available = 0u64;
            if unsafe {
                GetDiskFreeSpaceExW(
                    wide.as_ptr(),
                    &mut available,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if available < size.saturating_add(64 * 1048576) {
                return Err(io::Error::other(
                    "Not enough free space (64 MiB reserve required)",
                ));
            }
            let path = directory.join(format!(
                ".obsidian-disk-{}-{}.tmp",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .share_mode(0)
                .custom_flags(
                    FILE_FLAG_NO_BUFFERING
                        | FILE_FLAG_OVERLAPPED
                        | FILE_FLAG_WRITE_THROUGH
                        | FILE_FLAG_DELETE_ON_CLOSE,
                )
                .open(&path)?;
            let result = Self { file, size };
            let handle = result.file.as_raw_handle();
            let mut slot = Slot::new(1048576)?;
            for offset in (0..size).step_by(1048576) {
                if cancel.load(Ordering::Relaxed) {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"));
                }
                slot.submit(handle, offset, true)?;
                slot.finish(handle)?;
            }
            Ok(result)
        }
        pub fn measure(
            &self,
            workload: Workload,
            write: bool,
            duration: Duration,
            cancel: &AtomicBool,
        ) -> io::Result<Score> {
            if workload.queue == 0
                || workload.queue > 64
                || workload.block == 0
                || !workload.block.is_multiple_of(4096)
                || self.size < workload.block as u64
                || duration.is_zero()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid workload or duration",
                ));
            }
            let handle = self.file.as_raw_handle();
            let mut slots = (0..workload.queue)
                .map(|_| Slot::new(workload.block))
                .collect::<io::Result<Vec<_>>>()?;
            let mut sequence = 0u64;
            let mut random = 0x5eced917fu64;
            let blocks = self.size / workload.block as u64;
            let mut next = || {
                let index = if workload.random {
                    random ^= random << 13;
                    random ^= random >> 7;
                    random ^= random << 17;
                    random % blocks
                } else {
                    let n = sequence % blocks;
                    sequence += 1;
                    n
                };
                index * workload.block as u64
            };
            let start = Instant::now();
            let mut count = 0u64;
            let mut latency = 0.0;
            // Rotate event priority so a fast lower-index slot cannot starve other completions.
            let mut events: Vec<_> = slots.iter().map(|slot| slot.ov.hEvent).collect();
            let mut first = 0;
            let run = (|| -> io::Result<()> {
                for slot in &mut slots {
                    slot.submit(handle, next(), write)?;
                }
                while start.elapsed() < duration && !cancel.load(Ordering::Relaxed) {
                    let ready = unsafe {
                        WaitForMultipleObjects(events.len() as u32, events.as_ptr(), 0, 50)
                    };
                    if ready == WAIT_TIMEOUT {
                        continue;
                    }
                    if ready == WAIT_FAILED {
                        return Err(io::Error::last_os_error());
                    }
                    if ready as usize >= slots.len() {
                        return Err(io::Error::other("Unexpected I/O event"));
                    }
                    let index = (first + ready as usize) % slots.len();
                    let slot = &mut slots[index];
                    slot.finish(handle)?;
                    count += 1;
                    latency += slot.started.elapsed().as_secs_f64();
                    if start.elapsed() < duration && !cancel.load(Ordering::Relaxed) {
                        slot.submit(handle, next(), write)?;
                    }
                    let shift = ready as usize + 1;
                    let length = events.len();
                    events.rotate_left(shift % length);
                    first = (index + 1) % slots.len();
                }
                Ok(())
            })();
            // Always drain every outstanding operation before releasing buffers, including errors.
            let mut drain_error = None;
            for slot in &mut slots {
                if slot.pending {
                    match slot.finish(handle) {
                        Ok(()) => {
                            count += 1;
                            latency += slot.started.elapsed().as_secs_f64();
                        }
                        Err(e) => {
                            drain_error = Some(e);
                        }
                    }
                }
            }
            run?;
            if let Some(e) = drain_error {
                return Err(e);
            }
            let elapsed = start.elapsed().as_secs_f64();
            Ok(Score {
                mbps: count as f64 * workload.block as f64 / elapsed / 1e6,
                iops: count as f64 / elapsed,
                latency_us: latency * 1e6 / count.max(1) as f64,
            })
        }
    }
}
#[cfg(windows)]
pub use windows::TestFile;

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn direct_io_all_workloads_and_cleanup() {
        let directory = std::env::temp_dir().join(format!("obsidian-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let cancel = AtomicBool::new(false);
        {
            let file = TestFile::create(&directory, 16 * 1048576, &cancel).unwrap();
            for workload in WORKLOADS {
                for write in [false, true] {
                    let score = file
                        .measure(workload, write, Duration::from_millis(100), &cancel)
                        .unwrap();
                    assert!(score.mbps.is_finite() && score.mbps > 0.0);
                    assert!(score.iops > 0.0 && score.latency_us > 0.0);
                    assert!((score.mbps / score.iops - workload.block as f64 / 1e6).abs() < 1e-8);
                    eprintln!("{} write={write}: {score:?}", workload.name);
                }
            }
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
        cancel.store(true, Ordering::Relaxed);
        assert!(TestFile::create(&directory, 1048576, &cancel).is_err());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn cancel_running_queue_and_validate_inputs() {
        let directory =
            std::env::temp_dir().join(format!("obsidian-cancel-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("keep.txt"), b"existing user file").unwrap();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        {
            let file = TestFile::create(&directory, 16 * 1048576, &cancel).unwrap();
            assert!(
                file.measure(WORKLOADS[0], false, Duration::ZERO, &cancel)
                    .is_err()
            );
            let timer_cancel = cancel.clone();
            let timer = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                timer_cancel.store(true, Ordering::Relaxed);
            });
            let started = Instant::now();
            file.measure(WORKLOADS[2], true, Duration::from_secs(10), &cancel)
                .unwrap();
            assert!(started.elapsed() < Duration::from_secs(2));
            timer.join().unwrap();
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        assert_eq!(
            std::fs::read(directory.join("keep.txt")).unwrap(),
            b"existing user file"
        );
        std::fs::remove_file(directory.join("keep.txt")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
