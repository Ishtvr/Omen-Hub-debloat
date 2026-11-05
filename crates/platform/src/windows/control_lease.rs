//! Cross-process ownership for Omen Hub++ hardware transactions.
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
    },
    System::Threading::{
        CreateMutexW, OpenMutexW, ReleaseMutex, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject,
    },
};

/// Stable machine-readable contention signal. Control-flow consumers compare
/// this code, never translated display prose.
pub(super) const HARDWARE_BUSY: &str = "⟦control.err.busy⟧";

pub struct ControlLease {
    _hardware: MutexGuard,
    _foreground: Option<MutexGuard>,
}
struct MutexGuard {
    handle: HANDLE,
}
const HARDWARE: &str = "Global\\OmenHubPP.HardwareController";
const FOREGROUND: &str = "Global\\OmenHubPP.ForegroundController";
/// Manual controls wait on workers, never on the window thread. Each mutex
/// has a finite deadline; an in-flight hardware transaction is never killed.
const MANUAL_WAIT_MS: u32 = 15_000;
impl ControlLease {
    pub fn acquire() -> Result<Self, String> {
        Ok(Self {
            _hardware: MutexGuard::named(HARDWARE, 0)?,
            _foreground: None,
        })
    }
    /// Explicit Apply/Restore has precedence over opportunistic preset keeping.
    pub fn acquire_foreground() -> Result<Self, String> {
        Self::foreground(HARDWARE, FOREGROUND, MANUAL_WAIT_MS)
    }
    fn foreground(hardware: &str, foreground: &str, wait_ms: u32) -> Result<Self, String> {
        let intent = MutexGuard::named(foreground, wait_ms)?;
        let hardware = MutexGuard::named(hardware, wait_ms)?;
        Ok(Self {
            _hardware: hardware,
            _foreground: Some(intent),
        })
    }
    /// Background checks yield as soon as a manual command is waiting. The
    /// second check closes the race between opening the intent and hardware.
    pub fn acquire_background() -> Result<Self, String> {
        Self::background(HARDWARE, FOREGROUND)
    }
    fn background(hardware: &str, foreground: &str) -> Result<Self, String> {
        if foreground_exists(foreground) {
            return Err(HARDWARE_BUSY.into());
        }
        let lease = Self {
            _hardware: MutexGuard::named(hardware, 0)?,
            _foreground: None,
        };
        if foreground_exists(foreground) {
            return Err(HARDWARE_BUSY.into());
        }
        Ok(lease)
    }
    pub(crate) fn hp_ownership() -> Result<Self, String> {
        Ok(Self {
            _hardware: MutexGuard::named("Global\\OmenHubPP.HpControllerOwner", 0)?,
            _foreground: None,
        })
    }
}
fn foreground_exists(name: &str) -> bool {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: Probe only this fixed named object; any handle closes immediately.
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if handle.is_null() {
            return GetLastError() != ERROR_FILE_NOT_FOUND;
        }
        CloseHandle(handle);
        true
    }
}
impl MutexGuard {
    fn named(name: &str, wait_ms: u32) -> Result<Self, String> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: Fixed terminated name, default security, no initial ownership.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: Valid mutex handle. Only worker/command paths request a wait.
        let wait = unsafe { WaitForSingleObject(handle, wait_ms) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            // SAFETY: This branch did not acquire the mutex; close only the handle.
            unsafe {
                CloseHandle(handle);
            }
            return Err(HARDWARE_BUSY.into());
        }
        // An abandoned mutex is owned now; the durable journal separately blocks retry.
        Ok(Self { handle })
    }
}
impl Drop for MutexGuard {
    fn drop(&mut self) {
        // SAFETY: This thread acquired the mutex. There is no Send/Sync implementation.
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contention_code_matches_the_language_neutral_message_contract() {
        assert_eq!(
            HARDWARE_BUSY,
            omen_core::message::msg("control.err.busy", &[])
        );
    }

    #[test]
    fn foreground_waits_for_an_existing_check_and_background_yields() {
        use std::{sync::mpsc, thread, time::Duration};
        let suffix = format!(
            "{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let hardware = format!("Local\\OmenHubPP.TestHardware.{suffix}");
        let foreground = format!("Local\\OmenHubPP.TestIntent.{suffix}");
        let background = ControlLease::background(&hardware, &foreground).unwrap();
        let (tx, rx) = mpsc::channel();
        let (release, done) = mpsc::channel();
        let (h, f) = (hardware.clone(), foreground.clone());
        let worker = thread::spawn(move || {
            let _manual = ControlLease::foreground(&h, &f, 2_000).unwrap();
            tx.send(()).unwrap();
            done.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        for _ in 0..100 {
            if foreground_exists(&foreground) {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(foreground_exists(&foreground));
        assert!(rx.try_recv().is_err());
        assert!(ControlLease::background(&hardware, &foreground).is_err());
        drop(background);
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(ControlLease::background(&hardware, &foreground).is_err());
        release.send(()).unwrap();
        worker.join().unwrap();
        assert!(ControlLease::background(&hardware, &foreground).is_ok());
    }
}
