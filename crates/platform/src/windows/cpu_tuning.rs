//! Curve Optimizer, PBO scalar and the PPT/STAPM/slow/TDC/EDC/Tctl limits through
//! fixed SMU messages. Margins and scalar have SMU GETs; each limit is read back from the
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

/// Limit, its setter, measured PM-table cell (version 0x540208), the factor from
/// the table unit to the profile unit and the allowed profile range. Upper
/// bounds are the fused PPT/TDC, the observed EDC and a 95 °C Tctl ceiling.
type LimitSpec = (Feature, Message, usize, f32, (i64, i64));
const LIMITS: [LimitSpec; 5] = [
    (
        Feature::CpuPptLimitMw,
        Message::SetPptLimit,
        2,
        1000.0,
        (15_000, 160_000),
    ),
    (
        Feature::CpuSlowPptLimitMw,
        Message::SetSlowLimit,
        4,
        1000.0,
        (15_000, 160_000),
    ),
    (
        Feature::CpuTdcMa,
        Message::SetTdcLimit,
        8,
        1000.0,
        (30_000, 108_000),
    ),
    (
        Feature::CpuEdcMa,
        Message::SetEdcLimit,
        62,
        1000.0,
        (30_000, 160_000),
    ),
    (
        Feature::CpuTemperatureLimitC,
        Message::SetTctlMax,
        10,
        1.0,
        (60, 95),
    ),
];
// Both the original 16 October probe and the new 22 October probe returned
// decimal 5505544. The previous hexadecimal transcription, 0x540108, was wrong
// and blocked every limit capture before any write; no new layout is inferred.
const PM_TABLE_VERSION: u32 = 0x540208;
const SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
const READBACK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(2);
const READBACK_QUIET: std::time::Duration = std::time::Duration::from_millis(100);
/// The BIOS hands HP limits to the SMU a little later than RSMU messages land.
const HP_SETTLE: std::time::Duration = std::time::Duration::from_millis(600);
fn limit(feature: Feature) -> Option<(Message, usize, f32, (i64, i64))> {
    LIMITS
        .iter()
        .find(|l| l.0 == feature)
        .map(|l| (l.1, l.2, l.3, l.4))
}
/// Sends one fixed RSMU limit; the general HP PL1 command is deliberately not
/// used for independent CPU tuning. Its two fields did not stay independent.
fn send_limit(smu: &Smu, feature: Feature, value: i64) -> Result<(), String> {
    let (message, ..) = limit(feature).ok_or("No limit")?;
    smu.send(message, u32::try_from(value).map_err(|e| e.to_string())?)?;
    std::thread::sleep(SETTLE);
    Ok(())
}
/// STAPM and slow limit in whole watts after an HP `0x29` write, read from
/// their measured PM-table cells once the BIOS has handed them to the SMU.
pub fn hp_limits_w(inventory: &Inventory) -> Result<(i64, i64), String> {
    qualified(inventory)?;
    std::thread::sleep(HP_SETTLE);
    let smu = Smu::new(inventory)?;
    let pair = read_hp_pair(&smu)?;
    Ok((pair.stapm_mw / 1000, pair.slow_mw / 1000))
}
pub fn range(feature: Feature) -> (i64, i64) {
    match feature {
        Feature::CpuPboScalar => PBO_RANGE,
        f => limit(f).map_or(MARGIN_RANGE, |l| l.3),
    }
}

pub fn handles(feature: Feature) -> bool {
    feature != Feature::CpuStapmLimitMw
        && (matches!(
            feature,
            Feature::CpuCurveOptimizerAllCore
                | Feature::CpuCurveOptimizerPerCore
                | Feature::CpuPboScalar
        ) || limit(feature).is_some())
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
#[serde(deny_unknown_fields)]
pub struct HpLimitPair {
    pub stapm_mw: i64,
    pub slow_mw: i64,
}
impl HpLimitPair {
    fn validate(&self) -> Result<(), String> {
        if [self.stapm_mw, self.slow_mw]
            .iter()
            .any(|v| !(15_000..=160_000).contains(v) || v % 1000 != 0)
        {
            return Err(
                "Both HP limits must be exactly restorable whole watts in the qualified range"
                    .into(),
            );
        }
        Ok(())
    }
}

fn read_hp_pair(smu: &Smu) -> Result<HpLimitPair, String> {
    let (version, table) = smu.pm_table(8)?;
    decode_hp_pair(version, &table)
}
fn decode_hp_pair(version: u32, table: &[f32]) -> Result<HpLimitPair, String> {
    if version != PM_TABLE_VERSION
        || table.len() < 5
        || !table[0].is_finite()
        || !table[4].is_finite()
    {
        return Err("Paired HP limits have an unavailable PM-table contract".into());
    }
    let pair = HpLimitPair {
        stapm_mw: (table[0] * 1000.0).round() as i64,
        slow_mw: (table[4] * 1000.0).round() as i64,
    };
    pair.validate()?;
    Ok(pair)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SmuImage {
    /// Margin of every core, CCD-major.
    Margins { margins: Vec<i32> },
    /// The SMU's f32 PBO scalar as raw bits.
    PboScalar { bits: u32 },
    /// Fixed RSMU control, bound to the measured PM-table layout. Legacy HP
    /// limit images remain decodable but cannot select this transport.
    RsmuLimit { value: i64, table_version: u32 },
    /// A limit in profile units (mW, mA or °C) as read from its PM-table cell.
    Limit {
        value: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_pair: Option<HpLimitPair>,
    },
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
        return Ok(SmuImage::RsmuLimit {
            value: (raw * factor).round() as i64,
            table_version: version,
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
        (f, SmuImage::Limit { value, .. } | SmuImage::RsmuLimit { value, .. })
            if limit(f).is_some() =>
        {
            Value::Integer(*value)
        }
        _ => return Err("SMU image does not belong to this feature".into()),
    })
}

pub fn validate(feature: Feature, value: &Value) -> Result<(), String> {
    if feature == Feature::CpuStapmLimitMw {
        return Err("Independent STAPM control is not qualified on this firmware; use the measured Fast/Slow CPU limits".into());
    }
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
            if (lo..=hi).contains(v) {
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

/// Whether a captured SMU state equals the requested profile value.
pub fn image_matches(feature: Feature, image: &SmuImage, value: &Value) -> Result<bool, String> {
    Ok(match (feature, value, image) {
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
        (Feature::CpuPboScalar, Value::Integer(wanted), SmuImage::PboScalar { bits }) => {
            f32::from_bits(*bits) == *wanted as f32
        }
        _ => &self::value(feature, image)? == value,
    })
}

#[derive(Debug, Serialize)]
pub struct ReadbackSample {
    pub elapsed_ms: u64,
    pub image: SmuImage,
}
#[derive(Debug, Serialize)]
pub struct CpuReadbackTrace {
    pub feature: Feature,
    pub purpose: &'static str,
    pub target: Option<Value>,
    pub original: Option<SmuImage>,
    pub samples: Vec<ReadbackSample>,
    pub result: Result<(), String>,
}

fn observe_until(
    smu: &Smu,
    feature: Feature,
    matches: impl Fn(&SmuImage) -> Result<bool, String>,
    mut sample: impl FnMut(u64, &SmuImage),
) -> Result<SmuImage, String> {
    use omen_core::readback::{Decision, Settling};
    use std::time::Instant;
    let start = Instant::now();
    let mut settling = Settling::new(READBACK_DEADLINE, READBACK_QUIET);
    loop {
        // Errors and identity/type mismatches are immediate failures. Only a
        // valid old control value is observed again; the setter is never called.
        let live = capture(smu, feature)?;
        let elapsed = start.elapsed();
        sample(elapsed.as_millis() as u64, &live);
        let matched = matches(&live)?;
        if limit(feature).is_none() {
            return if matched {
                Ok(live)
            } else {
                Err(format!(
                    "SMU readback differs for {feature:?}: observed {live:?}"
                ))
            };
        }
        match settling.observe(elapsed, matched) {
            Decision::Matched => return Ok(live),
            Decision::TimedOut => {
                return Err(format!(
                    "SMU readback did not settle for {feature:?} within 2 seconds: observed {live:?}"
                ));
            }
            Decision::Wait => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

pub fn verify_target(
    smu: &Smu,
    feature: Feature,
    target: &Value,
    sample: impl FnMut(u64, &SmuImage),
) -> Result<SmuImage, String> {
    observe_until(
        smu,
        feature,
        |live| image_matches(feature, live, target),
        sample,
    )
    .map_err(|error| format!("{error}; requested {target:?}"))
}

pub fn image_owned(
    feature: Feature,
    original: &SmuImage,
    live: &SmuImage,
    target: &Value,
) -> Result<bool, String> {
    if let (SmuImage::Margins { margins: old }, SmuImage::Margins { margins: now }) =
        (original, live)
    {
        if old.len() != usize::from(CORES) || now.len() != old.len() {
            return Ok(false);
        }
        validate(feature, target)?;
        return Ok(old.iter().zip(now).enumerate().all(|(index, (old, now))| {
            let wanted = match target {
                Value::Integer(value) if feature == Feature::CpuCurveOptimizerAllCore => {
                    Some(*value as i32)
                }
                Value::PerCore(cores) if feature == Feature::CpuCurveOptimizerPerCore => cores
                    .iter()
                    .find(|c| usize::from(c.ccd) * 8 + usize::from(c.core) == index)
                    .map(|c| c.steps),
                _ => None,
            };
            old == now || wanted == Some(*now)
        }));
    }
    Ok(original == live || image_matches(feature, live, target)?)
}

pub fn restore(
    smu: &Smu,
    feature: Feature,
    original: &SmuImage,
    sample: impl FnMut(u64, &SmuImage),
) -> Result<(), String> {
    validate_original(feature, original)?;
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
        SmuImage::RsmuLimit { value, .. } => send_limit(smu, feature, *value)?,
        SmuImage::Limit { .. } => {
            return Err("Legacy HP limit image cannot select an RSMU restore".into());
        }
    }
    observe_until(smu, feature, |live| Ok(live == original), sample).map(|_| ())
}

pub fn validate_original(feature: Feature, original: &SmuImage) -> Result<(), String> {
    if feature == Feature::CpuStapmLimitMw
        || matches!(
            original,
            SmuImage::Limit {
                hp_pair: Some(_),
                ..
            }
        )
    {
        return Err("Legacy HP CPU original uses an unqualified independent STAPM path; no automatic restore is attempted".into());
    }
    match original {
        SmuImage::Margins { margins }
            if matches!(
                feature,
                Feature::CpuCurveOptimizerAllCore | Feature::CpuCurveOptimizerPerCore
            ) && margins.len() == usize::from(CORES)
                && margins.iter().all(|value| {
                    (MARGIN_RANGE.0..=MARGIN_RANGE.1).contains(&i64::from(*value))
                }) =>
        {
            Ok(())
        }
        SmuImage::PboScalar { bits } if feature == Feature::CpuPboScalar => {
            let scalar = f32::from_bits(*bits);
            let percent = (scalar * 100.0).round();
            if scalar.is_finite() && (1.0..=10.0).contains(&scalar) && percent / 100.0 == scalar {
                Ok(())
            } else {
                Err("PBO original is not a finite exactly restorable scalar".into())
            }
        }
        SmuImage::RsmuLimit {
            value,
            table_version,
        } if *table_version == PM_TABLE_VERSION && limit(feature).is_some() => {
            validate(feature, &Value::Integer(*value))
        }
        _ => Err("SMU original type, shape or range does not belong to this feature".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar_projection_does_not_hide_other_cpu_margins_or_fractional_pbo() {
        let mut margins = vec![-20; 16];
        margins[1] = -15;
        let original = SmuImage::Margins { margins };
        assert_eq!(
            value(Feature::CpuCurveOptimizerAllCore, &original).unwrap(),
            Value::Integer(-20)
        );
        assert!(
            !image_matches(
                Feature::CpuCurveOptimizerAllCore,
                &original,
                &Value::Integer(-20)
            )
            .unwrap()
        );
        let fractional = SmuImage::PboScalar {
            bits: 1.5f32.to_bits(),
        };
        assert!(!image_matches(Feature::CpuPboScalar, &fractional, &Value::Integer(2)).unwrap());
    }

    #[test]
    fn partial_margin_recovery_accepts_only_original_or_requested_cores() {
        let original = SmuImage::Margins {
            margins: vec![-20; 16],
        };
        let mut margins = vec![-20; 16];
        margins[0] = -19;
        let partial = SmuImage::Margins {
            margins: margins.clone(),
        };
        assert!(
            image_owned(
                Feature::CpuCurveOptimizerAllCore,
                &original,
                &partial,
                &Value::Integer(-19)
            )
            .unwrap()
        );
        margins[1] = -17;
        assert!(
            !image_owned(
                Feature::CpuCurveOptimizerAllCore,
                &original,
                &SmuImage::Margins { margins },
                &Value::Integer(-19)
            )
            .unwrap()
        );
    }

    #[test]
    fn retired_hp_images_and_invalid_smu_originals_cannot_restore() {
        let pair = HpLimitPair {
            stapm_mw: 80_000,
            slow_mw: 66_000,
        };
        let original = SmuImage::Limit {
            value: 80_000,
            hp_pair: Some(pair.clone()),
        };
        assert!(validate_original(Feature::CpuStapmLimitMw, &original).is_err());
        assert!(
            validate_original(
                Feature::CpuStapmLimitMw,
                &SmuImage::Limit {
                    value: 80_000,
                    hp_pair: None
                }
            )
            .is_err()
        );
        assert!(
            HpLimitPair {
                stapm_mw: 80_500,
                ..pair.clone()
            }
            .validate()
            .is_err()
        );
        assert!(validate_original(Feature::CpuSlowPptLimitMw, &original).is_err());
        assert!(
            validate_original(
                Feature::CpuSlowPptLimitMw,
                &SmuImage::Limit {
                    value: 55_000,
                    hp_pair: None
                }
            )
            .is_err()
        );
        assert!(
            validate_original(
                Feature::CpuSlowPptLimitMw,
                &SmuImage::RsmuLimit {
                    value: 55_000,
                    table_version: 0x540108
                }
            )
            .is_err()
        );
        assert!(
            validate_original(
                Feature::CpuSlowPptLimitMw,
                &SmuImage::RsmuLimit {
                    value: 55_000,
                    table_version: PM_TABLE_VERSION
                }
            )
            .is_ok()
        );
        for scalar in [f32::NAN, f32::INFINITY, 0.0, 11.0, 1.234] {
            assert!(
                validate_original(
                    Feature::CpuPboScalar,
                    &SmuImage::PboScalar {
                        bits: scalar.to_bits()
                    }
                )
                .is_err()
            );
        }
        assert!(
            validate_original(
                Feature::CpuPboScalar,
                &SmuImage::PboScalar {
                    bits: 1.5f32.to_bits()
                }
            )
            .is_ok()
        );
        assert!(
            validate_original(
                Feature::CpuCurveOptimizerAllCore,
                &SmuImage::Margins {
                    margins: vec![-20; 15]
                }
            )
            .is_err()
        );
        assert!(
            validate_original(
                Feature::CpuTdcMa,
                &SmuImage::Margins {
                    margins: vec![-20; 16]
                }
            )
            .is_err()
        );
    }
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

    #[test]
    fn handles_and_ranges_cover_all_tuning_domains() {
        for f in [
            Feature::CpuCurveOptimizerAllCore,
            Feature::CpuCurveOptimizerPerCore,
            Feature::CpuPboScalar,
            Feature::CpuPptLimitMw,
            Feature::CpuSlowPptLimitMw,
            Feature::CpuTdcMa,
            Feature::CpuEdcMa,
            Feature::CpuTemperatureLimitC,
        ] {
            assert!(handles(f), "{f:?} should be handled");
        }
        assert!(!handles(Feature::GpuCoreOffsetMhz));
        assert!(!handles(Feature::CpuStapmLimitMw));
        assert!(!handles(Feature::WindowsBoostMode));

        assert_eq!(range(Feature::CpuPboScalar), (1, 10));
        assert_eq!(range(Feature::CpuPptLimitMw), (15_000, 160_000));
        assert_eq!(range(Feature::CpuTdcMa), (30_000, 108_000));
        assert_eq!(range(Feature::CpuEdcMa), (30_000, 160_000));
        assert_eq!(range(Feature::CpuTemperatureLimitC), (60, 95));
        assert_eq!(range(Feature::CpuCurveOptimizerAllCore), (-50, 30));
    }

    #[test]
    fn limit_validation_checks_bounds_and_hp_whole_watts() {
        assert!(validate(Feature::CpuPptLimitMw, &Value::Integer(45_000)).is_ok());
        assert!(validate(Feature::CpuPptLimitMw, &Value::Integer(10_000)).is_err());
        assert!(validate(Feature::CpuPptLimitMw, &Value::Integer(200_000)).is_err());

        // HP limits (STAPM and Slow PPT) require whole watts (v % 1000 == 0)
        assert!(validate(Feature::CpuStapmLimitMw, &Value::Integer(45_000)).is_err());
        assert!(validate(Feature::CpuStapmLimitMw, &Value::Integer(45_500)).is_err());
        assert!(validate(Feature::CpuSlowPptLimitMw, &Value::Integer(30_000)).is_ok());
        assert!(validate(Feature::CpuSlowPptLimitMw, &Value::Integer(30_250)).is_ok());

        assert!(validate(Feature::CpuTdcMa, &Value::Integer(80_000)).is_ok());
        assert!(validate(Feature::CpuTdcMa, &Value::Integer(20_000)).is_err());
        assert!(validate(Feature::CpuTdcMa, &Value::Integer(120_000)).is_err());

        assert!(validate(Feature::CpuEdcMa, &Value::Integer(140_000)).is_ok());
        assert!(validate(Feature::CpuEdcMa, &Value::Integer(20_000)).is_err());
        assert!(validate(Feature::CpuEdcMa, &Value::Integer(180_000)).is_err());

        assert!(validate(Feature::CpuTemperatureLimitC, &Value::Integer(85)).is_ok());
        assert!(validate(Feature::CpuTemperatureLimitC, &Value::Integer(50)).is_err());
        assert!(validate(Feature::CpuTemperatureLimitC, &Value::Integer(100)).is_err());

        // Type mismatches
        assert!(
            validate(
                Feature::CpuPptLimitMw,
                &Value::Choice(omen_core::profiles::Choice::Manual)
            )
            .is_err()
        );
    }

    #[test]
    fn per_core_validation_rejects_invalid_ccd_core_or_steps() {
        let valid_cores: Vec<_> = (0..16)
            .map(|i| CoreDelta {
                ccd: (i / 8) as u8,
                core: (i % 8) as u8,
                steps: -15,
            })
            .collect();
        assert!(
            validate(
                Feature::CpuCurveOptimizerPerCore,
                &Value::PerCore(valid_cores)
            )
            .is_ok()
        );

        let invalid_ccd = vec![CoreDelta {
            ccd: 2,
            core: 0,
            steps: -10,
        }];
        assert!(
            validate(
                Feature::CpuCurveOptimizerPerCore,
                &Value::PerCore(invalid_ccd)
            )
            .is_err()
        );

        let invalid_core = vec![CoreDelta {
            ccd: 0,
            core: 8,
            steps: -10,
        }];
        assert!(
            validate(
                Feature::CpuCurveOptimizerPerCore,
                &Value::PerCore(invalid_core)
            )
            .is_err()
        );

        let invalid_steps = vec![CoreDelta {
            ccd: 0,
            core: 0,
            steps: -55,
        }];
        assert!(
            validate(
                Feature::CpuCurveOptimizerPerCore,
                &Value::PerCore(invalid_steps)
            )
            .is_err()
        );
    }

    #[test]
    fn smu_image_round_trips_and_maps_values() {
        for image in [
            SmuImage::Margins {
                margins: vec![-10, 0, 5, -30],
            },
            SmuImage::PboScalar {
                bits: 2.0f32.to_bits(),
            },
            SmuImage::Limit {
                value: 65_000,
                hp_pair: None,
            },
        ] {
            let text = serde_json::to_string(&image).unwrap();
            let parsed: SmuImage = serde_json::from_str(&text).unwrap();
            assert_eq!(parsed, image);
        }

        assert_eq!(
            value(
                Feature::CpuPptLimitMw,
                &SmuImage::Limit {
                    value: 55_000,
                    hp_pair: None
                }
            )
            .unwrap(),
            Value::Integer(55_000)
        );
        assert_eq!(
            value(
                Feature::CpuCurveOptimizerAllCore,
                &SmuImage::Margins {
                    margins: vec![-25; 16]
                }
            )
            .unwrap(),
            Value::Integer(-25)
        );

        assert!(value(Feature::CpuPptLimitMw, &SmuImage::PboScalar { bits: 0 }).is_err());
    }
}
