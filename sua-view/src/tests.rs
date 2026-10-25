use super::*;
use std::ffi::{CStr, CString};
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicU64, Ordering};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "sua-view-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn zip(&self, name: &str, entries: &[(&str, &[u8])], compressed: bool) -> PathBuf {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let compression = if compressed {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let options = zip::write::SimpleFileOptions::default().compression_method(compression);
        for (name, content) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        self.file(name, &zip.finish().unwrap().into_inner())
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn natural_order_is_numeric_and_unicode_safe() {
    let mut names = [
        "10.jpg",
        "02.jpg",
        "2.jpg",
        "1.jpg",
        "10000000000000000000000.jpg",
        "만화10.jpg",
        "만화2.jpg",
    ];
    names.sort_by(|a, b| natural::compare(&a.to_lowercase(), &b.to_lowercase()));
    assert_eq!(
        names,
        [
            "1.jpg",
            "2.jpg",
            "02.jpg",
            "10.jpg",
            "10000000000000000000000.jpg",
            "만화2.jpg",
            "만화10.jpg"
        ]
    );
    assert_eq!(
        natural::compare("a2.png", "a10.png"),
        std::cmp::Ordering::Less
    );
}

#[test]
fn buffered_absolute_and_relative_seeks_preserve_exact_file_positions() {
    use std::io::{Seek, SeekFrom};
    let sandbox = Sandbox::new();
    let bytes: Vec<_> = (0..200_000).map(|index| (index % 251) as u8).collect();
    let path = sandbox.file("seek.bin", &bytes);
    let mut reader = buffered_file::BufferedFile::new(File::open(path).unwrap());
    let mut result = [0_u8; 19];
    for position in [0, 20, 2, 100_000, 65_535, 199_900, 100] {
        assert_eq!(reader.seek(SeekFrom::Start(position)).unwrap(), position);
        reader.read_exact(&mut result).unwrap();
        assert_eq!(
            &result,
            &bytes[position as usize..position as usize + result.len()]
        );
        assert_eq!(
            reader.stream_position().unwrap(),
            position + result.len() as u64
        );
        reader.seek(SeekFrom::Current(-19)).unwrap();
        assert_eq!(reader.stream_position().unwrap(), position);
    }
    assert_eq!(
        reader.seek(SeekFrom::End(-19)).unwrap(),
        bytes.len() as u64 - 19
    );
    reader.read_exact(&mut result).unwrap();
    assert_eq!(&result, &bytes[bytes.len() - 19..]);
    assert!(reader.seek(SeekFrom::Start(0)).is_ok());
    assert!(reader.seek(SeekFrom::Current(-1)).is_err());
}

#[test]
fn zip_index_filters_metadata_and_reads_randomly_without_extracting() {
    let sandbox = Sandbox::new();
    let path = sandbox.zip(
        "book.cbz",
        &[
            ("10.JPG", b"ten"),
            ("2.png", b"two"),
            ("1.webp", b"one"),
            ("__MACOSX/._1.webp", b"metadata"),
            (".hidden.jpg", b"hidden"),
            ("sub/.hidden/4.jpg", b"hidden"),
            ("ComicInfo.xml", b"xml"),
            ("../outside.jpg", b"bad path"),
            ("sub/3.jpg", b"three"),
        ],
        true,
    );
    let mut book = Book::open(path).unwrap();
    assert_eq!(book.source_kind(), "zip-indexed");
    assert_eq!(book.len(), 4);
    assert_eq!(book.page_name(0), Some("1.webp"));
    assert_eq!(book.page_name(1), Some("2.png"));
    assert_eq!(book.read_page(3).unwrap(), b"three");
    assert_eq!(book.read_page(0).unwrap(), b"one");
    assert_eq!(book.read_page(2).unwrap(), b"ten");
    assert!(book.read_page(4).is_err());
    assert_eq!(
        fs::read_dir(&sandbox.0).unwrap().count(),
        1,
        "No archive member is extracted"
    );
}

#[test]
fn stored_zip_is_supported() {
    let sandbox = Sandbox::new();
    let path = sandbox.zip("book.zip", &[("page.png", b"stored bytes")], false);
    assert_eq!(
        Book::open(path).unwrap().read_page(0).unwrap(),
        b"stored bytes"
    );
}

#[test]
fn zip_content_uses_indexed_reader_even_with_cbr_extension() {
    let sandbox = Sandbox::new();
    let path = sandbox.zip(
        "mislabeled.cbr",
        &[("./page2.jpg", b"two"), ("./page1.jpg", b"one")],
        false,
    );
    let mut book = Book::open(path).unwrap();
    assert_eq!(book.source_kind(), "zip-indexed");
    assert_eq!(book.page_name(0), Some("page1.jpg"));
    assert_eq!(book.read_page(1).unwrap(), b"two");
}

#[test]
fn corrupt_zip_page_has_error_after_successful_metadata_index() {
    let sandbox = Sandbox::new();
    let path = sandbox.zip("corrupt.cbz", &[("page1.jpg", b"uncorrupted")], false);
    let mut bytes = fs::read(&path).unwrap();
    let data_offset = 30
        + u16::from_le_bytes([bytes[26], bytes[27]]) as usize
        + u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    bytes[data_offset] ^= 0xFF;
    fs::write(&path, bytes).unwrap();
    let mut book = Book::open(path).unwrap();
    assert!(book.read_page(0).is_err());
}

#[test]
fn folder_is_recursive_and_single_image_opens_siblings_at_selected_page() {
    let sandbox = Sandbox::new();
    sandbox.file("10.jpg", b"ten");
    let selected = sandbox.file("2.jpg", b"two");
    sandbox.file("1.jpg", b"one");
    sandbox.file("sub/3.jpg", b"three");
    sandbox.file(".hidden/4.jpg", b"hidden");
    sandbox.file("__MACOSX/5.jpg", b"metadata");
    let mut folder = Book::open(&sandbox.0).unwrap();
    assert_eq!(folder.len(), 4);
    assert_eq!(folder.read_page(1).unwrap(), b"two");
    let mut image = Book::open(selected).unwrap();
    assert_eq!(image.len(), 3);
    assert_eq!(image.initial_page(), 1);
    assert_eq!(image.read_page(image.initial_page()).unwrap(), b"two");
}

#[test]
fn missing_invalid_empty_and_unsupported_inputs_have_errors() {
    let sandbox = Sandbox::new();
    assert!(Book::open(sandbox.0.join("missing.zip")).is_err());
    assert!(
        Book::open(&sandbox.0)
            .err()
            .unwrap()
            .contains("No supported")
    );
    assert!(
        Book::open(sandbox.file("bad.zip", b"not zip"))
            .err()
            .unwrap()
            .contains("Invalid ZIP")
    );
    assert!(Book::open(sandbox.file("notes.txt", b"notes")).is_err());
    let empty = sandbox.zip("empty.cbz", &[("notes.txt", b"text")], false);
    assert!(Book::open(empty).err().unwrap().contains("No supported"));
}

#[test]
fn page_size_is_bounded_before_allocation() {
    assert!(
        read_bounded(Cursor::new(b"small"), MAX_PAGE_BYTES + 1)
            .unwrap_err()
            .contains("256 MiB")
    );
}

#[test]
fn ffi_owns_strings_buffers_and_reports_bad_calls() {
    use crate::ffi::*;
    let sandbox = Sandbox::new();
    let path = sandbox.zip("book.cbz", &[("2.jpg", b"two"), ("1.jpg", b"one")], false);
    let path = CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        assert_eq!(sua_page_is_landscape(1600.0, 2400.0), 0);
        assert_eq!(sua_page_is_landscape(2400.0, 1600.0), 1);
        assert_eq!(sua_can_pair(0, 3, 1, 1, 0, 0), 0);
        assert_eq!(sua_can_pair(1, 3, 1, 1, 0, 0), 1);
        assert_eq!(sua_next_start(1, 3, 2), 1);
        assert_eq!(sua_previous_start(3, 1, 1, 0, 0), 1);
        assert_eq!(sua_path_is_image(std::ptr::null()), 0);
        let image_path = CString::new("한글/책.JpG").unwrap();
        assert_eq!(sua_path_is_image(image_path.as_ptr()), 1);
        let mut error = std::ptr::null_mut();
        let book = sua_book_open(path.as_ptr(), &mut error);
        assert!(!book.is_null());
        assert!(error.is_null());
        assert_eq!(sua_book_count(book), 2);
        assert_eq!(sua_book_initial_page(book), 0);
        let name = sua_book_page_name(book, 0);
        assert_eq!(CStr::from_ptr(name).to_str().unwrap(), "1.jpg");
        sua_string_free(name);
        let mut length = 0;
        let bytes = sua_book_read_page(book, 1, &mut length, &mut error);
        assert!(error.is_null());
        assert_eq!(std::slice::from_raw_parts(bytes, length), b"two");
        sua_bytes_free(bytes, length);
        assert!(sua_book_read_page(book, 999, &mut length, &mut error).is_null());
        assert_eq!(length, 0);
        assert!(!error.is_null());
        sua_string_free(error);
        sua_book_close(book);
        assert!(sua_book_open(std::ptr::null(), &mut error).is_null());
        assert!(!error.is_null());
        sua_string_free(error);
        assert_eq!(sua_book_count(std::ptr::null()), 0);
        sua_book_close(std::ptr::null_mut());
        sua_bytes_free(std::ptr::null_mut(), 0);
        sua_string_free(std::ptr::null_mut());
    }
}

#[cfg(unix)]
#[test]
fn libarchive_fallback_reads_forward_backward_and_natural_order() {
    fn tar_header(name: &str, data: &[u8]) -> Vec<u8> {
        let mut header = [0u8; 512];
        header[..name.len()].copy_from_slice(name.as_bytes());
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        header[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
        header[136..148].copy_from_slice(b"00000000000\0");
        header[148..156].fill(b' ');
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
        let mut result = header.to_vec();
        result.extend_from_slice(data);
        result.resize(result.len().div_ceil(512) * 512, 0);
        result
    }
    let sandbox = Sandbox::new();
    let mut tar = tar_header("./10.jpg", b"ten");
    tar.extend_from_slice(&tar_header("./2.jpg", b"two"));
    tar.extend_from_slice(&tar_header(".hidden.jpg", b"metadata"));
    tar.resize(tar.len() + 1024, 0);
    let mut book = Book::open(sandbox.file("book.cbt", &tar)).unwrap();
    assert_eq!(book.source_kind(), "libarchive-sequential");
    assert_eq!(book.len(), 2);
    assert_eq!(book.read_page(0).unwrap(), b"two");
    assert_eq!(book.read_page(1).unwrap(), b"ten");
    assert_eq!(book.read_page(0).unwrap(), b"two");
}

#[cfg(unix)]
#[test]
fn directory_does_not_follow_symlinks() {
    let sandbox = Sandbox::new();
    sandbox.file("1.jpg", b"one");
    std::os::unix::fs::symlink(&sandbox.0, sandbox.0.join("loop")).unwrap();
    std::os::unix::fs::symlink(sandbox.0.join("1.jpg"), sandbox.0.join("2.jpg")).unwrap();
    assert_eq!(Book::open(&sandbox.0).unwrap().len(), 1);
}
