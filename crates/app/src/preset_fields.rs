//! Declarative presentation of typed preset features. No register/API knowledge.
use crate::i18n::t;
use omen_core::{
    features::{Feature as F, Unit, ValueKind},
    profiles::{Choice as C, Value},
};

/// Language keys of the collapsible groups, in display order.
pub const GROUPS: [&str; 8] = [
    "group.gpu",
    "group.dynamic_boost",
    "group.limits",
    "group.pbo",
    "group.curve_optimizer",
    "group.curve_optimizer_per_core",
    "group.processor",
    "group.energy_display",
];
/// First group of each section and the section's language key.
pub const SECTIONS: &[(usize, &str)] = &[
    (0, "section.gpu"),
    (2, "section.cpu"),
    (6, "section.windows"),
];
pub struct Field {
    pub feature: F,
    pub group: usize,
    pub seed: i64,
}
/// Legacy keys remain readable. Explicit Save retires only the removed options.
/// Dynamic Boost has its own qualified adapter; it is not a retired control.
pub const REMOVED_GPU_OPTIONS: &[F] = &[
    F::GpuVoltageOffsetMv,
    F::GpuMemoryVoltageOffsetMv,
    F::GpuVoltageLimitMv,
    F::GpuTemperatureLimitC,
    F::GpuCoreClockLimitMhz,
    F::GpuMemoryClockLimitMhz,
    F::GpuFrameRateLimit,
    // The HP mode is the dashboard's switch (30 October 2026).
    F::OmenPerformanceMode,
    // NVML refuses the power-limit GET/SET on the RTX 5070 Laptop (status 3,
    // not supported); cTGP, Dynamic Boost and TPP set the GPU envelope instead.
    F::GpuPowerLimitMw,
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
];
/// No Dragon Range control exists for these: the SMU and the original Hub
/// offer no AMD boost profile (Windows EPP/boost cover it) and no all-core
/// millivolt offset (Curve Optimizer is the Zen 4 offset). Fixed clocks,
/// and fixed voltages are removed at the user's request
/// (17 October 2026): most laptop firmware locks them, so the editor keeps
/// to the UXTU power, PBO and Curve Optimizer settings.
pub const REMOVED_CPU_OPTIONS: &[F] = &[
    F::CpuBoostProfile,
    F::CpuAllCoreVoltageOffsetMv,
    F::AmdCpuVoltageMv,
    F::AmdCpuClockMhz,
];
/// Max-Fan belongs to the separate cooling control. Legacy profile intent is
/// removed only by explicit Save; its recovery images remain fully supported.
pub const REMOVED_PRESET_OPTIONS: &[F] = &[F::MaxFan];
/// Volatile values the firmware and driver set again at every boot. While no
/// control ran since boot, the live reading is this device's own default.
pub const DEVICE_DEFAULTS: &[F] = &[
    F::CpuPptLimitMw,
    F::CpuSlowPptLimitMw,
    F::CpuTdcMa,
    F::CpuEdcMa,
    F::CpuTemperatureLimitC,
    F::CpuPboScalar,
    F::CpuCurveOptimizerAllCore,
    F::GpuCoreOffsetMhz,
    F::GpuMemoryOffsetMhz,
];
/// Slider bounds in whole watts, taken from the OMEN Hub's own panel for this
/// model (1101.2611.2.0, PowerControl.JSON.Khalilah_DRG_N22X4X6, 8E35/F.13):
/// TPP TppMinValue-TppMaxValue 55-72 W; CPU PL1 in Unleashed
/// UnleashedModeMinPL1-UnleashedModeMaxPL1 45-80 W in 1-W steps, written
/// as SetPowerLimit = 0x29 [PL1, PL1, FF, FF]. Without Dynamic Boost the
/// CPU slider spans the qualified Slow range instead. Validation and Apply
/// keep their own checks; the slider only guides the value.
pub fn slider_watts(feature: F, dynamic_boost: bool) -> Option<(i64, i64)> {
    match feature {
        F::HpSharedPowerMw => Some((55, 72)),
        F::CpuSlowPptLimitMw if dynamic_boost => Some((45, 80)),
        F::CpuSlowPptLimitMw => Some((15, 160)),
        _ => None,
    }
}
pub fn retired(feature: F) -> bool {
    feature.retired()
        || REMOVED_GPU_OPTIONS.contains(&feature)
        || REMOVED_APU_OPTIONS.contains(&feature)
        || REMOVED_CPU_OPTIONS.contains(&feature)
        || REMOVED_PRESET_OPTIONS.contains(&feature)
}
/// Present the same Dynamic Boost spelling/precedence that Apply resolves.
/// Saving still uses the existing alias-aware optimistic merge in core.
pub fn display_value(
    profile: &omen_core::profiles::PerformanceProfile,
    feature: F,
) -> Option<&Value> {
    omen_core::preset::display_value(
        profile,
        if feature == F::GpuUnleashed {
            F::GpuDynamicBoost
        } else {
            feature
        },
    )
}
pub fn fields() -> Vec<Field> {
    let rows = [
        (F::GpuCoreOffsetMhz, 0, 0),
        (F::GpuMemoryOffsetMhz, 0, 0),
        (F::GpuVfCurve, 0, 0),
        // Dynamic Boost group, as in the Hub's Unleashed panel: the mode
        // switch, cTGP, the shared CPU+GPU budget (TPP) and the CPU watt
        // slider (firmware PL1 = Slow PPT, written through HP 0x29).
        (F::GpuUnleashed, 1, 0),
        // cTGP is on unless the user turns it off (user decision, 30 October 2026).
        (F::GpuCustomTgp, 1, 1),
        // The Hub's Unleashed default budget (TppMaxValue of this model).
        (F::HpSharedPowerMw, 1, 72000),
        (F::CpuSlowPptLimitMw, 1, 55000),
        (F::CpuTemperatureLimitC, 2, 85),
        (F::CpuPptLimitMw, 2, 80000),
        (F::CpuTdcMa, 2, 108000),
        (F::CpuEdcMa, 2, 160000),
        (F::CpuPboScalar, 3, 1),
        (F::CpuCurveOptimizerAllCore, 4, 0),
        (F::CpuCurveOptimizerPerCore, 5, 0),
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
        const LEGACY: &[(F, usize)] = &[
            (F::GpuCoreOffsetMhz, 0),
            (F::GpuMemoryOffsetMhz, 1),
            (F::GpuVfCurve, 2),
            (F::GpuUnleashed, 4),
            (F::CpuTemperatureLimitC, 5),
            (F::CpuSlowPptLimitMw, 6),
            (F::CpuPptLimitMw, 7),
            (F::CpuTdcMa, 8),
            (F::CpuEdcMa, 9),
            (F::CpuPboScalar, 10),
            (F::CpuCurveOptimizerAllCore, 11),
            (F::CpuCurveOptimizerPerCore, 12),
            (F::WindowsBoostMode, 18),
            (F::WindowsMinimumProcessorPercent, 19),
            (F::WindowsMaximumProcessorPercent, 20),
            (F::WindowsProcessorEppPercent, 21),
            (F::WindowsCoreParkingMinPercent, 22),
            (F::WindowsCoreParkingMaxPercent, 23),
            (F::CpuCcdAffinity, 24),
            (F::WindowsEnergyMode, 25),
            (F::WindowsPowerPlan, 26),
            (F::DisplayRefreshRateHz, 27),
            (F::GraphicsMode, 28),
        ];
        LEGACY
            .iter()
            .find_map(|(feature, slot)| (*feature == self.feature).then_some(*slot))
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
            F::WindowsEnergyMode => &[C::BestEfficiency, C::Balanced, C::BestPerformance],
            F::WindowsPowerPlan => &[C::PowerSaver, C::Balanced, C::HighPerformance],
            F::GraphicsMode => &[C::IntegratedOnly, C::Hybrid, C::DiscreteOnly],
            _ => &[],
        }
    }
    pub fn initial(&self) -> Value {
        match self.feature.descriptor().value_kind {
            ValueKind::Choice => Value::Choice(self.choices()[0]),
            ValueKind::Boolean => Value::Boolean(self.seed != 0),
            ValueKind::PerCore => Value::PerCore(vec![]),
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
    fn dynamic_boost_display_and_saved_edit_follow_apply_precedence() {
        use omen_core::profiles::{DeviceBinding, PerformanceProfile, Setting};
        let mut profile = PerformanceProfile {
            schema: 1,
            id: "alias-test".into(),
            name: "Alias".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec!["Test CPU".into()],
                nvidia_uuid: Some("GPU-test".into()),
                nvidia_driver: Some("test".into()),
                nvidia_vbios: Some("test".into()),
            },
            settings: vec![Setting {
                feature: F::GpuDynamicBoost,
                value: Value::Boolean(true),
            }],
        };
        assert_eq!(
            display_value(&profile, F::GpuUnleashed),
            Some(&Value::Boolean(true))
        );
        profile.settings.push(Setting {
            feature: F::GpuUnleashed,
            value: Value::Boolean(false),
        });
        assert_eq!(
            display_value(&profile, F::GpuUnleashed),
            Some(&Value::Boolean(true))
        );
        let edited = omen_core::preset::merge(
            &profile,
            &profile,
            "Alias",
            &[(F::GpuUnleashed, Some(Value::Boolean(false)))],
        )
        .unwrap();
        assert_eq!(
            display_value(&edited, F::GpuUnleashed),
            Some(&Value::Boolean(false))
        );
        assert!(
            edited
                .effective()
                .settings
                .iter()
                .all(|s| s.value == Value::Boolean(false))
        );
    }
    #[test]
    fn dynamic_boost_group_holds_the_hub_style_watt_sliders() {
        let group = |feature| {
            fields()
                .iter()
                .find(|f| f.feature == feature)
                .map(|f| f.group)
        };
        for feature in [
            F::GpuUnleashed,
            F::GpuCustomTgp,
            F::HpSharedPowerMw,
            F::CpuSlowPptLimitMw,
        ] {
            assert_eq!(group(feature), Some(1), "{feature:?}");
        }
        assert_eq!(GROUPS[1], "group.dynamic_boost");
        // OMEN Hub 1101.2611.2.0 panel bounds for 8E35 (Khalilah_DRG_N22X4X6).
        assert_eq!(slider_watts(F::HpSharedPowerMw, true), Some((55, 72)));
        assert_eq!(slider_watts(F::CpuSlowPptLimitMw, true), Some((45, 80)));
        assert_eq!(slider_watts(F::CpuSlowPptLimitMw, false), Some((15, 160)));
        assert_eq!(slider_watts(F::CpuPptLimitMw, true), None);
        // The seeds start inside their sliders.
        for field in fields() {
            if let Some((lo, hi)) = slider_watts(field.feature, true) {
                assert!(
                    (lo * 1000..=hi * 1000).contains(&field.seed),
                    "{:?}",
                    field.feature
                );
            }
        }
    }
    #[test]
    fn current_fields_have_no_retired_controls_or_empty_groups() {
        let fields = fields();
        for field in &fields {
            assert!(!retired(field.feature));
            assert!(F::all().contains(&field.feature));
            assert!(field.group < GROUPS.len());
            assert!(!(13..=17).contains(&field.control_slot()));
        }
        for group in 0..GROUPS.len() {
            assert!(fields.iter().any(|field| field.group == group));
        }
        assert_eq!(
            SECTIONS,
            &[
                (0, "section.gpu"),
                (2, "section.cpu"),
                (6, "section.windows")
            ]
        );
        let old = serde_json::from_str::<F>("\"intel_pl1_mw\"").unwrap();
        assert!(retired(old));
        assert!(retired(F::MaxFan));
        assert!(
            !F::MaxFan.retired(),
            "The independent fan/recovery contract is still supported"
        );
    }
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
                .find(|f| f.feature == F::HpSharedPowerMw)
                .unwrap()
                .control_slot()
                >= 64
        );
        // Unsupported on this GPU, hidden from the editor.
        assert!(fields.iter().all(|f| f.feature != F::GpuPowerLimitMw));
        // On/off fields are plain ticks: cTGP defaults on, all others off.
        for field in fields
            .iter()
            .filter(|f| f.feature.descriptor().value_kind == ValueKind::Boolean)
        {
            assert_eq!(
                field.initial(),
                Value::Boolean(field.feature == F::GpuCustomTgp),
                "{:?}",
                field.feature
            );
        }
        assert!(!F::GpuCustomTgp.needs_nvidia() && F::GpuVfCurve.needs_nvidia());
    }
}
