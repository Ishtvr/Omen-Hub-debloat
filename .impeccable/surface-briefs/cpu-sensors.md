# CPU sensors extension

Mode: Operate. Platform: native Windows x64, Rust/Win32. Build path: code. Scope: ordinary local extension of the existing native dashboard; scoped seed `cpu-sensors-extension`.

## Direction contract

THESIS: Explicit real CPU telemetry joins the GPU-first native report. Package-energy mean watts and labelled Tctl let the user inspect both processors before opening inventory or profile controls.

OWN-WORLD: Inherit the established Win32 report, buttons, Segoe UI and Windows system colors. The extension introduces no new global visual identity. The dark curve workbench retains its separately scoped direction.

STORY: Begin with CPU hardware values unavailable. Choose CPU-Sensoren …, approve Windows-UAC, obtain an energy baseline, then show mean watts over the measured interval and Tctl with source. Show understandable German cause/recovery text when support, approval, connection or sampling fails. Pause in tray/suspend, discard the baseline and begin fresh on return.

FIRST VIEWPORT: GPU wattage and temperature lead the report. CPU package watts, Tctl, utilization and qualified Windows MHz follow immediately, before system inventory and profile controls. The CPU block names the interval/source when active and explains explicit activation when unavailable. A footer button exposes the session-only CPU approval and off action alongside existing native controls.

FORM: Extend the established read-only Win32 multiline report and standard buttons. Preserve Windows colors and Segoe UI; keep privilege prompting explicit. The button shows CPU-Sensoren … while off/error, CPU-Sensoren aus while active/paused, and is disabled while approval is starting. Measurement state is separate from profile drafts and firmware write transactions.

QUALITY BAR: Read measured package-energy mean, measured interval and Tctl provenance together; never label Windows MHz as boost or a PPT/PL1 limit as watts. Default-unavailable, baseline, active, paused and error states must remain understandable. Support errors require a relevant remedy rather than a generic retry, while recoverable UAC/connection/sample failures provide an appropriate activation/recovery action.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review and documentation.

## Persistent facts checked

- `crates/app/src/ui.rs` preserves standard `STATIC`, read-only multiline `EDIT`, `BUTTON` and `COMBOBOX` controls and Segoe UI via GDI `lfHeight = -18`. Native system colors remain the incumbent visual authority. No new palette, font asset, web runtime, diagram or global design rule is introduced.
- The GPU/CPU report appears before system inventory. The CPU activation button is in the footer at the existing 20-unit left inset. The current own-client render is 790×633; the footer crop is 790×153. These are observed implementation/capture dimensions, not a verified DPI or responsive system.
- `core/cpu.rs` calculates package-energy mean watts and decodes Tctl independently of Windows. Source, temperature kind and measured interval are typed in `CpuHardwareSample`. An initial baseline has no watts; Windows-reported MHz remain explicitly separate.
- `platform/windows/cpu_hardware.rs` gates HP8E35/F.13/Ryzen9 8940HX, CPUID 10882834 and Package1, the already running PawnIO 2.2.0 driver's exact identity and hash, and the signed AMDFamily17 0.2.11 module's exact hash. It dispatches only fixed energy-unit/package-energy MSR reads and Tctl SMN reads. SCM access queries metadata only; no driver is installed, started or stopped, and no AMD SDK is loaded. [CPU_SENSORS.md](../../docs/CPU_SENSORS.md) records the precise hashes and public contracts.
- `app/cpu_sensor_bridge.rs` uses a transient same-binary UAC launcher, then a dedicated approved CPU reader. A same-user/SYSTEM local message pipe checks both PIDs, session token, schema, sequence and frame bounds. Its only operations are Sample/Pause/Stop. Inventories run in a bounded disposable child, outside both persistent processes. Pause drops the reader and baseline; the main worker and helper block while hidden/suspended.
- `ui.rs` shows German support/approval/connection/read cause and recovery messages. Exact technical detail remains separately available through `last-cpu-error.json` and `cpu-sensor-status`; recording it does not alter hardware recovery or startup state. Missing build support or an unqualified device is not presented as a retryable reading failure.
- `prepare-cpu-module.ps1` stages the signed public module only at build time. The unchanged module is LGPL-2.1-or-later, with corresponding module/full source and COPYING in the release's third-party directory; original Rust remains MIT. No driver is bundled and runtime remains offline.
- Opened and checked `.impeccable/review/cpu-dashboard.png` and `cpu-controls.png`: they show the expected GPU-first native report, default CPU-unavailable state, explicit activation, profile-preview wording and existing footer controls. They are own-client WM_PRINT renderings, not screenshots of the desktop. No mock or approved comp exists for this code-first local extension.

## Evidence boundary and existing context gap

The finish reviewer scored the single German error-copy/recovery correction **resolved**, disposition **ship**, and accepted both required recaptures as valid. That verdict does not certify active/error-state visual rendering, alternate DPI settings, titlebar behavior or comprehensive accessibility. Actual approved CPU readings and pause/fresh-baseline behavior are supported by the native control-text/function test and device probes, not by the default-unavailable rendering. The HTML/CSS detector does not apply to this native surface and was not run.

Real standby/resume/reboot and longer sensor/idle runs remain separate validation work. Existing HP services were untouched; the independent CPU transport does not prove complete OEM-controller replacement. Current release memory measurements belong in [VALIDATION.md](../../docs/VALIDATION.md), with no inherited or invented memory figure here.

The root still has no global `DESIGN.md` or `.impeccable/design.json`. `PRODUCT.md` leaves the dashboard's broader identity open. This ordinary extension documents and preserves the incumbent interface; it does not repair that pre-existing global design-context gap or claim a new world.
