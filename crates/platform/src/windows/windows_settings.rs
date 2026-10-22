//! Windows power-scheme and display settings, plus the CCD selection the
//! background service enforces. Every control has an exact GET, so capture,
//! readback and restore are complete.
use omen_core::{
    ccd,
    features::Feature,
    message::msg,
    profiles::{Choice, Value},
};
use serde::{Deserialize, Serialize};
use std::ptr;
use windows_sys::{
    Win32::{
        Foundation::{ERROR_SUCCESS, LocalFree},
        Graphics::Gdi::{
            CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_SUCCESSFUL,
            DM_DISPLAYFREQUENCY, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW,
        },
        System::Power::{
            PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex,
            PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex,
        },
    },
    core::GUID,
};

const SUB_PROCESSOR: u128 = 0x54533251_82be_4824_96c1_47b60b740d00;
const BOOST_MODE: u128 = 0xbe337238_0d82_4146_a960_4f3749d470c7;
const MIN_STATE: u128 = 0x893dee8e_2bef_41e0_89c6_b55d0929964c;
const MAX_STATE: u128 = 0xbc5038f7_23e0_4960_96da_33abaf5935ec;
const EPP: u128 = 0x36687f9e_e3a5_4dbf_b1dc_15eb381c6863;
const PARKING_MIN: u128 = 0x0cc5b647_c1df_4637_891a_dec35c318583;
const PARKING_MAX: u128 = 0xea062031_0e34_4ff1_9b6d_eb1059334028;
const PLAN_POWER_SAVER: u128 = 0xa1841308_3541_4fab_bc81_f71556f20b4a;
const PLAN_BALANCED: u128 = 0x381b4222_f694_41f0_9685_ff5bb260df2e;
const PLAN_HIGH: u128 = 0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c;
/// Windows 11 power-mode overlays (Settings > Power > Power mode).
const OVERLAY_EFFICIENCY: u128 = 0x961cc777_2547_4f9d_8174_7d86181b8a7a;
const OVERLAY_BALANCED: u128 = 0;
const OVERLAY_PERFORMANCE: u128 = 0xded574b5_45a0_4f42_8737_46345c09c238;

const BOOST_CHOICES: [Choice; 7] = [
    Choice::Disabled,
    Choice::Enabled,
    Choice::Aggressive,
    Choice::EfficientEnabled,
    Choice::EfficientAggressive,
    Choice::AggressiveAtGuaranteed,
    Choice::EfficientAggressiveAtGuaranteed,
];

pub fn handles(feature: Feature) -> bool {
    matches!(
        feature,
        Feature::WindowsBoostMode
            | Feature::WindowsEnergyMode
            | Feature::WindowsPowerPlan
            | Feature::WindowsMinimumProcessorPercent
            | Feature::WindowsMaximumProcessorPercent
            | Feature::WindowsProcessorEppPercent
            | Feature::WindowsCoreParkingMinPercent
            | Feature::WindowsCoreParkingMaxPercent
            | Feature::DisplayRefreshRateHz
            | Feature::CpuCcdAffinity
    )
}

mod guid_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{value:032x}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let clean = s.replace('-', "");
        u128::from_str_radix(&clean, 16)
            .or_else(|_| clean.parse::<u128>())
            .map_err(serde::de::Error::custom)
    }
}

/// Complete original of one control. Power indices keep AC and DC separately;
/// a profile writes both, so the captured pair restores either power source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindowsImage {
    PowerIndex {
        #[serde(with = "guid_hex")]
        scheme: u128,
        #[serde(with = "guid_hex")]
        setting: u128,
        ac: u32,
        dc: u32,
    },
    ActiveScheme {
        #[serde(with = "guid_hex")]
        scheme: u128,
    },
    Overlay {
        #[serde(with = "guid_hex")]
        overlay: u128,
    },
    /// Configured overlay, distinct from the effective policy (which may be
    /// overridden by the plan or Energy Saver).
    ConfiguredOverlay {
        #[serde(with = "guid_hex")]
        overlay: u128,
    },
    Refresh {
        width: u32,
        height: u32,
        hz: u32,
    },
    /// Saved CCD selection (bit 0 = first CCD); None = every CCD.
    CcdSelection {
        selection: Option<u8>,
    },
}

/// The per-user file the background service reads every cycle.
fn ccd_store() -> Result<crate::profile_store::ProfileStore, String> {
    crate::profile_store::ProfileStore::user_data()
}

fn guid(value: u128) -> GUID {
    GUID::from_u128(value)
}
fn to_u128(value: &GUID) -> u128 {
    (u128::from(value.data1) << 96)
        | (u128::from(value.data2) << 80)
        | (u128::from(value.data3) << 64)
        | u128::from(u64::from_be_bytes(value.data4))
}
fn check(code: u32, what: &str) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!(
            "{what}: {}",
            std::io::Error::from_raw_os_error(code as i32)
        ))
    }
}
fn active_scheme() -> Result<u128, String> {
    let mut scheme: *mut GUID = ptr::null_mut();
    // SAFETY: Windows allocates the GUID; it is copied and freed with LocalFree.
    unsafe {
        check(
            PowerGetActiveScheme(ptr::null_mut(), &mut scheme),
            "Active power scheme",
        )?;
        let value = to_u128(&*scheme);
        LocalFree(scheme.cast());
        Ok(value)
    }
}
fn power_setting(feature: Feature) -> Option<u128> {
    Some(match feature {
        Feature::WindowsBoostMode => BOOST_MODE,
        Feature::WindowsMinimumProcessorPercent => MIN_STATE,
        Feature::WindowsMaximumProcessorPercent => MAX_STATE,
        Feature::WindowsProcessorEppPercent => EPP,
        Feature::WindowsCoreParkingMinPercent => PARKING_MIN,
        Feature::WindowsCoreParkingMaxPercent => PARKING_MAX,
        _ => return None,
    })
}
fn read_index(scheme: u128, setting: u128) -> Result<(u32, u32), String> {
    let (s, sub, set) = (guid(scheme), guid(SUB_PROCESSOR), guid(setting));
    let (mut ac, mut dc) = (0u32, 0u32);
    // SAFETY: Valid GUID pointers and writable outputs for the call duration.
    unsafe {
        check(
            PowerReadACValueIndex(ptr::null_mut(), &s, &sub, &set, &mut ac),
            "Power AC value",
        )?;
        check(
            PowerReadDCValueIndex(ptr::null_mut(), &s, &sub, &set, &mut dc),
            "Power DC value",
        )?;
    }
    Ok((ac, dc))
}
fn write_index(scheme: u128, setting: u128, ac: u32, dc: u32) -> Result<(), String> {
    let (s, sub, set) = (guid(scheme), guid(SUB_PROCESSOR), guid(setting));
    // SAFETY: Valid GUID pointers; Windows copies the values.
    unsafe {
        check(
            PowerWriteACValueIndex(ptr::null_mut(), &s, &sub, &set, ac),
            "Power AC write",
        )?;
        check(
            PowerWriteDCValueIndex(ptr::null_mut(), &s, &sub, &set, dc),
            "Power DC write",
        )?;
        // Re-activating the scheme makes the new indices effective immediately.
        // Restoring a policy of an inactive destination plan must not select it.
        // This also avoids undoing a concurrent external plan switch.
        if active_scheme()? == scheme {
            check(PowerSetActiveScheme(ptr::null_mut(), &s), "Activate scheme")?;
        }
        Ok(())
    }
}
fn plan_scheme(choice: Choice) -> Option<u128> {
    Some(match choice {
        Choice::PowerSaver => PLAN_POWER_SAVER,
        Choice::Balanced => PLAN_BALANCED,
        Choice::HighPerformance => PLAN_HIGH,
        _ => return None,
    })
}
fn overlay_guid(choice: Choice) -> Option<u128> {
    Some(match choice {
        Choice::BestEfficiency => OVERLAY_EFFICIENCY,
        Choice::Balanced => OVERLAY_BALANCED,
        Choice::BestPerformance => OVERLAY_PERFORMANCE,
        _ => return None,
    })
}

/// Undocumented but stable powrprof exports behind Settings > Power mode.
type GetOverlay = unsafe extern "system" fn(*mut GUID) -> u32;
type SetOverlay = unsafe extern "system" fn(GUID) -> u32;
fn overlay_api() -> Result<(libloading::Library, GetOverlay, GetOverlay, SetOverlay), String> {
    // SAFETY: System DLL; symbol signatures match the shipped powrprof exports.
    unsafe {
        let library = libloading::Library::new("powrprof.dll").map_err(|e| e.to_string())?;
        let actual = *library
            .get::<GetOverlay>(b"PowerGetActualOverlayScheme\0")
            .map_err(|_| "Configured Windows power mode is not available on this system")?;
        let effective = *library
            .get::<GetOverlay>(b"PowerGetEffectiveOverlayScheme\0")
            .map_err(|_| "Windows power mode is not available on this system")?;
        let set = *library
            .get::<SetOverlay>(b"PowerSetActiveOverlayScheme\0")
            .map_err(|_| "Windows power mode is not available on this system")?;
        Ok((library, actual, effective, set))
    }
}
fn read_overlay() -> Result<u128, String> {
    let (_library, get, _, _) = overlay_api()?;
    let mut value = guid(0);
    // SAFETY: Writable GUID output; library stays loaded through the call.
    check(unsafe { get(&mut value) }, "Power mode")?;
    Ok(to_u128(&value))
}
fn read_effective_overlay() -> Result<u128, String> {
    let (_library, _, get, _) = overlay_api()?;
    let mut value = guid(0);
    // SAFETY: Exact writable GUID output, library outlives the call.
    check(unsafe { get(&mut value) }, "Effective power mode")?;
    Ok(to_u128(&value))
}
fn write_overlay(overlay: u128) -> Result<(), String> {
    let (_library, _, _, set) = overlay_api()?;
    let value = guid(overlay);
    // SAFETY: Valid GUID; library stays loaded through the call.
    check(unsafe { set(value) }, "Power mode write")
}

fn current_mode() -> Result<DEVMODEW, String> {
    // SAFETY: Zeroed DEVMODEW with dmSize set is the documented input.
    unsafe {
        let mut mode: DEVMODEW = std::mem::zeroed();
        mode.dmSize = size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) == 0 {
            return Err("Current display mode is unavailable".into());
        }
        Ok(mode)
    }
}
/// Refresh rates the primary display offers at its current resolution.
pub fn refresh_rates() -> Result<Vec<u32>, String> {
    let current = current_mode()?;
    let mut rates = vec![];
    // SAFETY: Same documented enumeration with an initialized structure.
    unsafe {
        let mut index = 0;
        loop {
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(ptr::null(), index, &mut mode) == 0 {
                break;
            }
            if mode.dmPelsWidth == current.dmPelsWidth
                && mode.dmPelsHeight == current.dmPelsHeight
                && mode.dmBitsPerPel == current.dmBitsPerPel
                && !rates.contains(&mode.dmDisplayFrequency)
            {
                rates.push(mode.dmDisplayFrequency);
            }
            index += 1;
        }
    }
    rates.sort_unstable();
    Ok(rates)
}
fn write_refresh(width: u32, height: u32, hz: u32) -> Result<(), String> {
    let mut mode = current_mode()?;
    if mode.dmPelsWidth != width || mode.dmPelsHeight != height {
        return Err("Display resolution changed; refresh rate is not written".into());
    }
    mode.dmDisplayFrequency = hz;
    mode.dmFields = DM_DISPLAYFREQUENCY;
    // SAFETY: Initialized current mode with only the frequency field selected.
    let result = unsafe {
        ChangeDisplaySettingsExW(
            ptr::null(),
            &mode,
            ptr::null_mut(),
            CDS_UPDATEREGISTRY,
            ptr::null(),
        )
    };
    if result != DISP_CHANGE_SUCCESSFUL {
        return Err(format!("Display refresh change failed: {result}"));
    }
    Ok(())
}

pub fn capture(feature: Feature) -> Result<WindowsImage, String> {
    capture_for_scheme(feature, None)
}

pub fn requested_scheme(settings: &[omen_core::profiles::Setting]) -> Result<Option<u128>, String> {
    let target = settings
        .iter()
        .find(|setting| setting.feature == Feature::WindowsPowerPlan)
        .map(|setting| {
            validate(setting.feature, &setting.value)?;
            let Value::Choice(choice) = setting.value else {
                return Err("Power plan is not a choice".into());
            };
            plan_scheme(choice).ok_or_else(|| "Unknown target power plan".to_owned())
        })
        .transpose()?;
    if settings
        .iter()
        .any(|setting| setting.feature == Feature::WindowsEnergyMode)
    {
        let effective = target.map_or_else(active_scheme, Ok)?;
        if effective != PLAN_BALANCED {
            return Err(msg("windows.err.overlay_plan", &[]));
        }
    }
    Ok(target)
}

pub fn capture_for_scheme(
    feature: Feature,
    target_scheme: Option<u128>,
) -> Result<WindowsImage, String> {
    if let Some(setting) = power_setting(feature) {
        let scheme = target_scheme.map_or_else(active_scheme, Ok)?;
        let (ac, dc) = read_index(scheme, setting)?;
        return Ok(WindowsImage::PowerIndex {
            scheme,
            setting,
            ac,
            dc,
        });
    }
    match feature {
        Feature::WindowsPowerPlan => Ok(WindowsImage::ActiveScheme {
            scheme: active_scheme()?,
        }),
        Feature::WindowsEnergyMode => Ok(WindowsImage::ConfiguredOverlay {
            overlay: read_overlay()?,
        }),
        Feature::DisplayRefreshRateHz => {
            let mode = current_mode()?;
            Ok(WindowsImage::Refresh {
                width: mode.dmPelsWidth,
                height: mode.dmPelsHeight,
                hz: mode.dmDisplayFrequency,
            })
        }
        Feature::CpuCcdAffinity => Ok(WindowsImage::CcdSelection {
            selection: ccd_store()?.ccd_selection()?,
        }),
        _ => Err("Not a Windows setting".into()),
    }
}

/// Read the exact policy represented by an original, including inactive plans.
pub fn capture_like(feature: Feature, image: &WindowsImage) -> Result<WindowsImage, String> {
    if matches!(image, WindowsImage::Overlay { .. }) {
        return Err(
            "Legacy effective-overlay image cannot restore the configured Windows power mode"
                .into(),
        );
    }
    if let WindowsImage::PowerIndex {
        scheme, setting, ..
    } = image
    {
        if power_setting(feature) != Some(*setting) {
            return Err("Power-index restore image belongs to another setting".into());
        }
        return capture_for_scheme(feature, Some(*scheme));
    }
    capture(feature)
}

pub fn verify_target(
    feature: Feature,
    original: &WindowsImage,
    wanted: &Value,
) -> Result<bool, String> {
    if let WindowsImage::PowerIndex { scheme, .. } = original {
        if active_scheme()? != *scheme {
            return Err("Active power plan changed during policy application".into());
        }
    }
    if feature == Feature::WindowsEnergyMode {
        let Value::Choice(choice) = wanted else {
            return Err("Power mode target is not a choice".into());
        };
        let expected = overlay_guid(*choice).ok_or("Unknown power mode")?;
        if active_scheme()? != PLAN_BALANCED {
            return Err(msg("windows.err.overlay_plan", &[]));
        }
        for attempt in 0..5 {
            if read_overlay()? == expected && read_effective_overlay()? == expected {
                return Ok(true);
            }
            if attempt < 4 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        return Ok(false);
    }
    image_matches(feature, &capture_like(feature, original)?, wanted)
}

/// The profile value of a captured image. Power indices report the AC value;
/// a differing DC value still restores exactly from the image.
pub fn value(feature: Feature, image: &WindowsImage) -> Result<Value, String> {
    Ok(match (feature, image) {
        (Feature::WindowsBoostMode, WindowsImage::PowerIndex { ac, .. }) => Value::Choice(
            *BOOST_CHOICES
                .get(*ac as usize)
                .ok_or("Unknown processor boost mode")?,
        ),
        (_, WindowsImage::PowerIndex { ac, .. }) => Value::Integer(i64::from(*ac)),
        (Feature::WindowsPowerPlan, WindowsImage::ActiveScheme { scheme }) => {
            Value::Choice(match *scheme {
                PLAN_POWER_SAVER => Choice::PowerSaver,
                PLAN_BALANCED => Choice::Balanced,
                PLAN_HIGH => Choice::HighPerformance,
                // A custom plan is restorable by GUID but has no profile choice.
                _ => Choice::Manual,
            })
        }
        (
            Feature::WindowsEnergyMode,
            WindowsImage::Overlay { overlay } | WindowsImage::ConfiguredOverlay { overlay },
        ) => Value::Choice(match *overlay {
            OVERLAY_EFFICIENCY => Choice::BestEfficiency,
            OVERLAY_BALANCED => Choice::Balanced,
            OVERLAY_PERFORMANCE => Choice::BestPerformance,
            _ => Choice::Manual,
        }),
        (Feature::DisplayRefreshRateHz, WindowsImage::Refresh { hz, .. }) => {
            Value::Integer(i64::from(*hz))
        }
        (Feature::CpuCcdAffinity, WindowsImage::CcdSelection { selection }) => {
            let all = ccd::all(super::ccd::ccd_masks()?.len());
            Value::Integer(i64::from(selection.unwrap_or(all)))
        }
        _ => return Err("Windows image does not belong to this feature".into()),
    })
}

pub fn read(feature: Feature) -> Result<Value, String> {
    value(feature, &capture(feature)?)
}

/// A profile writes both power sources, even though the editor displays AC.
pub fn image_matches(
    feature: Feature,
    image: &WindowsImage,
    wanted: &Value,
) -> Result<bool, String> {
    if let WindowsImage::PowerIndex {
        setting, ac, dc, ..
    } = image
    {
        if power_setting(feature) != Some(*setting) {
            return Err("Power-index image belongs to another setting".into());
        }
        let index = match wanted {
            Value::Choice(choice) if feature == Feature::WindowsBoostMode => BOOST_CHOICES
                .iter()
                .position(|value| value == choice)
                .ok_or("Unknown boost mode")?
                as u32,
            Value::Integer(value) => u32::try_from(*value).map_err(|e| e.to_string())?,
            _ => return Err("Wrong power-index target type".into()),
        };
        return Ok(*ac == index && *dc == index);
    }
    Ok(value(feature, image)? == *wanted)
}

/// A failed AC/DC write can leave a mixture of original and requested values.
/// Unrelated values are never treated as owned during recovery.
pub fn image_owned(
    feature: Feature,
    original: &WindowsImage,
    live: &WindowsImage,
    target: &Value,
) -> Result<bool, String> {
    if let (
        WindowsImage::PowerIndex {
            scheme,
            setting,
            ac,
            dc,
        },
        WindowsImage::PowerIndex {
            scheme: live_scheme,
            setting: live_setting,
            ac: live_ac,
            dc: live_dc,
        },
    ) = (original, live)
    {
        if scheme != live_scheme
            || setting != live_setting
            || power_setting(feature) != Some(*setting)
        {
            return Ok(false);
        }
        let wanted = match target {
            Value::Choice(choice) if feature == Feature::WindowsBoostMode => BOOST_CHOICES
                .iter()
                .position(|v| v == choice)
                .ok_or("Unknown boost target")?
                as u32,
            Value::Integer(value) => u32::try_from(*value).map_err(|e| e.to_string())?,
            _ => return Err("Wrong power target".into()),
        };
        return Ok(
            (*live_ac == *ac || *live_ac == wanted) && (*live_dc == *dc || *live_dc == wanted)
        );
    }
    Ok(original == live || image_matches(feature, live, target)?)
}

/// Integer limits for the capability check; None for choices.
pub fn range(feature: Feature) -> Result<Option<(i64, i64)>, String> {
    Ok(match feature {
        Feature::DisplayRefreshRateHz => {
            let rates = refresh_rates()?;
            Some((
                i64::from(*rates.first().ok_or("No display modes")?),
                i64::from(*rates.last().ok_or("No display modes")?),
            ))
        }
        f if power_setting(f).is_some() && f != Feature::WindowsBoostMode => Some((0, 100)),
        Feature::CpuCcdAffinity => {
            let ccds = super::ccd::ccd_masks()?.len();
            ccd::validate(1, ccds)?;
            Some((1, i64::from(ccd::all(ccds))))
        }
        _ => None,
    })
}

/// Rejects values Windows cannot take before any setting of the profile is written.
pub fn validate(feature: Feature, value: &Value) -> Result<(), String> {
    match (feature, value) {
        (Feature::WindowsBoostMode, Value::Choice(c)) if BOOST_CHOICES.contains(c) => Ok(()),
        (Feature::WindowsPowerPlan, Value::Choice(c)) => {
            let scheme = plan_scheme(*c).ok_or("Unknown power plan")?;
            // Reading any index proves the plan exists on this system.
            read_index(scheme, MAX_STATE)
                .map(|_| ())
                .map_err(|_| msg("windows.err.plan_missing", &[]))
        }
        (Feature::WindowsEnergyMode, Value::Choice(c)) => overlay_guid(*c)
            .map(|_| ())
            .ok_or_else(|| "Unknown power mode".into()),
        (Feature::DisplayRefreshRateHz, Value::Integer(hz)) => {
            if refresh_rates()?.iter().any(|r| i64::from(*r) == *hz) {
                Ok(())
            } else {
                Err(msg("windows.err.refresh_rate", &[hz]))
            }
        }
        (Feature::CpuCcdAffinity, Value::Integer(v)) => {
            ccd::validate(*v, super::ccd::ccd_masks()?.len()).map(|_| ())
        }
        (f, Value::Integer(v)) if power_setting(f).is_some() && f != Feature::WindowsBoostMode => {
            if (0..=100).contains(v) {
                Ok(())
            } else {
                Err(msg("windows.err.percent", &[]))
            }
        }
        _ => Err("Wrong value type for this Windows setting".into()),
    }
}

/// Writes a profile value on top of a captured original (same scheme/mode).
pub fn write(feature: Feature, original: &WindowsImage, value: &Value) -> Result<(), String> {
    validate(feature, value)?;
    match (original, value) {
        (
            WindowsImage::PowerIndex {
                scheme, setting, ..
            },
            value,
        ) => {
            if active_scheme()? != *scheme {
                return Err("Active power plan changed; setting is not written".into());
            }
            let index = match value {
                Value::Choice(c) => BOOST_CHOICES
                    .iter()
                    .position(|b| b == c)
                    .ok_or("Unknown boost mode")? as u32,
                Value::Integer(v) => u32::try_from(*v).map_err(|e| e.to_string())?,
                _ => return Err("Wrong power value".into()),
            };
            write_index(*scheme, *setting, index, index)
        }
        (WindowsImage::ActiveScheme { .. }, Value::Choice(c)) => {
            let scheme = guid(plan_scheme(*c).ok_or("Unknown power plan")?);
            // SAFETY: Valid GUID pointer.
            check(
                unsafe { PowerSetActiveScheme(ptr::null_mut(), &scheme) },
                "Activate power plan",
            )
        }
        (WindowsImage::ConfiguredOverlay { .. }, Value::Choice(c)) => {
            if active_scheme()? != PLAN_BALANCED {
                return Err(msg("windows.err.overlay_plan", &[]));
            }
            write_overlay(overlay_guid(*c).ok_or("Unknown power mode")?)
        }
        (WindowsImage::Refresh { width, height, .. }, Value::Integer(hz)) => write_refresh(
            *width,
            *height,
            u32::try_from(*hz).map_err(|e| e.to_string())?,
        ),
        (WindowsImage::CcdSelection { .. }, Value::Integer(v)) => {
            let ccds = super::ccd::ccd_masks()?.len();
            let selection = ccd::validate(*v, ccds)?;
            // "Every CCD" is stored as no file, the state before any selection.
            ccd_store()?.save_ccd_selection((selection != ccd::all(ccds)).then_some(selection))
        }
        _ => Err("Windows image and value do not match".into()),
    }
}

/// Restores the complete captured original and verifies it.
pub fn restore(feature: Feature, original: &WindowsImage) -> Result<(), String> {
    match original {
        WindowsImage::PowerIndex {
            scheme,
            setting,
            ac,
            dc,
        } => write_index(*scheme, *setting, *ac, *dc)?,
        WindowsImage::ActiveScheme { scheme } => {
            let scheme = guid(*scheme);
            // SAFETY: Valid GUID pointer.
            check(
                unsafe { PowerSetActiveScheme(ptr::null_mut(), &scheme) },
                "Restore power plan",
            )?
        }
        WindowsImage::ConfiguredOverlay { overlay } => write_overlay(*overlay)?,
        WindowsImage::Overlay { .. } => {
            return Err("Legacy effective-overlay image is not a configured-mode original".into());
        }
        WindowsImage::Refresh { width, height, hz } => write_refresh(*width, *height, *hz)?,
        WindowsImage::CcdSelection { selection } => ccd_store()?.save_ccd_selection(*selection)?,
    }
    if capture_like(feature, original)? != *original {
        return Err("Windows setting did not return to its captured original".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_effective_overlay_from_an_old_journal_cannot_restore_a_user_choice() {
        let old = WindowsImage::Overlay {
            overlay: OVERLAY_BALANCED,
        };
        assert!(capture_like(Feature::WindowsEnergyMode, &old).is_err());
        assert!(restore(Feature::WindowsEnergyMode, &old).is_err());
        let configured = WindowsImage::ConfiguredOverlay {
            overlay: OVERLAY_EFFICIENCY,
        };
        assert_eq!(
            value(Feature::WindowsEnergyMode, &configured).unwrap(),
            Value::Choice(Choice::BestEfficiency)
        );
    }
    #[test]
    fn ac_equality_does_not_skip_a_different_dc_policy() {
        let original = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: EPP,
            ac: 80,
            dc: 40,
        };
        assert!(
            !image_matches(
                Feature::WindowsProcessorEppPercent,
                &original,
                &Value::Integer(80)
            )
            .unwrap()
        );
        let complete = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: EPP,
            ac: 80,
            dc: 80,
        };
        assert!(
            image_matches(
                Feature::WindowsProcessorEppPercent,
                &complete,
                &Value::Integer(80)
            )
            .unwrap()
        );
    }

    #[test]
    fn partial_ac_dc_restore_does_not_accept_an_unrelated_policy_or_scheme() {
        let original = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: EPP,
            ac: 80,
            dc: 40,
        };
        let partial = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: EPP,
            ac: 70,
            dc: 40,
        };
        assert!(
            image_owned(
                Feature::WindowsProcessorEppPercent,
                &original,
                &partial,
                &Value::Integer(70)
            )
            .unwrap()
        );
        let foreign = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: EPP,
            ac: 70,
            dc: 41,
        };
        assert!(
            !image_owned(
                Feature::WindowsProcessorEppPercent,
                &original,
                &foreign,
                &Value::Integer(70)
            )
            .unwrap()
        );
        let foreign = WindowsImage::PowerIndex {
            scheme: PLAN_POWER_SAVER,
            setting: EPP,
            ac: 70,
            dc: 40,
        };
        assert!(
            !image_owned(
                Feature::WindowsProcessorEppPercent,
                &original,
                &foreign,
                &Value::Integer(70)
            )
            .unwrap()
        );
    }

    #[test]
    fn guid_round_trip_keeps_byte_order() {
        for value in [PLAN_BALANCED, BOOST_MODE, OVERLAY_PERFORMANCE, 0] {
            assert_eq!(to_u128(&guid(value)), value);
        }
    }

    #[test]
    fn ccd_selection_image_round_trips_through_the_journal() {
        for selection in [None, Some(1), Some(2)] {
            let image = WindowsImage::CcdSelection { selection };
            let text = serde_json::to_string(&image).unwrap();
            assert_eq!(serde_json::from_str::<WindowsImage>(&text).unwrap(), image);
        }
    }

    #[test]
    fn boost_choices_follow_windows_indices() {
        let image = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: BOOST_MODE,
            ac: 2,
            dc: 0,
        };
        assert_eq!(
            value(Feature::WindowsBoostMode, &image).unwrap(),
            Value::Choice(Choice::Aggressive)
        );
    }

    #[test]
    fn handles_all_ten_windows_features() {
        for f in [
            Feature::WindowsBoostMode,
            Feature::WindowsEnergyMode,
            Feature::WindowsPowerPlan,
            Feature::WindowsMinimumProcessorPercent,
            Feature::WindowsMaximumProcessorPercent,
            Feature::WindowsProcessorEppPercent,
            Feature::WindowsCoreParkingMinPercent,
            Feature::WindowsCoreParkingMaxPercent,
            Feature::DisplayRefreshRateHz,
            Feature::CpuCcdAffinity,
        ] {
            assert!(handles(f), "{f:?} should be handled");
        }
        assert!(!handles(Feature::GpuCoreOffsetMhz));
        assert!(!handles(Feature::CpuFanCurve));
    }

    #[test]
    fn all_windows_images_round_trip_through_json() {
        let images = [
            WindowsImage::PowerIndex {
                scheme: PLAN_BALANCED,
                setting: MIN_STATE,
                ac: 5,
                dc: 5,
            },
            WindowsImage::ActiveScheme { scheme: PLAN_HIGH },
            WindowsImage::Overlay {
                overlay: OVERLAY_PERFORMANCE,
            },
            WindowsImage::Refresh {
                width: 2560,
                height: 1440,
                hz: 165,
            },
        ];
        for image in images {
            let text = serde_json::to_string(&image).unwrap();
            let parsed: WindowsImage = serde_json::from_str(&text).unwrap();
            assert_eq!(parsed, image);
        }
    }

    #[test]
    fn value_mapping_for_schemes_overlays_and_refresh() {
        for (scheme, expected) in [
            (PLAN_POWER_SAVER, Choice::PowerSaver),
            (PLAN_BALANCED, Choice::Balanced),
            (PLAN_HIGH, Choice::HighPerformance),
            (0x1234_5678, Choice::Manual),
        ] {
            let image = WindowsImage::ActiveScheme { scheme };
            assert_eq!(
                value(Feature::WindowsPowerPlan, &image).unwrap(),
                Value::Choice(expected)
            );
        }

        for (overlay, expected) in [
            (OVERLAY_EFFICIENCY, Choice::BestEfficiency),
            (OVERLAY_BALANCED, Choice::Balanced),
            (OVERLAY_PERFORMANCE, Choice::BestPerformance),
            (0x9999_8888, Choice::Manual),
        ] {
            let image = WindowsImage::Overlay { overlay };
            assert_eq!(
                value(Feature::WindowsEnergyMode, &image).unwrap(),
                Value::Choice(expected)
            );
        }

        let refresh_image = WindowsImage::Refresh {
            width: 1920,
            height: 1080,
            hz: 144,
        };
        assert_eq!(
            value(Feature::DisplayRefreshRateHz, &refresh_image).unwrap(),
            Value::Integer(144)
        );

        let percent_image = WindowsImage::PowerIndex {
            scheme: PLAN_BALANCED,
            setting: MIN_STATE,
            ac: 10,
            dc: 5,
        };
        assert_eq!(
            value(Feature::WindowsMinimumProcessorPercent, &percent_image).unwrap(),
            Value::Integer(10)
        );
    }

    #[test]
    fn offline_validation_checks_boost_and_percentages() {
        assert!(
            validate(
                Feature::WindowsBoostMode,
                &Value::Choice(Choice::Aggressive)
            )
            .is_ok()
        );
        assert!(
            validate(
                Feature::WindowsBoostMode,
                &Value::Choice(Choice::HighPerformance)
            )
            .is_err()
        );
        assert!(validate(Feature::WindowsBoostMode, &Value::Integer(1)).is_err());

        for percent_feat in [
            Feature::WindowsMinimumProcessorPercent,
            Feature::WindowsMaximumProcessorPercent,
            Feature::WindowsProcessorEppPercent,
            Feature::WindowsCoreParkingMinPercent,
            Feature::WindowsCoreParkingMaxPercent,
        ] {
            assert!(validate(percent_feat, &Value::Integer(0)).is_ok());
            assert!(validate(percent_feat, &Value::Integer(100)).is_ok());
            assert!(validate(percent_feat, &Value::Integer(101)).is_err());
            assert!(validate(percent_feat, &Value::Integer(-1)).is_err());
        }
    }
}
