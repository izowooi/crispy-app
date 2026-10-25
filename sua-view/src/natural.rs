use std::cmp::Ordering;

pub(crate) fn compare(left: &str, right: &str) -> Ordering {
    let (a, b) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (start_a, start_b) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let mut value_a = start_a;
            let mut value_b = start_b;
            while value_a < i && a[value_a] == b'0' {
                value_a += 1;
            }
            while value_b < j && b[value_b] == b'0' {
                value_b += 1;
            }
            let order = (i - value_a)
                .cmp(&(j - value_b))
                .then_with(|| a[value_a..i].cmp(&b[value_b..j]));
            if order != Ordering::Equal {
                return order;
            }
            let padding = (i - start_a).cmp(&(j - start_b));
            if padding != Ordering::Equal {
                return padding;
            }
        } else {
            let order = a[i].cmp(&b[j]);
            if order != Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    a.len().cmp(&b.len())
}
