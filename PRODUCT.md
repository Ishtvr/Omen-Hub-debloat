# Omen Hub++

<!-- impeccable:product-schema 1 -->

## Platform

Native Windows desktop (Win32, x64). This is not web, iOS or Android. The user's explicit Windows requirement takes precedence over the Impeccable platform enum; apply its Operate guidance to Windows conventions.

## Users

Owners of HP OMEN gaming laptops who want to check GPU performance and temperatures, then switch system profiles. The user confirmed this opening task on 13 October 2026 and explicitly required CPU performance alongside it in the same first view.

## Product Purpose

A small, native, open-source replacement for OMEN Gaming Hub combining relevant HP system controls with CPU/APU tuning in one tool. The intended end state includes performance profiles, supported OEM GPU power/Dynamic Boost, power limits, cooling, MUX and keyboard lighting.

The user clarified on 13 October 2026 that this must be a complete replacement: the target engine must operate without the original OMEN Hub and its background hardware controllers. Their DLLs and RPC services are analysis sources, not required runtime components for the target product. Necessary behavior such as hotkeys, state restoration and power transitions must be implemented independently.

## Positioning

HP-specific system control with no advertising, overlay, account requirement, runtime telemetry, background updater or network dependency. G-Helper is the user's functional comparison; UXTU defines the requested tuning breadth. Neither establishes an approved visual identity.

## Operating Context

Check GPU wattage and temperature together with CPU performance first; profile switching follows. Both processors must be findable immediately, without opening a details page. A tray application handles background presence. The reference device is OMEN 16-ap0xxx, board 8E35, BIOS F.13, Ryzen 9 8940HX and RTX 5070 Laptop GPU.

## Capabilities and Constraints

- Native Rust and Windows-first; technical names `omen-hub-plus-plus` and `omen-hub-pp`.
- **50 MB working-set budget in tray idle**, raised explicitly by the user on 13 October 2026. Measure the release build and distinguish MB from MiB in records.
- Current implementation reads Windows inventory, power source/battery, public NVIDIA telemetry, CPU busy-time utilization and explicitly qualified Windows-reported CPU MHz. A separate, explicitly approved CPU hardware reader provides package-energy mean watts and labelled Tctl on the exact qualified device. No fabricated sensor values.
- CPU package watts and **Tctl** are validated only on **HP8E35/F.13/Ryzen9 8940HX** with the already installed/running, exact-hash **PawnIO 2.2.0** driver and signed **AMDFamily17 0.2.11** module. Wattage is an energy-counter mean over the displayed measured interval; the first energy baseline has no watts. Tctl is control temperature, not a claimed core/CCD temperature. Windows MHz remain separate and are not measured boost/effective clock, which is still planned. Never substitute a configured PPT/PL1 limit, utilization or an unassigned ACPI thermal zone for a measurement. Exact identities and public read contracts: docs/CPU_SENSORS.md.
- The established native report keeps GPU wattage/temperature first and CPU performance immediately after it, before inventory and profile controls. The footer's **CPU-Sensoren …** button requests Windows-UAC through a transient launch process for a session-only read helper. Same-user/SYSTEM access, both process IDs, session token, schema, sequence and bounded messages restrict IPC to **Sample/Pause/Stop**; it accepts no tuners, profiles, register addresses or driver blobs. Tray/suspend releases the hardware reader and baseline while both persistent processes block. Show/resume/refresh/AC changes require a fresh baseline. No driver installation/start or proprietary AMD SDK loading occurs. Existing HP services remained untouched; full replacement is unproven.
- CPU errors discard stale measurements and show German cause/recovery text for build support, device/BIOS, driver identity, UAC, connection and reading failures. Exact technical details persist separately in `last-cpu-error.json` and `cpu-sensor-status`, without changing hardware recovery journals or startup approvals; this last-error record is not live session status.
- Optional CPU support is staged only at build time by `scripts/prepare-cpu-module.ps1` from the signed public PawnIO.Modules 0.2.11 release. The embedded module is LGPL-2.1-or-later; corresponding source, full source archive and `COPYING` accompany it in `dist/third-party/PawnIO.Modules/`. Original Rust remains MIT. No driver is bundled and runtime stays offline; unstaged builds report CPU hardware values unavailable.
- GPU core/VRAM MHz offsets have public NVML setters with measured readback and original-state restoration on 8E35/F.13, RTX 5070 Laptop, driver 617.14 and VBIOS 98.06.2a.40.da. The native GPU editor separates save, apply and restore; apply/restore use fixed UAC children. Arbitrary offsets are not load-stability certified.
- Complete Graphics V/F frequency control is implemented on the exact hashed NVIDIA driver: 127 immutable voltage anchors, complete original control capture and restore, native plateau editing and save-only persistence. It supports frequency-at-voltage tuning, without claiming a hard physical voltage ceiling or measured load stability. The memory setter's measured curve-reset side effect is handled by dependency order and full Graphics preservation.
- The native curve workbench opens a dark, chart-focused window matching the user's MSI Afterburner reference. Its full GPU grid keeps all 127 voltage anchors immutable. Square draft points, a dotted loaded reference, selected guides labelled **Ausgewählt**, a contiguous selection band and adaptive axes distinguish editing state from measured state. Numeric/profile controls open with **Profil / Werte …**.
- GPU editing supports loaded-baseline Shift offsets, current-draft Alt offsets, Ctrl tilting around the opposite endpoint, range selection/flattening, 64-step undo/redo and zoom/pan/fit. Separate CPU/GPU fan drafts retain their own editing history; fan points can be added/removed. Editing/saving writes metadata only and preserves other profile settings. Fan hardware application, fixed voltage/clock lock and OC Scanner remain unsupported; a saved fan setting blocks the entire hardware apply.
- Custom profiles contain 85 typed feature keys. Core/memory offsets, complete Graphics V/F curves and the qualified four-zone keyboard RGB/static-versus-firmware mode are writable; unsupported settings block the entire apply. Profiles without NVIDIA settings, including lighting-only profiles, do not initialize a discrete GPU session for identity checks. HP built-in modes remain previews, with real Unleashed, direct voltage, power, fan and MUX writes still pending.
- HP GPU-status GET is limited to 8E35/F.13. Firmware returns [1,1,0,87] while the user reports Unleashed in the original Hub. Known flags are decoded; D-state 0 remains explicitly unknown. Fresh system-design GET reports a 230 W shipping adapter and Extreme-support bit, without claiming the current performance mode or power limits.
- Profile data and durable recovery journals live in the Windows token's user profile under .omen-hub-pp, independent of MSIX AppData virtualization. A protected, fixed per-user logon task and protected approval support the exact previously applied profile revision. Task execution was tested with zero offsets; no actual reboot occurred. The current automatic selection is disabled.
- Native `keyboard-read` and the Windows PowerShell 5.1 acquisition script successfully read capability and complete legacy color/brightness state on 8E35/F.13: FourZoneWithNumpad, capability [1,0,0,0], matching 128-byte state blocks, brightness retained as a raw byte. Normal processes receive AccessDenied. A direct, typed HP BIOS adapter writes only the twelve RGB bytes and the exact 0x64/0xE4 static/custom-versus-firmware-preset mode. Short trials and separate durable apply/recovery processes on 14 October 2026 verified target readback, full original restoration and unchanged GPU controls. This evidence does not establish reboot/resume/hotkey behavior, physical animation or service-independent replacement; original HP controllers still ran.
- The native four-zone keyboard editor uses standard Win32 controls, RGB hex fields, Windows color pickers and draft swatches. It separates metadata save, explicit UAC firmware check, whole-profile apply and restore of the selected profile's recorded original transaction. Saving preserves unrelated profile settings and detects conflicting lighting edits. Firmware labels show only the latest explicit GET snapshot; profile reload/save discards it, and firmware check or any potentially mutating attempt clears it before dispatch. Unsupported CPU/Windows/fan/MUX/Unleashed settings still block the entire profile. Keyboard color and mode capabilities remain `startup_safe = false`; brightness percentages, off/animated/per-key writes and automatic lighting reapplication are not implemented.
- Reads/unsupported states and future controllable hardware states must be clearly distinguished. No promise that a 115 W driver constraint is measured sustained performance.

## Brand Commitments

The product name is **Omen Hub++**. German is the present UI language. No official HP endorsement, copied logo or proprietary assets are implied. The user expressly requested UI-UX Pro Max and the installed Impeccable v4.3.1 for UI work, with no plugin update in this session.

## Evidence on Hand

README.md, docs/CPU_SENSORS.md, docs/CURVE_EDITOR.md, docs/GPU_PROFILES.md, docs/KEYBOARD_UI.md, docs/KEYBOARD_LIGHTING.md, docs/ROADMAP.md, docs/REVERSE_ENGINEERING.md, docs/models/8e35.md and docs/VALIDATION.md. Local sensor/probe records remain ignored. There are 77 passing Rust tests (57 core, 16 platform and four app tests), with successful format, Clippy and release checks. All five native UI smokes pass on the final CPU release. Actual negative/positive offset and full-curve readback/restore, persisted profile recovery and scheduled-entry validation remain release-scoped in VALIDATION.md. The UI smokes perform no hardware parameter writes. The curve smoke covers all-127 Shift editing, range flattening, undo/redo, zoom/fit, numerical/fan edits and save/reload without changing programmed GPU controls. A fresh visual reviewer returned **SHIP** for the curve charts and pictured client controls at the captured large/small sizes; the labelled example is unsaved and unapplied. Titlebar, full numeric view, multiple DPI settings and comprehensive accessibility review remain outside that verdict. The keyboard native render at 764×741 shows pictured layout, labels, buttons and draft swatches; hidden EDIT/COMBOBOX text, titlebar and other DPI settings are excluded. Its finish verdict scores the single stale-GET correction resolved, without certifying the full window or DPI behavior. Keyboard hardware validation used isolated durable profile storage for the separate apply/recovery proof and left production journal/startup hashes unchanged. CPU hardware and actual UI/UAC probes confirm an absent-watts first baseline, mean watts with measured interval, labelled Tctl, explicit off/on and fresh baseline after tray and simulated suspend/resume handlers, with unchanged checked GPU/HWCR controls. These are development samples, not gaming/idle/stability benchmarks or real standby/reboot tests. The CPU finish reviewer scored the single German cause/recovery-text correction **resolved**, disposition **ship**, with valid own-client 790×633 and 790×153 footer recaptures. Active/error-state visual rendering, alternate DPI and comprehensive accessibility remain outside that verdict. Final CPU-release cold-tray and combined UI/helper measurements after two tray-return cycles are recorded in VALIDATION.md; earlier release measurements must not be reused as this build's result. No GPU-load benchmark proves full Unleashed, physical undervolting effect or overclock stability yet.

## Product Principles

1. Show actual state before offering control.
2. Make support and unavailable states understandable without guessing values.
3. Preserve a fast, native, offline interaction and low idle cost.
4. Validate hardware behavior per board/BIOS before exposing writes.
5. Keep the tray useful and quiet; avoid background work without purpose.

## Open Decisions

Visual direction for the first dashboard remains open. The curve editor has a confirmed dark MSI Afterburner reference, scoped to that surface; it establishes no global palette or design world. The user confirmed a code-first workflow, recorded in `.impeccable/config.json`.
