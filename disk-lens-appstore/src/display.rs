use std::path::Path;
use unicode_normalization::UnicodeNormalization;

// macOS file names often use decomposed Hangul. egui renders precomposed syllables
// reliably; normalize labels only, preserving the original filesystem paths.
// 아랍 문자가 들어 있으면 글자를 잇고 화면 순서로 놓는다 (arabic.rs).
pub fn text(value: &str) -> String {
    let composed: String = value.nfc().collect();
    crate::arabic::visual(&composed, None)
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
