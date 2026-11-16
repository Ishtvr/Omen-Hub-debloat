//! Keyboard preview and RGB draft; all hardware writes still use DirectLighting.
use super::*;
use omen_core::lighting_editor::{Draft, Group};

fn group_index(group: Group) -> i32 {
    match group {
        Group::Wasd => 0,
        Group::Rest => 1,
        Group::NumpadRight => 2,
    }
}
fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
fn key(label: &str, x: f32, y: f32, w: f32, h: f32, zone: usize) -> KeyCap {
    KeyCap {
        label: label.into(),
        x,
        y,
        w,
        h,
        zone: zone as i32,
        group: group_index(Group::for_zone(zone)),
    }
}
/// Original schematic ISO layout, not vendor artwork or a per-key control map.
fn keyboard(german: bool) -> Vec<KeyCap> {
    let zone = |x: f32| {
        if x < 5.0 {
            2
        } else if x < 10.5 {
            1
        } else {
            0
        }
    };
    let mut keys = vec![key("Esc", 0.0, 0.0, 1.0, 0.8, 2)];
    for i in 0..12 {
        let x = 1.5 + i as f32 * 0.95;
        keys.push(key(&format!("F{}", i + 1), x, 0.0, 0.95, 0.8, zone(x)));
    }
    keys.push(key(
        if german { "Einfg" } else { "Ins" },
        13.0,
        0.0,
        1.0,
        0.8,
        0,
    ));
    keys.push(key(
        if german { "Entf" } else { "Del" },
        14.0,
        0.0,
        1.0,
        0.8,
        0,
    ));
    let mut row = |labels: &[&str], y: f32, first: f32, last: f32| {
        let mut x = 0.0;
        for (i, label) in labels.iter().enumerate() {
            let w = if i == 0 {
                first
            } else if i == labels.len() - 1 {
                last
            } else {
                1.0
            };
            let z = if matches!(*label, "W" | "A" | "S" | "D") {
                3
            } else {
                zone(x)
            };
            keys.push(key(label, x, y, w, 1.0, z));
            x += w;
        }
    };
    row(
        if german {
            &[
                "^", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "ß", "´", "Back",
            ]
        } else {
            &[
                "`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Back",
            ]
        },
        1.0,
        1.0,
        2.0,
    );
    row(
        if german {
            &[
                "Tab", "Q", "W", "E", "R", "T", "Z", "U", "I", "O", "P", "Ü", "+",
            ]
        } else {
            &[
                "Tab", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]",
            ]
        },
        2.0,
        1.5,
        1.0,
    );
    row(
        if german {
            &[
                "Caps", "A", "S", "D", "F", "G", "H", "J", "K", "L", "Ö", "Ä", "#",
            ]
        } else {
            &[
                "Caps", "A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'", "\\",
            ]
        },
        3.0,
        1.75,
        0.75,
    );
    row(
        if german {
            &[
                "Shift", "<", "Y", "X", "C", "V", "B", "N", "M", ",", ".", "-", "Shift",
            ]
        } else {
            &[
                "Shift", "\\", "Z", "X", "C", "V", "B", "N", "M", ",", ".", "/", "Shift",
            ]
        },
        4.0,
        1.25,
        2.75,
    );
    keys.push(key("Enter", 13.5, 2.0, 1.5, 2.0, 0));
    for (label, x, w) in [
        ("Ctrl", 0.0, 1.25),
        ("Fn", 1.25, 1.0),
        ("Alt", 2.25, 1.25),
        ("", 3.5, 6.0),
        ("Alt Gr", 9.5, 1.25),
        ("Ctrl", 10.75, 1.25),
    ] {
        keys.push(key(label, x, 5.1, w, 1.2, zone(x)));
    }
    for (label, x, y) in [
        ("←", 12.0, 5.75),
        ("↑", 13.0, 5.1),
        ("↓", 13.0, 5.75),
        ("→", 14.0, 5.75),
    ] {
        keys.push(key(label, x, y, 1.0, 0.6, 0));
    }
    for (r, labels) in [
        ["Num", "/", "*", "-"],
        ["7", "8", "9", "+"],
        ["4", "5", "6", ""],
        ["1", "2", "3", "Enter"],
    ]
    .iter()
    .enumerate()
    {
        for (c, label) in labels.iter().enumerate() {
            if label.is_empty() {
                continue;
            }
            keys.push(key(
                label,
                16.0 + c as f32,
                1.0 + r as f32,
                1.0,
                if *label == "+" || *label == "Enter" {
                    2.0
                } else {
                    1.0
                },
                0,
            ));
        }
    }
    keys.push(key("0", 16.0, 5.0, 2.0, 1.0, 0));
    keys.push(key(if german { "," } else { "." }, 18.0, 5.0, 1.0, 1.0, 0));
    keys
}
impl Host {
    pub(super) fn show_lighting(&self, state: KeyboardState) {
        let result = state.colors().and_then(|c| {
            <[[u8; 3]; 4]>::try_from(c).map_err(|_| t("lighting.four_zones").to_owned())
        });
        match result {
            Ok(colors) => {
                self.init_lighting(colors, state.custom_colors_enabled().unwrap_or(false));
                *self.lighting.borrow_mut() = Some(state);
                self.status(t("ui.ready"), false);
            }
            Err(e) => self.status(&crate::i18n::error_message(&e), true),
        }
    }
    pub(super) fn keyboard_layout(&self, german: bool) {
        if let Some(w) = self.window.upgrade() {
            let d = w.global::<Workbench>();
            d.set_keyboard_german(german);
            d.set_keyboard_keys(Rc::new(VecModel::from(keyboard(german))).into());
        }
    }
    pub(super) fn init_lighting(&self, colors: [[u8; 3]; 4], enabled: bool) {
        let mut draft = Draft::new(colors, enabled);
        if let Some(old) = self.lighting_draft.borrow().as_ref() {
            draft.group = old.group;
        }
        *self.lighting_draft.borrow_mut() = Some(draft);
        if let Some(w) = self.window.upgrade() {
            let d = w.global::<Workbench>();
            d.set_lighting_ready(true);
            d.set_lighting_confirmed(
                tf(
                    "lighting.last_read",
                    &[&t(if enabled { "common.on" } else { "common.off" })],
                )
                .into(),
            );
            d.set_lighting_error("".into());
        }
        self.draw_lighting(true);
    }
    pub(super) fn draw_lighting(&self, inputs: bool) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let borrowed = self.lighting_draft.borrow();
        let Some(draft) = borrowed.as_ref() else {
            return;
        };
        let d = w.global::<Workbench>();
        d.set_zone_colors(
            Rc::new(VecModel::from(
                draft
                    .colors
                    .map(|c| slint::Color::from_rgb_u8(c[0], c[1], c[2]))
                    .to_vec(),
            ))
            .into(),
        );
        d.set_lighting_on(draft.enabled);
        d.set_lighting_group(group_index(draft.group));
        d.set_lighting_dirty(draft.dirty());
        d.set_lighting_level(f32::from(draft.brightness().unwrap_or(0)));
        if inputs {
            d.set_lighting_color(draft.color().map_or_else(String::new, hex).into());
            d.set_lighting_brightness(
                draft
                    .brightness()
                    .map_or_else(String::new, |b| b.to_string())
                    .into(),
            );
        }
    }
    pub(super) fn lighting_dirty(&self) -> bool {
        self.lighting_draft
            .borrow()
            .as_ref()
            .is_some_and(Draft::dirty)
            || self
                .window
                .upgrade()
                .is_some_and(|w| !w.global::<Workbench>().get_lighting_error().is_empty())
    }
    pub(super) fn select_lighting_group(&self, index: i32) {
        if self.blocked() {
            return;
        }
        if let Some(group) = usize::try_from(index).ok().and_then(|i| Group::ALL.get(i))
            && let Some(draft) = self.lighting_draft.borrow_mut().as_mut()
        {
            draft.group = *group;
        }
        if let Some(w) = self.window.upgrade() {
            w.global::<Workbench>().set_lighting_error("".into());
        }
        self.draw_lighting(true);
    }
    pub(super) fn edit_lighting_color(&self, value: &str) {
        if self.blocked() {
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        if let Some(color) = omen_core::appearance::parse_hex(value.trim()) {
            if let Some(draft) = self.lighting_draft.borrow_mut().as_mut() {
                draft.set_color(color);
                d.set_lighting_brightness(draft.brightness().unwrap_or(0).to_string().into());
            }
            d.set_lighting_error("".into());
            self.draw_lighting(false);
        } else {
            d.set_lighting_error(t("lighting.invalid_color").into());
        }
    }
    pub(super) fn edit_lighting_brightness(&self, value: &str) {
        if self.blocked() {
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        if let Ok(value) = value.trim().parse::<u8>()
            && value <= 100
        {
            if let Some(draft) = self.lighting_draft.borrow_mut().as_mut() {
                if let Err(e) = draft.set_brightness(value) {
                    self.status(&crate::i18n::error_message(&e), true);
                    return;
                }
                d.set_lighting_color(draft.color().map_or_else(String::new, hex).into());
            }
            d.set_lighting_error("".into());
            self.draw_lighting(false);
        } else {
            d.set_lighting_error(t("lighting.invalid_brightness").into());
        }
    }
    pub(super) fn apply_lighting(&self) {
        if self.blocked() {
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        if !w.global::<Workbench>().get_lighting_error().is_empty() {
            return;
        }
        let target = self
            .lighting_draft
            .borrow()
            .as_ref()
            .map(|d| (d.colors, d.enabled));
        let Some((colors, enabled)) = target else {
            return;
        };
        if self.test {
            if let Some(d) = self.lighting_draft.borrow_mut().as_mut() {
                d.accept();
            }
            self.draw_lighting(true);
            self.status(t("ui.test_saved"), false);
            self.completed(t("ui.test_saved"));
            return;
        }
        if let Some(expected) = self.lighting.borrow().clone() {
            self.send_aux(Aux::Lighting(Some((expected, colors, enabled))));
        }
    }
    pub(super) fn pick_lighting_color(&self) {
        self.open_color_picker(3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagram_uses_wasd_and_right_numpad_groups_in_both_layouts() {
        for german in [true, false] {
            let keys = keyboard(german);
            for label in ["W", "A", "S", "D"] {
                assert!(
                    keys.iter()
                        .any(|k| k.label == label && k.zone == 3 && k.group == 0)
                );
            }
            assert!(
                keys.iter()
                    .filter(|k| k.x >= 16.0)
                    .all(|k| k.zone == 0 && k.group == 2)
            );
            assert!(
                keys.iter()
                    .all(|k| k.x >= 0.0 && k.x + k.w <= 20.5 && k.y + k.h <= 6.5)
            );
        }
    }
}
