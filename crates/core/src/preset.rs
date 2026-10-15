//! Platform-free preset editing: exact units and optimistic, per-field merging.
use crate::{
    features::Feature,
    profiles::{PerformanceProfile, Setting, Value},
};
use std::collections::BTreeSet;

/// Parse a displayed integer/decimal without float rounding or NaN. scale=1000
/// displays watts/amperes while keeping the serialized milliwatt/milliamp units.
pub fn parse_number(input: &str, scale: i64) -> Result<i64, String> {
    if !matches!(scale, 1 | 1000) {
        return Err("Unsupported display scale".into());
    }
    let input = input.trim().replace(',', ".");
    let negative = input.starts_with('-');
    let unsigned = input.strip_prefix(['-', '+']).unwrap_or(&input);
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || parts.next().is_some()
        || (scale == 1 && !fraction.is_empty())
        || fraction.len() > 3
    {
        return Err("Gültige Zahl eingeben.".into());
    }
    let whole: i64 = whole.parse().map_err(|_| "Zahl zu groß")?;
    let fraction: i64 = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<3}")
            .parse()
            .map_err(|_| "Ungültige Nachkommastellen")?
    };
    let number = whole
        .checked_mul(scale)
        .and_then(|n| n.checked_add(fraction))
        .ok_or("Zahl zu groß")?;
    if negative {
        number.checked_neg().ok_or("Zahl zu klein".into())
    } else {
        Ok(number)
    }
}
pub fn format_number(value: i64, scale: i64) -> String {
    if scale == 1 {
        return value.to_string();
    }
    let sign = if value < 0 { "-" } else { "" };
    let magnitude = value.unsigned_abs();
    if magnitude % 1000 == 0 {
        format!("{sign}{}", magnitude / 1000)
    } else {
        format!("{sign}{}.{:03}", magnitude / 1000, magnitude % 1000)
            .trim_end_matches('0')
            .to_string()
    }
}
/// Apply only fields the user changed. Preserve another editor's independent
/// changes, hidden settings and the complete stored GPU curve byte-for-value.
pub fn merge(
    baseline: &PerformanceProfile,
    fresh: &PerformanceProfile,
    desired_name: &str,
    edits: &[(Feature, Option<Value>)],
) -> Result<PerformanceProfile, String> {
    if baseline.id != fresh.id || baseline.binding != fresh.binding {
        return Err("Preset-Bindung wurde geändert. Erneut laden.".into());
    }
    let mut result = fresh.clone();
    if desired_name != baseline.name {
        if fresh.name != baseline.name {
            return Err("Preset-Name wurde zwischenzeitlich geändert.".into());
        }
        result.name = desired_name.into();
    }
    let mut unique = BTreeSet::new();
    let get = |p: &PerformanceProfile, f: Feature| {
        p.settings
            .iter()
            .find(|s| s.feature == f)
            .map(|s| s.value.clone())
    };
    for (feature, desired) in edits {
        if !unique.insert(*feature) {
            return Err("Doppelte Preset-Einstellung".into());
        }
        let before = get(baseline, *feature);
        if before == *desired {
            continue;
        }
        if get(fresh, *feature) != before {
            return Err(format!(
                "{:?} wurde in einem anderen Fenster geändert. Erneut laden.",
                feature
            ));
        }
        result.settings.retain(|s| s.feature != *feature);
        if let Some(value) = desired {
            let setting = Setting {
                feature: *feature,
                value: value.clone(),
            };
            setting.validate()?;
            result.settings.push(setting);
        }
    }
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{DeviceBinding, PROFILE_SCHEMA, VfPoint};
    #[test]
    fn exact_display_units_reject_fraction_loss_nan_and_overflow() {
        assert_eq!(parse_number("45,125", 1000).unwrap(), 45125);
        assert_eq!(parse_number("-0.015", 1000).unwrap(), -15);
        assert_eq!(format_number(-15, 1000), "-0.015");
        for value in ["NaN", "inf", "1e3", "1.0001", "1.2.3", ""] {
            assert!(parse_number(value, 1000).is_err());
        }
        assert!(parse_number("0.5", 1).is_err());
        assert!(parse_number("9223372036854775807", 1000).is_err());
    }
    #[test]
    fn independent_edits_merge_full_curve_and_hidden_settings_but_conflicts_stop() {
        let base = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "merge".into(),
            name: "Original".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec!["Ryzen".into()],
                nvidia_uuid: Some("test".into()),
                nvidia_driver: Some("617.14".into()),
                nvidia_vbios: Some("test".into()),
            },
            settings: vec![
                Setting {
                    feature: Feature::GpuVfCurve,
                    value: Value::VfCurve(vec![
                        VfPoint {
                            voltage_mv: 700,
                            frequency_mhz: 1500,
                        },
                        VfPoint {
                            voltage_mv: 900,
                            frequency_mhz: 1800,
                        },
                    ]),
                },
                Setting {
                    feature: Feature::KeyboardColors,
                    value: Value::Colors(vec![[1, 2, 3]; 4]),
                },
                Setting {
                    feature: Feature::GpuMemoryOffsetMhz,
                    value: Value::Integer(0),
                },
            ],
        };
        let mut fresh = base.clone();
        fresh.name = "Concurrent rename".into();
        fresh.settings[0].value = Value::VfCurve(vec![
            VfPoint {
                voltage_mv: 700,
                frequency_mhz: 1515,
            },
            VfPoint {
                voltage_mv: 900,
                frequency_mhz: 1815,
            },
        ]);
        let edits = vec![(Feature::GpuMemoryOffsetMhz, Some(Value::Integer(-24)))];
        let result = merge(&base, &fresh, &base.name, &edits).unwrap();
        assert_eq!(result.name, fresh.name);
        assert_eq!(result.settings[..2], fresh.settings[..2]);
        assert!(merge(&base, &fresh, "My rename", &edits).is_err());
        fresh.settings[2].value = Value::Integer(24);
        assert!(merge(&base, &fresh, &base.name, &edits).is_err());
    }
}
