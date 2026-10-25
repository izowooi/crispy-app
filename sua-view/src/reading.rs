//! Platform-independent reading rules. Native UIs supply decoded image metadata;
//! neither AppKit nor WPF should duplicate decisions about page boundaries.

/// Invalid/unknown dimensions must not cause a second page to be consumed.
pub fn is_landscape(width: f64, height: f64) -> bool {
    !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > height * 1.25
}

pub fn can_pair(
    index: usize,
    end: usize,
    spread: bool,
    cover_alone: bool,
    first_landscape: bool,
    second_landscape: bool,
) -> bool {
    spread
        && !(cover_alone && index == 0)
        && !first_landscape
        && !second_landscape
        && index.checked_add(1).is_some_and(|next| next < end)
}

/// A spread ending on the final page must stay put, rather than show that last
/// page a second time. An empty book always has position zero.
pub fn next_start(index: usize, count: usize, displayed: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let index = index.min(count - 1);
    match index.checked_add(displayed.max(1)) {
        Some(next) if next < count => next,
        _ => index,
    }
}

/// Resolve a previous spread after a jump/restore when no exact history exists.
/// Metadata states: 0 = portrait, 1 = landscape, -1 = unreadable/unknown.
/// Pair only known portraits and never overlap the current spread.
pub fn previous_start(
    index: usize,
    spread: bool,
    cover_alone: bool,
    previous_landscape: i32,
    earlier_landscape: i32,
) -> usize {
    let previous = index.saturating_sub(1);
    if previous > 0
        && can_pair(
            previous - 1,
            index,
            spread,
            cover_alone,
            earlier_landscape != 0,
            previous_landscape != 0,
        )
    {
        previous - 1
    } else {
        previous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_consume_every_page_once_in_both_directions() {
        for cover in [false, true] {
            for spread in [false, true] {
                // Adjacent landscapes, cover, and final portrait exercise boundaries.
                let landscape = [false, false, false, true, true, false, false, false];
                let count = landscape.len();
                let mut starts = Vec::new();
                let mut visited = Vec::new();
                let mut index = 0;
                loop {
                    starts.push(index);
                    let pair = can_pair(
                        index,
                        count,
                        spread,
                        cover,
                        landscape[index],
                        landscape.get(index + 1).copied().unwrap_or(true),
                    );
                    let displayed = if pair { 2 } else { 1 };
                    visited.extend(index..index + displayed);
                    let next = next_start(index, count, displayed);
                    if next == index {
                        break;
                    }
                    index = next;
                }
                assert_eq!(visited, (0..count).collect::<Vec<_>>());
                // Restored/jumped positions need not align with a canonical spread.
                for current in 1..count {
                    let previous = previous_start(
                        current,
                        spread,
                        cover,
                        i32::from(landscape[current - 1]),
                        if current > 1 {
                            i32::from(landscape[current - 2])
                        } else {
                            -1
                        },
                    );
                    let pair = can_pair(
                        previous,
                        current,
                        spread,
                        cover,
                        landscape[previous],
                        landscape.get(previous + 1).copied().unwrap_or(true),
                    );
                    assert_eq!(previous + if pair { 2 } else { 1 }, current);
                }
                assert_eq!(starts.first(), Some(&0));
            }
        }
    }

    #[test]
    fn invalid_metadata_and_extreme_indices_are_conservative() {
        assert!(is_landscape(f64::NAN, 100.0));
        assert!(is_landscape(100.0, 0.0));
        assert!(!is_landscape(125.0, 100.0));
        assert!(is_landscape(126.0, 100.0));
        assert!(!can_pair(usize::MAX, usize::MAX, true, false, false, false));
        assert_eq!(next_start(usize::MAX, 0, 2), 0);
        assert_eq!(next_start(0, 2, usize::MAX), 0);
        assert_eq!(previous_start(0, true, true, 0, 0), 0);
        assert_eq!(previous_start(3, true, false, -1, 0), 2);
        assert_eq!(previous_start(3, true, false, 0, -1), 2);
        assert_eq!(previous_start(2, true, true, 0, 0), 1);
    }
}
