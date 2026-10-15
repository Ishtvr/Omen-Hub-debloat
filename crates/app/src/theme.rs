//! Shared dark native surfaces. Windows retains control roles and keyboard
//! behavior. GDI objects have bounded lifetimes; no continuous render timer.
use std::ptr;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        Controls::{
            COMBOBOXINFO, DRAWITEMSTRUCT, GetComboBoxInfo, ODS_DISABLED, ODS_FOCUS, ODS_SELECTED,
            SetWindowTheme, WM_MOUSELEAVE,
        },
        Input::KeyboardAndMouse::{GetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}
pub const BACKGROUND: COLORREF = rgb(16, 18, 21);
pub const PANEL: COLORREF = rgb(25, 29, 34);
pub const FIELD: COLORREF = rgb(36, 43, 51);
pub const BORDER: COLORREF = rgb(101, 120, 136);
pub const FOREGROUND: COLORREF = rgb(237, 242, 247);
pub const MUTED: COLORREF = rgb(166, 177, 188);
pub const ACCENT: COLORREF = rgb(104, 219, 191);
pub const ACCENT_INK: COLORREF = rgb(11, 28, 23);
pub const ERROR: COLORREF = rgb(247, 167, 167);
const HOVER: &str = "OmenHubPP.Hover";
const SURFACE: &str = "OmenHubPP.Surface";
const MUTED_PROP: &str = "OmenHubPP.Muted";
const ERROR_PROP: &str = "OmenHubPP.Error";
pub struct Theme {
    pub background: HBRUSH,
    pub panel: HBRUSH,
    pub field: HBRUSH,
    pub body: HFONT,
    pub small: HFONT,
    pub heading: HFONT,
    pub title: HFONT,
    pub metric: HFONT,
    pub high_contrast: bool,
}
impl Theme {
    pub fn new() -> Self {
        unsafe {
            let mut hc: HIGHCONTRASTW = std::mem::zeroed();
            hc.cbSize = size_of::<HIGHCONTRASTW>() as u32;
            let high_contrast = SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                hc.cbSize,
                (&mut hc as *mut HIGHCONTRASTW).cast(),
                0,
            ) != 0
                && hc.dwFlags & HCF_HIGHCONTRASTON != 0;
            Self {
                background: CreateSolidBrush(if high_contrast {
                    GetSysColor(COLOR_WINDOW)
                } else {
                    BACKGROUND
                }),
                panel: CreateSolidBrush(if high_contrast {
                    GetSysColor(COLOR_WINDOW)
                } else {
                    PANEL
                }),
                field: CreateSolidBrush(if high_contrast {
                    GetSysColor(COLOR_WINDOW)
                } else {
                    FIELD
                }),
                body: font(16, 400),
                small: font(14, 400),
                heading: font(20, 600),
                title: font(28, 600),
                metric: font(36, 600),
                high_contrast,
            }
        }
    }
    pub fn text(&self) -> COLORREF {
        unsafe {
            if self.high_contrast {
                GetSysColor(COLOR_WINDOWTEXT)
            } else {
                FOREGROUND
            }
        }
    }
    pub fn muted(&self) -> COLORREF {
        if self.high_contrast {
            self.text()
        } else {
            MUTED
        }
    }
    pub fn bg(&self) -> COLORREF {
        unsafe {
            if self.high_contrast {
                GetSysColor(COLOR_WINDOW)
            } else {
                BACKGROUND
            }
        }
    }
    pub fn panel_color(&self) -> COLORREF {
        if self.high_contrast { self.bg() } else { PANEL }
    }
    pub fn field_color(&self) -> COLORREF {
        if self.high_contrast { self.bg() } else { FIELD }
    }
    pub unsafe fn frame(&self, hwnd: HWND) {
        unsafe {
            if !self.high_contrast {
                let dark = 1i32;
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    (&dark as *const i32).cast(),
                    4,
                );
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_CAPTION_COLOR as u32,
                    (&BACKGROUND as *const u32).cast(),
                    4,
                );
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TEXT_COLOR as u32,
                    (&FOREGROUND as *const u32).cast(),
                    4,
                );
            }
        }
    }
    pub unsafe fn color(&self, m: u32, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        if !matches!(
            m,
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX
        ) {
            return None;
        }
        unsafe {
            let dc = w as HDC;
            let mut class = [0u16; 32];
            let n = GetClassNameW(l as HWND, class.as_mut_ptr(), 32);
            let edit =
                String::from_utf16_lossy(&class[..n.max(0) as usize]).eq_ignore_ascii_case("edit");
            let input = edit || m == WM_CTLCOLOREDIT || m == WM_CTLCOLORLISTBOX;
            let panel = !GetPropW(l as HWND, wide(SURFACE).as_ptr()).is_null();
            let muted = !GetPropW(l as HWND, wide(MUTED_PROP).as_ptr()).is_null();
            let error = !GetPropW(l as HWND, wide(ERROR_PROP).as_ptr()).is_null();
            SetTextColor(
                dc,
                if error && !self.high_contrast {
                    ERROR
                } else if muted {
                    self.muted()
                } else {
                    self.text()
                },
            );
            SetBkColor(
                dc,
                if input {
                    self.field_color()
                } else if panel {
                    self.panel_color()
                } else {
                    self.bg()
                },
            );
            Some(if input {
                self.field
            } else if panel {
                self.panel
            } else {
                self.background
            } as isize)
        }
    }
    pub unsafe fn fill(&self, hwnd: HWND, dc: HDC) {
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut r);
            FillRect(dc, &r, self.background);
        }
    }
    pub unsafe fn button(&self, item: &DRAWITEMSTRUCT, font: HFONT, primary: bool) {
        unsafe {
            let disabled = item.itemState & ODS_DISABLED != 0;
            let pressed = item.itemState & ODS_SELECTED != 0;
            let hot = !GetPropW(item.hwndItem, wide(HOVER).as_ptr()).is_null();
            let color = if self.high_contrast {
                GetSysColor(COLOR_BTNFACE)
            } else if disabled {
                PANEL
            } else if primary {
                if pressed {
                    rgb(76, 182, 153)
                } else if hot {
                    rgb(133, 233, 208)
                } else {
                    ACCENT
                }
            } else if pressed {
                PANEL
            } else if hot {
                rgb(48, 57, 67)
            } else {
                FIELD
            };
            let fill = CreateSolidBrush(color);
            FillRect(item.hDC, &item.rcItem, fill);
            DeleteObject(fill);
            let pen = CreatePen(
                PS_SOLID,
                1,
                if self.high_contrast {
                    self.text()
                } else if primary && !disabled {
                    color
                } else {
                    BORDER
                },
            );
            let old = SelectObject(item.hDC, pen);
            let brush = SelectObject(item.hDC, GetStockObject(HOLLOW_BRUSH));
            Rectangle(
                item.hDC,
                item.rcItem.left,
                item.rcItem.top,
                item.rcItem.right,
                item.rcItem.bottom,
            );
            SelectObject(item.hDC, brush);
            SelectObject(item.hDC, old);
            DeleteObject(pen);
            let old = SelectObject(item.hDC, font);
            SetBkMode(item.hDC, TRANSPARENT as i32);
            SetTextColor(
                item.hDC,
                if self.high_contrast {
                    GetSysColor(if disabled {
                        COLOR_GRAYTEXT
                    } else {
                        COLOR_BTNTEXT
                    })
                } else if disabled {
                    MUTED
                } else if primary {
                    ACCENT_INK
                } else {
                    FOREGROUND
                },
            );
            let mut title = [0u16; 256];
            let n = GetWindowTextW(item.hwndItem, title.as_mut_ptr(), 256);
            let mut r = item.rcItem;
            DrawTextW(
                item.hDC,
                title.as_ptr(),
                n,
                &mut r,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(item.hDC, old);
            if item.itemState & ODS_FOCUS != 0 {
                r.left += 3;
                r.top += 3;
                r.right -= 3;
                r.bottom -= 3;
                let fill = CreateSolidBrush(if self.high_contrast {
                    self.text()
                } else if primary {
                    ACCENT_INK
                } else {
                    ACCENT
                });
                FrameRect(item.hDC, &r, fill);
                r.left += 1;
                r.top += 1;
                r.right -= 1;
                r.bottom -= 1;
                FrameRect(item.hDC, &r, fill);
                DeleteObject(fill);
            }
        }
    }
}
impl Drop for Theme {
    fn drop(&mut self) {
        unsafe {
            for o in [
                self.background as HGDIOBJ,
                self.panel as HGDIOBJ,
                self.field as HGDIOBJ,
                self.body,
                self.small,
                self.heading,
                self.title,
                self.metric,
            ] {
                if !o.is_null() {
                    DeleteObject(o);
                }
            }
        }
    }
}
unsafe fn font(height: i32, weight: i32) -> HFONT {
    unsafe {
        let mut f: LOGFONTW = std::mem::zeroed();
        f.lfHeight = -height;
        f.lfWeight = weight;
        f.lfQuality = CLEARTYPE_QUALITY;
        for (a, b) in f.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
            *a = b;
        }
        CreateFontIndirectW(&f)
    }
}
pub unsafe fn surface(hwnd: HWND) {
    unsafe {
        SetPropW(hwnd, wide(SURFACE).as_ptr(), 1usize as HANDLE);
    }
}
pub unsafe fn muted(hwnd: HWND) {
    unsafe {
        SetPropW(hwnd, wide(MUTED_PROP).as_ptr(), 1usize as HANDLE);
    }
}
pub unsafe fn error(hwnd: HWND, active: bool) {
    unsafe {
        if active {
            SetPropW(hwnd, wide(ERROR_PROP).as_ptr(), 1usize as HANDLE);
        } else {
            RemovePropW(hwnd, wide(ERROR_PROP).as_ptr());
        }
    }
}
pub unsafe fn prepare(hwnd: HWND, class: &str, style: u32) {
    unsafe {
        if matches!(class, "BUTTON" | "EDIT" | "COMBOBOX") {
            SetWindowTheme(hwnd, wide("").as_ptr(), wide("").as_ptr());
            if class == "BUTTON" && style & BS_TYPEMASK as u32 <= BS_DEFPUSHBUTTON as u32 {
                SetWindowLongPtrW(
                    hwnd,
                    GWL_STYLE,
                    ((GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & !(BS_TYPEMASK as u32))
                        | BS_OWNERDRAW as u32) as isize,
                );
            }
            SetWindowSubclass(hwnd, Some(control_proc), 1, 0);
        }
    }
}
unsafe extern "system" fn control_proc(
    hwnd: HWND,
    m: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        // Preserve native checkbox roles, automatic toggling and keyboard input;
        // replace only classic painting (including embossed disabled lettering).
        if matches!(m, WM_PAINT | WM_PRINT | WM_PRINTCLIENT) {
            let kind = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & BS_TYPEMASK as u32;
            let mut class = [0u16; 32];
            let n = GetClassNameW(hwnd, class.as_mut_ptr(), 32);
            if matches!(kind,k if k==BS_AUTOCHECKBOX as u32||k==BS_CHECKBOX as u32)
                && String::from_utf16_lossy(&class[..n.max(0) as usize])
                    .eq_ignore_ascii_case("button")
            {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                let dc = if m == WM_PAINT {
                    BeginPaint(hwnd, &mut paint)
                } else {
                    w as HDC
                };
                if !dc.is_null() {
                    checkbox(hwnd, dc);
                }
                if m == WM_PAINT {
                    EndPaint(hwnd, &paint);
                }
                return 0;
            }
        }
        if m == WM_MOUSEMOVE && GetPropW(hwnd, wide(HOVER).as_ptr()).is_null() {
            SetPropW(hwnd, wide(HOVER).as_ptr(), 1usize as HANDLE);
            let mut t = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut t);
            InvalidateRect(hwnd, ptr::null(), 0);
        }
        if m == WM_MOUSELEAVE {
            RemovePropW(hwnd, wide(HOVER).as_ptr());
            InvalidateRect(hwnd, ptr::null(), 0);
        }
        if m == WM_NCDESTROY {
            RemovePropW(hwnd, wide(HOVER).as_ptr());
            RemovePropW(hwnd, wide(SURFACE).as_ptr());
            RemovePropW(hwnd, wide(MUTED_PROP).as_ptr());
            RemoveWindowSubclass(hwnd, Some(control_proc), id);
        }
        let result = DefSubclassProc(hwnd, m, w, l);
        if matches!(m, WM_SETFOCUS | WM_KILLFOCUS) {
            InvalidateRect(hwnd, ptr::null(), 0);
        }
        if matches!(m, WM_PAINT | WM_PRINTCLIENT) {
            let mut class = [0u16; 32];
            let n = GetClassNameW(hwnd, class.as_mut_ptr(), 32);
            let mut hc: HIGHCONTRASTW = std::mem::zeroed();
            hc.cbSize = size_of::<HIGHCONTRASTW>() as u32;
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                hc.cbSize,
                (&mut hc as *mut HIGHCONTRASTW).cast(),
                0,
            );
            if String::from_utf16_lossy(&class[..n.max(0) as usize])
                .eq_ignore_ascii_case("combobox")
                && hc.dwFlags & HCF_HIGHCONTRASTON == 0
            {
                let dc = if m == WM_PRINTCLIENT {
                    w as HDC
                } else {
                    GetDC(hwnd)
                };
                if !dc.is_null() {
                    let saved = SaveDC(dc);
                    let mut r: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut r);
                    let fill = CreateSolidBrush(FIELD);
                    FillRect(dc, &r, fill);
                    DeleteObject(fill);
                    let mut info: COMBOBOXINFO = std::mem::zeroed();
                    info.cbSize = size_of::<COMBOBOXINFO>() as u32;
                    if GetComboBoxInfo(hwnd, &mut info) != 0 {
                        let index = SendMessageW(hwnd, CB_GETCURSEL, 0, 0);
                        let length = SendMessageW(hwnd, CB_GETLBTEXTLEN, index as usize, 0);
                        if (0..4096).contains(&length) {
                            let mut value = vec![0u16; length as usize + 1];
                            SendMessageW(
                                hwnd,
                                CB_GETLBTEXT,
                                index as usize,
                                value.as_mut_ptr() as isize,
                            );
                            let f = SendMessageW(hwnd, WM_GETFONT, 0, 0);
                            if f != 0 {
                                SelectObject(dc, f as HFONT);
                            }
                            SetBkMode(dc, TRANSPARENT as i32);
                            SetTextColor(dc, FOREGROUND);
                            let mut text = info.rcItem;
                            text.left += 8;
                            DrawTextW(
                                dc,
                                value.as_ptr(),
                                length as i32,
                                &mut text,
                                DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
                            );
                        }
                        let x = (info.rcButton.left + info.rcButton.right) / 2;
                        let y = (info.rcButton.top + info.rcButton.bottom) / 2;
                        let pen = CreatePen(PS_SOLID, 2, FOREGROUND);
                        let old = SelectObject(dc, pen);
                        MoveToEx(dc, x - 4, y - 2, ptr::null_mut());
                        LineTo(dc, x, y + 2);
                        LineTo(dc, x + 4, y - 2);
                        SelectObject(dc, old);
                        DeleteObject(pen);
                    }
                    let border = CreateSolidBrush(if GetFocus() == hwnd { ACCENT } else { BORDER });
                    FrameRect(dc, &r, border);
                    DeleteObject(border);
                    RestoreDC(dc, saved);
                    if m != WM_PRINTCLIENT {
                        ReleaseDC(hwnd, dc);
                    }
                }
            }
        }
        result
    }
}

unsafe fn checkbox(hwnd: HWND, dc: HDC) {
    unsafe {
        let saved = SaveDC(dc);
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let background =
            SendMessageW(GetParent(hwnd), WM_CTLCOLORBTN, dc as usize, hwnd as isize) as HBRUSH;
        FillRect(dc, &rect, background);
        let mut hc: HIGHCONTRASTW = std::mem::zeroed();
        hc.cbSize = size_of::<HIGHCONTRASTW>() as u32;
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            (&mut hc as *mut HIGHCONTRASTW).cast(),
            0,
        );
        let contrast = hc.dwFlags & HCF_HIGHCONTRASTON != 0;
        let enabled = windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
        let checked = SendMessageW(hwnd, BM_GETCHECK, 0, 0) == 1;
        let accent = if contrast {
            GetSysColor(COLOR_HIGHLIGHT)
        } else {
            ACCENT
        };
        let foreground = if contrast {
            GetSysColor(if enabled {
                COLOR_WINDOWTEXT
            } else {
                COLOR_GRAYTEXT
            })
        } else if enabled {
            FOREGROUND
        } else {
            MUTED
        };
        let top = (rect.bottom - 16) / 2;
        let box_rect = RECT {
            left: 2,
            top,
            right: 18,
            bottom: top + 16,
        };
        let fill = CreateSolidBrush(if checked && enabled {
            accent
        } else if contrast {
            GetSysColor(COLOR_WINDOW)
        } else {
            FIELD
        });
        FillRect(dc, &box_rect, fill);
        DeleteObject(fill);
        let border = CreateSolidBrush(
            if enabled && (GetFocus() == hwnd || !GetPropW(hwnd, wide(HOVER).as_ptr()).is_null()) {
                accent
            } else if contrast {
                foreground
            } else {
                BORDER
            },
        );
        FrameRect(dc, &box_rect, border);
        DeleteObject(border);
        if checked {
            let pen = CreatePen(
                PS_SOLID,
                2,
                if enabled {
                    if contrast {
                        GetSysColor(COLOR_HIGHLIGHTTEXT)
                    } else {
                        ACCENT_INK
                    }
                } else {
                    foreground
                },
            );
            let old = SelectObject(dc, pen);
            MoveToEx(dc, 5, top + 8, ptr::null_mut());
            LineTo(dc, 9, top + 12);
            LineTo(dc, 15, top + 4);
            SelectObject(dc, old);
            DeleteObject(pen);
        }
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0);
        if font != 0 {
            SelectObject(dc, font as HFONT);
        }
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, foreground);
        let mut label = [0u16; 512];
        let n = GetWindowTextW(hwnd, label.as_mut_ptr(), 512);
        let mut label_rect = RECT {
            left: 26,
            top: 0,
            right: rect.right,
            bottom: rect.bottom,
        };
        DrawTextW(
            dc,
            label.as_ptr(),
            n,
            &mut label_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        if GetFocus() == hwnd {
            let mut focus = rect;
            InflateRect(&mut focus, -1, -2);
            DrawFocusRect(dc, &focus);
        }
        RestoreDC(dc, saved);
    }
}
