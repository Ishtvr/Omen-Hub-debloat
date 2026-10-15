//! Curve Optimizer and PBO scalar through fixed SMU messages. Both have exact
//! SMU GETs, so the complete per-core image is captured and restored.
use super::smu::{Message, Smu, core_mask, margin_arg, margin_value};
use omen_core::{
    Inventory,
    features::Feature,
    profiles::{CoreDelta, Value},
};
use serde::{Deserialize, Serialize};

/// Ryzen 9 8940HX: two CCDs with eight cores each.
const CORES: u8 = 16;
/// Zen 4 Curve Optimizer bounds the user may request.
pub const MARGIN_RANGE: (i64, i64) = (-50, 30);
pub const PBO_RANGE: (i64, i64) = (1, 10);

pub fn handles(feature: Feature) -> bool {
    matches!(
        feature,
        Feature::CpuCurveOptimizerAllCore
            | Feature::CpuCurveOptimizerPerCore
            | Feature::CpuPboScalar
    )
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
        return Err("Diese App-Version enthält keinen SMU-Adapter.".into());
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
                    return Err(format!(
                        "Curve Optimizer: CCD {} Kern {} mit {} liegt außerhalb {}…{}",
                        c.ccd + 1,
                        c.core + 1,
                        c.steps,
                        MARGIN_RANGE.0,
                        MARGIN_RANGE.1
                    ));
                }
            }
            Ok(())
        }
        (Feature::CpuPboScalar, Value::Integer(v)) if (PBO_RANGE.0..=PBO_RANGE.1).contains(v) => {
            Ok(())
        }
        _ => Err("Wert außerhalb des erlaubten Bereichs".into()),
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
