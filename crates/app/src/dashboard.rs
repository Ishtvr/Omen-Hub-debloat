//! Native dashboard presentation. It never reads or changes hardware.
use crate::{
    i18n::{t, tf},
    theme::{self, Theme},
};
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
        SetWindowTextW(
            GetDlgItem(hwnd, id),
            wide(&crate::i18n::message(value)).as_ptr(),
        );
    }
}
unsafe fn metric(hwnd: HWND, id: i32, th: &Theme, value: Option<String>) {
    unsafe {
        let c = GetDlgItem(hwnd, id);
        SendMessageW(
            c,
            WM_SETFONT,
            if value.is_some() { th.metric } else { th.body } as usize,
            1,
        );
        SetWindowTextW(
            c,
            wide(value.as_deref().unwrap_or(t("common.unavailable"))).as_ptr(),
        );
    }
}
pub fn cpu_detail(s: &Snapshot) -> String {
    match &s.cpu_hardware_status {
        CpuHardwareStatus::Off => t("dash.cpu_off").into(),
        CpuHardwareStatus::Starting => t("dash.cpu_starting").into(),
        CpuHardwareStatus::Paused => String::new(),
        CpuHardwareStatus::Active => {
            let Some(hardware) = s.cpu.as_ref().and_then(|c| c.hardware.as_ref()) else {
                return String::new();
            };
            if let Some(clocks) = &hardware.effective_clocks {
                let interval = clocks
                    .processors
                    .iter()
                    .map(|p| p.interval_ms)
                    .max()
                    .unwrap_or(0);
                tf(
                    "dash.effective_clock_interval",
                    &[
                        &clocks.logical_processor_count.to_string(),
                        &crate::i18n::number(&format!("{:.1}", interval as f64 / 1000.0)),
                    ],
                )
            } else if hardware
                .warnings
                .iter()
                .any(|w| w.starts_with("CPU effective clock"))
            {
                t("dash.effective_clock_unavailable").into()
            } else {
                t("dash.effective_clock_measuring").into()
            }
        }
        CpuHardwareStatus::Error(e) => crate::ui::cpu_error_message(e).into(),
    }
}
pub fn cpu_load_line(s: &Snapshot) -> String {
    let cpu = s.cpu.as_ref();
    let load = cpu
        .and_then(|c| c.busy_percent)
        .map_or(t("dash.load_unavailable").into(), |v| {
            format!("{} %", crate::i18n::number(&format!("{v:.1}")))
        });
    let clocks = matches!(s.cpu_hardware_status, CpuHardwareStatus::Active)
        .then(|| {
            cpu.and_then(|c| c.hardware.as_ref())
                .and_then(|h| h.effective_clocks.as_ref())
        })
        .flatten();
    if let Some(mhz) = clocks.and_then(|c| c.max_mhz()) {
        tf("dash.effective_clock_load", &[&format!("{mhz:.0}"), &load])
    } else {
        tf("dash.load", &[&load])
    }
}
pub unsafe fn create(hwnd: HWND, th: &Theme) {
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
        label(120, "Omen Hub++", th.title, false, false);
        label(121, t("dash.subtitle"), th.body, false, true);
        label(122, t("dash.power_reading"), th.body, false, true);
        for (id, value, font, panel, muted) in [
            (130, t("dash.gpu_title"), th.heading, true, false),
            (131, t("dash.gpu_detecting"), th.small, true, true),
            (132, t("dash.power_draw"), th.body, true, true),
            (133, t("common.unavailable"), th.body, true, false),
            (134, t("dash.temperature"), th.body, true, true),
            (135, t("common.unavailable"), th.body, true, false),
            (136, t("dash.gpu_clock_unavailable"), th.body, true, false),
            (140, t("dash.cpu_title"), th.heading, true, false),
            (139, t("dash.cpu_detecting"), th.small, true, true),
            (147, t("dash.cpu_package_power"), th.body, true, true),
            (141, t("common.unavailable"), th.body, true, false),
            (148, t("dash.cpu_temperature"), th.body, true, true),
            (142, t("common.unavailable"), th.body, true, false),
            (144, t("dash.load_measuring"), th.body, true, false),
            (
                145,
                t("dash.windows_clock_unavailable"),
                th.small,
                true,
                true,
            ),
            (146, "", th.small, true, true),
            (150, t("dash.controls"), th.heading, false, false),
            (160, t("dash.system"), th.heading, false, false),
            (161, t("dash.mode"), th.body, false, false),
            (181, t("mode.bios_unreadable"), th.small, false, true),
        ] {
            label(id, value, font, panel, muted);
        }
        // Explains the four built-in modes (owner-drawn round "i").
        control(
            182,
            "BUTTON",
            "i",
            WS_TABSTOP | BS_OWNERDRAW as u32,
            th.small,
            false,
            false,
        );
        control(
            101,
            "BUTTON",
            t("dash.refresh"),
            WS_TABSTOP,
            th.body,
            false,
            false,
        );
        control(
            103,
            "BUTTON",
            t("preset.title"),
            WS_TABSTOP,
            th.body,
            false,
            false,
        );
        control(
            104,
            "BUTTON",
            t("dash.fan_curve"),
            WS_TABSTOP,
            th.body,
            false,
            false,
        );
        control(
            105,
            "BUTTON",
            t("dash.lighting"),
            WS_TABSTOP,
            th.body,
            false,
            false,
        );
        // Switch captions hug their switch; the restart note sits under "Graphics".
        for (id, value, font) in [
            (151, t("fan.quick.label"), th.body),
            (152, t("mux.label"), th.body),
            (153, "", th.small),
        ] {
            control(
                id,
                "STATIC",
                value,
                windows_sys::Win32::System::SystemServices::SS_RIGHT,
                font,
                false,
                true,
            );
        }
        // Position switches: fans Auto/Curve/Max, graphics Hybrid/GPU only.
        crate::switch::create(
            hwnd,
            106,
            t("fan.quick.label"),
            &[
                t("fan.quick.auto"),
                t("fan.quick.curve"),
                t("fan.quick.max"),
            ],
            th,
        );
        crate::switch::create(
            hwnd,
            107,
            t("mux.label"),
            &[t("mux.hybrid"), t("mux.discrete")],
            th,
        );
        let combo = control(
            102,
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32,
            th.body,
            false,
            false,
        );
        for mode in [
            "mode.quiet",
            "mode.balanced",
            "mode.performance",
            "mode.unleashed",
        ] {
            let value = tf("mode.default_entry", &[&t(mode)]);
            SendMessageW(combo, CB_ADDSTRING, 0, wide(&value).as_ptr() as isize);
        }
        SendMessageW(combo, CB_SETCURSEL, 1, 0);
        // Choosing a mode only selects it; this button applies it.
        control(
            108,
            "BUTTON",
            t("dash.apply"),
            WS_TABSTOP,
            th.body,
            false,
            false,
        );
        for (id, label) in [
            (190, t("settings.autostart")),
            (191, t("settings.close_to_tray")),
            (192, t("settings.hp_service")),
        ] {
            control(
                id,
                "BUTTON",
                label,
                WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                th.body,
                false,
                false,
            );
        }
        // Preset applied automatically without the charger.
        label(193, t("settings.battery"), th.body, false, false);
        control(
            194,
            "COMBOBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            th.body,
            false,
            false,
        );
        // Round "i" after each setting.
        for id in SETTING_INFO {
            control(
                id,
                "BUTTON",
                "i",
                WS_TABSTOP | BS_OWNERDRAW as u32,
                th.small,
                false,
                false,
            );
        }
        control(
            180,
            "EDIT",
            t("dash.hardware_detecting"),
            ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32 | WS_VSCROLL,
            th.small,
            false,
            false,
        );
        layout(hwnd);
    }
}
/// Info buttons of the autostart, tray, HP service and battery settings.
pub const SETTING_INFO: [i32; 4] = [195, 196, 197, 198];
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
            (150, [28, PANEL_BOTTOM + 24, width - 640, 30]),
        ]);
        // Fan and graphics switches, right-aligned on the Controls heading
        // line; their option names sit underneath the stops.
        let switch_top = PANEL_BOTTOM + 18;
        let switch_height = crate::segments::TRACK + crate::segments::LABEL_GAP + 20;
        let caption = |id: i32, value: &str| {
            let label = GetDlgItem(hwnd, id);
            let font = SendMessageW(label, WM_GETFONT, 0, 0) as HFONT;
            crate::native_ui::text_width(label, font, value) + 12
        };
        let mux_width = 172;
        let fan_width = 228;
        let mux_x = width - 28 - mux_width;
        // The restart note sits under the caption, so the column fits both.
        let mux_caption = caption(152, t("mux.label")).max(caption(153, t("mux.restart_note")));
        let fan_x = mux_x - mux_caption - 36 - fan_width;
        let fan_caption = caption(151, t("fan.quick.label"));
        boxes.extend([
            (
                151,
                [fan_x - fan_caption, switch_top + 3, fan_caption - 10, 24],
            ),
            (106, [fan_x, switch_top, fan_width, switch_height]),
            (
                152,
                [mux_x - mux_caption, switch_top + 3, mux_caption - 10, 24],
            ),
            (
                153,
                [mux_x - mux_caption, switch_top + 30, mux_caption - 10, 20],
            ),
            (107, [mux_x, switch_top, mux_width, switch_height]),
        ]);
        let action = (width - 56 - 32) / 3;
        for (i, id) in [103, 104, 105].iter().enumerate() {
            let x = 28 + i as i32 * (action + 16);
            boxes.push((*id, [x, PANEL_BOTTOM + 80, action, 44]));
        }
        // Content stays top-aligned; a taller window only adds space below it.
        let bottom = PANEL_BOTTOM + 165;
        let label = GetDlgItem(hwnd, 161);
        let font = SendMessageW(label, WM_GETFONT, 0, 0) as HFONT;
        let mode_label = crate::native_ui::text_width(label, font, t("dash.mode")) + 4;
        boxes.extend([
            (160, [28, bottom, panel, 28]),
            (180, [28, bottom + 35, panel - 8, 74]),
            (161, [right, bottom, mode_label, 28]),
            (182, [right + mode_label + 6, bottom + 2, 24, 24]),
            (102, [right, bottom + 34, panel - 136, 220]),
            (181, [right, bottom + 78, panel, 44]),
        ]);
        // Settings rows under the system details; each control fits its
        // label and is followed by its info button.
        let settings_top = bottom + 128;
        let fitted = |id: i32, extra: i32| {
            let control = GetDlgItem(hwnd, id);
            let font = SendMessageW(control, WM_GETFONT, 0, 0) as HFONT;
            let mut label = [0u16; 128];
            let n = GetWindowTextW(control, label.as_mut_ptr(), 128).max(0) as usize;
            crate::native_ui::text_width(control, font, &String::from_utf16_lossy(&label[..n]))
                + extra
        };
        let mut x = 28;
        for (id, info) in [(190, 195), (191, 196), (192, 197)] {
            let width = fitted(id, 36);
            boxes.push((id, [x, settings_top, width, 26]));
            boxes.push((info, [x + width + 2, settings_top + 1, 24, 24]));
            x += width + 26 + 32;
        }
        let battery_top = settings_top + 38;
        let label_width = fitted(193, 8);
        boxes.extend([
            (193, [28, battery_top + 4, label_width, 24]),
            (194, [28 + label_width + 8, battery_top, 260, 260]),
            (
                198,
                [28 + label_width + 8 + 260 + 8, battery_top + 3, 24, 24],
            ),
        ]);
        for (id, b) in boxes {
            MoveWindow(GetDlgItem(hwnd, id), b[0], b[1], b[2], b[3], 1);
        }
        // The Apply button matches the closed selector's real height.
        let mut combo: RECT = std::mem::zeroed();
        GetWindowRect(GetDlgItem(hwnd, 102), &mut combo);
        MoveWindow(
            GetDlgItem(hwnd, 108),
            right + panel - 124,
            bottom + 34,
            124,
            (combo.bottom - combo.top).max(30),
            1,
        );
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}
pub unsafe fn paint(hwnd: HWND, th: &Theme, dc: HDC) {
    unsafe {
        th.fill(hwnd, dc);
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let panel = (r.right - 76) / 2;
        let old = SelectObject(dc, th.panel);
        let pen = CreatePen(
            PS_SOLID,
            1,
            if th.high_contrast {
                th.text()
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
pub unsafe fn update(hwnd: HWND, th: &Theme, s: &Snapshot) {
    unsafe {
        let gpu = s.gpu.as_ref();
        text(
            hwnd,
            131,
            gpu.map_or(t("common.unavailable"), |g| g.name.as_str()),
        );
        metric(
            hwnd,
            133,
            th,
            gpu.and_then(|g| g.power_w)
                .map(|v| format!("{} W", crate::i18n::number(&format!("{v:.1}")))),
        );
        metric(
            hwnd,
            135,
            th,
            gpu.and_then(|g| g.temperature_c).map(|v| format!("{v} °C")),
        );
        text(
            hwnd,
            136,
            &tf(
                "dash.gpu_clock",
                &[&gpu
                    .and_then(|g| g.clock_mhz)
                    .map_or(t("common.unavailable").into(), |v| format!("{v} MHz"))],
            ),
        );
        text(
            hwnd,
            139,
            s.inventory
                .cpus
                .first()
                .map_or(t("common.unavailable"), String::as_str),
        );
        let cpu = s.cpu.as_ref();
        metric(
            hwnd,
            141,
            th,
            cpu.and_then(|c| c.package_power_w)
                .map(|v| format!("{} W", crate::i18n::number(&format!("{v:.1}")))),
        );
        metric(
            hwnd,
            142,
            th,
            cpu.and_then(|c| c.temperature_c)
                .map(|v| format!("{} °C", crate::i18n::number(&format!("{v:.1}")))),
        );
        if matches!(s.cpu_hardware_status, CpuHardwareStatus::Active)
            && cpu.is_some_and(|c| c.hardware.is_some() && c.package_power_w.is_none())
        {
            text(hwnd, 141, t("dash.measuring"));
        }
        text(hwnd, 144, &cpu_load_line(s));
        text(
            hwnd,
            145,
            &tf(
                "dash.windows_clock",
                &[&cpu
                    .and_then(|c| c.windows_clock_mhz)
                    .map_or(t("common.unavailable").into(), |v| format!("{v} MHz"))],
            ),
        );
        theme::error(
            GetDlgItem(hwnd, 146),
            matches!(s.cpu_hardware_status, CpuHardwareStatus::Error(_)),
        );
        text(hwnd, 146, &cpu_detail(s));
        let ac = match s.power.ac_connected {
            Some(true) => t("dash.ac"),
            Some(false) => t("dash.battery"),
            None => t("dash.power_unknown"),
        };
        text(
            hwnd,
            122,
            &tf(
                "dash.power_line",
                &[
                    &ac,
                    &s.power
                        .battery_percent
                        .map_or(t("common.unknown").into(), |v| format!("{v} %")),
                ],
            ),
        );
        text(
            hwnd,
            180,
            &tf(
                "dash.inventory",
                &[
                    &s.inventory.manufacturer,
                    &s.inventory.model,
                    &s.inventory.board,
                    &s.inventory.bios,
                    &if s.inventory.hp_bios_interface_present {
                        t("dash.present")
                    } else {
                        t("dash.not_present")
                    },
                ],
            ),
        );
    }
}
