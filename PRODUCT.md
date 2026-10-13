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
- Current implementation reads Windows inventory, power source/battery, public NVIDIA telemetry, CPU busy-time utilization and explicitly qualified Windows-reported CPU MHz. No fabricated sensor values.
- CPU package watts, processor temperature and measured boost/effective clock remain planned until a validated CPU sensor adapter exists. Never substitute a configured PPT/PL1 limit or an unassigned ACPI thermal zone for a measurement.
- GPU core/VRAM MHz offsets have public NVML setters with measured readback and original-state restoration on 8E35/F.13, RTX 5070 Laptop, driver 617.14 and VBIOS 98.06.2a.40.da. The native GPU editor separates save, apply and restore; apply/restore use fixed UAC children. Arbitrary offsets are not load-stability certified.
- Complete Graphics V/F frequency control is implemented on the exact hashed NVIDIA driver: 127 immutable voltage anchors, complete original control capture and restore, native plateau editing and save-only persistence. It supports frequency-at-voltage tuning, without claiming a hard physical voltage ceiling or measured load stability. The memory setter's measured curve-reset side effect is handled by dependency order and full Graphics preservation.
- The native curve workbench edits GPU V/F and separate CPU/GPU fan curves with dragging, keyboard controls and numeric fields. Fan points can be added/removed; other profile settings are preserved. Editing/saving writes metadata only. Fan hardware application remains unsupported, and any such setting blocks the entire hardware apply.
- Custom profiles contain 85 typed feature keys. Core/memory offsets and complete Graphics V/F curves are writable; unsupported settings block the entire apply. CPU/Windows/iGPU-only profiles do not need an awake discrete GPU for identity checks. HP built-in modes remain previews, with real Unleashed, direct voltage, power, fan, MUX and RGB writes still pending.
- HP GPU-status GET is limited to 8E35/F.13. Firmware returns [1,1,0,87] while the user reports Unleashed in the original Hub. Known flags are decoded; D-state 0 remains explicitly unknown. Fresh system-design GET reports a 230 W shipping adapter and Extreme-support bit, without claiming the current performance mode or power limits.
- Profile data and durable recovery journals live in the Windows token's user profile under .omen-hub-pp, independent of MSIX AppData virtualization. A protected, fixed per-user logon task and protected approval support the exact previously applied profile revision. Task execution was tested with zero offsets; no actual reboot occurred. The current automatic selection is disabled.
- Native `keyboard-read` and the Windows PowerShell 5.1 acquisition script successfully read capability and complete legacy color/brightness state on 8E35/F.13: FourZoneWithNumpad, matching 128-byte state blocks, brightness retained as a raw byte. Normal processes receive AccessDenied. No color writes or restore support are implemented; operation without the still-running original controllers remains unmeasured.
- Reads/unsupported states and future controllable hardware states must be clearly distinguished. No promise that a 115 W driver constraint is measured sustained performance.

## Brand Commitments

The product name is **Omen Hub++**. German is the present UI language. No official HP endorsement, copied logo or proprietary assets are implied. The user expressly requested UI-UX Pro Max and the installed Impeccable v4.3.1 for UI work, with no plugin update in this session.

## Evidence on Hand

README.md, docs/GPU_PROFILES.md, docs/ROADMAP.md, docs/REVERSE_ENGINEERING.md, docs/models/8e35.md and docs/VALIDATION.md. Local sensor/probe records remain ignored. There are 57 passing Rust tests plus native dashboard/GPU-editor tests, actual negative/positive offset and full-curve readback/restore, persisted profile recovery and scheduled-entry validation. Memory results are recorded per release in VALIDATION.md. No GPU-load benchmark proves full Unleashed, physical undervolting effect or overclock stability yet; native screenshot/DPI review remains outstanding.

## Product Principles

1. Show actual state before offering control.
2. Make support and unavailable states understandable without guessing values.
3. Preserve a fast, native, offline interaction and low idle cost.
4. Validate hardware behavior per board/BIOS before exposing writes.
5. Keep the tray useful and quiet; avoid background work without purpose.

## Open Decisions

Visual direction is being selected for the first dashboard. No palette or design world is yet approved. The user confirmed a code-first workflow; it is recorded only in `.impeccable/config.json`.
