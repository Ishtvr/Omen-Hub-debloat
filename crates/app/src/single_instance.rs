//! One window per kind: a second launch brings the running window to the
//! front and exits. The dashboard also restarts itself with administrator
//! rights, so hardware actions run without a Windows prompt each time.
use crate::native_ui::wide;
use std::ptr;
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND,
    },
    System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE},
    UI::{
        Shell::{IsUserAnAdmin, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::*,
    },
};

/// Posted to a running dashboard: show yourself (it may be hidden in the tray).
pub const ACTIVATE: &str = "OmenHubPP.Activate";

/// Held for the lifetime of the process; Windows releases it on exit.
pub struct Instance(#[allow(dead_code)] HANDLE);

fn mutex_name(kind: &str) -> String {
    format!("Local\\OmenHubPP.SingleInstance.{kind}")
}

/// Claims the window kind for this process. `None`: another instance of the
/// kind already runs. Kinds are per Windows session.
pub fn claim(kind: &str) -> Option<Instance> {
    let name = wide(&mutex_name(kind));
    // SAFETY: Terminated name; the handle is owned by the returned value.
    let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    let error = unsafe { GetLastError() };
    // Access is denied when an elevated instance created the mutex and this
    // launch runs without administrator rights: it exists either way.
    if (handle.is_null() && error == ERROR_ACCESS_DENIED) || error == ERROR_ALREADY_EXISTS {
        if !handle.is_null() {
            unsafe { CloseHandle(handle) };
        }
        return None;
    }
    Some(Instance(handle))
}

/// Whether another instance of `kind` is running, without claiming it.
pub fn running(kind: &str) -> bool {
    let name = wide(&mutex_name(kind));
    // SAFETY: Read-only open of a named object; the handle is closed at once.
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if handle.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        CloseHandle(handle);
        true
    }
}

/// Finds the running window of `class` and brings it forward. The dashboard
/// may be hidden in the tray, so it is asked to show itself (`ask`); other
/// windows are raised directly. The first instance may still be creating its
/// window, so look for up to two seconds.
pub fn activate(class: &str, ask: bool) {
    let class = wide(class);
    for _ in 0..20 {
        // SAFETY: Terminated class name; the window may close at any time,
        // which only makes the following calls fail.
        unsafe {
            let hwnd: HWND = FindWindowW(class.as_ptr(), ptr::null());
            if !hwnd.is_null() {
                let mut process = 0;
                GetWindowThreadProcessId(hwnd, &mut process);
                // This launch received the foreground from the user; pass it on.
                AllowSetForegroundWindow(process);
                if ask {
                    PostMessageW(hwnd, activation_message(), 0, 0);
                } else {
                    bring_forward(hwnd);
                }
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Registered message a second launch posts to the dashboard.
pub fn activation_message() -> u32 {
    static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    // SAFETY: Terminated name; the same value is returned in every process.
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(wide(ACTIVATE).as_ptr()) })
}

/// Lets a launch without administrator rights reach an elevated dashboard.
///
/// # Safety
/// `hwnd` is a live window owned by the calling thread.
pub unsafe fn accept_activation(hwnd: HWND) {
    unsafe {
        let message = activation_message();
        if message != 0 {
            ChangeWindowMessageFilterEx(hwnd, message, MSGFLT_ALLOW, ptr::null_mut());
        }
    }
}

/// Restores and raises a window.
///
/// # Safety
/// `hwnd` is a live top-level window owned by the calling thread.
pub unsafe fn bring_forward(hwnd: HWND) {
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
}

/// Whether this process already has administrator rights.
pub fn elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// Starts this executable again with administrator rights and the same
/// dashboard arguments. `false` when Windows did not start it, for example
/// because the prompt was declined; the caller then continues without.
pub fn relaunch_elevated(arguments: &str) -> bool {
    let Ok(file) = std::env::current_exe() else {
        return false;
    };
    let file = wide(&file.to_string_lossy());
    let parameters = wide(arguments);
    let verb = wide("runas");
    let mut execute: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    execute.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    execute.fMask = SEE_MASK_NOASYNC;
    execute.lpVerb = verb.as_ptr();
    execute.lpFile = file.as_ptr();
    execute.lpParameters = parameters.as_ptr();
    execute.nShow = SW_SHOWNORMAL;
    // SAFETY: All strings outlive the call; no process handle is requested.
    unsafe { ShellExecuteExW(&mut execute) != 0 }
}
