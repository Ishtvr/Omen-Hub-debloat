//! Declarative presentation of typed preset features. No register/API knowledge.
use omen_core::{
    features::{Feature as F, Unit, ValueKind},
    profiles::{Choice as C, Value},
};

pub const GROUPS: [&str; 11] = [
    "NVIDIA GPU",
    "Temperatur",
    "Leistungslimits",
    "Stromlimits (VRM)",
    "Precision Boost Overdrive",
    "Curve Optimizer",
    "Curve Optimizer pro Kern",
    "Takt und Spannung",
    "Intel",
    "Windows-Prozessorleistung",
    "Windows und Anzeige",
];
pub struct Field {
    pub feature: F,
    pub group: usize,
    pub label: &'static str,
    pub seed: i64,
}
pub fn fields() -> Vec<Field> {
    let rows = &[
        (F::GpuCoreOffsetMhz, 0, "GPU-Takt-Offset", 0),
        (F::GpuMemoryOffsetMhz, 0, "VRAM-Takt-Offset", 0),
        (F::GpuVfCurve, 0, "GPU-Leistungskurve", 0),
        (F::GpuUnleashed, 0, "Dynamic Boost", 0),
        (F::GpuPowerLimitMw, 0, "GPU-Leistungslimit", 80000),
        (F::GpuTemperatureLimitC, 0, "GPU-Temperaturlimit", 87),
        (F::GpuVoltageOffsetMv, 0, "GPU-Spannungs-Offset", 0),
        (F::GpuMemoryVoltageOffsetMv, 0, "VRAM-Spannungs-Offset", 0),
        (F::GpuVoltageLimitMv, 0, "GPU-Spannungslimit", 900),
        (F::GpuCoreClockLimitMhz, 0, "GPU-Taktlimit", 2000),
        (F::GpuMemoryClockLimitMhz, 0, "VRAM-Taktlimit", 7000),
        (F::GpuFrameRateLimit, 0, "Bildratenlimit", 60),
        (F::GpuCustomTgp, 0, "Eigener GPU-Leistungsrahmen", 0),
        (F::OmenPerformanceMode, 0, "Leistungsmodus", 0),
        (F::MaxFan, 0, "Maximale Lüfterdrehzahl", 0),
        (F::CpuTemperatureLimitC, 1, "CPU-Temperaturlimit", 95),
        (
            F::CpuSkinTemperatureLimitC,
            1,
            "Oberflächentemperaturlimit",
            45,
        ),
        (F::CpuPptLimitMw, 2, "PPT", 45000),
        (F::CpuFastPptLimitMw, 2, "PPT Fast", 55000),
        (F::CpuSlowPptLimitMw, 2, "PPT Slow", 45000),
        (F::CpuStapmLimitMw, 2, "STAPM", 45000),
        (F::CpuStapmTimeSeconds, 2, "STAPM-Zeitfenster", 28),
        (F::CpuSlowPptTimeSeconds, 2, "PPT-Slow-Zeitfenster", 28),
        (F::CpuTdcMa, 3, "CPU TDC", 65000),
        (F::CpuEdcMa, 3, "CPU EDC", 90000),
        (F::SocTdcMa, 3, "SoC TDC", 15000),
        (F::SocEdcMa, 3, "SoC EDC", 25000),
        (F::GfxTdcMa, 3, "iGPU TDC", 15000),
        (F::GfxEdcMa, 3, "iGPU EDC", 25000),
        (F::CpuPboScalar, 4, "PBO Scalar", 1),
        (F::CpuBoostProfile, 4, "AMD Boost-Profil", 0),
        (F::CpuCurveOptimizerAllCore, 5, "CPU · alle Kerne", 0),
        (F::GfxCurveOptimizer, 5, "iGPU Curve Optimizer", 0),
        (
            F::CpuCurveOptimizerPerCore,
            6,
            "Curve Optimizer pro Kern",
            0,
        ),
        (
            F::CpuAllCoreVoltageOffsetMv,
            7,
            "CPU-Spannungs-Offset · alle Kerne",
            0,
        ),
        (F::AmdCpuVoltageMv, 7, "CPU-Spannung", 1000),
        (F::AmdCpuClockMhz, 7, "CPU-Takt", 3000),
        (F::AmdIgpuClockMhz, 7, "iGPU-Takt", 800),
        (F::IgpuSoftMinMhz, 7, "iGPU · minimaler Takt", 400),
        (F::IgpuSoftMaxMhz, 7, "iGPU · maximaler Takt", 1200),
        (F::CpuSoftMinMhz, 7, "CPU · minimaler Takt", 1000),
        (F::CpuSoftMaxMhz, 7, "CPU · maximaler Takt", 4000),
        (F::DataSoftMinMhz, 7, "Data Clock · Minimum", 800),
        (F::DataSoftMaxMhz, 7, "Data Clock · Maximum", 1600),
        (F::FabricSoftMinMhz, 7, "Fabric Clock · Minimum", 800),
        (F::FabricSoftMaxMhz, 7, "Fabric Clock · Maximum", 1600),
        (F::VcnSoftMinMhz, 7, "VCN · minimaler Takt", 400),
        (F::VcnSoftMaxMhz, 7, "VCN · maximaler Takt", 1000),
        (F::SocSoftMinMhz, 7, "SoC · minimaler Takt", 400),
        (F::SocSoftMaxMhz, 7, "SoC · maximaler Takt", 1000),
        (F::IntelPl1Mw, 8, "PL1", 45000),
        (F::IntelPl2Mw, 8, "PL2", 65000),
        (F::IntelPl4Mw, 8, "PL4", 90000),
        (F::IntelIccMaxMa, 8, "ICC Max", 90000),
        (F::IntelCpuPowerBalance, 8, "CPU Power Balance", 0),
        (F::IntelGpuPowerBalance, 8, "iGPU Power Balance", 0),
        (F::IntelGpuVoltageOffsetMv, 8, "iGPU-Spannungs-Offset", 0),
        (F::IntelCacheVoltageOffsetMv, 8, "Cache-Spannungs-Offset", 0),
        (
            F::IntelSystemAgentVoltageOffsetMv,
            8,
            "System-Agent-Spannungs-Offset",
            0,
        ),
        (
            F::IntelCoreRatios,
            8,
            "Core-Ratios · durch Komma getrennt",
            40,
        ),
        (F::WindowsBoostMode, 9, "Prozessor-Boost", 0),
        (
            F::WindowsMinimumProcessorPercent,
            9,
            "Minimaler Prozessorzustand",
            5,
        ),
        (
            F::WindowsMaximumProcessorPercent,
            9,
            "Maximaler Prozessorzustand",
            100,
        ),
        (
            F::WindowsProcessorEppPercent,
            9,
            "Energiepräferenz (EPP)",
            50,
        ),
        (
            F::WindowsCoreParkingMinPercent,
            9,
            "Core Parking · Minimum",
            0,
        ),
        (
            F::WindowsCoreParkingMaxPercent,
            9,
            "Core Parking · Maximum",
            100,
        ),
        (F::WindowsEnergyMode, 10, "Windows-Energiemodus", 0),
        (F::WindowsPowerPlan, 10, "Windows-Energiesparplan", 0),
        (F::DisplayRefreshRateHz, 10, "Bildwiederholrate", 60),
        (F::GraphicsMode, 10, "Grafikmodus", 0),
        (F::CpuCcdAffinity, 10, "CCD-Affinitätsmaske", 3),
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
