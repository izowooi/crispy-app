#[cfg(target_os = "macos")]
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    Korean,
}

impl Language {
    #[cfg(target_os = "macos")]
    pub fn detect() -> Self {
        match Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
        {
            Ok(output) if output.status.success() => {
                preferred_language(&String::from_utf8_lossy(&output.stdout)).unwrap_or_default()
            }
            _ => std::env::var("LANG")
                .ok()
                .and_then(|locale| preferred_language(&locale))
                .unwrap_or_default(),
        }
    }

    #[cfg(windows)]
    pub fn detect() -> Self {
        preferred_language(&crate::platform::locale()).unwrap_or_default()
    }

    pub fn text<'a>(self, en: &'a str, ko: &'a str) -> &'a str {
        match self {
            Self::English => en,
            Self::Korean => ko,
        }
    }
}

fn preferred_language(value: &str) -> Option<Language> {
    value
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | ','))
        .filter_map(|token| {
            let language = token.split(['-', '_', '.']).next()?;
            ((2..=3).contains(&language.len()) && language.chars().all(|c| c.is_ascii_alphabetic()))
                .then_some(language)
        })
        .next()
        .map(|language| {
            if language.eq_ignore_ascii_case("ko") {
                Language::Korean
            } else {
                Language::English
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_preferred_language_has_priority() {
        assert_eq!(
            preferred_language("(\n \"en-KR\",\n \"ko-KR\"\n)"),
            Some(Language::English)
        );
        assert_eq!(
            preferred_language("(\n \"ko-KR\",\n en\n)"),
            Some(Language::Korean)
        );
        assert_eq!(
            preferred_language("(ja-JP, ko-KR)"),
            Some(Language::English)
        );
        assert_eq!(preferred_language("(zh-Hans, ko)"), Some(Language::English));
    }

    #[test]
    fn supports_locale_environment_format_and_empty_defaults() {
        assert_eq!(preferred_language("ko_KR.UTF-8"), Some(Language::Korean));
        assert_eq!(preferred_language("en_US.UTF-8"), Some(Language::English));
        assert_eq!(preferred_language("( )"), None);
        assert_eq!(preferred_language("C"), None);
        assert_eq!(Language::default(), Language::English);
    }
}
