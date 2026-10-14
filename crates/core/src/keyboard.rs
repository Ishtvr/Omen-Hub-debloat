//! Observed HP keyboard state and pure RGB transformations. No OS/hardware IO.
//! Brightness remains a raw byte; no percentage or animation mode is inferred.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyboardKind {
    Normal,
    FourZoneWithNumpad,
    FourZoneWithoutNumpad,
    RgbPerKey,
    OneZoneWithNumpad,
    OneZoneWithoutNumpad,
    FourZoneMCUWithNumpad,
    FourZoneMCUWithoutNumpad,
}

impl KeyboardKind {
    /// Only these classic types have the investigated legacy state contract.
    pub fn legacy_zone_count(self) -> Option<usize> {
        match self {
            Self::FourZoneWithNumpad | Self::FourZoneWithoutNumpad => Some(4),
            Self::OneZoneWithNumpad | Self::OneZoneWithoutNumpad => Some(1),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardCapability {
    pub keyboard_type: u8,
    pub kind: KeyboardKind,
    pub raw: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneColor {
    pub zone: usize,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyLightingState {
    pub colors_rgb: Vec<ZoneColor>,
    /// Bit 7 participates in HP hotkey/default-mode logic; this is not a percent.
    pub brightness_raw_byte: u8,
    pub colors_raw: Vec<u8>,
    pub brightness_raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardState {
    pub board: String,
    pub bios: String,
    pub capability: KeyboardCapability,
    /// None for types requiring a different, unvalidated lighting transport.
    pub legacy: Option<LegacyLightingState>,
}

impl KeyboardState {
    pub fn custom_colors_enabled(&self) -> Result<bool, String> {
        self.validate()?;
        Ok(self
            .legacy
            .as_ref()
            .ok_or("No classic lighting state")?
            .brightness_raw_byte
            & 0x80
            != 0)
    }
    /// Exact manufacturer enable/disable algorithm, not a brightness percentage:
    /// enable selects E4 if bit7 was clear; disable selects64.
    pub fn with_classic_mode(&self, custom: bool) -> Result<Self, String> {
        self.validate()?;
        let mut target = self.clone();
        let legacy = target.legacy.as_mut().ok_or("No classic lighting state")?;
        let raw = if custom {
            if legacy.brightness_raw_byte & 0x80 == 0 {
                0xe4
            } else {
                legacy.brightness_raw_byte
            }
        } else {
            0x64
        };
        legacy.brightness_raw[0] = raw;
        legacy.brightness_raw_byte = raw;
        Ok(target)
    }
    /// Validate semantic fields against the complete retained wire image.
    pub fn validate(&self) -> Result<(), String> {
        let capability = decode_keyboard_capability(0, &self.capability.raw)?;
        if capability != self.capability {
            return Err("Keyboard capability and raw image differ".into());
        }
        if let Some(legacy) = &self.legacy {
            let decoded = decode_legacy_lighting(
                capability.kind,
                0,
                &legacy.colors_raw,
                0,
                &legacy.brightness_raw,
            )?;
            if decoded != *legacy {
                return Err("Keyboard semantic fields and raw image differ".into());
            }
        } else if capability.kind.legacy_zone_count().is_some() {
            return Err("Complete legacy keyboard state is missing".into());
        }
        Ok(())
    }
    pub fn colors(&self) -> Result<Vec<[u8; 3]>, String> {
        self.validate()?;
        let legacy = self
            .legacy
            .as_ref()
            .ok_or("No validated classic RGB layout")?;
        Ok(legacy.colors_rgb.iter().map(|c| [c.r, c.g, c.b]).collect())
    }
    /// Only RGB bytes are editable. Every reserved byte and the entire brightness
    /// image are retained, including HP's hotkey/default-mode bits.
    pub fn with_colors(&self, colors: &[[u8; 3]]) -> Result<Self, String> {
        self.validate()?;
        let count = self
            .capability
            .kind
            .legacy_zone_count()
            .ok_or("No validated classic RGB layout")?;
        if colors.len() != count {
            return Err(format!("This keyboard requires exactly {count} RGB zones"));
        }
        let mut target = self.clone();
        let legacy = target
            .legacy
            .as_mut()
            .ok_or("Complete legacy state is missing")?;
        for (i, color) in colors.iter().enumerate() {
            legacy.colors_raw[25 + i * 3..28 + i * 3].copy_from_slice(color);
        }
        target.legacy = Some(decode_legacy_lighting(
            target.capability.kind,
            0,
            &legacy.colors_raw,
            0,
            &legacy.brightness_raw,
        )?);
        Ok(target)
    }
    /// Recovery never sends opaque bytes from a mutable journal. They must still
    /// match a fresh read; only the recorded, bounded RGB components may differ.
    pub fn same_opaque_controls(&self, current: &Self) -> Result<bool, String> {
        self.same_invariant_controls(current, false)
    }
    pub fn same_invariant_controls(
        &self,
        current: &Self,
        allow_mode: bool,
    ) -> Result<bool, String> {
        self.validate()?;
        current.validate()?;
        if self.board != current.board
            || self.bios != current.bios
            || self.capability != current.capability
        {
            return Ok(false);
        }
        let before = self
            .legacy
            .as_ref()
            .ok_or("Complete legacy state is missing")?;
        let now = current
            .legacy
            .as_ref()
            .ok_or("Complete legacy state is missing")?;
        let count = self
            .capability
            .kind
            .legacy_zone_count()
            .ok_or("No classic RGB layout")?;
        Ok(before
            .brightness_raw
            .iter()
            .zip(&now.brightness_raw)
            .enumerate()
            .all(|(i, (a, b))| (allow_mode && i == 0) || a == b)
            && before
                .colors_raw
                .iter()
                .zip(&now.colors_raw)
                .enumerate()
                .all(|(i, (a, b))| (25..25 + 3 * count).contains(&i) || a == b))
    }
}

fn validate_reply(return_code: u32, data: &[u8], expected_size: usize) -> Result<(), String> {
    if return_code != 0 {
        return Err(format!("HP BIOS return code: {return_code:#x}"));
    }
    if data.len() != expected_size {
        return Err(format!(
            "HP keyboard reply needs exactly {expected_size} bytes, received {}",
            data.len()
        ));
    }
    Ok(())
}

pub fn decode_keyboard_capability(
    return_code: u32,
    data: &[u8],
) -> Result<KeyboardCapability, String> {
    validate_reply(return_code, data, 4)?;
    let kind = match data[0] {
        0 => KeyboardKind::Normal,
        1 => KeyboardKind::FourZoneWithNumpad,
        2 => KeyboardKind::FourZoneWithoutNumpad,
        3 => KeyboardKind::RgbPerKey,
        4 => KeyboardKind::OneZoneWithNumpad,
        5 => KeyboardKind::OneZoneWithoutNumpad,
        6 => KeyboardKind::FourZoneMCUWithNumpad,
        7 => KeyboardKind::FourZoneMCUWithoutNumpad,
        other => return Err(format!("Unknown HP keyboard type: {other}")),
    };
    Ok(KeyboardCapability {
        keyboard_type: data[0],
        kind,
        raw: [data[0], data[1], data[2], data[3]],
    })
}

pub fn decode_legacy_lighting(
    kind: KeyboardKind,
    colors_return_code: u32,
    colors: &[u8],
    brightness_return_code: u32,
    brightness: &[u8],
) -> Result<LegacyLightingState, String> {
    let zones = kind
        .legacy_zone_count()
        .ok_or_else(|| "Keyboard type has no validated legacy lighting layout".to_string())?;
    validate_reply(colors_return_code, colors, 128)?;
    validate_reply(brightness_return_code, brightness, 128)?;
    let colors_rgb = (0..zones)
        .map(|zone| {
            let offset = 25 + 3 * zone;
            ZoneColor {
                zone,
                r: colors[offset],
                g: colors[offset + 1],
                b: colors[offset + 2],
            }
        })
        .collect();
    Ok(LegacyLightingState {
        colors_rgb,
        brightness_raw_byte: brightness[0],
        colors_raw: colors.to_vec(),
        brightness_raw: brightness.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manufacturer_mode_transform_keeps_every_unrelated_byte_and_has_exact_restore() {
        let before = original();
        let preset = before.with_classic_mode(false).unwrap();
        assert_eq!(preset.legacy.as_ref().unwrap().brightness_raw_byte, 0x64);
        assert!(!preset.custom_colors_enabled().unwrap());
        assert_eq!(preset.with_classic_mode(true).unwrap(), before);
        assert!(!before.same_opaque_controls(&preset).unwrap());
        assert!(before.same_invariant_controls(&preset, true).unwrap());
    }
    fn original() -> KeyboardState {
        let colors = (0..128).map(|x| x as u8).collect::<Vec<_>>();
        let mut brightness = vec![0; 128];
        brightness[0] = 0xe4;
        KeyboardState {
            board: "8E35".into(),
            bios: "F.13".into(),
            capability: decode_keyboard_capability(0, &[1, 0, 0, 0]).unwrap(),
            legacy: Some(
                decode_legacy_lighting(
                    KeyboardKind::FourZoneWithNumpad,
                    0,
                    &colors,
                    0,
                    &brightness,
                )
                .unwrap(),
            ),
        }
    }
    #[test]
    fn rgb_patch_retains_all_opaque_fields_and_can_restore_exactly() {
        let before = original();
        let colors = [[0, 255, 10], [11, 12, 13], [255, 255, 255], [0, 0, 0]];
        let target = before.with_colors(&colors).unwrap();
        assert_eq!(target.colors().unwrap(), colors);
        assert!(before.same_opaque_controls(&target).unwrap());
        assert_eq!(
            target.with_colors(&before.colors().unwrap()).unwrap(),
            before
        );
        assert_eq!(target.legacy.as_ref().unwrap().brightness_raw_byte, 0xe4);
        assert!(before.with_colors(&colors[..3]).is_err());
    }
    #[test]
    fn recovery_rejects_opaque_changes_and_forged_semantic_snapshots() {
        let before = original();
        let mut changed = before.clone();
        changed.legacy.as_mut().unwrap().colors_raw[10] ^= 1;
        assert!(!before.same_opaque_controls(&changed).unwrap());
        changed = before.clone();
        changed.legacy.as_mut().unwrap().brightness_raw[1] = 1;
        assert!(!before.same_opaque_controls(&changed).unwrap());
        changed = before.clone();
        changed.capability.keyboard_type = 4;
        assert!(changed.validate().is_err());
        changed = before.clone();
        changed.legacy.as_mut().unwrap().colors_rgb[0].r = 255;
        assert!(changed.validate().is_err());
    }

    #[test]
    fn failed_zero_reply_does_not_fall_back_to_normal_keyboard() {
        assert!(decode_keyboard_capability(255, &[0; 4]).is_err());
        assert_eq!(
            decode_keyboard_capability(0, &[0; 4]).unwrap().kind,
            KeyboardKind::Normal
        );
    }

    #[test]
    fn capability_rejects_wrong_sizes_and_unknown_types() {
        for size in [0, 1, 3, 5, 128] {
            assert!(decode_keyboard_capability(0, &vec![1; size]).is_err());
        }
        for kind in [8, 128, 255] {
            assert!(decode_keyboard_capability(0, &[kind, 0, 0, 0]).is_err());
        }
    }

    #[test]
    fn capability_preserves_uninterpreted_bytes() {
        let capability = decode_keyboard_capability(0, &[1, 5, 17, 255]).unwrap();
        assert_eq!(capability.kind, KeyboardKind::FourZoneWithNumpad);
        assert_eq!(capability.raw, [1, 5, 17, 255]);
    }

    #[test]
    fn classic_state_uses_rgb_offsets_and_retains_all_bytes() {
        let colors: Vec<u8> = (0..128).collect();
        let mut brightness = vec![19; 128];
        brightness[0] = 0xE4;
        let state =
            decode_legacy_lighting(KeyboardKind::FourZoneWithNumpad, 0, &colors, 0, &brightness)
                .unwrap();
        assert_eq!(state.colors_rgb.len(), 4);
        assert_eq!(
            state.colors_rgb[0],
            ZoneColor {
                zone: 0,
                r: 25,
                g: 26,
                b: 27
            }
        );
        assert_eq!(
            state.colors_rgb[3],
            ZoneColor {
                zone: 3,
                r: 34,
                g: 35,
                b: 36
            }
        );
        assert_eq!(state.colors_raw, colors);
        assert_eq!(state.brightness_raw, brightness);
        assert_eq!(state.brightness_raw_byte, 0xE4);
    }

    #[test]
    fn one_zone_does_not_decode_unused_color_slots() {
        let state = decode_legacy_lighting(
            KeyboardKind::OneZoneWithoutNumpad,
            0,
            &[0; 128],
            0,
            &[0; 128],
        )
        .unwrap();
        assert_eq!(state.colors_rgb.len(), 1);
        assert!(
            KeyboardKind::FourZoneMCUWithNumpad
                .legacy_zone_count()
                .is_none()
        );
        assert!(KeyboardKind::RgbPerKey.legacy_zone_count().is_none());
    }

    #[test]
    fn classic_state_rejects_errors_sizes_and_other_transports() {
        let kind = KeyboardKind::FourZoneWithNumpad;
        assert!(decode_legacy_lighting(kind, 1, &[0; 128], 0, &[0; 128]).is_err());
        assert!(decode_legacy_lighting(kind, 0, &[0; 128], 255, &[0; 128]).is_err());
        for size in [0, 4, 127, 129] {
            assert!(decode_legacy_lighting(kind, 0, &vec![0; size], 0, &[0; 128]).is_err());
            assert!(decode_legacy_lighting(kind, 0, &[0; 128], 0, &vec![0; size]).is_err());
        }
        for kind in [
            KeyboardKind::Normal,
            KeyboardKind::RgbPerKey,
            KeyboardKind::FourZoneMCUWithNumpad,
            KeyboardKind::FourZoneMCUWithoutNumpad,
        ] {
            assert!(decode_legacy_lighting(kind, 0, &[0; 128], 0, &[0; 128]).is_err());
        }
    }
}
