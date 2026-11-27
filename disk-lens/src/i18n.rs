use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    Korean,
    ChineseSimplified,
    Japanese,
    German,
    French,
    Italian,
    Spanish,
    Arabic,
    Polish,
    Russian,
}

/// (언어, `--lang` 과 시스템 설정에 쓰는 코드, 메뉴에 보이는 이름)
pub const LANGUAGES: [(Language, &str, &str); 11] = [
    (Language::English, "en", "English"),
    (Language::Korean, "ko", "한국어"),
    (Language::ChineseSimplified, "zh", "简体中文"),
    (Language::Japanese, "ja", "日本語"),
    (Language::German, "de", "Deutsch"),
    (Language::French, "fr", "Français"),
    (Language::Italian, "it", "Italiano"),
    (Language::Spanish, "es", "Español"),
    (Language::Arabic, "ar", "العربية"),
    (Language::Polish, "pl", "Polski"),
    (Language::Russian, "ru", "Русский"),
];

struct Translations {
    /// 영어 문구 → 9개 언어 (번역표 그대로)
    logical: HashMap<&'static str, &'static [&'static str; 9]>,
    /// 영어 문구 → 화면에 그릴 순서로 바꾼 아랍어
    arabic_visual: HashMap<&'static str, &'static str>,
}
const ARABIC_COLUMN: usize = 6;

fn translations() -> &'static Translations {
    static CELL: OnceLock<Translations> = OnceLock::new();
    CELL.get_or_init(|| Translations {
        logical: crate::translations::TABLE
            .iter()
            .map(|(english, others)| (*english, others))
            .collect(),
        // 한 번만 만들어 프로그램이 끝날 때까지 쓴다.
        arabic_visual: crate::translations::TABLE
            .iter()
            .map(|(english, others)| {
                let visual = crate::arabic::visual(others[ARABIC_COLUMN], Some(true));
                (*english, &*Box::leak(visual.into_boxed_str()))
            })
            .collect(),
    })
}

impl Language {
    pub fn detect() -> Self {
        preferred_language(&system_languages()).unwrap_or_default()
    }

    pub fn from_code(code: &str) -> Option<Self> {
        LANGUAGES
            .iter()
            .find(|(_, known, _)| *known == code)
            .map(|(language, _, _)| *language)
    }

    fn column(self) -> Option<usize> {
        match self {
            Self::English | Self::Korean => None,
            Self::ChineseSimplified => Some(0),
            Self::Japanese => Some(1),
            Self::German => Some(2),
            Self::French => Some(3),
            Self::Italian => Some(4),
            Self::Spanish => Some(5),
            Self::Arabic => Some(ARABIC_COLUMN),
            Self::Polish => Some(7),
            Self::Russian => Some(8),
        }
    }

    /// 화면에 그대로 그릴 문구. 아랍어는 글자를 잇고 오른쪽→왼쪽 순서로 놓은 결과를 준다.
    pub fn text<'a>(self, en: &'a str, ko: &'a str) -> &'a str {
        if self == Self::Arabic {
            return translations().arabic_visual.get(en).copied().unwrap_or(en);
        }
        self.part(en, ko)
    }

    /// 다른 값과 이어 붙이거나 시스템 창에 넘길 문구. 아랍어도 원래 순서 그대로다.
    /// 이어 붙인 결과는 `line` 을 거쳐 화면에 그린다.
    pub fn part<'a>(self, en: &'a str, ko: &'a str) -> &'a str {
        match self {
            Self::English => en,
            Self::Korean => ko,
            other => translations()
                .logical
                .get(en)
                .map_or(en, |row| row[other.column().unwrap_or(0)]),
        }
    }

    /// `part` 들과 숫자를 이어 붙인 한 줄을 화면에 그릴 수 있게 한다.
    /// 아랍어에서는 줄 전체가 오른쪽에서 시작하고, 숫자와 단위는 왼쪽→오른쪽으로 남는다.
    pub fn line(self, text: String) -> String {
        if self == Self::Arabic {
            crate::arabic::visual(&text, Some(true))
        } else {
            text
        }
    }
}

#[cfg(target_os = "macos")]
fn system_languages() -> String {
    match std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
    {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).into(),
        _ => std::env::var("LANG").unwrap_or_default(),
    }
}
#[cfg(windows)]
fn system_languages() -> String {
    crate::platform::locale()
}

fn preferred_language(value: &str) -> Option<Language> {
    value
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | ','))
        .filter_map(|token| {
            let mut parts = token.split(['-', '_', '.']);
            let language = parts.next()?;
            ((2..=3).contains(&language.len()) && language.chars().all(|c| c.is_ascii_alphabetic()))
                .then(|| (language.to_ascii_lowercase(), parts.collect::<Vec<_>>()))
        })
        .next()
        .map(|(language, rest)| {
            // 번체 중국어 번역은 없다. 간체로 보여 주는 대신 영어로 둔다.
            let traditional = rest.iter().any(|part| {
                ["Hant", "TW", "HK", "MO"]
                    .iter()
                    .any(|t| part.eq_ignore_ascii_case(t))
            });
            if language == "zh" && traditional {
                return Language::English;
            }
            Language::from_code(&language).unwrap_or_default()
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
            Some(Language::Japanese)
        );
        assert_eq!(preferred_language("(nl-NL, ko)"), Some(Language::English));
    }

    #[test]
    fn supports_locale_environment_format_and_empty_defaults() {
        assert_eq!(preferred_language("ko_KR.UTF-8"), Some(Language::Korean));
        assert_eq!(preferred_language("en_US.UTF-8"), Some(Language::English));
        assert_eq!(preferred_language("de_DE.UTF-8"), Some(Language::German));
        assert_eq!(preferred_language("( )"), None);
        assert_eq!(preferred_language("C"), None);
        assert_eq!(Language::default(), Language::English);
    }

    #[test]
    fn simplified_chinese_only() {
        assert_eq!(
            preferred_language("(\"zh-Hans-CN\")"),
            Some(Language::ChineseSimplified)
        );
        assert_eq!(
            preferred_language("zh_CN.UTF-8"),
            Some(Language::ChineseSimplified)
        );
        assert_eq!(
            preferred_language("(\"zh-Hant-TW\")"),
            Some(Language::English)
        );
        assert_eq!(preferred_language("zh-HK"), Some(Language::English));
    }

    #[test]
    fn every_language_has_a_unique_code() {
        for (language, code, _) in LANGUAGES {
            assert_eq!(Language::from_code(code), Some(language));
        }
    }

    /// 소스에 적힌 모든 화면 문구가 번역표에 있어야 한다. 빠지면 그 언어에서 영어로 보인다.
    #[test]
    fn every_ui_string_is_translated() {
        let sources = [include_str!("main.rs"), include_str!("charts.rs")];
        let mut missing = Vec::new();
        for source in sources {
            for marker in [".text(", ".part("] {
                for (index, _) in source.match_indices(marker) {
                    let rest = source[index + marker.len()..].trim_start();
                    let Some(rest) = rest.strip_prefix('"') else {
                        continue;
                    };
                    let mut key = String::new();
                    let mut chars = rest.chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => key.extend(chars.next()),
                            c => key.push(c),
                        }
                    }
                    if !translations().logical.contains_key(key.as_str()) {
                        missing.push(key);
                    }
                }
            }
        }
        assert!(missing.is_empty(), "번역표에 없는 문구: {missing:?}");
    }

    #[test]
    fn translations_are_complete_and_arabic_is_shaped() {
        for (english, others) in crate::translations::TABLE {
            assert!(others.iter().all(|t| !t.trim().is_empty()), "{english}");
        }
        // 아랍어 문구는 그리기 전에 위치별 모양으로 바뀐다. 원래 글자가 남아 있으면 끊어져 보인다.
        let stop = Language::Arabic.text("Stop", "중지");
        assert!(
            stop.chars().all(|c| !('\u{621}'..='\u{64A}').contains(&c)),
            "{stop}"
        );
        assert_eq!(Language::Arabic.part("Stop", "중지"), "إيقاف");
        assert_eq!(Language::German.text("Stop", "중지"), "Stopp");
        assert_eq!(Language::Korean.text("Stop", "중지"), "중지");
        assert_eq!(
            Language::French.text("not in the table", "없음"),
            "not in the table"
        );
        let line = Language::Arabic.line(format!("12 {}", Language::Arabic.part("files", "파일")));
        assert!(line.ends_with("12"), "{line}");
    }
}
