//! Geometry and keyboard rules of the dashboard's position switches
//! (`●─○─○`). Each option owns an equal column; its stop sits in the
//! column centre and its label underneath. No Windows dependency.

/// Height of the rounded track that holds the stops.
pub const TRACK: i32 = 26;
/// Gap between track and option labels.
pub const LABEL_GAP: i32 = 4;

/// Horizontal centre of every stop for a control `width` pixels wide.
pub fn stops(width: i32, count: usize) -> Vec<i32> {
    let count = count.max(1) as i32;
    (0..count)
        .map(|i| (2 * i + 1) * width / (2 * count))
        .collect()
}

/// Left and right edge of the track: half a track height around the outer stops.
pub fn track(width: i32, count: usize) -> (i32, i32) {
    let stops = stops(width, count);
    let first = stops.first().copied().unwrap_or(0);
    let last = stops.last().copied().unwrap_or(width);
    ((first - TRACK / 2).max(0), (last + TRACK / 2).min(width))
}

/// Column under a click; positions outside the control clamp to the ends.
pub fn hit(x: i32, width: i32, count: usize) -> usize {
    if count == 0 || width <= 0 {
        return 0;
    }
    let column = x.clamp(0, width - 1) as i64 * count as i64 / width as i64;
    column as usize
}

/// Arrow, Home and End navigation; other keys leave the selection alone.
pub enum Key {
    Previous,
    Next,
    First,
    Last,
}
pub fn navigate(selected: usize, count: usize, key: Key) -> usize {
    let last = count.saturating_sub(1);
    match key {
        Key::Previous => selected.saturating_sub(1),
        Key::Next => (selected + 1).min(last),
        Key::First => 0,
        Key::Last => last,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_sit_in_equal_columns() {
        assert_eq!(stops(180, 3), vec![30, 90, 150]);
        assert_eq!(stops(120, 2), vec![30, 90]);
        assert_eq!(track(180, 3), (17, 163));
        assert_eq!(track(120, 2), (17, 103));
    }

    #[test]
    fn clicks_pick_the_column_and_clamp() {
        assert_eq!(hit(0, 180, 3), 0);
        assert_eq!(hit(59, 180, 3), 0);
        assert_eq!(hit(60, 180, 3), 1);
        assert_eq!(hit(179, 180, 3), 2);
        assert_eq!(hit(500, 180, 3), 2);
        assert_eq!(hit(-20, 120, 2), 0);
        assert_eq!(hit(10, 0, 2), 0);
    }

    #[test]
    fn keys_stay_inside_the_options() {
        assert_eq!(navigate(0, 3, Key::Previous), 0);
        assert_eq!(navigate(1, 3, Key::Previous), 0);
        assert_eq!(navigate(1, 3, Key::Next), 2);
        assert_eq!(navigate(2, 3, Key::Next), 2);
        assert_eq!(navigate(2, 3, Key::First), 0);
        assert_eq!(navigate(0, 2, Key::Last), 1);
    }
}
