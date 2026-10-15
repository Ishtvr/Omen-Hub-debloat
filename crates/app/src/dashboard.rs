//! Native dashboard presentation. It never reads or changes hardware.
use crate::theme::{self, Theme};
use omen_core::{CpuHardwareStatus, Snapshot};
use std::ptr;
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn text(hwnd: HWND, id: i32, value: &str) {
    unsafe {
        SetWindowTextW(GetDlgItem(hwnd, id), wide(value).as_ptr());
    }
}
unsafe fn metric(hwnd: HWND, id: i32, t: &Theme, value: Option<String>) {
    unsafe {
        let c = GetDlgItem(hwnd, id);
        SendMessageW(
            c,
            WM_SETFONT,
            if value.is_some() { t.metric } else { t.body } as usize,
            1,
        );
        SetWindowTextW(
            c,
            wide(value.as_deref().unwrap_or("Nicht verfügbar")).as_ptr(),
        );
    }
}
pub fn cpu_detail(s: &Snapshot) -> String {
    // Measured values speak for themselves; only states that need attention
    // get a line.
    match &s.cpu_hardware_status {
        CpuHardwareStatus::Off => "CPU-Sensoren nicht aktiv. Aktualisieren startet sie.".into(),
        CpuHardwareStatus::Starting => "Windows-Freigabe wird angefordert …".into(),
        CpuHardwareStatus::Paused | CpuHardwareStatus::Active => String::new(),
        CpuHardwareStatus::Error(e) => crate::ui::cpu_error_message(e).into(),
    }
}
pub unsafe fn create(hwnd: HWND, t: &Theme) {
    unsafe {
        let control = |id: i32,
                       class: &str,
                       label: &str,
                       style: u32,
                       font: HFONT,
                       panel: bool,
                       muted: bool| {
            let h = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                0,
                0,
                10,
                10,
                hwnd,
                id as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            );
            SendMessageW(h, WM_SETFONT, font as usize, 0);
            theme::prepare(h, class, style);
            if panel {
                theme::surface(h);
            }
            if muted {
                theme::muted(h);
            }
            h
        };
        let label =
            |id, value, font, panel, muted| control(id, "STATIC", value, 0, font, panel, muted);
        label(120, "Omen Hub++", t.title, false, false);
        label(121, "Systemübersicht", t.body, false, true);
        label(122, "Versorgung wird gelesen …", t.body, false, true);
        for (id, value, font, panel, muted) in [
            (130, "GPU · NVIDIA", t.heading, true, false),
            (131, "GPU wird erkannt …", t.small, true, true),
            (132, "Leistungsaufnahme", t.body, true, true),
            (133, "Nicht verfügbar", t.body, true, false),
            (134, "Temperatur", t.body, true, true),
            (135, "Nicht verfügbar", t.body, true, false),
            (136, "GPU-Takt: Nicht verfügbar", t.body, true, false),
            (140, "CPU", t.heading, true, false),
            (139, "CPU wird erkannt …", t.small, true, true),
            (147, "CPU-Paketleistung", t.body, true, true),
            (141, "Nicht verfügbar", t.body, true, false),
            (148, "CPU-Temperatur (Tctl)", t.body, true, true),
            (142, "Nicht verfügbar", t.body, true, false),
            (144, "Auslastung: Wird gemessen", t.body, true, false),
            (145, "Windows-Takt: Nicht verfügbar", t.small, true, true),
            (146, "", t.small, true, true),
            (150, "Steuerung", t.heading, false, false),
            (160, "System", t.heading, false, false),
            (161, "Leistungsmodus", t.body, false, false),
            (
                181,
                "BIOS-Modus nicht lesbar; Auswahl wendet ihn an.",
                t.small,
                false,
                true,
            ),
        ] {
            label(id, value, font, panel, muted);
        }
        control(
            101,
            "BUTTON",
            "Aktualisieren",
            WS_TABSTOP,
            t.body,
            false,
            false,
        );
        control(
            103,
            "BUTTON",
            "Leistungsprofile",
            WS_TABSTOP,
            t.body,
            false,
            false,
        );
        control(
            104,
            "BUTTON",
            "Lüfterkurve",
            WS_TABSTOP,
            t.body,
            false,
            false,
        );
        control(
            105,
            "BUTTON",
            "Beleuchtung",
            WS_TABSTOP,
            t.body,
            false,
            false,
        );
        let combo = control(
            102,
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32,
            t.body,
            false,
            false,
        );
        for value in [
            "Quiet (Default)",
            "Balanced (Default)",
            "Performance (Default)",
            "Unleashed (Default)",
        ] {
            SendMessageW(combo, CB_ADDSTRING, 0, wide(value).as_ptr() as isize);
        }
        SendMessageW(combo, CB_SETCURSEL, 1, 0);
        control(
            180,
            "EDIT",
            "Hardware wird erkannt …",
            ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32 | WS_VSCROLL,
            t.small,
            false,
            false,
        );
        layout(hwnd);
    }
}
/// Lower edge of the GPU and CPU panels, shared by layout and painting.
const PANEL_BOTTOM: i32 = 380;
pub unsafe fn layout(hwnd: HWND) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let width = r.right;
        let margin = 28;
        let gap = 20;
        let panel = (width - 2 * margin - gap) / 2;
        let right = margin + panel + gap;
        let content = panel - 48;
        let mut boxes = vec![
            (120, [28, 22, width - 240, 38]),
            (121, [30, 64, width - 260, 24]),
            (122, [width - 420, 66, 230, 24]),
            (101, [width - 178, 56, 150, 36]),
        ];
        for (base, x) in [(0, margin), (10, right)] {
            let title = if base == 0 { 130 } else { 140 };
            let name = if base == 0 { 131 } else { 139 };
            let power_label = if base == 0 { 132 } else { 147 };
            let power = if base == 0 { 133 } else { 141 };
            let temp_label = if base == 0 { 134 } else { 148 };
            let temp = if base == 0 { 135 } else { 142 };
            let half = (content - 20) / 2;
            boxes.extend([
                (title, [x + 24, 126, content, 28]),
                (name, [x + 24, 163, content, 24]),
                (power_label, [x + 24, 198, half, 22]),
                (power, [x + 24, 224, half, 50]),
                (temp_label, [x + 24 + half + 20, 198, half, 22]),
                (temp, [x + 24 + half + 20, 224, half, 50]),
            ]);
        }
        boxes.extend([
            (136, [margin + 24, 290, content, 24]),
            (144, [right + 24, 290, content, 24]),
            (145, [right + 24, 318, content, 22]),
            (146, [right + 24, 344, content, 26]),
            (150, [28, PANEL_BOTTOM + 24, width - 56, 30]),
        ]);
        let action = (width - 56 - 32) / 3;
        for (i, id) in [103, 104, 105].iter().enumerate() {
            let x = 28 + i as i32 * (action + 16);
            boxes.push((*id, [x, PANEL_BOTTOM + 65, action, 44]));
        }
        // Content stays top-aligned; a taller window only adds space below it.
        let bottom = PANEL_BOTTOM + 150;
        boxes.extend([
            (160, [28, bottom, panel, 28]),
            (180, [28, bottom + 35, panel - 8, 74]),
            (161, [right, bottom, panel, 28]),
            (102, [right, bottom + 34, panel, 220]),
            (181, [right, bottom + 78, panel, 44]),
        ]);
        for (id, b) in boxes {
            MoveWindow(GetDlgItem(hwnd, id), b[0], b[1], b[2], b[3], 1);
        }
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}
pub unsafe fn paint(hwnd: HWND, t: &Theme, dc: HDC) {
    unsafe {
        t.fill(hwnd, dc);
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let panel = (r.right - 76) / 2;
        let old = SelectObject(dc, t.panel);
        let pen = CreatePen(
            PS_SOLID,
            1,
            if t.high_contrast {
                t.text()
            } else {
                theme::PANEL
            },
        );
        let previous = SelectObject(dc, pen);
        for x in [28, 48 + panel] {
            RoundRect(dc, x, 108, x + panel, PANEL_BOTTOM, 24, 24);
        }
        SelectObject(dc, old);
        SelectObject(dc, previous);
        DeleteObject(pen);
    }
}
pub unsafe fn update(hwnd: HWND, t: &Theme, s: &Snapshot) {
    unsafe {
        let gpu = s.gpu.as_ref();
        text(
            hwnd,
            131,
            gpu.map_or("Nicht verfügbar", |g| g.name.as_str()),
        );
        metric(
            hwnd,
            133,
            t,
            gpu.and_then(|g| g.power_w).map(|v| format!("{v:.1} W")),
        );
        metric(
            hwnd,
            135,
            t,
            gpu.and_then(|g| g.temperature_c).map(|v| format!("{v} °C")),
        );
        text(
            hwnd,
            136,
            &format!(
                "GPU-Takt: {}",
                gpu.and_then(|g| g.clock_mhz)
                    .map_or("Nicht verfügbar".into(), |v| format!("{v} MHz"))
            ),
        );
        text(
            hwnd,
            139,
            s.inventory
                .cpus
                .first()
                .map_or("Nicht verfügbar", String::as_str),
        );
        let cpu = s.cpu.as_ref();
        metric(
            hwnd,
            141,
            t,
            cpu.and_then(|c| c.package_power_w)
                .map(|v| format!("{v:.1} W")),
        );
        metric(
            hwnd,
            142,
            t,
            cpu.and_then(|c| c.temperature_c)
                .map(|v| format!("{v:.1} °C")),
        );
        if matches!(s.cpu_hardware_status, CpuHardwareStatus::Active)
            && cpu.is_some_and(|c| c.hardware.is_some() && c.package_power_w.is_none())
        {
            text(hwnd, 141, "Wird gemessen");
        }
        text(
            hwnd,
            144,
            &format!(
                "Auslastung: {}",
                cpu.and_then(|c| c.busy_percent)
                    .map_or("Wird gemessen / nicht verfügbar".into(), |v| format!(
                        "{v:.1} %"
                    ))
            ),
        );
        text(
            hwnd,
            145,
            &format!(
                "Windows-Takt: {}",
                cpu.and_then(|c| c.windows_clock_mhz)
                    .map_or("Nicht verfügbar".into(), |v| format!("{v} MHz"))
            ),
        );
        theme::error(
            GetDlgItem(hwnd, 146),
            matches!(s.cpu_hardware_status, CpuHardwareStatus::Error(_)),
        );
        text(hwnd, 146, &cpu_detail(s));
        let ac = match s.power.ac_connected {
            Some(true) => "Netzbetrieb",
            Some(false) => "Akkubetrieb",
            None => "Versorgung unbekannt",
        };
        text(
            hwnd,
            122,
            &format!(
                "{ac} · Akku {}",
                s.power
                    .battery_percent
                    .map_or("unbekannt".into(), |v| format!("{v} %"))
            ),
        );
        text(
            hwnd,
            180,
            &format!(
                "{} {}\r\nBoard: {} · BIOS: {}\r\nHP-WMI: {}",
                s.inventory.manufacturer,
                s.inventory.model,
                s.inventory.board,
                s.inventory.bios,
                if s.inventory.hp_bios_interface_present {
                    "vorhanden"
                } else {
                    "nicht verfügbar"
                }
            ),
        );
    }
}
