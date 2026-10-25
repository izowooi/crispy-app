use crate::bulk;
use std::collections::{HashMap, HashSet};
#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::fs;
use std::io;
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Node {
    pub component: PathBuf,
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub is_dir: bool,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub file_count: u64,
    pub error_count: u64,
}

// Only the root stores a full path. Other nodes retain their original OS name,
// avoiding a copy of every ancestor path for every file in a large scan.
pub fn node_path(nodes: &[Node], id: usize) -> PathBuf {
    let mut components = Vec::new();
    let mut index = id;
    while let Some(parent) = nodes[index].parent {
        components.push(&nodes[index].component);
        index = parent;
    }
    let mut path = nodes[index].component.clone();
    for name in components.into_iter().rev() {
        path.push(name);
    }
    path
}

#[derive(Debug)]
pub struct ScanResult {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
    pub elapsed: Duration,
    pub cancelled: bool,
}

#[derive(Debug)]
pub struct ScanProgress {
    pub entries: u64,
    pub logical_bytes: u64,
    pub current_path: PathBuf,
}

#[derive(Debug)]
pub enum ScanEvent {
    Progress(ScanProgress),
    Preview(ScanResult),
    Finished(Result<ScanResult, String>),
}

#[derive(Clone, Copy, Default)]
struct Totals {
    logical: u64,
    allocated: u64,
    files: u64,
    errors: u64,
}
impl Totals {
    fn add(&mut self, node: &Node) {
        self.logical = self.logical.saturating_add(node.logical_bytes);
        self.allocated = self.allocated.saturating_add(node.allocated_bytes);
        self.files += node.file_count;
        self.errors += node.error_count;
    }
}
struct PreviewState {
    top_ancestor: Vec<usize>,
    totals: Totals,
    top_totals: HashMap<usize, Totals>,
    next_emit: Duration,
}
impl PreviewState {
    fn new(root: &Node) -> Self {
        let mut totals = Totals::default();
        totals.add(root);
        Self {
            top_ancestor: vec![0],
            totals,
            top_totals: HashMap::new(),
            next_emit: Duration::from_millis(400),
        }
    }
    fn add(&mut self, index: usize, parent: usize, node: &Node) {
        let top = if parent == 0 {
            index
        } else {
            self.top_ancestor[parent]
        };
        self.top_ancestor.push(top);
        self.totals.add(node);
        self.top_totals.entry(top).or_default().add(node);
    }
    fn error(&mut self, index: usize) {
        self.totals.errors += 1;
        if index != 0 {
            self.top_totals
                .entry(self.top_ancestor[index])
                .or_default()
                .errors += 1;
        }
    }
    fn snapshot(&self, nodes: &[Node], elapsed: Duration) -> ScanResult {
        fn shallow(node: &Node, parent: Option<usize>, total: Totals) -> Node {
            Node {
                component: node.component.clone(),
                name: node.name.clone(),
                parent,
                children: Vec::new(),
                is_dir: node.is_dir,
                logical_bytes: total.logical,
                allocated_bytes: total.allocated,
                file_count: total.files,
                error_count: total.errors,
            }
        }
        let mut preview = Vec::with_capacity(self.top_totals.len() + 1);
        preview.push(shallow(&nodes[0], None, self.totals));
        // The source root already supplies stable direct-child order; descendants
        // remain solely in the worker's final tree and are never cloned here.
        for &top in &nodes[0].children {
            let index = preview.len();
            preview.push(shallow(&nodes[top], Some(0), self.top_totals[&top]));
            preview[0].children.push(index);
        }
        ScanResult {
            root: node_path(nodes, 0),
            nodes: preview,
            elapsed,
            cancelled: true,
        }
    }
    fn emit_if_due(&mut self, nodes: &[Node], elapsed: Duration, sender: &mpsc::Sender<ScanEvent>) {
        if elapsed >= self.next_emit {
            let _ = sender.send(ScanEvent::Preview(self.snapshot(nodes, elapsed)));
            self.next_emit = elapsed + Duration::from_secs(1);
        }
    }
}

pub struct ScanHandle {
    pub receiver: mpsc::Receiver<ScanEvent>,
    pub cancel: Arc<AtomicBool>,
}

pub fn spawn_scan(root: PathBuf) -> ScanHandle {
    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    std::thread::spawn(move || {
        let result = scan_path(&root, &worker_cancel, Some(&sender)).map_err(|e| e.to_string());
        let _ = sender.send(ScanEvent::Finished(result));
    });
    ScanHandle { receiver, cancel }
}

pub fn scan_path(
    root: &Path,
    cancel: &AtomicBool,
    sender: Option<&mpsc::Sender<ScanEvent>>,
) -> io::Result<ScanResult> {
    let workers = std::env::var("DISK_LENS_SCAN_WORKERS")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(4, |n| n.get().saturating_mul(4).clamp(4, 32))
        });
    scan_path_with_workers(root, cancel, sender, true, workers.clamp(1, 64))
}

#[cfg(test)]
fn scan_path_with(
    root: &Path,
    cancel: &AtomicBool,
    sender: Option<&mpsc::Sender<ScanEvent>>,
    prefer_bulk: bool,
) -> io::Result<ScanResult> {
    scan_path_with_workers(root, cancel, sender, prefer_bulk, 4)
}

fn scan_path_with_workers(
    root: &Path,
    cancel: &AtomicBool,
    sender: Option<&mpsc::Sender<ScanEvent>>,
    prefer_bulk: bool,
    workers: usize,
) -> io::Result<ScanResult> {
    let start = Instant::now();
    let root = if root.is_absolute() {
        root.to_owned()
    } else {
        std::env::current_dir()?.join(root)
    };
    let root_meta = crate::platform::metadata(&root)?;
    let device = root_meta.dev();
    let mut nodes = vec![make_node(root.clone(), None, &root_meta)];
    let mut preview = sender.map(|_| PreviewState::new(&nodes[0]));
    let mut seen = HashSet::new();
    if !root_meta.is_dir() && root_meta.nlink() > 1 {
        seen.insert((root_meta.dev(), root_meta.ino()));
    }
    let mut pending = if root_meta.is_dir() {
        vec![0]
    } else {
        Vec::new()
    };
    let mut entries = 1;
    let mut logical_bytes = nodes[0].logical_bytes;
    let mut last_update = Instant::now();
    std::thread::scope(|scope| {
        let (jobs, receiver) = mpsc::sync_channel::<(usize, PathBuf)>(workers);
        let receiver = Arc::new(Mutex::new(receiver));
        let (output, completed) = mpsc::sync_channel(workers);
        for _ in 0..workers {
            let receiver = receiver.clone();
            let output = output.clone();
            scope.spawn(move || {
                bulk::prevent_file_hydration();
                let mut buffer = bulk::Buffer::default();
                loop {
                    let Ok((parent, path)) = receiver.lock().unwrap().recv() else {
                        break;
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        read_entries(&path, cancel, prefer_bulk, &mut buffer)
                    }))
                    .unwrap_or_else(|_| Err(io::Error::other("Directory worker failed")));
                    if output.send((parent, path, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(output);
        let mut in_flight = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if let (Some(preview), Some(sender)) = (&mut preview, sender) {
                preview.emit_if_due(&nodes, start.elapsed(), sender);
            }
            while in_flight < workers
                && let Some(parent) = pending.pop()
            {
                let path = node_path(&nodes, parent);
                if jobs.send((parent, path)).is_err() {
                    break;
                }
                in_flight += 1;
            }
            if in_flight == 0 {
                break;
            }
            let Ok((parent, directory_path, result)) = completed.recv() else {
                break;
            };
            in_flight -= 1;
            let (entries_in_directory, errors) = match result {
                Ok(value) => value,
                Err(_) if cancel.load(Ordering::Relaxed) => break,
                Err(_) => {
                    nodes[parent].error_count += 1;
                    if let Some(preview) = &mut preview {
                        preview.error(parent);
                    }
                    continue;
                }
            };
            nodes[parent].error_count += errors;
            if let Some(preview) = &mut preview {
                for _ in 0..errors {
                    preview.error(parent);
                }
            }
            nodes[parent].children.reserve(entries_in_directory.len());
            for entry in entries_in_directory {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if entry.dev != device {
                    continue;
                }
                let duplicate =
                    !entry.is_dir && entry.nlink > 1 && !seen.insert((entry.dev, entry.ino));
                let mut node = Node {
                    name: crate::display::text(&entry.name.to_string_lossy()),
                    component: PathBuf::from(entry.name),
                    parent: Some(parent),
                    children: Vec::new(),
                    is_dir: entry.is_dir,
                    logical_bytes: entry.logical,
                    allocated_bytes: entry.allocated,
                    file_count: u64::from(!entry.is_dir),
                    error_count: 0,
                };
                if duplicate {
                    node.logical_bytes = 0;
                    node.allocated_bytes = 0;
                }
                logical_bytes += node.logical_bytes;
                let index = nodes.len();
                if node.is_dir {
                    pending.push(index);
                }
                if let Some(preview) = &mut preview {
                    preview.add(index, parent, &node);
                }
                nodes.push(node);
                nodes[parent].children.push(index);
                entries += 1;
                if let (Some(preview), Some(sender)) = (&mut preview, sender) {
                    preview.emit_if_due(&nodes, start.elapsed(), sender);
                }
                if last_update.elapsed() >= Duration::from_millis(120) {
                    if let Some(sender) = sender {
                        let _ = sender.send(ScanEvent::Progress(ScanProgress {
                            entries,
                            logical_bytes,
                            current_path: directory_path.clone(),
                        }));
                    }
                    last_update = Instant::now();
                }
            }
        }
        drop(jobs);
        drop(completed);
    });
    // Every parent precedes its children, so reverse aggregation is iterative and stack safe.
    for index in (1..nodes.len()).rev() {
        let parent = nodes[index].parent.unwrap();
        nodes[parent].logical_bytes = nodes[parent]
            .logical_bytes
            .saturating_add(nodes[index].logical_bytes);
        nodes[parent].allocated_bytes = nodes[parent]
            .allocated_bytes
            .saturating_add(nodes[index].allocated_bytes);
        nodes[parent].file_count += nodes[index].file_count;
        nodes[parent].error_count += nodes[index].error_count;
    }
    Ok(ScanResult {
        root,
        nodes,
        elapsed: start.elapsed(),
        cancelled: cancel.load(Ordering::Relaxed),
    })
}

fn read_entries(
    path: &Path,
    cancel: &AtomicBool,
    prefer_bulk: bool,
    buffer: &mut bulk::Buffer,
) -> io::Result<(Vec<bulk::Entry>, u64)> {
    if prefer_bulk {
        match bulk::read_dir(path, cancel, buffer) {
            Ok(entries) => return Ok((entries, 0)),
            Err(error) if cancel.load(Ordering::Relaxed) || crate::platform::is_loop(&error) => {
                return Err(error);
            }
            Err(_) => {} // Unsupported filesystems and partial native records retry safely.
        }
    }
    let mut entries = Vec::new();
    let mut errors = 0;
    for entry in fs::read_dir(path)? {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Scan cancelled"));
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let metadata = match crate::platform::metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        entries.push(bulk::Entry {
            name: entry.file_name(),
            is_dir: metadata.is_dir(),
            dev: metadata.dev(),
            ino: metadata.ino(),
            nlink: metadata.nlink(),
            logical: if metadata.is_dir() { 0 } else { metadata.len() },
            allocated: metadata.blocks().saturating_mul(512),
        });
    }
    Ok((entries, errors))
}

fn make_node(path: PathBuf, parent: Option<usize>, metadata: &crate::platform::Metadata) -> Node {
    let is_dir = metadata.is_dir();
    Node {
        name: path
            .file_name()
            .map(|s| crate::display::text(&s.to_string_lossy()))
            .unwrap_or_else(|| crate::display::path(&path)),
        component: if parent.is_some() {
            path.file_name().map(PathBuf::from).unwrap_or(path)
        } else {
            path
        },
        parent,
        children: Vec::new(),
        is_dir,
        logical_bytes: if is_dir { 0 } else { metadata.len() },
        allocated_bytes: metadata.blocks().saturating_mul(512),
        file_count: u64::from(!is_dir),
        error_count: 0,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiskSpace {
    pub total: u64,
    pub available: u64,
    pub free: u64,
}

#[cfg(target_os = "macos")]
pub fn disk_space(path: &Path) -> io::Result<DiskSpace> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Path contains NUL"))?;
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // statfs writes the structure only on success; CString supplies a valid C path.
    if unsafe { libc::statfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    let block = stat.f_bsize as u64;
    Ok(DiskSpace {
        total: stat.f_blocks.saturating_mul(block),
        available: stat.f_bavail.saturating_mul(block),
        free: stat.f_bfree.saturating_mul(block),
    })
}

#[cfg(windows)]
pub fn disk_space(path: &Path) -> io::Result<DiskSpace> {
    crate::platform::disk_space(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "space-disk-test-{}-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
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

    #[cfg(windows)]
    #[test]
    fn windows_sparse_allocation_is_smaller_than_logical_size() {
        use std::{io::Write, os::windows::io::AsRawHandle};
        use windows_sys::Win32::System::{IO::DeviceIoControl, Ioctl::FSCTL_SET_SPARSE};
        let fixture = Fixture::new();
        let path = fixture.0.join("sparse");
        let mut file = fs::File::create(&path).unwrap();
        let mut returned = 0;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    file.as_raw_handle(),
                    FSCTL_SET_SPARSE,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        file.set_len(16 * 1024 * 1024).unwrap();
        file.write_all(&[1u8; 4096]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let result = scan_path(&fixture.0, &AtomicBool::new(false), None).unwrap();
        assert_eq!(result.nodes[0].logical_bytes, 16 * 1024 * 1024);
        assert!(result.nodes[0].allocated_bytes >= 4096);
        assert!(result.nodes[0].allocated_bytes < result.nodes[0].logical_bytes);
    }

    #[cfg(windows)]
    #[test]
    fn windows_hardlinks_unicode_and_junctions() {
        let fixture = Fixture::new();
        let nested = fixture.0.join("한글연구");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("data"), [7u8; 8192]).unwrap();
        fs::hard_link(nested.join("data"), fixture.0.join("duplicate")).unwrap();
        let junction = fixture.0.join("loop");
        let output = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&fixture.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = scan_path(&fixture.0, &AtomicBool::new(false), None).unwrap();
        assert_eq!(result.nodes.len(), 5);
        assert_eq!(result.nodes[0].logical_bytes, 8192);
        assert_eq!(result.nodes[0].error_count, 0);
        assert_eq!(
            result.nodes[0].allocated_bytes,
            crate::platform::metadata(nested.join("data"))
                .unwrap()
                .blocks()
                * 512
        );
        assert!(
            !result
                .nodes
                .iter()
                .find(|n| n.name == "loop")
                .unwrap()
                .is_dir
        );
        assert!(result.nodes.iter().any(|n| n.name == "한글연구"));
        fs::remove_dir(junction).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn aggregates_and_deduplicates_hardlinks_without_following_symlinks() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("nested")).unwrap();
        fs::write(fixture.0.join("nested/data"), [3u8; 8192]).unwrap();
        fs::hard_link(fixture.0.join("nested/data"), fixture.0.join("duplicate")).unwrap();
        symlink(&fixture.0, fixture.0.join("loop")).unwrap();
        let result = scan_path(&fixture.0, &AtomicBool::new(false), None).unwrap();
        assert_eq!(result.nodes.len(), 5);
        assert_eq!(result.nodes[0].file_count, 3);
        let link_length = crate::platform::metadata(fixture.0.join("loop"))
            .unwrap()
            .len();
        assert_eq!(result.nodes[0].logical_bytes, 8192 + link_length);
        assert_eq!(result.nodes[0].error_count, 0);
        assert!(!result.cancelled);
    }

    #[test]
    fn preview_matches_final_totals_without_mutating_worker_tree() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("nested")).unwrap();
        fs::write(fixture.0.join("nested/data"), [1u8; 8192]).unwrap();
        fs::write(fixture.0.join("root-file"), [2u8; 13]).unwrap();
        let complete = scan_path(&fixture.0, &AtomicBool::new(false), None).unwrap();
        let mut raw = Vec::new();
        for (index, node) in complete.nodes.iter().enumerate() {
            let path = node_path(&complete.nodes, index);
            let meta = crate::platform::metadata(&path).unwrap();
            let mut local = make_node(path, node.parent, &meta);
            local.children = node.children.clone();
            raw.push(local);
        }
        let mut tracker = PreviewState::new(&raw[0]);
        for (i, node) in raw.iter().enumerate().skip(1) {
            tracker.add(i, node.parent.unwrap(), node);
        }
        let nested = raw.iter().position(|n| n.name == "nested").unwrap();
        tracker.error(nested);
        let (sender, receiver) = mpsc::channel();
        tracker.emit_if_due(&raw, Duration::from_millis(399), &sender);
        assert!(receiver.try_recv().is_err());
        tracker.emit_if_due(&raw, Duration::from_millis(400), &sender);
        let ScanEvent::Preview(snapshot) = receiver.try_recv().unwrap() else {
            panic!("expected preview")
        };
        assert!(snapshot.cancelled);
        assert_eq!(snapshot.nodes.len(), raw[0].children.len() + 1);
        assert_eq!(
            snapshot.nodes[0].logical_bytes,
            complete.nodes[0].logical_bytes
        );
        assert_eq!(
            snapshot.nodes[0].allocated_bytes,
            complete.nodes[0].allocated_bytes
        );
        assert_eq!(snapshot.nodes[0].file_count, complete.nodes[0].file_count);
        assert_eq!(snapshot.nodes[0].error_count, 1);
        for (i, node) in snapshot.nodes.iter().enumerate().skip(1) {
            assert_eq!(node.parent, Some(0));
            assert!(node.children.is_empty());
            assert_eq!(snapshot.nodes[0].children[i - 1], i);
            let final_node = complete
                .nodes
                .iter()
                .find(|n| n.component == node.component)
                .unwrap();
            assert_eq!(node.allocated_bytes, final_node.allocated_bytes);
            assert_eq!(node.logical_bytes, final_node.logical_bytes);
            assert_eq!(node.file_count, final_node.file_count);
        }
        assert_eq!(raw[0].logical_bytes, 0);
        assert_eq!(raw[nested].error_count, 0);
        tracker.emit_if_due(&raw, Duration::from_millis(1399), &sender);
        assert!(receiver.try_recv().is_err());
        tracker.emit_if_due(&raw, Duration::from_millis(1400), &sender);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            ScanEvent::Preview(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn decomposed_names_display_composed_and_sparse_files_keep_physical_size() {
        let fixture = Fixture::new();
        let file_path = fixture.0.join("연구보고서");
        let file = fs::File::create(&file_path).unwrap();
        file.set_len(16 * 1024 * 1024).unwrap();
        let result = scan_path(&fixture.0, &AtomicBool::new(false), None).unwrap();
        let node = &result.nodes[1];
        assert_eq!(node.name, "연구보고서");
        assert_eq!(node_path(&result.nodes, 1), file_path);
        assert!(!node.component.is_absolute());
        assert_eq!(node.logical_bytes, 16 * 1024 * 1024);
        assert_eq!(
            node.allocated_bytes,
            fs::metadata(&file_path).unwrap().blocks() * 512
        );
        assert!(node.allocated_bytes < node.logical_bytes);
    }

    #[test]
    fn bulk_and_standard_walk_have_identical_trees_and_accounting() {
        let fixture = Fixture::new();
        for directory in 0..12 {
            let path = fixture.0.join(format!("directory-{directory}"));
            fs::create_dir(&path).unwrap();
            for file in 0..180 {
                fs::write(path.join(format!("file-{file:03}-한글")), [4u8; 37]).unwrap();
            }
        }
        let cancel = AtomicBool::new(false);
        let bulk = scan_path_with(&fixture.0, &cancel, None, true).unwrap();
        let standard = scan_path_with(&fixture.0, &cancel, None, false).unwrap();
        let map = |r: &ScanResult| {
            r.nodes
                .iter()
                .enumerate()
                .map(|(id, n)| {
                    (
                        node_path(&r.nodes, id),
                        (
                            n.logical_bytes,
                            n.allocated_bytes,
                            n.file_count,
                            n.error_count,
                        ),
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(map(&bulk), map(&standard));
        assert_eq!(bulk.nodes.len(), 2173);
        assert!(
            bulk.nodes
                .iter()
                .skip(1)
                .all(|n| n.component.components().count() == 1)
        );
    }

    #[test]
    fn parallel_workers_deduplicate_hardlinks_across_directories() {
        let fixture = Fixture::new();
        let payload = fixture.0.join("payload");
        fs::write(&payload, [7u8; 65536]).unwrap();
        for i in 0..80 {
            let directory = fixture.0.join(format!("dir-{i}"));
            fs::create_dir(&directory).unwrap();
            fs::hard_link(&payload, directory.join("shared")).unwrap();
        }
        let cancel = AtomicBool::new(false);
        let serial = scan_path_with_workers(&fixture.0, &cancel, None, true, 1).unwrap();
        let parallel = scan_path_with_workers(&fixture.0, &cancel, None, true, 16).unwrap();
        assert_eq!(parallel.nodes.len(), serial.nodes.len());
        assert_eq!(parallel.nodes[0].logical_bytes, 65536);
        assert_eq!(
            parallel.nodes[0].allocated_bytes,
            serial.nodes[0].allocated_bytes
        );
        assert_eq!(parallel.nodes[0].file_count, 81);
        assert_eq!(parallel.nodes[0].error_count, 0);
    }

    #[test]
    fn cancelling_parallel_scan_releases_workers() {
        let fixture = Fixture::new();
        for i in 0..100 {
            fs::create_dir(fixture.0.join(format!("dir-{i}"))).unwrap();
        }
        let root = fixture.0.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let result = scan_path_with_workers(&root, &worker_cancel, None, true, 16);
            tx.send(result).unwrap();
        });
        cancel.store(true, Ordering::Relaxed);
        assert!(
            rx.recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap()
                .cancelled
        );
        handle.join().unwrap();
    }

    #[test]
    fn cancellation_returns_partial_tree() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("data"), b"hello").unwrap();
        let result = scan_path(&fixture.0, &AtomicBool::new(true), None).unwrap();
        assert!(result.cancelled);
        assert_eq!(result.nodes.len(), 1);
    }

    #[test]
    fn missing_root_is_reported_and_volume_capacity_is_available() {
        let fixture = Fixture::new();
        assert!(scan_path(&fixture.0.join("missing"), &AtomicBool::new(false), None).is_err());
        let space = disk_space(&fixture.0).unwrap();
        assert!(space.total > 0);
        assert!(space.available <= space.free && space.free <= space.total);
    }
}
