mod archive;
mod buffered_file;
mod ffi;
mod natural;
pub mod reading;

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MAX_PAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PAGES: usize = 100_000;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
struct Page {
    name: String,
    location: Location,
}

#[derive(Debug)]
enum Location {
    File(PathBuf),
    Zip(usize),
    Archive(usize),
}

enum Source {
    Directory,
    Zip(Box<zip::ZipArchive<buffered_file::BufferedFile>>),
    Archive(archive::ArchiveSource),
}

pub struct Book {
    pages: Vec<Page>,
    source: Source,
    initial_page: usize,
}

impl Book {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = fs::canonicalize(path.as_ref()).map_err(|e| format!("Cannot open book: {e}"))?;
        if path.is_dir() {
            return Self::directory(&path, None, true);
        }
        if is_image(&path.to_string_lossy()) {
            let parent = path.parent().ok_or("Image has no parent folder")?;
            return Self::directory(parent, Some(&path), false);
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "zip" | "cbz" => Self::zip(&path),
            "rar" | "cbr" | "7z" | "cb7" | "tar" | "cbt" => {
                let mut file =
                    File::open(&path).map_err(|e| format!("Cannot read archive: {e}"))?;
                let mut magic = [0_u8; 4];
                if file.read_exact(&mut magic).is_ok()
                    && matches!(&magic, b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08")
                {
                    Self::zip(&path)
                } else {
                    Self::archive(&path)
                }
            }
            _ => Err("Open a ZIP/CBZ, RAR/CBR, 7z, image folder, or image file.".into()),
        }
    }

    fn finish(pages: Vec<Page>, source: Source, selected: Option<&Path>) -> Result<Self> {
        if pages.is_empty() {
            return Err("No supported image pages were found in this book.".into());
        }
        if pages.len() > MAX_PAGES {
            return Err(format!("Book exceeds the {MAX_PAGES} page limit."));
        }
        let mut keyed: Vec<_> = pages
            .into_iter()
            .map(|page| (page.name.to_lowercase(), page))
            .collect();
        keyed.sort_by(|a, b| natural::compare(&a.0, &b.0).then_with(|| a.1.name.cmp(&b.1.name)));
        let pages: Vec<_> = keyed.into_iter().map(|(_, page)| page).collect();
        let initial_page = selected
            .and_then(|selected| {
                pages.iter().position(
                    |page| matches!(&page.location, Location::File(path) if path == selected),
                )
            })
            .unwrap_or(0);
        Ok(Self {
            pages,
            source,
            initial_page,
        })
    }

    fn directory(root: &Path, selected: Option<&Path>, recursive: bool) -> Result<Self> {
        let mut pages = Vec::new();
        collect_directory(root, root, &mut pages, recursive, 0)?;
        Self::finish(pages, Source::Directory, selected)
    }

    fn zip(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| format!("Cannot read archive: {e}"))?;
        let zip = zip::ZipArchive::new(buffered_file::BufferedFile::new(file))
            .map_err(|e| format!("Invalid ZIP archive: {e}"))?;
        if zip.len() > MAX_PAGES * 4 {
            return Err("Archive contains too many entries.".into());
        }
        let mut pages = Vec::new();
        for index in 0..zip.len() {
            let entry = zip
                .by_index_data(index)
                .map_err(|e| format!("Cannot inspect ZIP entry: {e}"))?;
            let name = entry
                .name()
                .map_err(|e| format!("Cannot decode ZIP filename: {e}"))?;
            if entry.is_file() && !entry.is_symlink() && visible_image(&name) {
                pages.push(Page {
                    name: normalize_name(&name),
                    location: Location::Zip(index),
                });
            }
        }
        Self::finish(pages, Source::Zip(Box::new(zip)), None)
    }

    fn archive(path: &Path) -> Result<Self> {
        let (source, entries) = archive::ArchiveSource::open(path, MAX_PAGES * 4)?;
        let pages = entries
            .into_iter()
            .filter(|(_, name)| visible_image(name))
            .map(|(index, name)| Page {
                name: normalize_name(&name),
                location: Location::Archive(index),
            })
            .collect();
        Self::finish(pages, Source::Archive(source), None)
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
    pub fn initial_page(&self) -> usize {
        self.initial_page
    }
    pub fn page_name(&self, index: usize) -> Option<&str> {
        self.pages.get(index).map(|page| page.name.as_str())
    }
    pub fn source_kind(&self) -> &'static str {
        match self.source {
            Source::Directory => "folder",
            Source::Zip(_) => "zip-indexed",
            Source::Archive(_) => "libarchive-sequential",
        }
    }

    pub fn read_page(&mut self, index: usize) -> Result<Vec<u8>> {
        let page = self
            .pages
            .get(index)
            .ok_or_else(|| format!("Page {} is outside this book.", index + 1))?;
        match (&mut self.source, &page.location) {
            (Source::Directory, Location::File(path)) => {
                let file =
                    File::open(path).map_err(|e| format!("Cannot read {}: {e}", page.name))?;
                let length = file.metadata().map_err(|e| e.to_string())?.len();
                read_bounded(file, length)
            }
            (Source::Zip(zip), Location::Zip(index)) => {
                let entry = zip
                    .by_index(*index)
                    .map_err(|e| format!("Cannot read ZIP page: {e}"))?;
                let length = entry.size();
                read_bounded(entry, length)
            }
            (Source::Archive(source), Location::Archive(index)) => source.read(*index),
            _ => Err("Book source does not match its page index.".into()),
        }
    }
}

fn read_bounded(reader: impl Read, declared: u64) -> Result<Vec<u8>> {
    if declared > MAX_PAGE_BYTES {
        return Err("Image exceeds the 256 MiB per-page limit.".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(declared.min(8 * 1024 * 1024) as usize)
        .map_err(|_| "Not enough memory to read this page")?;
    reader
        .take(MAX_PAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot decompress page: {e}"))?;
    if bytes.len() as u64 > MAX_PAGE_BYTES {
        return Err("Image exceeds the 256 MiB per-page limit.".into());
    }
    Ok(bytes)
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    pages: &mut Vec<Page>,
    recursive: bool,
    depth: usize,
) -> Result<()> {
    if depth > 64 {
        return Err("Image folder nesting exceeds 64 levels.".into());
    }
    let entries = fs::read_dir(directory).map_err(|e| format!("Cannot read folder: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if !visible_path(&name) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() && recursive {
            collect_directory(root, &path, pages, recursive, depth + 1)?;
        }
        if kind.is_file() && is_image(&name) {
            pages.push(Page {
                name,
                location: Location::File(path),
            });
            if pages.len() > MAX_PAGES {
                return Err("Image folder has too many pages.".into());
            }
        }
    }
    Ok(())
}

fn visible_path(name: &str) -> bool {
    !name
        .split(['/', '\\'])
        .any(|part| (part.starts_with('.') && part != ".") || part == "__MACOSX")
}

fn normalize_name(name: &str) -> String {
    let name = name.replace('\\', "/");
    name.trim_start_matches("./").to_owned()
}

fn visible_image(name: &str) -> bool {
    visible_path(name) && is_image(name)
}

fn is_image(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "webp"
            | "gif"
            | "bmp"
            | "tif"
            | "tiff"
            | "avif"
            | "heic"
            | "heif"
            | "jxl"
    )
}

#[cfg(test)]
mod tests;
