//! Declarative presentation of typed preset features. No register/API knowledge.
use omen_core::{
    features::{Feature as F, Unit, ValueKind},
    profiles::{Choice as C, Value},
};

pub const GROUPS: [&str; 9] = [
    "NVIDIA GPU",
    "Limits",
    "Precision Boost Overdrive",
    "Curve Optimizer",
    "Curve Optimizer pro Kern",
    "Takt und Spannung",
    "Intel",
    "Prozessorleistung",
    "Energie und Anzeige",
];
pub const SECTIONS: &[(usize, &str)] = &[(0, "GPU"), (1, "CPU"), (7, "Windows")];
pub struct Field {
    pub feature: F,
    pub group: usize,
    pub label: &'static str,
    pub seed: i64,
}
/// Legacy keys remain readable. Explicit Save retires only the removed options.
/// Dynamic Boost stays available as an explicit planned
/// profile instruction; its hardware adapter still requires qualification.
pub const REMOVED_GPU_OPTIONS: &[F] = &[
    F::GpuVoltageOffsetMv,
    F::GpuMemoryVoltageOffsetMv,
    F::GpuVoltageLimitMv,
    F::GpuPowerLimitMw,
    F::GpuTemperatureLimitC,
    F::GpuCoreClockLimitMhz,
    F::GpuMemoryClockLimitMhz,
    F::GpuFrameRateLimit,
    F::GpuCustomTgp,
    F::OmenPerformanceMode,
];
/// APU power/VRM/soft-clock and integrated-graphics fields do not belong in the
/// requested CPU-only tuning UI. Retain decoding; retire intent on explicit Save.
pub const REMOVED_APU_OPTIONS: &[F] = &[
    F::CpuSkinTemperatureLimitC,
    F::CpuFastPptLimitMw,
    F::CpuSlowPptLimitMw,
    F::CpuStapmLimitMw,
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
pub fn retired(feature: F) -> bool {
    REMOVED_GPU_OPTIONS.contains(&feature) || REMOVED_APU_OPTIONS.contains(&feature)
}
pub fn fields() -> Vec<Field> {
    let rows = &[
        (F::GpuCoreOffsetMhz, 0, "GPU-Takt-Offset", 0),
        (F::GpuMemoryOffsetMhz, 0, "VRAM-Takt-Offset", 0),
        (F::GpuVfCurve, 0, "GPU-Leistungskurve", 0),
        (F::MaxFan, 0, "Maximale Lüfterdrehzahl", 0),
        (F::GpuUnleashed, 0, "Dynamic Boost", 0),
        (F::CpuTemperatureLimitC, 1, "CPU-Temperaturlimit", 95),
        (F::CpuPptLimitMw, 1, "PPT", 45000),
        (F::CpuTdcMa, 1, "CPU TDC", 65000),
        (F::CpuEdcMa, 1, "CPU EDC", 90000),
        (F::CpuPboScalar, 2, "PBO Scalar", 1),
        (F::CpuBoostProfile, 2, "AMD Boost-Profil", 0),
        (F::CpuCurveOptimizerAllCore, 3, "CPU · alle Kerne", 0),
        (
            F::CpuCurveOptimizerPerCore,
            4,
            "Curve Optimizer pro Kern",
            0,
        ),
        (
            F::CpuAllCoreVoltageOffsetMv,
            5,
            "CPU-Spannungs-Offset · alle Kerne",
            0,
        ),
        (F::AmdCpuVoltageMv, 5, "CPU-Spannung", 1000),
        (F::AmdCpuClockMhz, 5, "CPU-Takt", 3000),
        (F::IntelPl1Mw, 6, "PL1", 45000),
        (F::IntelPl2Mw, 6, "PL2", 65000),
        (F::IntelPl4Mw, 6, "PL4", 90000),
        (F::IntelIccMaxMa, 6, "ICC Max", 90000),
        (F::IntelCpuPowerBalance, 6, "CPU Power Balance", 0),
        (F::IntelCacheVoltageOffsetMv, 6, "Cache-Spannungs-Offset", 0),
        (
            F::IntelSystemAgentVoltageOffsetMv,
            6,
            "System-Agent-Spannungs-Offset",
            0,
        ),
        (
            F::IntelCoreRatios,
            6,
            "Core-Ratios · durch Komma getrennt",
            40,
        ),
        (F::WindowsBoostMode, 7, "Prozessor-Boost", 0),
        (
            F::WindowsMinimumProcessorPercent,
            7,
            "Minimaler Prozessorzustand",
            5,
        ),
        (
            F::WindowsMaximumProcessorPercent,
            7,
            "Maximaler Prozessorzustand",
            100,
        ),
        (
            F::WindowsProcessorEppPercent,
            7,
            "Energiepräferenz (EPP)",
            50,
        ),
        (
            F::WindowsCoreParkingMinPercent,
            7,
            "Core Parking · Minimum",
            0,
        ),
        (
            F::WindowsCoreParkingMaxPercent,
            7,
            "Core Parking · Maximum",
            100,
        ),
        (F::WindowsEnergyMode, 8, "Windows-Energiemodus", 0),
        (F::WindowsPowerPlan, 8, "Windows-Energiesparplan", 0),
        (F::DisplayRefreshRateHz, 8, "Bildwiederholrate", 60),
        (F::GraphicsMode, 8, "Grafikmodus", 0),
        (F::CpuCcdAffinity, 7, "CCD-Affinitätsmaske", 3),
    ];
    rows.iter()
        .map(|(feature, group, label, seed)| Field {
            feature: *feature,
            group: *group,
            label,
            seed: *seed,
        })
        .collect()
}
impl Field {
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
            Unit::CurveStep => "Schritte",
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
    match choice {
        C::Disabled => "Deaktiviert",
        C::Enabled => "Aktiviert",
        C::Aggressive => "Aggressiv",
        C::EfficientEnabled => "Effizient",
        C::EfficientAggressive => "Effizient aggressiv",
        C::AggressiveAtGuaranteed => "Aggressiv ab Basistakt",
        C::EfficientAggressiveAtGuaranteed => "Effizient aggressiv ab Basistakt",
        C::BestEfficiency => "Energiesparen",
        C::BestPerformance => "Beste Leistung",
        C::PowerSaver => "Energiesparen",
        C::HighPerformance => "Höchstleistung",
        C::IntegratedOnly => "Nur integrierte GPU",
        C::Hybrid => "Hybrid",
        C::DiscreteOnly => "Nur NVIDIA GPU",
        C::Quiet => "Quiet",
        C::Balanced => "Balanced",
        C::Performance => "Performance",
        C::Unleashed => "Dynamic Boost",
        C::Auto => "Automatisch",
        _ => "Nicht verfügbar",
    }
}
