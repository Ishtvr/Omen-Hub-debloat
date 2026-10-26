//! Declarative presentation of typed preset features. No register/API knowledge.
use crate::i18n::t;
use omen_core::{
    features::{Feature as F, Unit, ValueKind},
    profiles::{Choice as C, Value},
};

/// Language keys of the collapsible groups, in display order.
pub const GROUPS: [&str; 8] = [
    "group.gpu",
    "group.limits",
    "group.pbo",
    "group.curve_optimizer",
    "group.curve_optimizer_per_core",
    "group.intel",
    "group.processor",
    "group.energy_display",
];
/// First group of each section and the section's language key.
pub const SECTIONS: &[(usize, &str)] = &[
    (0, "section.gpu"),
    (1, "section.cpu"),
    (6, "section.windows"),
];
pub struct Field {
    pub feature: F,
    pub group: usize,
    pub seed: i64,
}
/// Legacy keys remain readable. Explicit Save retires only the removed options.
/// Dynamic Boost stays available as an explicit planned
/// profile instruction; its hardware adapter still requires qualification.
pub const REMOVED_GPU_OPTIONS: &[F] = &[
    F::GpuVoltageOffsetMv,
    F::GpuMemoryVoltageOffsetMv,
    F::GpuVoltageLimitMv,
    F::GpuTemperatureLimitC,
    F::GpuCoreClockLimitMhz,
    F::GpuMemoryClockLimitMhz,
    F::GpuFrameRateLimit,
    F::GpuCustomTgp,
    F::OmenPerformanceMode,
];
/// Independent HP STAPM writes failed qualification. The CPU slow/fast
/// ceilings use their own measured RSMU messages, not an APU STAPM control.
/// APU power/VRM/soft-clock and integrated-graphics fields do not belong in the
/// requested CPU-only tuning UI. Retain decoding; retire intent on explicit Save.
pub const REMOVED_APU_OPTIONS: &[F] = &[
    F::CpuStapmLimitMw,
    F::CpuSkinTemperatureLimitC,
    F::CpuFastPptLimitMw,
    F::CpuStapmTimeSeconds,
    F::CpuSlowPptTimeSeconds,
    F::SocTdcMa,
    F::SocEdcMa,
    F::GfxTdcMa,
    F::GfxEdcMa,
    F::GfxCurveOptimizer,
    F::AmdIgpuClockMhz,
    F::IgpuSoftMinMhz,
    F::IgpuSoftMaxMhz,
    F::CpuSoftMinMhz,
    F::CpuSoftMaxMhz,
    F::DataSoftMinMhz,
    F::DataSoftMaxMhz,
    F::FabricSoftMinMhz,
    F::FabricSoftMaxMhz,
    F::VcnSoftMinMhz,
    F::VcnSoftMaxMhz,
    F::SocSoftMinMhz,
    F::SocSoftMaxMhz,
    F::IntelGpuPowerBalance,
    F::IntelGpuVoltageOffsetMv,
];
/// No Dragon Range control exists for these: the SMU and the original Hub
/// offer no AMD boost profile (Windows EPP/boost cover it) and no all-core
/// millivolt offset (Curve Optimizer is the Zen 4 offset). Fixed clocks,
/// core ratios and fixed voltages are removed at the user's request
/// (17 October 2026): most laptop firmware locks them, so the editor keeps
/// to the UXTU power, PBO and Curve Optimizer settings.
pub const REMOVED_CPU_OPTIONS: &[F] = &[
    F::CpuBoostProfile,
    F::CpuAllCoreVoltageOffsetMv,
    F::AmdCpuVoltageMv,
    F::AmdCpuClockMhz,
    F::IntelCoreRatios,
    F::IntelCacheVoltageOffsetMv,
    F::IntelSystemAgentVoltageOffsetMv,
];
pub fn retired(feature: F) -> bool {
    REMOVED_GPU_OPTIONS.contains(&feature)
        || REMOVED_APU_OPTIONS.contains(&feature)
        || REMOVED_CPU_OPTIONS.contains(&feature)
}
pub fn fields() -> Vec<Field> {
    let rows = [
        (F::GpuCoreOffsetMhz, 0, 0),
        (F::GpuMemoryOffsetMhz, 0, 0),
        (F::GpuPowerLimitMw, 0, 90000),
        (F::GpuVfCurve, 0, 0),
        (F::MaxFan, 0, 0),
        (F::GpuUnleashed, 0, 0),
        (F::CpuTemperatureLimitC, 1, 85),
        (F::CpuSlowPptLimitMw, 1, 55000),
        (F::CpuPptLimitMw, 1, 80000),
        (F::CpuTdcMa, 1, 108000),
        (F::CpuEdcMa, 1, 160000),
        (F::CpuPboScalar, 2, 1),
        (F::CpuCurveOptimizerAllCore, 3, 0),
        (F::CpuCurveOptimizerPerCore, 4, 0),
        (F::IntelPl1Mw, 5, 45000),
        (F::IntelPl2Mw, 5, 65000),
        (F::IntelPl4Mw, 5, 90000),
        (F::IntelIccMaxMa, 5, 90000),
        (F::IntelCpuPowerBalance, 5, 0),
        (F::WindowsBoostMode, 6, 0),
        (F::WindowsMinimumProcessorPercent, 6, 5),
        (F::WindowsMaximumProcessorPercent, 6, 100),
        (F::WindowsProcessorEppPercent, 6, 50),
        (F::WindowsCoreParkingMinPercent, 6, 0),
        (F::WindowsCoreParkingMaxPercent, 6, 100),
        (F::CpuCcdAffinity, 6, 3),
        (F::WindowsEnergyMode, 7, 0),
        (F::WindowsPowerPlan, 7, 0),
        (F::DisplayRefreshRateHz, 7, 60),
        (F::GraphicsMode, 7, 0),
    ];
    rows.iter()
        .map(|(feature, group, seed)| Field {
            feature: *feature,
            group: *group,
            seed: *seed,
        })
        .collect()
}
impl Field {
    /// Preserve existing native automation IDs as new fields are inserted.
    /// Layout order is independent of control identity.
    pub fn control_slot(&self) -> usize {
        const LEGACY: &[F] = &[
            F::GpuCoreOffsetMhz,
            F::GpuMemoryOffsetMhz,
            F::GpuVfCurve,
            F::MaxFan,
            F::GpuUnleashed,
            F::CpuTemperatureLimitC,
            F::CpuSlowPptLimitMw,
            F::CpuPptLimitMw,
            F::CpuTdcMa,
            F::CpuEdcMa,
            F::CpuPboScalar,
            F::CpuCurveOptimizerAllCore,
            F::CpuCurveOptimizerPerCore,
            F::IntelPl1Mw,
            F::IntelPl2Mw,
            F::IntelPl4Mw,
            F::IntelIccMaxMa,
            F::IntelCpuPowerBalance,
            F::WindowsBoostMode,
            F::WindowsMinimumProcessorPercent,
            F::WindowsMaximumProcessorPercent,
            F::WindowsProcessorEppPercent,
            F::WindowsCoreParkingMinPercent,
            F::WindowsCoreParkingMaxPercent,
            F::CpuCcdAffinity,
            F::WindowsEnergyMode,
            F::WindowsPowerPlan,
            F::DisplayRefreshRateHz,
            F::GraphicsMode,
        ];
        LEGACY
            .iter()
            .position(|f| *f == self.feature)
            .unwrap_or_else(|| {
                64 + F::all()
                    .iter()
                    .position(|f| *f == self.feature)
                    .expect("Catalog feature")
            })
    }
    /// Display name from the language file (`field.<Feature>`).
    pub fn label(&self) -> &'static str {
        t(&format!("field.{:?}", self.feature))
    }
    /// What the setting changes and what it does (`info.<Feature>`).
    pub fn info(&self) -> &'static str {
        t(&format!("info.{:?}", self.feature))
    }
    pub fn scale(&self) -> i64 {
        if matches!(
            self.feature.descriptor().unit,
            Unit::Milliwatt | Unit::Milliampere
        ) {
            1000
        } else {
            1
        }
    }
    pub fn unit(&self) -> &'static str {
        match self.feature.descriptor().unit {
            Unit::Mhz => "MHz",
            Unit::Milliwatt => "W",
            Unit::Milliampere => "A",
            Unit::Millivolt => "mV",
            Unit::Celsius => "°C",
            Unit::Seconds => "s",
            Unit::Percent => "%",
            Unit::Hz => "Hz",
            Unit::FramesPerSecond => "FPS",
            Unit::CurveStep => t("unit.steps"),
            Unit::None => "",
        }
    }
    pub fn choices(&self) -> &'static [C] {
        match self.feature {
            F::WindowsBoostMode => &[
                C::Disabled,
                C::Enabled,
                C::Aggressive,
                C::EfficientEnabled,
                C::EfficientAggressive,
                C::AggressiveAtGuaranteed,
                C::EfficientAggressiveAtGuaranteed,
            ],
            F::CpuBoostProfile => &[C::Auto, C::PowerSaver, C::Performance],
            F::WindowsEnergyMode => &[C::BestEfficiency, C::Balanced, C::BestPerformance],
            F::WindowsPowerPlan => &[C::PowerSaver, C::Balanced, C::HighPerformance],
            F::GraphicsMode => &[C::IntegratedOnly, C::Hybrid, C::DiscreteOnly],
            F::OmenPerformanceMode => &[C::Quiet, C::Balanced, C::Performance, C::Unleashed],
            _ => &[],
        }
    }
    pub fn initial(&self) -> Value {
        match self.feature.descriptor().value_kind {
            ValueKind::Choice => Value::Choice(self.choices()[0]),
            ValueKind::Boolean => Value::Boolean(false),
            ValueKind::PerCore => Value::PerCore(vec![]),
            ValueKind::CoreRatios => Value::CoreRatios(vec![self.seed as u8]),
            ValueKind::VfCurve => Value::VfCurve(vec![]),
            _ => Value::Integer(self.seed),
        }
    }
}

pub fn choice_label(choice: C) -> &'static str {
    t(&format!("choice.{choice:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_power_field_keeps_chart_cpu_windows_control_ids_and_has_a_unique_identity() {
        let fields = fields();
        assert_eq!(
            fields
                .iter()
                .find(|f| f.feature == F::GpuVfCurve)
                .unwrap()
                .control_slot(),
            2
        );
        assert_eq!(
            fields
                .iter()
                .find(|f| f.feature == F::CpuPptLimitMw)
                .unwrap()
                .control_slot(),
            7
        );
        assert_eq!(
            fields
                .iter()
                .find(|f| f.feature == F::WindowsBoostMode)
                .unwrap()
                .control_slot(),
            18
        );
        let slots: std::collections::BTreeSet<_> = fields.iter().map(Field::control_slot).collect();
        assert_eq!(slots.len(), fields.len());
        assert!(
            fields
                .iter()
                .find(|f| f.feature == F::GpuPowerLimitMw)
                .unwrap()
                .control_slot()
                >= 64
        );
    }
}
