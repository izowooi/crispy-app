use crate::charts::ChartMode;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// 샌드박스 안에서 HOME 은 앱 컨테이너다. 설정은 그 안에만 저장된다.
fn preferences_path() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory is unavailable"))?;
    Ok(PathBuf::from(home).join("Library/Application Support/DiskLens/preferences"))
}

pub fn load_chart() -> ChartMode {
    preferences_path()
        .map(|path| load_chart_from(&path))
        .unwrap_or(ChartMode::Treemap)
}

pub fn save_chart(mode: ChartMode) -> io::Result<()> {
    save_chart_to(&preferences_path()?, mode)
}

fn load_chart_from(path: &Path) -> ChartMode {
    let sunburst = fs::read_to_string(path)
        .is_ok_and(|contents| contents.lines().any(|line| line.trim() == "chart=sunburst"));
    if sunburst {
        ChartMode::Sunburst
    } else {
        ChartMode::Treemap
    }
}

fn save_chart_to(path: &Path, mode: ChartMode) -> io::Result<()> {
    let contents = match mode {
        ChartMode::Sunburst => "chart=sunburst\n",
        ChartMode::Treemap => "chart=treemap\n",
    };
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

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "disk-lens-store-preferences-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn chart_round_trips_and_unknown_values_fall_back_to_treemap() {
        let fixture = Fixture::new();
        let path = fixture.0.join("nested/preferences");
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        save_chart_to(&path, ChartMode::Sunburst).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Sunburst);
        save_chart_to(&path, ChartMode::Treemap).unwrap();
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        fs::write(&path, "chart=unknown\n").unwrap();
        assert!(load_chart_from(&path) == ChartMode::Treemap);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
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
}
