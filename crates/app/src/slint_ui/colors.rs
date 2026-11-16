//! Shared local color editor. Acceptance changes a UI draft, never hardware.
use super::*;
use omen_core::{
    appearance::{PalettePreset, parse_hex},
    color::{Hsv, SWATCHES, hex},
};

pub(super) struct Picker {
    target: i32,
    original: [u8; 3],
    rgb: [u8; 3],
    hsv: Hsv,
    valid: bool,
}
fn color(rgb: [u8; 3]) -> slint::Color {
    slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2])
}
impl Host {
    pub(super) fn palette_choices(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let choices = PalettePreset::ALL
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c = if *p == PalettePreset::Custom {
                    self.appearance_draft.borrow().custom.clone()
                } else {
                    p.colors()
                };
                PaletteChoice {
                    name: match i {
                        0 => "Slate",
                        1 => t("settings.graphite"),
                        2 => "Vesper",
                        3 => t("settings.light"),
                        _ => t("settings.custom"),
                    }
                    .into(),
                    canvas: color(parse_hex(&c.background).unwrap_or([32, 33, 39])),
                    ink: color(parse_hex(&c.foreground).unwrap_or([242, 243, 245])),
                    accent: color(parse_hex(&c.accent).unwrap_or([124, 135, 248])),
                }
            })
            .collect::<Vec<_>>();
        w.global::<Workbench>()
            .set_palette_choices(Rc::new(VecModel::from(choices)).into());
    }
    pub(super) fn sync_color_fields(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        let fallback = self.appearance_tokens.get();
        d.set_background_preview(color(
            parse_hex(d.get_color_background().as_str()).unwrap_or(fallback.canvas),
        ));
        d.set_foreground_preview(color(
            parse_hex(d.get_color_foreground().as_str()).unwrap_or(fallback.ink),
        ));
        d.set_accent_preview(color(
            parse_hex(d.get_color_accent().as_str()).unwrap_or(fallback.accent),
        ));
        self.palette_choices();
    }
    pub(super) fn open_color_picker(&self, target: i32) {
        if self.blocked() {
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        let (rgb, title) = match target {
            0..=2 if d.get_settings_ready() => {
                let text = match target {
                    0 => d.get_color_background(),
                    1 => d.get_color_foreground(),
                    _ => d.get_color_accent(),
                };
                let rgb = parse_hex(text.as_str()).unwrap_or([255; 3]);
                (
                    rgb,
                    t(match target {
                        0 => "settings.background",
                        1 => "settings.foreground",
                        _ => "settings.accent",
                    }),
                )
            }
            3 if d.get_lighting_ready() => {
                let borrowed = self.lighting_draft.borrow();
                let Some(draft) = borrowed.as_ref() else {
                    return;
                };
                (
                    draft
                        .color()
                        .unwrap_or(draft.colors[draft.group.zones()[0]]),
                    t("lighting.color"),
                )
            }
            _ => return,
        };
        *self.color_picker.borrow_mut() = Some(Picker {
            target,
            original: rgb,
            rgb,
            hsv: Hsv::from_rgb(rgb),
            valid: true,
        });
        let editor = w.global::<ColorEditor>();
        editor.set_title(title.into());
        editor.set_swatches(Rc::new(VecModel::from(SWATCHES.map(color).to_vec())).into());
        editor.set_swatch_labels(strings(SWATCHES.map(hex)));
        d.set_confirm_kind(4);
        self.draw_color_picker(true);
    }
    pub(super) fn draw_color_picker(&self, fill_hex: bool) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let borrowed = self.color_picker.borrow();
        let Some(p) = borrowed.as_ref() else {
            return;
        };
        let e = w.global::<ColorEditor>();
        e.set_original(color(p.original));
        e.set_current(color(p.rgb));
        e.set_hue(p.hsv.hue);
        e.set_saturation(p.hsv.saturation);
        e.set_value(p.hsv.value);
        e.set_hue_color(color(
            Hsv {
                hue: p.hsv.hue,
                saturation: 1.0,
                value: 1.0,
            }
            .rgb(),
        ));
        e.set_valid(p.valid);
        if fill_hex {
            e.set_hex(hex(p.rgb).into());
        }
    }
    pub(super) fn picker_hsv(&self, h: f32, s: f32, v: f32) {
        let Some(hsv) = Hsv::new(h, s, v) else {
            return;
        };
        if let Some(p) = self.color_picker.borrow_mut().as_mut() {
            p.hsv = hsv;
            p.rgb = hsv.rgb();
            p.valid = true;
        }
        self.draw_color_picker(true);
    }
    pub(super) fn picker_hex(&self, text: &str) {
        if let Some(p) = self.color_picker.borrow_mut().as_mut() {
            p.valid = parse_hex(text.trim()).is_some();
            if let Some(rgb) = parse_hex(text.trim()) {
                let mut hsv = Hsv::from_rgb(rgb);
                if hsv.saturation == 0.0 {
                    hsv.hue = p.hsv.hue;
                }
                p.rgb = rgb;
                p.hsv = hsv;
            }
        }
        self.draw_color_picker(false);
    }
    pub(super) fn picker_swatch(&self, index: i32) {
        if let Some(rgb) = usize::try_from(index).ok().and_then(|i| SWATCHES.get(i)) {
            self.picker_hex(&hex(*rgb));
            self.draw_color_picker(true);
        }
    }
    pub(super) fn close_color_picker(&self, accept: bool) {
        if accept
            && self
                .color_picker
                .borrow()
                .as_ref()
                .is_some_and(|p| !p.valid)
        {
            return;
        }
        let Some(p) = self.color_picker.borrow_mut().take() else {
            return;
        };
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        d.set_confirm_kind(0);
        if accept {
            let text = hex(p.rgb);
            match p.target {
                0 => d.set_color_background(text.clone().into()),
                1 => d.set_color_foreground(text.clone().into()),
                2 => d.set_color_accent(text.clone().into()),
                3 => {
                    self.edit_lighting_color(&text);
                    self.draw_lighting(true);
                }
                _ => (),
            }
            if p.target < 3 {
                self.edit_colors();
                self.sync_color_fields();
            }
        }
        w.invoke_color_picker_closed(p.target);
    }
}
