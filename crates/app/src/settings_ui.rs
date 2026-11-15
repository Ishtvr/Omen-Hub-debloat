//! Settings share the Slint shell and its existing persistent transaction.
use windows_sys::Win32::Foundation::HWND;
pub fn run(_owner: HWND) -> Result<(), String> {
    crate::slint_ui::entry(4, None)
}
