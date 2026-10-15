//! Curve Optimizer, PBO scalar and the PPT/STAPM/slow/TDC/EDC/Tctl limits through
//! fixed SMU messages and the HP power-limit command. Margins and scalar have SMU GETs; each limit is read back from the
//! PM-table cell measured on 16 October 2026 (cpu-smu-calibrate-limits).
use super::smu::{Message, Smu, core_mask, margin_arg, margin_value};
use omen_core::{
    Inventory,
    features::Feature,
    message::msg,
    profiles::{CoreDelta, Value},
};
use serde::{Deserialize, Serialize};

/// Ryzen 9 8940HX: two CCDs with eight cores each.
const CORES: u8 = 16;
/// Zen 4 Curve Optimizer bounds the user may request.
pub const MARGIN_RANGE: (i64, i64) = (-50, 30);
pub const PBO_RANGE: (i64, i64) = (1, 10);

/// How a limit is written: a fixed RSMU message, or byte `n` of the HP BIOS
/// power-limit command (`0x29`), which on this board carries slow (byte 0) and
/// STAPM (byte 1) in whole watts (measured 16 October 2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setter {
    Smu(Message),
    Hp(usize),
}
/// Limit, its setter, measured PM-table cell (version 0x540108), the factor from
/// the table unit to the profile unit and the allowed profile range. Upper
/// bounds are the fused PPT/TDC, the observed EDC and a 95 °C Tctl ceiling.
type LimitSpec = (Feature, Setter, usize, f32, (i64, i64));
const LIMITS: [LimitSpec; 6] = [
    (
        Feature::CpuPptLimitMw,
        Setter::Smu(Message::SetPptLimit),
        2,
        1000.0,
        (15_000, 160_000),
    ),
    (
        Feature::CpuStapmLimitMw,
        Setter::Hp(1),
        0,
        1000.0,
        (15_000, 160_000),
    ),
    (
        Feature::CpuSlowPptLimitMw,
        Setter::Hp(0),
        4,
        1000.0,
        (15_000, 160_000),
    ),
    (
        Feature::CpuTdcMa,
        Setter::Smu(Message::SetTdcLimit),
        8,
        1000.0,
        (30_000, 108_000),
    ),
    (
        Feature::CpuEdcMa,
        Setter::Smu(Message::SetEdcLimit),
        62,
        1000.0,
        (30_000, 160_000),
    ),
    (
        Feature::CpuTemperatureLimitC,
        Setter::Smu(Message::SetTctlMax),
        10,
        1.0,
        (60, 95),
    ),
];
const PM_TABLE_VERSION: u32 = 0x540108;
const SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
/// The BIOS hands HP limits to the SMU a little later than RSMU messages land.
const HP_SETTLE: std::time::Duration = std::time::Duration::from_millis(600);
fn limit(feature: Feature) -> Option<(Setter, usize, f32, (i64, i64))> {
    LIMITS
        .iter()
        .find(|l| l.0 == feature)
        .map(|l| (l.1, l.2, l.3, l.4))
}
/// Sends one limit. The HP command always carries both slow and STAPM, so the
/// other one is written with its live value instead of relying on `0xFF`.
fn send_limit(smu: &Smu, feature: Feature, value: i64) -> Result<(), String> {
    let (setter, ..) = limit(feature).ok_or("No limit")?;
    match setter {
        Setter::Smu(message) => {
            smu.send(message, u32::try_from(value).map_err(|e| e.to_string())?)?;
            // The PM table reflects a new limit after a short refresh interval.
            std::thread::sleep(SETTLE);
        }
        Setter::Hp(byte) => {
            let (version, table) = smu.pm_table(8)?;
            if version != PM_TABLE_VERSION {
                return Err(format!(
                    "PM table {version:#x} differs from the measured layout"
                ));
            }
            let watts = |cell: usize| {
                let w = table[cell].round();
                if (15.0..=160.0).contains(&w) {
                    Ok(w as u8)
                } else {
                    Err(format!("PM-table limit {w} W is not plausible"))
                }
            };
            let mut payload = [watts(4)?, watts(0)?, 0xFF, 0xFF];
            payload[byte] = u8::try_from(value / 1000).map_err(|e| e.to_string())?;
            smu.hp()?
                .set_mode_payload(omen_core::hp_mode::POWER_LIMITS_TYPE, payload)
                .map_err(|e| e.to_string())?;
            std::thread::sleep(HP_SETTLE);
        }
    }
    Ok(())
}
pub fn range(feature: Feature) -> (i64, i64) {
    match feature {
        Feature::CpuPboScalar => PBO_RANGE,
        f => limit(f).map_or(MARGIN_RANGE, |l| l.3),
    }
}

pub fn handles(feature: Feature) -> bool {
    matches!(
        feature,
        Feature::CpuCurveOptimizerAllCore
            | Feature::CpuCurveOptimizerPerCore
            | Feature::CpuPboScalar
    ) || limit(feature).is_some()
}

/// Board/CPU qualification without opening the driver (needs elevation).
pub fn qualified(inventory: &Inventory) -> Result<(), String> {
    if inventory.manufacturer != "HP"
        || inventory.board != "8E35"
        || inventory.bios != "F.13"
        || inventory.cpus.len() != 1
        || !inventory.cpus[0].contains("AMD Ryzen 9 8940HX")
    {
        return Err("CPU tuning is qualified only for HP8E35/F.13/Ryzen9 8940HX".into());
    }
    if !super::smu::module_present() {
        return Err(msg("cpu.err.no_smu_module", &[]));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SmuImage {
    /// Margin of every core, CCD-major.
    Margins { margins: Vec<i32> },
    /// The SMU's f32 PBO scalar as raw bits.
    PboScalar { bits: u32 },
    /// A limit in profile units (mW, mA or °C) as read from its PM-table cell.
    Limit { value: i64 },
}

fn margins(smu: &Smu) -> Result<Vec<i32>, String> {
    (0..CORES)
        .map(|i| {
            smu.send(Message::GetCoreMargin, core_mask(i / 8, i % 8))
                .map(|r| margin_value(r[0]))
        })
        .collect()
}

pub fn capture(smu: &Smu, feature: Feature) -> Result<SmuImage, String> {
    if let Some((_, cell, factor, _)) = limit(feature) {
        let (version, table) = smu.pm_table(64)?;
        if version != PM_TABLE_VERSION {
            return Err(format!(
                "PM table {version:#x} differs from the measured layout"
            ));
        }
        let raw = *table.get(cell).ok_or("PM table is too short")?;
        if !raw.is_finite() || raw <= 0.0 {
            return Err("PM-table limit is not plausible".into());
        }
        return Ok(SmuImage::Limit {
            value: (raw * factor).round() as i64,
        });
    }
    match feature {
        Feature::CpuCurveOptimizerAllCore | Feature::CpuCurveOptimizerPerCore => {
            Ok(SmuImage::Margins {
                margins: margins(smu)?,
            })
        }
        Feature::CpuPboScalar => Ok(SmuImage::PboScalar {
            bits: smu.send(Message::GetPboScalar, 0)?[0],
        }),
        _ => Err("Not an SMU tuning feature".into()),
    }
}

pub fn value(feature: Feature, image: &SmuImage) -> Result<Value, String> {
    Ok(match (feature, image) {
        // A uniform image reads as its margin; a mixed one as the first core's.
        (Feature::CpuCurveOptimizerAllCore, SmuImage::Margins { margins }) => {
            Value::Integer(i64::from(*margins.first().ok_or("No cores")?))
        }
        (Feature::CpuCurveOptimizerPerCore, SmuImage::Margins { margins }) => Value::PerCore(
            margins
                .iter()
                .enumerate()
                .map(|(i, steps)| CoreDelta {
                    ccd: (i / 8) as u8,
                    core: (i % 8) as u8,
                    steps: *steps,
                })
                .collect(),
        ),
        (Feature::CpuPboScalar, SmuImage::PboScalar { bits }) => {
            Value::Integer(f32::from_bits(*bits).round() as i64)
        }
        (f, SmuImage::Limit { value }) if limit(f).is_some() => Value::Integer(*value),
        _ => return Err("SMU image does not belong to this feature".into()),
    })
}

pub fn validate(feature: Feature, value: &Value) -> Result<(), String> {
    let in_margin = |v: i64| (MARGIN_RANGE.0..=MARGIN_RANGE.1).contains(&v);
    match (feature, value) {
        (Feature::CpuCurveOptimizerAllCore, Value::Integer(v)) if in_margin(*v) => Ok(()),
        (Feature::CpuCurveOptimizerPerCore, Value::PerCore(cores)) => {
            for c in cores {
                if c.ccd > 1 || c.core > 7 || !in_margin(i64::from(c.steps)) {
                    return Err(msg(
                        "cpu.err.core_margin",
                        &[
                            &(c.ccd + 1),
                            &(c.core + 1),
                            &c.steps,
                            &MARGIN_RANGE.0,
                            &MARGIN_RANGE.1,
                        ],
                    ));
                }
            }
            Ok(())
        }
        (Feature::CpuPboScalar, Value::Integer(v)) if (PBO_RANGE.0..=PBO_RANGE.1).contains(v) => {
            Ok(())
        }
        (f, Value::Integer(v)) if limit(f).is_some() => {
            let (lo, hi) = range(f);
            let whole_watts = !matches!(limit(f), Some((Setter::Hp(_), ..))) || v % 1000 == 0;
            if (lo..=hi).contains(v) && whole_watts {
                Ok(())
            } else {
                // Shown in the profile unit (W, A or °C), not mW/mA.
                let unit = limit(f).map_or(1, |l| l.2 as i64);
                Err(msg(
                    "cpu.err.limit_range",
                    &[&(v / unit), &(lo / unit), &(hi / unit)],
                ))
            }
        }
        _ => Err(msg("cpu.err.out_of_range", &[])),
    }
}

pub fn write(smu: &Smu, feature: Feature, value: &Value) -> Result<(), String> {
    validate(feature, value)?;
    match (feature, value) {
        (Feature::CpuCurveOptimizerAllCore, Value::Integer(v)) => {
            smu.send(Message::SetAllCoreMargin, margin_arg(*v as i32))?;
        }
        (Feature::CpuCurveOptimizerPerCore, Value::PerCore(cores)) => {
            for c in cores {
                smu.send(
                    Message::SetCoreMargin,
                    core_mask(c.ccd, c.core) | margin_arg(c.steps),
                )?;
            }
        }
        (Feature::CpuPboScalar, Value::Integer(v)) => {
            smu.send(Message::SetPboScalar, (*v as u32) * 100)?;
        }
        (f, Value::Integer(v)) if limit(f).is_some() => send_limit(smu, f, *v)?,
        _ => return Err("Wrong SMU value".into()),
    }
    Ok(())
}

/// Whether the live SMU state equals the requested profile value.
pub fn matches(smu: &Smu, feature: Feature, value: &Value) -> Result<bool, String> {
    let live = capture(smu, feature)?;
    Ok(match (feature, value, &live) {
        (Feature::CpuCurveOptimizerAllCore, Value::Integer(v), SmuImage::Margins { margins }) => {
            margins.iter().all(|m| i64::from(*m) == *v)
        }
        (
            Feature::CpuCurveOptimizerPerCore,
            Value::PerCore(cores),
            SmuImage::Margins { margins },
        ) => cores
            .iter()
            .all(|c| margins.get(usize::from(c.ccd) * 8 + usize::from(c.core)) == Some(&c.steps)),
        _ => &self::value(feature, &live)? == value,
    })
}

pub fn restore(smu: &Smu, feature: Feature, original: &SmuImage) -> Result<(), String> {
    match original {
        SmuImage::Margins { margins } => {
            for (i, steps) in margins.iter().enumerate() {
                smu.send(
                    Message::SetCoreMargin,
                    core_mask((i / 8) as u8, (i % 8) as u8) | margin_arg(*steps),
                )?;
            }
        }
        SmuImage::PboScalar { bits } => {
            smu.send(
                Message::SetPboScalar,
                (f32::from_bits(*bits) * 100.0).round() as u32,
            )?;
        }
        SmuImage::Limit { value } => send_limit(smu, feature, *value)?,
    }
    if capture(smu, feature)? != *original {
        return Err("SMU setting did not return to its captured original".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn per_core_value_lists_both_ccds() {
        let image = SmuImage::Margins {
            margins: (0..16).map(|i| -i).collect(),
        };
        let Value::PerCore(cores) = value(Feature::CpuCurveOptimizerPerCore, &image).unwrap()
        else {
            panic!()
        };
        assert_eq!(cores.len(), 16);
        assert_eq!((cores[9].ccd, cores[9].core, cores[9].steps), (1, 1, -9));
        assert_eq!(
            value(
                Feature::CpuPboScalar,
                &SmuImage::PboScalar {
                    bits: 1.0f32.to_bits()
                }
            )
            .unwrap(),
            Value::Integer(1)
        );
        assert!(validate(Feature::CpuCurveOptimizerAllCore, &Value::Integer(-60)).is_err());
        assert!(validate(Feature::CpuPboScalar, &Value::Integer(11)).is_err());
    }
}
