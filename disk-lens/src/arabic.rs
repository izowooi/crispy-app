//! egui 는 아랍 문자의 이어 쓰기와 오른쪽→왼쪽 배치를 해 주지 않는다.
//! 그리기 전에 글자를 위치별 모양(Presentation Forms-B)으로 바꾸고, 화면에 놓일 순서로 다시 배열한다.
//! 숫자와 라틴 문자는 왼쪽→오른쪽 그대로 둔다.

/// U+0621..=U+064A 각 글자의 모양 개수. 4 = 양쪽으로 이어짐, 2 = 앞 글자와만 이어짐, 1 = 이어지지 않음.
const FORMS: [u8; 42] = [
    1, 2, 2, 2, 2, 4, 2, 4, 2, 4, 4, 4, 4, 4, 2, 2, 2, 2, 4, 4, 4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 0,
    4, 4, 4, 4, 4, 4, 4, 2, 2, 4,
];
const TATWEEL: char = '\u{640}';
const LAM: char = '\u{644}';

fn forms(c: char) -> u8 {
    match c {
        TATWEEL => 4,
        '\u{621}'..='\u{64A}' => FORMS[c as usize - 0x621],
        _ => 0,
    }
}
// 모음 부호는 앞뒤 글자의 이어짐을 끊지 않는다.
fn transparent(c: char) -> bool {
    matches!(c, '\u{64B}'..='\u{65F}' | '\u{670}')
}
/// (홀로, 끝, 처음, 가운데) 모양의 첫 코드 포인트.
fn first_form(c: char) -> u32 {
    0xFE80
        + FORMS[..c as usize - 0x621]
            .iter()
            .map(|&n| n as u32)
            .sum::<u32>()
}
fn lam_alef(alef: char) -> Option<u32> {
    match alef {
        '\u{622}' => Some(0xFEF5),
        '\u{623}' => Some(0xFEF7),
        '\u{625}' => Some(0xFEF9),
        '\u{627}' => Some(0xFEFB),
        _ => None,
    }
}

fn shape(text: &[char]) -> Vec<char> {
    let neighbour =
        |range: &mut dyn Iterator<Item = usize>| range.map(|i| text[i]).find(|&c| !transparent(c));
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        let count = forms(c);
        if count == 0 || c == TATWEEL {
            out.push(c);
            i += 1;
            continue;
        }
        let joins_previous =
            count >= 2 && neighbour(&mut (0..i).rev()).is_some_and(|p| forms(p) == 4);
        let next_index = (i + 1..text.len()).find(|&j| !transparent(text[j]));
        if c == LAM
            && let Some(j) = next_index
            && let Some(ligature) = lam_alef(text[j])
        {
            out.push(char::from_u32(ligature + joins_previous as u32).unwrap_or(c));
            out.extend(text[i + 1..j].iter().copied());
            i = j + 1;
            continue;
        }
        let joins_next = count == 4 && next_index.is_some_and(|j| forms(text[j]) >= 2);
        let offset = match (joins_previous, joins_next) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        };
        out.push(char::from_u32(first_form(c) + offset).unwrap_or(c));
        i += 1;
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Left,
    Right,
    Neutral,
}
fn direction(c: char) -> Direction {
    match c {
        '\u{590}'..='\u{8FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}' => {
            Direction::Right
        }
        c if c.is_alphanumeric() => Direction::Left,
        _ => Direction::Neutral,
    }
}
fn mirrored(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        c => c,
    }
}

pub fn has_right_to_left(text: &str) -> bool {
    text.chars().any(|c| direction(c) == Direction::Right)
}

/// 한 줄을 화면 순서로 바꾼다. `right_to_left` 가 None 이면 첫 글자의 방향을 따른다.
fn visual_line(line: &str, right_to_left: Option<bool>) -> String {
    let chars = shape(&line.chars().collect::<Vec<_>>());
    let mut directions: Vec<_> = chars.iter().map(|&c| direction(c)).collect();
    let base = match right_to_left {
        Some(true) => Direction::Right,
        Some(false) => Direction::Left,
        None => directions
            .iter()
            .copied()
            .find(|&d| d != Direction::Neutral)
            .unwrap_or(Direction::Left),
    };
    // 공백과 문장 부호는 양옆이 같은 방향이면 그 방향을, 아니면 줄의 방향을 따른다.
    let mut i = 0;
    while i < directions.len() {
        if directions[i] != Direction::Neutral {
            i += 1;
            continue;
        }
        let end = (i..directions.len())
            .find(|&j| directions[j] != Direction::Neutral)
            .unwrap_or(directions.len());
        let before = if i == 0 { base } else { directions[i - 1] };
        let after = directions.get(end).copied().unwrap_or(base);
        directions[i..end].fill(if before == after { before } else { base });
        i = end;
    }
    let mut cells: Vec<(char, Direction)> = chars
        .into_iter()
        .zip(directions)
        .map(|(c, d)| {
            (
                if d == Direction::Right {
                    mirrored(c)
                } else {
                    c
                },
                d,
            )
        })
        .collect();
    // 줄 전체를 줄의 방향에 맞게 놓은 뒤, 반대 방향 구간만 다시 뒤집는다.
    if base == Direction::Right {
        cells.reverse();
    }
    let mut i = 0;
    while i < cells.len() {
        let end = (i..cells.len())
            .find(|&j| cells[j].1 != cells[i].1)
            .unwrap_or(cells.len());
        if cells[i].1 != Direction::Left {
            if base == Direction::Left {
                cells[i..end].reverse();
            }
        } else if base == Direction::Right {
            cells[i..end].reverse();
        }
        i = end;
    }
    cells.into_iter().map(|(c, _)| c).collect()
}

/// 아랍 문자가 없으면 그대로 돌려준다. 줄바꿈은 줄마다 따로 처리해 줄 순서를 지킨다.
pub fn visual(text: &str, right_to_left: Option<bool>) -> String {
    if !has_right_to_left(text) {
        return text.to_owned();
    }
    text.split('\n')
        .map(|line| visual_line(line, right_to_left))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_take_positional_forms_and_run_right_to_left() {
        // "ملف" (파일): 처음 ﻣ, 가운데 ﻠ, 끝 ﻒ. 화면에는 끝 글자가 맨 왼쪽에 온다.
        assert_eq!(visual("ملف", None), "\u{FED2}\u{FEE0}\u{FEE3}");
        // 앞 글자와만 이어지는 글자(ا) 뒤에서는 이어짐이 끊긴다: "باب"
        assert_eq!(visual("باب", None), "\u{FE8F}\u{FE8E}\u{FE91}");
    }

    #[test]
    fn lam_alef_becomes_one_ligature() {
        assert_eq!(visual("لا", None), "\u{FEFB}");
        assert_eq!(visual("بلا", None), "\u{FEFC}\u{FE91}");
    }

    #[test]
    fn numbers_and_latin_stay_left_to_right_inside_right_to_left_text() {
        // 논리 순서 "56.3 GB متاح" → 화면: [متاح] [56.3 GB]. 숫자는 뒤집히지 않는다.
        let line = visual("56.3 GB متاح", Some(true));
        assert!(line.ends_with("56.3 GB"), "{line}");
        assert!(!line.contains("3.65"));
        // 번호 목록: 번호가 오른쪽 끝에 온다.
        assert!(visual("1. افتح", Some(true)).ends_with(" .1"));
    }

    #[test]
    fn left_to_right_paths_keep_their_order_around_arabic_names() {
        let path = visual("/Users/me/ملف/a.txt", None);
        assert!(path.starts_with("/Users/me/"));
        assert!(path.ends_with("/a.txt"));
        assert!(path.contains("\u{FED2}\u{FEE0}\u{FEE3}"));
    }

    #[test]
    fn text_without_arabic_is_untouched_and_lines_keep_their_order() {
        assert_eq!(visual("Downloads (2)", None), "Downloads (2)");
        let two = visual("ملف\nباب", Some(true));
        assert_eq!(two, "\u{FED2}\u{FEE0}\u{FEE3}\n\u{FE8F}\u{FE8E}\u{FE91}");
    }
}
