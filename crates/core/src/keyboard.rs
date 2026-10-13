//! Read-only decoding of the observed HP keyboard capability and legacy state.
//! Raw state is retained; no setters or brightness percentage are inferred.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct KeyboardCapability {
    pub keyboard_type: u8,
    pub kind: KeyboardKind,
    pub raw: [u8; 4],
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ZoneColor {
    pub zone: usize,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct LegacyLightingState {
    pub colors_rgb: Vec<ZoneColor>,
    /// Bit 7 participates in HP hotkey/default-mode logic; this is not a percent.
    pub brightness_raw_byte: u8,
    pub colors_raw: Vec<u8>,
    pub brightness_raw: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct KeyboardState {
    pub board: String,
    pub bios: String,
    pub capability: KeyboardCapability,
    /// None for types requiring a different, unvalidated lighting transport.
    pub legacy: Option<LegacyLightingState>,
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
