//! Appearance is a local draft. Invalid intermediate hex never repaints or saves.
use super::*;
use omen_core::appearance::{Appearance, Colors, PalettePreset, Tokens};

fn paint(theme: Theme<'_>, tokens: Tokens) {
    let color = |c: [u8; 3]| slint::Color::from_rgb_u8(c[0], c[1], c[2]);
    theme.on_tr(|key| t(key.as_str()).into());
    theme.set_dark(tokens.dark);
    theme.set_canvas(color(tokens.canvas));
    theme.set_surface(color(tokens.surface));
    theme.set_control(color(tokens.control));
    theme.set_hover(color(tokens.hover));
    theme.set_line(color(tokens.line));
    theme.set_ink(color(tokens.ink));
    theme.set_muted(color(tokens.muted));
    theme.set_quiet(color(tokens.quiet));
    theme.set_accent(color(tokens.accent));
    theme.set_on_accent(color(tokens.on_accent));
}
impl Host {
    pub(super) fn show_appearance(&self, value: Appearance) {
        if let Some(w) = self.window.upgrade() {
            let d = w.global::<Workbench>();
            let colors = value.colors();
            d.set_palette_index(
                PalettePreset::ALL
                    .iter()
                    .position(|p| *p == value.palette)
                    .unwrap_or(0) as i32,
            );
            d.set_color_background(colors.background.into());
            d.set_color_foreground(colors.foreground.into());
            d.set_color_accent(colors.accent.into());
        }
        *self.appearance_draft.borrow_mut() = value;
        self.preview_appearance();
        self.sync_color_fields();
    }
    pub(super) fn preview_appearance(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        match self
            .appearance_draft
            .borrow()
            .validate()
            .and_then(|_| self.appearance_draft.borrow().colors().tokens())
        {
            Ok(tokens) => {
                self.appearance_tokens.set(tokens);
                paint(w.global::<Theme>(), tokens);
                self.paint_about();
                w.window().with_winit_window(|native| {
                    native.set_theme(Some(if tokens.dark {
                        winit::window::Theme::Dark
                    } else {
                        winit::window::Theme::Light
                    }))
                });
                d.set_appearance_error("".into());
            }
            Err(e) => d.set_appearance_error(crate::i18n::error_message(&e).into()),
        }
        d.set_settings_dirty(self.settings_dirty());
    }
    pub(super) fn paint_about(&self) {
        if let Some(window) = self.about.borrow().as_ref() {
            paint(window.global::<Theme>(), self.appearance_tokens.get());
            window.window().with_winit_window(|native| {
                native.set_theme(Some(if self.appearance_tokens.get().dark {
                    winit::window::Theme::Dark
                } else {
                    winit::window::Theme::Light
                }))
            });
        }
    }
    pub(super) fn select_palette(&self, index: i32) {
        if self.blocked() {
            return;
        }
        if let Some(palette) = usize::try_from(index)
            .ok()
            .and_then(|i| PalettePreset::ALL.get(i))
        {
            let mut appearance = self.appearance_draft.borrow().clone();
            appearance.palette = *palette;
            if *palette != PalettePreset::Custom && appearance.custom.tokens().is_err() {
                appearance.custom = self
                    .baseline_settings
                    .borrow()
                    .as_ref()
                    .map_or_else(Colors::default, |s| s.appearance.custom.clone());
            }
            self.show_appearance(appearance);
        }
    }
    pub(super) fn edit_colors(&self) {
        if self.blocked() {
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        d.set_palette_index(4);
        *self.appearance_draft.borrow_mut() = Appearance {
            palette: PalettePreset::Custom,
            custom: Colors {
                background: d.get_color_background().to_string(),
                foreground: d.get_color_foreground().to_string(),
                accent: d.get_color_accent().to_string(),
            },
        };
        self.preview_appearance();
        self.sync_color_fields();
    }
}
