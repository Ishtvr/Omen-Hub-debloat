//! Reusable native GDI curve canvas. Every edit changes a draft only.
use omen_core::curve_editor::{Curve, Kind, Point as DataPoint};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    ptr,
    rc::Rc,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{GetFocus, GetKeyState, ReleaseCapture, SetCapture, SetFocus},
        WindowsAndMessaging::*,
    },
};

pub const CHANGED: u32 = WM_APP + 31;
struct State {
    model: RefCell<Option<Curve>>,
    font: HFONT,
    drag: Cell<bool>,
    error: RefCell<String>,
    ownership: Rc<Cell<u8>>,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn state<'a>(hwnd: HWND) -> Option<&'a State> {
    // SAFETY: Set only to our Box<State>, removed before WM_NCDESTROY releases it.
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const State;
    unsafe { p.as_ref() }
}
pub fn curve(hwnd: HWND) -> Option<Curve> {
    unsafe { state(hwnd) }?.model.borrow().clone()
}
pub fn error(hwnd: HWND) -> String {
    unsafe { state(hwnd) }.map_or(String::new(), |s| s.error.borrow().clone())
}
pub fn set(hwnd: HWND, model: Option<Curve>) {
    if let Some(s) = unsafe { state(hwnd) } {
        *s.model.borrow_mut() = model;
        refresh(hwnd);
    }
}
pub fn mutate(
    hwnd: HWND,
    action: impl FnOnce(&mut Curve) -> Result<bool, String>,
) -> Result<bool, String> {
    let s = unsafe { state(hwnd) }.ok_or("Kurvenansicht fehlt.")?;
    let result = {
        let mut model = s.model.borrow_mut();
        action(model.as_mut().ok_or("Kurve noch nicht verfügbar.")?)
    };
    if result.is_ok() {
        refresh(hwnd);
    }
    result
}
fn refresh(hwnd: HWND) {
    let caption = curve(hwnd).map_or("Kurve nicht verfügbar".into(), |c| {
        let p = c.points[c.selected];
        let (x, y) = c.kind.units();
        format!(
            "{}: Punkt {} von {}, {} {}, {} {}. Pfeiltasten wählen und bearbeiten Punkte.",
            c.kind.title(),
            c.selected + 1,
            c.points.len(),
            p.x,
            x,
            p.y,
            y
        )
    });
    // SAFETY: Our native control copies the bounded terminated caption.
    unsafe {
        SetWindowTextW(hwnd, wide(&caption).as_ptr());
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}
fn notify(hwnd: HWND, changed: usize) {
    // Post instead of Send: parent reads the model after mutable borrows end.
    unsafe {
        PostMessageW(GetParent(hwnd), CHANGED, changed, hwnd as isize);
    }
}
#[derive(Clone, Copy)]
struct Geometry {
    area: RECT,
    lo: DataPoint,
    hi: DataPoint,
}
impl Geometry {
    fn new(rect: RECT, curve: &Curve) -> Self {
        let (lo, hi) = curve.bounds();
        Self {
            area: RECT {
                left: rect.left + 72,
                top: rect.top + 34,
                right: rect.right - 28,
                bottom: rect.bottom - 64,
            },
            lo,
            hi,
        }
    }
    fn screen(self, p: DataPoint) -> POINT {
        POINT {
            x: self.area.left
                + ((i64::from(p.x - self.lo.x) * i64::from(self.area.right - self.area.left))
                    / (i64::from(self.hi.x - self.lo.x).max(1))) as i32,
            y: self.area.bottom
                - ((i64::from(p.y - self.lo.y) * i64::from(self.area.bottom - self.area.top))
                    / (i64::from(self.hi.y - self.lo.y).max(1))) as i32,
        }
    }
    fn data(self, x: i32, y: i32) -> DataPoint {
        DataPoint {
            x: self.lo.x
                + ((i64::from(x - self.area.left) * i64::from(self.hi.x - self.lo.x))
                    / (i64::from(self.area.right - self.area.left).max(1)))
                    as i32,
            y: self.lo.y
                + ((i64::from(self.area.bottom - y) * i64::from(self.hi.y - self.lo.y))
                    / (i64::from(self.area.bottom - self.area.top).max(1)))
                    as i32,
        }
    }
}
fn coordinate(value: LPARAM) -> (i32, i32) {
    (
        (value as u32 as u16 as i16) as i32,
        ((value as u32 >> 16) as u16 as i16) as i32,
    )
}
fn geometry(hwnd: HWND, c: &Curve) -> Geometry {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut r);
    }
    Geometry::new(r, c)
}
fn interact(hwnd: HWND, x: i32, y: i32) {
    let result = mutate(hwnd, |c| {
        let mut p = geometry(hwnd, c).data(x, y);
        if c.kind == Kind::Gpu {
            p.x = c.points[c.selected].x;
        }
        c.set(c.selected, p, true)
    });
    match result {
        Ok(true) => notify(hwnd, 1),
        Ok(false) => {}
        Err(e) => {
            if let Some(s) = unsafe { state(hwnd) } {
                *s.error.borrow_mut() = e;
            }
            notify(hwnd, 2);
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Pointer belongs to this canvas; messages run on its creating UI thread.
    unsafe {
        if message == WM_NCCREATE {
            let c = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, c.lpCreateParams as isize);
            (*(c.lpCreateParams as *const State)).ownership.set(1);
        }
        let Some(s) = state(hwnd) else {
            return DefWindowProcW(hwnd, message, w, l);
        };
        match message {
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let memory = CreateCompatibleDC(dc);
                let bitmap = CreateCompatibleBitmap(dc, rect.right.max(1), rect.bottom.max(1));
                if !memory.is_null() && !bitmap.is_null() {
                    let old = SelectObject(memory, bitmap);
                    draw(
                        memory,
                        rect,
                        s.model.borrow().as_ref(),
                        s.font,
                        GetFocus() == hwnd,
                    );
                    BitBlt(dc, 0, 0, rect.right, rect.bottom, memory, 0, 0, SRCCOPY);
                    SelectObject(memory, old);
                } else {
                    draw(
                        dc,
                        rect,
                        s.model.borrow().as_ref(),
                        s.font,
                        GetFocus() == hwnd,
                    );
                }
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                if !memory.is_null() {
                    DeleteDC(memory);
                }
                EndPaint(hwnd, &ps);
                0
            }
            WM_ERASEBKGND => 1,
            WM_SETFOCUS | WM_KILLFOCUS | WM_SIZE | WM_ENABLE => {
                InvalidateRect(hwnd, ptr::null(), 0);
                0
            }
            WM_GETDLGCODE => DLGC_WANTARROWS as isize,
            WM_LBUTTONDOWN => {
                SetFocus(hwnd);
                let (x, y) = coordinate(l);
                let near = {
                    let mut model = s.model.borrow_mut();
                    if let Some(c) = model.as_mut() {
                        let g = geometry(hwnd, c);
                        let (i, d) = c
                            .points
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                let p = g.screen(*p);
                                let d = i64::from(p.x - x).pow(2) + i64::from(p.y - y).pow(2);
                                (i, d)
                            })
                            .min_by_key(|p| p.1)
                            .unwrap();
                        c.selected = i;
                        d <= 225
                    } else {
                        false
                    }
                };
                if near {
                    s.drag.set(true);
                    SetCapture(hwnd);
                }
                refresh(hwnd);
                notify(hwnd, 0);
                0
            }
            WM_MOUSEMOVE => {
                if s.drag.get() {
                    let (x, y) = coordinate(l);
                    interact(hwnd, x, y);
                }
                0
            }
            WM_LBUTTONUP => {
                if s.drag.replace(false) {
                    ReleaseCapture();
                }
                0
            }
            WM_CAPTURECHANGED => {
                s.drag.set(false);
                0
            }
            WM_KEYDOWN => {
                let shift = GetKeyState(0x10) < 0;
                let ctrl = GetKeyState(0x11) < 0;
                let step = if shift { 10 } else { 1 };
                let result = mutate(hwnd, |c| {
                    if w == 0x24 {
                        c.selected = 0;
                        return Ok(false);
                    }
                    if w == 0x23 {
                        c.selected = c.points.len() - 1;
                        return Ok(false);
                    }
                    if (w == 0x25 || w == 0x27) && !ctrl {
                        c.selected = if w == 0x25 {
                            c.selected.saturating_sub(1)
                        } else {
                            (c.selected + 1).min(c.points.len() - 1)
                        };
                        return Ok(false);
                    }
                    let mut p = c.points[c.selected];
                    match w {
                        0x26 => p.y += step,
                        0x28 => p.y -= step,
                        0x25 if c.kind != Kind::Gpu => p.x -= step,
                        0x27 if c.kind != Kind::Gpu => p.x += step,
                        0x2d if c.kind != Kind::Gpu => {
                            c.insert()?;
                            return Ok(true);
                        }
                        0x2e if c.kind != Kind::Gpu => {
                            c.remove()?;
                            return Ok(true);
                        }
                        _ => return Ok(false),
                    }
                    c.set(c.selected, p, true)
                });
                match result {
                    Ok(changed) => notify(hwnd, usize::from(changed)),
                    Err(e) => {
                        *s.error.borrow_mut() = e;
                        notify(hwnd, 2);
                    }
                }
                0
            }
            WM_NCDESTROY => {
                s.ownership.set(2);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let p = s as *const State as *mut State;
                drop(Box::from_raw(p));
                DefWindowProcW(hwnd, message, w, l)
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}
pub fn create(parent: HWND, id: usize, font: HFONT) -> Result<HWND, String> {
    // SAFETY: Class text is copied by Windows; Box remains owned by the canvas.
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let name = wide("OmenHubPPCurveCanvas");
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(procedure);
        class.hInstance = instance;
        class.lpszClassName = name.as_ptr();
        class.hCursor = LoadCursorW(ptr::null_mut(), IDC_CROSS);
        if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let ownership = Rc::new(Cell::new(0));
        let boxed = Box::into_raw(Box::new(State {
            model: RefCell::new(None),
            font,
            drag: Cell::new(false),
            error: RefCell::new(String::new()),
            ownership: ownership.clone(),
        }));
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Kurvenansicht").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0,
            0,
            400,
            250,
            parent,
            id as HMENU,
            instance,
            boxed.cast(),
        );
        if hwnd.is_null() {
            if ownership.get() == 0 {
                drop(Box::from_raw(boxed));
            }
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(hwnd)
    }
}
fn blend(a: u32, b: u32, percent: u32) -> u32 {
    let mut color = 0;
    for shift in [0, 8, 16] {
        let x = (a >> shift) & 255;
        let y = (b >> shift) & 255;
        color |= ((x * (100 - percent) + y * percent) / 100) << shift;
    }
    color
}
unsafe fn label(dc: HDC, x: i32, y: i32, value: &str) {
    let s = wide(value);
    unsafe {
        TextOutW(dc, x, y, s.as_ptr(), (s.len() - 1) as i32);
    }
}
unsafe fn width(dc: HDC, value: &str) -> i32 {
    let s = wide(value);
    let mut size: SIZE = unsafe { std::mem::zeroed() };
    unsafe {
        GetTextExtentPoint32W(dc, s.as_ptr(), (s.len() - 1) as i32, &mut size);
    }
    size.cx
}
unsafe fn draw(dc: HDC, rect: RECT, curve: Option<&Curve>, font: HFONT, focus: bool) {
    // SAFETY: A living window or owned memory DC; every selected object is restored.
    unsafe {
        let saved = SaveDC(dc);
        let bg = GetSysColor(COLOR_WINDOW);
        let fg = GetSysColor(COLOR_WINDOWTEXT);
        let accent = GetSysColor(COLOR_HIGHLIGHT);
        let brush = CreateSolidBrush(bg);
        FillRect(dc, &rect, brush);
        DeleteObject(brush);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, fg);
        SelectObject(
            dc,
            if font.is_null() {
                GetStockObject(DEFAULT_GUI_FONT)
            } else {
                font
            },
        );
        if let Some(curve) = curve {
            let g = Geometry::new(rect, curve);
            let grid = CreatePen(PS_SOLID, 1, blend(bg, fg, 16));
            let old = SelectObject(dc, grid);
            let ystep = if curve.kind == Kind::Gpu {
                if g.hi.y <= 2000 { 500 } else { 1000 }
            } else {
                25
            };
            for y in (g.lo.y..=g.hi.y).step_by(ystep) {
                let point = g.screen(DataPoint { x: g.lo.x, y });
                MoveToEx(dc, g.area.left, point.y, ptr::null_mut());
                LineTo(dc, g.area.right, point.y);
                let value = y.to_string();
                label(
                    dc,
                    g.area.left - 10 - width(dc, &value),
                    point.y - 9,
                    &value,
                );
            }
            let mut xticks = if curve.kind == Kind::Gpu {
                let start = ((g.lo.x + 99) / 100) * 100;
                (start..=g.hi.x).step_by(200).collect::<Vec<_>>()
            } else {
                (g.lo.x..=g.hi.x).step_by(25).collect::<Vec<_>>()
            };
            xticks.extend([g.lo.x, g.hi.x]);
            xticks.sort_unstable();
            xticks.dedup();
            for x in xticks {
                let point = g.screen(DataPoint { x, y: g.lo.y });
                MoveToEx(dc, point.x, g.area.top, ptr::null_mut());
                LineTo(dc, point.x, g.area.bottom);
                let value = x.to_string();
                label(
                    dc,
                    point.x - width(dc, &value) / 2,
                    g.area.bottom + 10,
                    &value,
                );
            }
            SelectObject(dc, old);
            DeleteObject(grid);
            let (x, y) = curve.kind.units();
            label(dc, 14, 8, y);
            label(
                dc,
                (g.area.left + g.area.right - width(dc, x)) / 2,
                rect.bottom - 25,
                x,
            );
            let points: Vec<_> = curve.points.iter().map(|p| g.screen(*p)).collect();
            let pen = CreatePen(PS_SOLID, 2, accent);
            let old_pen = SelectObject(dc, pen);
            let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
            Polyline(dc, points.as_ptr(), points.len() as i32);
            for p in &points {
                Ellipse(dc, p.x - 2, p.y - 2, p.x + 3, p.y + 3);
            }
            let p = points[curve.selected];
            let fill = CreateSolidBrush(accent);
            SelectObject(dc, fill);
            Ellipse(dc, p.x - 7, p.y - 7, p.x + 8, p.y + 8);
            SelectObject(dc, old_brush);
            DeleteObject(fill);
            SelectObject(dc, old_pen);
            DeleteObject(pen);
        } else {
            label(
                dc,
                24,
                30,
                "Kurve nicht verfügbar. Lüfter-Entwürfe können weiterhin bearbeitet werden.",
            );
        }
        if focus {
            let mut r = rect;
            r.left += 3;
            r.top += 3;
            r.right -= 3;
            r.bottom -= 3;
            DrawFocusRect(dc, &r);
        }
        RestoreDC(dc, saved);
    }
}
/// Render our widget code to an owned DIB, never capture a desktop or other app.
pub fn render(path: &Path, curve: &Curve) -> Result<(), String> {
    unsafe {
        let w = 960_i32;
        let h = 460_i32;
        let dc = CreateCompatibleDC(ptr::null_mut());
        if dc.is_null() {
            return Err("Kein Render-Kontext verfügbar.".into());
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits = ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            return Err("Render-Bitmap fehlt.".into());
        }
        let old = SelectObject(dc, bitmap);
        let mut font_info: LOGFONTW = std::mem::zeroed();
        font_info.lfHeight = -18;
        for (a, b) in font_info.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
            *a = b;
        }
        let font = CreateFontIndirectW(&font_info);
        draw(
            dc,
            RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            },
            Some(curve),
            font,
            false,
        );
        GdiFlush();
        let count = (w * h * 4) as usize;
        let mut data = vec![];
        data.extend(b"BM");
        data.extend(((54 + count) as u32).to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(54u32.to_le_bytes());
        data.extend(40u32.to_le_bytes());
        data.extend(w.to_le_bytes());
        data.extend((-h).to_le_bytes());
        data.extend(1u16.to_le_bytes());
        data.extend(32u16.to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend((count as u32).to_le_bytes());
        data.extend([0u8; 16]);
        data.extend(std::slice::from_raw_parts(bits as *const u8, count));
        SelectObject(dc, old);
        if !font.is_null() {
            DeleteObject(font);
        }
        DeleteObject(bitmap);
        DeleteDC(dc);
        std::fs::write(path, data).map_err(|e| e.to_string())
    }
}
