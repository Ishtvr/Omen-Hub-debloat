//! Small shared Win32 helpers. Hardware work never runs in these functions.
use std::{
    ptr,
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub fn token() -> String {
    format!(
        "{:x}{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}
/// Sets a window's text; core/platform message markers are translated.
pub fn text(handle: HWND, value: &str) {
    unsafe {
        SetWindowTextW(handle, wide(&crate::i18n::message(value)).as_ptr());
    }
}
pub fn value(handle: HWND) -> String {
    unsafe {
        let count = GetWindowTextLengthW(handle).clamp(0, 8192) as usize;
        let mut value = vec![0u16; count + 1];
        let read = GetWindowTextW(handle, value.as_mut_ptr(), value.len() as i32);
        String::from_utf16_lossy(&value[..read.max(0) as usize])
    }
}
pub fn input(handle: HWND) -> Result<String, String> {
    let mut buffer = [0u16; 257];
    let count = unsafe { GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16(&buffer[..count.max(0) as usize]).map_err(|e| e.to_string())
}
pub fn positioned_control(
    parent: HWND,
    font: HFONT,
    id: usize,
    class: &str,
    label: &str,
    style: u32,
    rect: [i32; 4],
) -> HWND {
    unsafe {
        let handle = control(parent, font, id, class, label, style);
        place(handle, rect[0], rect[1], rect[2], rect[3]);
        handle
    }
}
pub unsafe fn control(
    parent: HWND,
    font: HFONT,
    id: usize,
    class: &str,
    label: &str,
    style: u32,
) -> HWND {
    unsafe {
        let handle = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            parent,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        );
        SendMessageW(handle, WM_SETFONT, font as usize, 0);
        crate::theme::prepare(handle, class, style);
        handle
    }
}
pub unsafe fn place(handle: HWND, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        MoveWindow(handle, x, y, w.max(1), h.max(1), 1);
    }
}
pub unsafe fn fill(dc: HDC, rect: &RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, rect, brush);
        DeleteObject(brush);
    }
}
pub unsafe fn draw_text(
    dc: HDC,
    font: HFONT,
    rect: &mut RECT,
    label: &str,
    color: COLORREF,
    flags: u32,
) {
    unsafe {
        let old = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, color);
        DrawTextW(dc, wide(label).as_ptr(), -1, rect, flags);
        SelectObject(dc, old);
    }
}
pub unsafe fn register(class: &str, procedure: WNDPROC) -> Result<(), String> {
    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: procedure,
            hInstance: GetModuleHandleW(ptr::null()),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        // Keep the temporary class string alive through RegisterClassW.
        let class_name = wide(class);
        let wc = WNDCLASSW {
            lpszClassName: class_name.as_ptr(),
            ..wc
        };
        if RegisterClassW(&wc) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}
pub unsafe fn create(
    class: &str,
    title: &str,
    width: i32,
    height: i32,
    state: *mut core::ffi::c_void,
    preview: bool,
) -> Result<HWND, String> {
    unsafe {
        let handle = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(title).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            if preview { -16000 } else { CW_USEDEFAULT },
            if preview { -16000 } else { CW_USEDEFAULT },
            width,
            height,
            ptr::null_mut(),
            ptr::null_mut(),
            GetModuleHandleW(ptr::null()),
            state,
        );
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(handle)
    }
}
pub unsafe fn messages(hwnd: HWND) {
    unsafe {
        let mut message = std::mem::zeroed();
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
/// Width of `text` in `font` as drawn in `window`, in window units.
pub unsafe fn text_width(window: HWND, font: HFONT, text: &str) -> i32 {
    unsafe {
        let dc = GetDC(window);
        let old = SelectObject(dc, font);
        let mut size = SIZE { cx: 0, cy: 0 };
        let wide = text.encode_utf16().collect::<Vec<_>>();
        GetTextExtentPoint32W(dc, wide.as_ptr(), wide.len() as i32, &mut size);
        SelectObject(dc, old);
        ReleaseDC(window, dc);
        size.cx
    }
}
