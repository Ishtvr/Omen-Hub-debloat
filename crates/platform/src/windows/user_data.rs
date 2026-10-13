//! Resolve the actual Windows user's profile, independently of package AppData.
use std::{path::PathBuf, ptr};
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Security::TOKEN_QUERY,
    System::Threading::{GetCurrentProcess, OpenProcessToken},
    UI::Shell::GetUserProfileDirectoryW,
};

pub(crate) fn profile_directory() -> Result<PathBuf, String> {
    let mut token = ptr::null_mut();
    // SAFETY: Query-only handle to this process's Windows user token.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let result = (|| {
        let mut count = 0;
        // SAFETY: Documented size query, no caller output buffer.
        unsafe {
            GetUserProfileDirectoryW(token, ptr::null_mut(), &mut count);
        }
        if count == 0 || count > 32768 {
            return Err("Windows user profile path size is invalid".into());
        }
        let mut buffer = vec![0; count as usize];
        // SAFETY: Buffer matches the size supplied by the first call, including NUL.
        if unsafe { GetUserProfileDirectoryW(token, buffer.as_mut_ptr(), &mut count) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let length = buffer
            .iter()
            .position(|c| *c == 0)
            .ok_or("Windows profile path is not terminated")?;
        let root = PathBuf::from(String::from_utf16(&buffer[..length]).map_err(|e| e.to_string())?);
        if !root.is_absolute() {
            return Err("Windows profile directory is not absolute".into());
        }
        // Outside AppData: Codex/MSIX children and Task Scheduler must see the
        // same durable files. This is per-user data, not an elevated command path.
        Ok(root.join(".omen-hub-pp"))
    })();
    // SAFETY: Every token path, including errors, closes its query-only handle.
    unsafe {
        CloseHandle(token);
    }
    result
}
