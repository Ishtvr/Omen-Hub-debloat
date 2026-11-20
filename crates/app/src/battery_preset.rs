//! Battery-optimised preset template ("race to idle"): short bursts stay
//! fast through a moderate fast limit and an efficient boost policy, while a
//! low sustained limit, the efficiency power mode and 60 Hz on the internal
//! panel cut continuous draw. Created only when the user picks it.
use omen_core::{
    features::Feature,
    profiles::{Choice, DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
};
use omen_platform::profile_store::ProfileStore;

/// Stable id, so the battery selection and Restore find the same preset.
pub const ID: &str = "preset-battery-optimized";

/// Template values within the ranges measured on 8E35 (Slow/STAPM through
/// HP 0x29 with the Balanced mode; PPT is writable outside Unleashed).
pub fn settings() -> Vec<Setting> {
    let choice = |feature, value| Setting {
        feature,
        value: Value::Choice(value),
    };
    let integer = |feature, value| Setting {
        feature,
        value: Value::Integer(value),
    };
    vec![
        Setting {
            feature: Feature::GpuDynamicBoost,
            value: Value::Boolean(false),
        },
        Setting {
            feature: Feature::GpuCustomTgp,
            value: Value::Boolean(false),
        },
        choice(Feature::WindowsPowerPlan, Choice::Balanced),
        choice(Feature::WindowsEnergyMode, Choice::BestEfficiency),
        choice(Feature::WindowsBoostMode, Choice::EfficientEnabled),
        integer(Feature::WindowsProcessorEppPercent, 70),
        integer(Feature::DisplayRefreshRateHz, 60),
        integer(Feature::CpuSlowPptLimitMw, 18_000),
        integer(Feature::CpuPptLimitMw, 30_000),
    ]
}

pub fn template(name: &str, binding: &DeviceBinding) -> PerformanceProfile {
    let settings = settings();
    PerformanceProfile {
        schema: PROFILE_SCHEMA,
        id: ID.into(),
        name: name.into(),
        binding: binding.for_settings(&settings),
        settings,
    }
}

/// Saves the template unless a preset with its id exists (user edits stay).
pub fn ensure(store: &ProfileStore) -> Result<(), String> {
    if store.load(ID).is_ok() {
        return Ok(());
    }
    let inventory = omen_platform::inventory();
    let binding = DeviceBinding {
        manufacturer: inventory.manufacturer,
        board: inventory.board,
        bios: inventory.bios,
        cpu_names: inventory.cpus,
        nvidia_uuid: None,
        nvidia_driver: None,
        nvidia_vbios: None,
    };
    store
        .save(&template(
            crate::i18n::t("battery.optimized_name"),
            &binding,
        ))
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_a_valid_battery_preset_without_gpu_binding() {
        let binding = DeviceBinding {
            manufacturer: "HP".into(),
            board: "8E35".into(),
            bios: "F.13".into(),
            cpu_names: vec!["AMD Ryzen 9 8940HX".into()],
            nvidia_uuid: Some("GPU-x".into()),
            nvidia_driver: Some("1".into()),
            nvidia_vbios: Some("v".into()),
        };
        let profile = template("Akku", &binding);
        profile.validate().unwrap();
        profile.validate_intent().unwrap();
        // Waking the dGPU just to check a binding would cost battery.
        assert_eq!(profile.binding.nvidia_uuid, None);
        // Balanced mode (Dynamic Boost off): the fast limit is not
        // firmware-owned, and the efficiency mode keeps its Balanced plan.
        assert_eq!(
            omen_core::hp_mode::HpMode::of_preset(&profile, false),
            Some(omen_core::hp_mode::HpMode::Balanced)
        );
        let effective = profile.effective().settings;
        assert!(
            effective
                .iter()
                .any(|s| s.feature == Feature::WindowsEnergyMode)
        );
        assert_eq!(
            omen_core::hp_mode::HpMode::cpu_pl1_of_profile(&profile),
            Some(18)
        );
    }
}
