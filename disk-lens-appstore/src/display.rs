use std::path::Path;
use unicode_normalization::UnicodeNormalization;

// macOS file names often use decomposed Hangul. egui renders precomposed syllables
// reliably; normalize labels only, preserving the original filesystem paths.
pub fn text(value: &str) -> String {
    value.nfc().collect()
}
pub fn path(value: &Path) -> String {
    text(&value.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decomposed_hangul_is_composed_without_changing_path() {
        let original = std::path::PathBuf::from("/tmp/2025연구보고서");
        assert_eq!(path(&original), "/tmp/2025연구보고서");
        assert_eq!(original.as_os_str(), "/tmp/2025연구보고서");
        assert_eq!(text("한글 abc 😀"), "한글 abc 😀");
    }
}
