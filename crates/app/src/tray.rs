//! Owned notification icon. Refresh before hiding; recover after Explorer
//! recreates its notification area. Failed registration gets a bounded retry,
//! never a permanent idle timer or a hidden, unreachable dashboard.
use crate::native_ui::{app_icon, wide};
use omen_core::APP_NAME;
use std::cell::Cell;
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub const CALLBACK: u32 = WM_APP + 2;
/// SDK macro NIN_KEYSELECT is not emitted as a constant by windows-sys.
pub const KEY_SELECT: u32 = NIN_SELECT | NINF_KEY;
pub const RETRY_TIMER: usize = 0x4F4D454E;
const MAX_RETRIES: u8 = 30;

#[derive(Default)]
pub struct TrayIcon {
    registered: Cell<bool>,
    retry_count: Cell<Option<u8>>,
}

fn identity(hwnd: HWND) -> NOTIFYICONDATAW {
    // SAFETY: Zero is valid for all fields; only HWND/uID identify this icon.
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data
}

impl TrayIcon {
    pub fn registered(&self) -> bool {
        self.registered.get()
    }

    pub fn retrying(&self) -> bool {
        self.retry_count.get().is_some()
    }

    /// An ADD can fail because the icon already exists. MODIFY first also
    /// detects a stale local flag after a missed shell-restart notification.
    pub unsafe fn ensure(&self, hwnd: HWND) -> bool {
        unsafe {
            let mut data = identity(hwnd);
            data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP | NIF_STATE;
            data.uCallbackMessage = CALLBACK;
            // Shared embedded icon (or shared Windows fallback), never owned.
            data.hIcon = app_icon(SM_CXSMICON);
            data.dwStateMask = NIS_HIDDEN;
            data.dwState = 0;
            for (target, value) in data.szTip.iter_mut().zip(wide(APP_NAME)) {
                *target = value;
            }
            let ok = !data.hIcon.is_null()
                && (Shell_NotifyIconW(NIM_MODIFY, &data) != 0
                    || Shell_NotifyIconW(NIM_ADD, &data) != 0);
            self.registered.set(ok);
            if ok {
                self.cancel_retry(hwnd);
                data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                // Callback decoding supports both versions if this is refused.
                Shell_NotifyIconW(NIM_SETVERSION, &data);
            } else if self.retry_count.get().is_none()
                && SetTimer(hwnd, RETRY_TIMER, 1000, None) != 0
            {
                self.retry_count.set(Some(0));
            }
            ok
        }
    }

    /// True while registration succeeded or a recovery attempt remains.
    pub unsafe fn retry(&self, hwnd: HWND) -> bool {
        unsafe {
            let Some(count) = self.retry_count.get() else {
                return self.registered();
            };
            if self.ensure(hwnd) {
                return true;
            }
            if count + 1 >= MAX_RETRIES {
                self.cancel_retry(hwnd);
                return false;
            }
            self.retry_count.set(Some(count + 1));
            true
        }
    }

    unsafe fn cancel_retry(&self, hwnd: HWND) {
        unsafe {
            if self.retry_count.take().is_some() {
                KillTimer(hwnd, RETRY_TIMER);
            }
        }
    }

    pub unsafe fn remove(&self, hwnd: HWND) {
        unsafe {
            self.cancel_retry(hwnd);
            // Delete even if the cached flag was invalidated by a shell error.
            Shell_NotifyIconW(NIM_DELETE, &identity(hwnd));
            self.registered.set(false);
        }
    }
}
