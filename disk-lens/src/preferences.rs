use crate::charts::ChartMode;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn preferences_path() -> io::Result<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) {
        "LOCALAPPDATA"
    } else {
        "HOME"
    })
    .filter(|value| !value.is_empty())
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory is unavailable"))?;
    Ok(PathBuf::from(home).join(if cfg!(windows) {
        "Disk Lens/preferences"
    } else {
        "Library/Application Support/Disk Lens/preferences"
    }))
}

pub fn load_chart() -> ChartMode {
    preferences_path()
        .map(|path| load_chart_from(&path))
        .unwrap_or(ChartMode::Treemap)
}

pub fn save_chart(mode: ChartMode) -> io::Result<()> {
    save_chart_to(&preferences_path()?, mode)
}

pub fn load_last_path() -> Option<PathBuf> {
    preferences_path()
        .ok()
        .and_then(|path| load_from(&path).last_path)
}

pub fn save_last_path(last_path: &Path) -> io::Result<()> {
    save_last_path_to(&preferences_path()?, last_path)
}

struct Preferences {
    chart: ChartMode,
    last_path: Option<PathBuf>,
}

fn load_from(path: &Path) -> Preferences {
    let mut preferences = Preferences {
        chart: ChartMode::Treemap,
        last_path: None,
    };
    if let Ok(contents) = fs::read_to_string(path) {
        for line in contents.lines() {
            if let Some(value) = line.trim().strip_prefix("chart=") {
                preferences.chart = if value == "sunburst" {
                    ChartMode::Sunburst
                } else {
                    ChartMode::Treemap
                };
            } else if let Some(value) = line.trim().strip_prefix("last_path_hex=") {
                preferences.last_path = decode_path(value);
            }
        }
    }
    preferences
}

fn decode_path(value: &str) -> Option<PathBuf> {
    if value.is_empty() || !value.len().is_multiple_of(2) || !value.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let text = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect();
    let bytes = bytes?;
    if cfg!(unix) && bytes.contains(&0) {
        return None;
    }
    path_from_bytes(bytes)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}
#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}
#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    Some(std::ffi::OsString::from_vec(bytes).into())
}
#[cfg(windows)]
fn path_from_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let wide: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    if wide.contains(&0) {
        return None;
    }
    Some(std::ffi::OsString::from_wide(&wide).into())
}

fn load_chart_from(path: &Path) -> ChartMode {
    load_from(path).chart
}

fn save_chart_to(path: &Path, mode: ChartMode) -> io::Result<()> {
    let mut preferences = load_from(path);
    preferences.chart = mode;
    write_preferences(path, &preferences)
}

fn save_last_path_to(path: &Path, last_path: &Path) -> io::Result<()> {
    let bytes = path_bytes(last_path);
    if bytes.is_empty() || last_path.as_os_str().to_string_lossy().contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Last path is empty or contains NUL",
        ));
    }
    let mut preferences = load_from(path);
    preferences.last_path = Some(last_path.to_owned());
    write_preferences(path, &preferences)
}

fn write_preferences(path: &Path, preferences: &Preferences) -> io::Result<()> {
    let mut contents = match preferences.chart {
        ChartMode::Sunburst => String::from("chart=sunburst\n"),
        ChartMode::Treemap => String::from("chart=treemap\n"),
    };
    if let Some(last_path) = &preferences.last_path {
        contents.push_str("last_path_hex=");
        const HEX: &[u8] = b"0123456789abcdef";
        for byte in path_bytes(last_path) {
            contents.push(HEX[(byte >> 4) as usize] as char);
            contents.push(HEX[(byte & 15) as usize] as char);
        }
        contents.push('\n');
    }
    atomic_write(path, contents.as_bytes())
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Preferences path has no parent directory",
        )
    })?;
    fs::create_dir_all(directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = directory.join(format!(".preferences-{}-{stamp}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        // A same-directory rename replaces the previous complete file atomically.
        crate::platform::replace_file(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "disk-lens-preferences-{}-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
                FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_and_invalid_preferences_default_to_treemap() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences");
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        assert!(!fixture.0.exists(), "loading must not create a directory");
        fs::create_dir_all(&fixture.0).unwrap();
        fs::write(&path, "chart=unknown\n").unwrap();
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Treemap);
    }

    #[test]
    fn saving_and_changing_chart_survives_reload() {
        let fixture = Fixture::new();
        let path = fixture.0.join("nested/preferences");
        save_chart_to(&path, ChartMode::Sunburst).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Sunburst);
        save_chart_to(&path, ChartMode::Treemap).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn either_save_order_preserves_chart_and_last_path() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences");
        let last = Path::new("/Users/example/Downloads");
        save_chart_to(&path, ChartMode::Sunburst).unwrap();
        save_last_path_to(&path, last).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Sunburst);
        assert_eq!(load_from(&path).last_path.as_deref(), Some(last));
        save_chart_to(&path, ChartMode::Treemap).unwrap();
        assert_eq!(load_from(&path).last_path.as_deref(), Some(last));
        let second = fixture.0.join("second");
        save_last_path_to(&second, last).unwrap();
        save_chart_to(&second, ChartMode::Sunburst).unwrap();
        assert!(load_chart_from(&second) == ChartMode::Sunburst);
        assert_eq!(load_from(&second).last_path.as_deref(), Some(last));
    }

    #[test]
    fn malformed_fields_fall_back_independently() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences");
        fs::create_dir_all(&fixture.0).unwrap();
        for invalid in ["", "0", "zz", "00", "한글"] {
            fs::write(&path, format!("chart=sunburst\nlast_path_hex={invalid}\n")).unwrap();
            assert!(load_chart_from(&path) == ChartMode::Sunburst);
            assert_eq!(load_from(&path).last_path, None);
        }
        let encoded: String = path_bytes(Path::new("/tmp"))
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        fs::write(&path, format!("chart=unknown\nlast_path_hex={encoded}\n")).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        assert_eq!(load_from(&path).last_path, Some(PathBuf::from("/tmp")));
    }

    #[cfg(unix)]
    #[test]
    fn raw_decomposed_unicode_and_non_utf8_paths_round_trip() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences");
        let mut raw = "/tmp/한글\n".as_bytes().to_vec();
        raw.push(0xff);
        let last = PathBuf::from(std::ffi::OsString::from_vec(raw.clone()));
        save_last_path_to(&path, &last).unwrap();
        assert_eq!(
            load_from(&path).last_path.unwrap().as_os_str().as_bytes(),
            raw
        );
        save_chart_to(&path, ChartMode::Sunburst).unwrap();
        assert_eq!(
            load_from(&path).last_path.unwrap().as_os_str().as_bytes(),
            raw
        );
    }

    #[test]
    fn save_failure_preserves_existing_destination() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences");
        fs::create_dir_all(&path).unwrap();
        assert!(save_chart_to(&path, ChartMode::Sunburst).is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_raw_utf16_paths_survive_replacement() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences");
        let mut wide: Vec<u16> = "C:\\한글\\연구\\".encode_utf16().collect();
        wide.push(0xd800); // Windows paths may contain an unpaired surrogate.
        let last = PathBuf::from(std::ffi::OsString::from_wide(&wide));
        save_last_path_to(&path, &last).unwrap();
        save_chart_to(&path, ChartMode::Sunburst).unwrap();
        assert_eq!(load_from(&path).last_path, Some(last));
        assert!(load_chart_from(&path) == ChartMode::Sunburst);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}
