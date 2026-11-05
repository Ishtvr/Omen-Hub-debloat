//! Geometry only. Native controls keep their text, selection, focus and history.
#[derive(Debug, Clone, Copy)]
pub struct RowLayout {
    pub check_width: i32,
    pub label_height: i32,
    pub editor_x: i32,
    pub editor_y: i32,
    pub editor_width: i32,
    pub note_y: i32,
    pub height: i32,
}

pub fn row(width: i32, label_width: i32, editor_width: i32, note: bool) -> RowLayout {
    let editor_width = editor_width.max(176).min((width - 52).max(176));
    let editor_x = width - 26 - editor_width;
    let desired_check = 28 + label_width + 6;
    let inline_space = editor_x - 16 - 16 - 32;
    let stacked = desired_check > inline_space;
    let check_width = if stacked {
        (width - 80).max(40)
    } else {
        desired_check
    };
    let label_height = if desired_check > check_width { 48 } else { 32 };
    let editor_y = if stacked { label_height + 4 } else { 0 };
    let height = if stacked {
        editor_y + 44
    } else if note {
        64
    } else {
        44
    };
    RowLayout {
        check_width,
        label_height,
        editor_x,
        editor_y,
        editor_width,
        note_y: if stacked { editor_y + 4 } else { 32 },
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wide_choices_and_long_labels_reflow_without_overlap() {
        for width in [600, 808, 1028, 1900] {
            for label in [140, 360, 580] {
                let r = row(width, label, 360, true);
                assert_eq!(r.editor_x + r.editor_width, width - 26);
                assert!(16 + r.check_width + 32 <= width - 26);
                assert!(r.height >= r.editor_y + 32);
                if r.editor_y == 0 {
                    assert!(16 + r.check_width + 32 < r.editor_x);
                } else {
                    assert!(r.editor_y >= r.label_height);
                }
            }
        }
    }
}
