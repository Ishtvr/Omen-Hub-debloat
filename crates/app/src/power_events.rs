//! Public Windows power subscriptions. This module never changes a power state.
use omen_core::power::Event;
use std::ptr;
use windows_sys::{
    Win32::{
        Foundation::HWND,
        System::{
            Power::*,
            SystemServices::{GUID_ACDC_POWER_SOURCE, GUID_SESSION_DISPLAY_STATUS},
        },
        UI::WindowsAndMessaging::*,
    },
    core::GUID,
};

pub struct Notifications {
    source: HPOWERNOTIFY,
    display: HPOWERNOTIFY,
    sleep: HPOWERNOTIFY,
}
impl Notifications {
    pub fn register(hwnd: HWND) -> Result<Self, String> {
        let mut notifications = Self {
            source: 0,
            display: 0,
            sleep: 0,
        };
        // SAFETY: A live owned window receives notifications until Drop.
        unsafe {
            notifications.source = RegisterPowerSettingNotification(
                hwnd,
                &GUID_ACDC_POWER_SOURCE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
            if notifications.source == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            notifications.display = RegisterPowerSettingNotification(
                hwnd,
                &GUID_SESSION_DISPLAY_STATUS,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
            if notifications.display == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            notifications.sleep =
                RegisterSuspendResumeNotification(hwnd, DEVICE_NOTIFY_WINDOW_HANDLE);
            if notifications.sleep == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(notifications)
    }
}
impl Drop for Notifications {
    fn drop(&mut self) {
        unsafe {
            if self.source != 0 {
                UnregisterPowerSettingNotification(self.source);
            }
            if self.display != 0 {
                UnregisterPowerSettingNotification(self.display);
            }
            if self.sleep != 0 {
                UnregisterSuspendResumeNotification(self.sleep);
            }
        }
    }
}

fn same_guid(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}
fn setting(guid: &GUID, data: &[u8]) -> Option<Event> {
    let value = u32::from_le_bytes(data.try_into().ok()?);
    if same_guid(guid, &GUID_SESSION_DISPLAY_STATUS) {
        match value {
            0 => Some(Event::Display(false)),
            1 | 2 => Some(Event::Display(true)),
            _ => None,
        }
    } else if same_guid(guid, &GUID_ACDC_POWER_SOURCE) && value <= 2 {
        Some(Event::SourceChanged)
    } else {
        None
    }
}

/// # Safety
/// `data` is the Windows-owned POWERBROADCAST_SETTING valid only for this
/// WM_POWERBROADCAST call, or null. Never post a cross-process pointer as lParam.
pub unsafe fn decode(kind: u32, data: isize) -> Option<Event> {
    match kind {
        PBT_APMSUSPEND => Some(Event::Suspend),
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Some(Event::Resume),
        PBT_APMPOWERSTATUSCHANGE => Some(Event::SourceChanged),
        PBT_POWERSETTINGCHANGE if data != 0 => unsafe {
            let header = data as *const POWERBROADCAST_SETTING;
            let length = ptr::addr_of!((*header).DataLength).read_unaligned();
            if length != 4 {
                return None;
            }
            let guid = ptr::addr_of!((*header).PowerSetting).read_unaligned();
            let value = ptr::addr_of!((*header).Data)
                .cast::<[u8; 4]>()
                .read_unaligned();
            setting(&guid, &value)
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_message_reads_exact_dword_and_ignores_bad_lengths() {
        #[repr(C)]
        struct NativeSetting {
            guid: GUID,
            length: u32,
            value: u32,
        }
        let mut native = NativeSetting {
            guid: GUID_SESSION_DISPLAY_STATUS,
            length: 4,
            value: 0,
        };
        let decode_native = |native: &NativeSetting| unsafe {
            decode(
                PBT_POWERSETTINGCHANGE,
                (native as *const NativeSetting) as isize,
            )
        };
        assert_eq!(decode_native(&native), Some(Event::Display(false)));
        native.value = 2;
        assert_eq!(decode_native(&native), Some(Event::Display(true)));
        native.length = u32::MAX;
        assert_eq!(decode_native(&native), None);
        native.length = 0;
        assert_eq!(decode_native(&native), None);
        assert_eq!(unsafe { decode(PBT_APMSUSPEND, 0) }, Some(Event::Suspend));
        assert_eq!(
            unsafe { decode(PBT_APMRESUMEAUTOMATIC, 0) },
            Some(Event::Resume)
        );
    }
    #[test]
    fn only_typed_complete_display_and_ac_values_are_accepted() {
        assert_eq!(
            setting(&GUID_SESSION_DISPLAY_STATUS, &0u32.to_le_bytes()),
            Some(Event::Display(false))
        );
        for value in [1u32, 2] {
            assert_eq!(
                setting(&GUID_SESSION_DISPLAY_STATUS, &value.to_le_bytes()),
                Some(Event::Display(true))
            );
        }
        assert_eq!(
            setting(&GUID_SESSION_DISPLAY_STATUS, &3u32.to_le_bytes()),
            None
        );
        assert_eq!(setting(&GUID_SESSION_DISPLAY_STATUS, &[0, 0, 0]), None);
        assert_eq!(setting(&GUID_SESSION_DISPLAY_STATUS, &[0; 8]), None);
        assert_eq!(
            setting(&GUID_ACDC_POWER_SOURCE, &2u32.to_le_bytes()),
            Some(Event::SourceChanged)
        );
        assert_eq!(setting(&GUID_ACDC_POWER_SOURCE, &3u32.to_le_bytes()), None);
        assert_eq!(setting(&GUID::from_u128(0), &[0; 4]), None);
        assert_eq!(unsafe { decode(PBT_POWERSETTINGCHANGE, 0) }, None);
    }
}
