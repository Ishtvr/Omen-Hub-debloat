//! Local appearance preferences. No OS, controller or startup permissions.
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PalettePreset {
    #[default]
    Slate,
    Graphite,
    Vesper,
    Light,
    Custom,
}
impl PalettePreset {
    pub const ALL: [Self; 5] = [
        Self::Slate,
        Self::Graphite,
        Self::Vesper,
        Self::Light,
        Self::Custom,
    ];
    pub fn colors(self) -> Colors {
        let (background, foreground, accent) = match self {
            Self::Slate | Self::Custom => ("#202127", "#F2F3F5", "#7C87F8"),
            Self::Graphite => ("#12151B", "#ECEEF2", "#7EB9DF"),
            Self::Vesper => ("#101010", "#ECE6E6", "#FFC799"),
            Self::Light => ("#F5F6FA", "#202127", "#4D57C8"),
        };
        Colors {
            background: background.into(),
            foreground: foreground.into(),
            accent: accent.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub background: String,
    pub foreground: String,
    pub accent: String,
}
impl Default for Colors {
    fn default() -> Self {
        PalettePreset::Slate.colors()
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub palette: PalettePreset,
    pub custom: Colors,
}

/// Strict, opaque sRGB. Partial edits must never turn the UI black or transparent.
pub fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&digits[0..2], 16).ok()?,
        u8::from_str_radix(&digits[2..4], 16).ok()?,
        u8::from_str_radix(&digits[4..6], 16).ok()?,
    ])
}
fn luminance(rgb: [u8; 3]) -> f64 {
    let linear = rgb.map(|v| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    });
    linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
}
pub fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn mix(a: [u8; 3], b: [u8; 3], amount: f64) -> [u8; 3] {
    std::array::from_fn(|i| {
        (f64::from(a[i]) * (1.0 - amount) + f64::from(b[i]) * amount).round() as u8
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub canvas: [u8; 3],
    pub surface: [u8; 3],
    pub control: [u8; 3],
    pub hover: [u8; 3],
    pub line: [u8; 3],
    pub ink: [u8; 3],
    pub muted: [u8; 3],
    pub quiet: [u8; 3],
    pub accent: [u8; 3],
    pub on_accent: [u8; 3],
    pub dark: bool,
}
impl Colors {
    pub fn tokens(&self) -> Result<Tokens, String> {
        let invalid = || crate::message::msg("settings.invalid_color", &[]);
        let bg = parse_hex(&self.background).ok_or_else(invalid)?;
        let ink = parse_hex(&self.foreground).ok_or_else(invalid)?;
        let accent = parse_hex(&self.accent).ok_or_else(invalid)?;
        let hover = mix(bg, ink, 0.11);
        if contrast(ink, hover) < 4.5 || contrast(accent, bg) < 3.0 {
            return Err(crate::message::msg("settings.color_contrast", &[]));
        }
        let muted = mix(bg, ink, 0.70);
        Ok(Tokens {
            canvas: bg,
            surface: mix(bg, ink, 0.035),
            control: mix(bg, ink, 0.065),
            hover,
            line: mix(bg, ink, 0.16),
            ink,
            muted: if contrast(muted, hover) >= 4.5 {
                muted
            } else {
                ink
            },
            quiet: mix(bg, ink, 0.40),
            accent,
            on_accent: if contrast(accent, [255; 3]) >= contrast(accent, [0; 3]) {
                [255; 3]
            } else {
                [0; 3]
            },
            dark: luminance(bg) < 0.3,
        })
    }
}
impl Appearance {
    pub fn colors(&self) -> Colors {
        if self.palette == PalettePreset::Custom {
            self.custom.clone()
        } else {
            self.palette.colors()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.custom.tokens()?;
        self.colors().tokens()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palettes_keep_text_and_focus_readable_on_controls() {
        for palette in PalettePreset::ALL {
            let t = palette.colors().tokens().unwrap();
            assert!(contrast(t.ink, t.hover) >= 4.5);
            assert!(contrast(t.muted, t.hover) >= 4.5);
            assert!(contrast(t.accent, t.canvas) >= 3.0);
            assert!(contrast(t.on_accent, t.accent) >= 4.5);
        }
    }
    #[test]
    fn invalid_or_unreadable_custom_colors_do_not_replace_the_last_preview() {
        for value in ["", "#12", "ffffff", "#000000ff", "#GG0000", "#é1234"] {
            assert!(parse_hex(value).is_none());
        }
        assert_eq!(parse_hex("#aB12fF"), Some([171, 18, 255]));
        let mut colors = Colors::default();
        colors.foreground.clone_from(&colors.background);
        assert!(colors.tokens().is_err());
        let a = Appearance {
            palette: PalettePreset::Custom,
            custom: colors,
        };
        assert!(a.validate().is_err());
    }
}
