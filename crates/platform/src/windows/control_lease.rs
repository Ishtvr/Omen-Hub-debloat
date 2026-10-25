//! Cross-process ownership for Omen Hub++ hardware transactions.
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

pub struct ControlLease {
    handle: HANDLE,
}
impl ControlLease {
    pub fn acquire() -> Result<Self, String> {
        Self::named("Global\\OmenHubPP.HardwareController")
    }
    pub(crate) fn hp_ownership() -> Result<Self, String> {
        Self::named("Global\\OmenHubPP.HpControllerOwner")
    }
    fn named(name: &str) -> Result<Self, String> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: Fixed terminated name, default security, no initial ownership.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: Valid mutex handle. Zero timeout avoids blocking the application.
        let wait = unsafe { WaitForSingleObject(handle, 0) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            // SAFETY: This branch did not acquire the mutex; close only the handle.
            unsafe {
                CloseHandle(handle);
            }
            return Err("Another Omen Hub++ hardware transaction is running".into());
        }
        // An abandoned mutex is owned now; the durable journal separately blocks retry.
        Ok(Self { handle })
    }
}
impl Drop for ControlLease {
    fn drop(&mut self) {
        // SAFETY: This thread acquired the mutex. There is no Send/Sync implementation.
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}
