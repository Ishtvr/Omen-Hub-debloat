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

/// Complete original of one control. Power indices keep AC and DC separately;
/// a profile writes both, so the captured pair restores either power source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindowsImage {
    PowerIndex {
        scheme: u128,
        setting: u128,
        ac: u32,
        dc: u32,
    },
    ActiveScheme {
        scheme: u128,
    },
    Overlay {
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
        check(PowerSetActiveScheme(ptr::null_mut(), &s), "Activate scheme")
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
type SetOverlay = unsafe extern "system" fn(*const GUID) -> u32;
fn overlay_api() -> Result<(libloading::Library, GetOverlay, SetOverlay), String> {
    // SAFETY: System DLL; symbol signatures match the shipped powrprof exports.
    unsafe {
        let library = libloading::Library::new("powrprof.dll").map_err(|e| e.to_string())?;
        let get = *library
            .get::<GetOverlay>(b"PowerGetEffectiveOverlayScheme\0")
            .map_err(|_| "Windows power mode is not available on this system")?;
        let set = *library
            .get::<SetOverlay>(b"PowerSetActiveOverlayScheme\0")
            .map_err(|_| "Windows power mode is not available on this system")?;
        Ok((library, get, set))
    }
}
fn read_overlay() -> Result<u128, String> {
    let (_library, get, _) = overlay_api()?;
    let mut value = guid(0);
    // SAFETY: Writable GUID output; library stays loaded through the call.
    check(unsafe { get(&mut value) }, "Power mode")?;
    Ok(to_u128(&value))
}
fn write_overlay(overlay: u128) -> Result<(), String> {
    let (_library, _, set) = overlay_api()?;
    let value = guid(overlay);
    // SAFETY: Valid GUID; library stays loaded through the call.
    check(unsafe { set(&value) }, "Power mode write")
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
    if let Some(setting) = power_setting(feature) {
        let scheme = active_scheme()?;
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
        Feature::WindowsEnergyMode => Ok(WindowsImage::Overlay {
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
        (Feature::WindowsEnergyMode, WindowsImage::Overlay { overlay }) => {
            Value::Choice(match *overlay {
                OVERLAY_EFFICIENCY => Choice::BestEfficiency,
                OVERLAY_BALANCED => Choice::Balanced,
                OVERLAY_PERFORMANCE => Choice::BestPerformance,
                _ => Choice::Manual,
            })
        }
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
        (f, Value::Integer(v)) if power_setting(f).is_some() => {
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
        (WindowsImage::Overlay { .. }, Value::Choice(c)) => {
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
        WindowsImage::Overlay { overlay } => write_overlay(*overlay)?,
        WindowsImage::Refresh { width, height, hz } => write_refresh(*width, *height, *hz)?,
        WindowsImage::CcdSelection { selection } => ccd_store()?.save_ccd_selection(*selection)?,
    }
    if capture(feature)? != *original {
        return Err("Windows setting did not return to its captured original".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
