# CPU sensors extension

Mode: Operate. Platform: native Windows x64, Rust/Win32. Build path: code. Original sensor extension seed: `cpu-sensors-extension`; the current presentation inherits the subsequently approved dark instrument system.

## Direction contract

THESIS: Explicit real CPU telemetry joins the GPU-first view. Package-energy mean watts and labelled Tctl let the user inspect both processors before opening inventory or profile controls.

OWN-WORLD: Inherit root DESIGN.md, the shared dark native fields/actions and Segoe UI. CPU readings occupy the right dashboard sensor panel. The curve chart retains its scoped data palette.

STORY: Begin with CPU hardware values unavailable. Choose CPU-Sensoren …, approve Windows-UAC, obtain an energy baseline, then show mean watts over the measured interval and Tctl with source. Show understandable German cause/recovery text when support, approval, connection or sampling fails. Pause in tray/suspend, discard the baseline and begin fresh on return.

FIRST VIEWPORT: GPU wattage and temperature lead the left panel. CPU package watts, Tctl, utilization and qualified Windows MHz appear alongside them before editor actions, system inventory and the HP preview. The CPU panel names interval/source when active and explains explicit activation when unavailable. CPU-Sensoren … sits inside that panel.

FORM: Shared native STATIC labels, measured-value hierarchy and rectangular BUTTON control. CPU-Sensoren … appears while off/error, CPU-Sensoren aus while active/paused, and Windows-Freigabe … is disabled while approval starts. Measurement state remains separate from profile drafts and firmware write transactions.

QUALITY BAR: Read measured package-energy mean, measured interval and Tctl provenance together; never label Windows MHz as boost or a PPT/PL1 limit as watts. Default-unavailable, baseline, active, paused and error states must remain understandable. Support errors require a relevant remedy; recoverable UAC/connection/sample failures provide an appropriate activation/recovery action.

FINISH: finish review and current build validation are recorded in docs/VALIDATION.md.

## Persistent facts checked

- `app/theme.rs` supplies the shared dark surfaces and Segoe UI roles. `dashboard.rs` places the CPU readings immediately beside the GPU and distinguishes large available measurements from body-size unavailable/baseline text. Cause/recovery text uses the error color when the typed status is Error.
- `core/cpu.rs` calculates package-energy mean watts and decodes Tctl independently of Windows. Source, temperature kind and measured interval are typed in CpuHardwareSample. An initial baseline has no watts; Windows-reported MHz remain separate.
- `platform/windows/cpu_hardware.rs` gates HP8E35/F.13/Ryzen9 8940HX, CPUID 10882834 and Package1, the already running PawnIO 2.2.0 driver's exact identity/hash, and the signed AMDFamily17 0.2.11 module's exact hash. It dispatches fixed energy-unit/package-energy MSR reads and Tctl SMN reads. SCM queries metadata only; no driver is installed, started or stopped and no AMD SDK is loaded. [CPU_SENSORS.md](../../docs/CPU_SENSORS.md) records hashes and public contracts.
- The same driver hash check streams at most 1 MiB through an 8-KiB stack buffer, plus a bounded extra byte to detect oversized input. It avoids the old growing file Vec without changing the hash, qualification or hardware read contract.
- `app/cpu_sensor_bridge.rs` uses a transient same-binary UAC launcher, then a dedicated approved CPU reader. A same-user/SYSTEM local message pipe checks both PIDs, session token, schema, sequence and frame bounds. Operations are Sample/Pause/Stop. Inventories use a bounded disposable child. Pause drops the reader and baseline; main worker/helper block while hidden/suspended.
- `ui.rs` shows German support/approval/connection/read cause and recovery messages. Exact technical detail remains in last-cpu-error.json and cpu-sensor-status; recording it does not alter hardware recovery or startup state. Missing build support and an unqualified device are not presented as retryable reading failures.
- `ui.rs` destroys/rebuilds dashboard child controls around tray visibility. CPU approval remains independent in the worker and the returned measurement starts with a fresh energy baseline.
- `app/telemetry_worker.rs` separately owns the visible read-only NVIDIA session and its FFI resources on a disposable thread. Only owned results cross channels; five-second sample bounds, stale-result discard and the existing 30-second retry pause preserve failure reporting. Healthy Stop/drop performs original-thread cleanup; a vendor hang can remain until process exit. This does not add a CPU operation or change CPU approval.
- Build-time module staging and corresponding LGPL source/license packaging remain unchanged. No driver is bundled and runtime remains offline.
- Current `dark-dashboard.png` (1122×773) and `dark-dashboard-small.png` (942×743) were opened and checked. They show the default CPU-unavailable explanation and native activation control beside the GPU. They supersede the earlier white multiline report/footer captures for current visual appearance.

## Evidence boundary

Actual approved CPU readings and pause/fresh-baseline behavior are supported by the native control-text/function test and qualified device probes. The dark captures show the off state; they do not visually certify every active/error state. The earlier CPU finish verdict scored the single German cause/recovery fix resolved/ship and remains historical scoped evidence.

The current dark verdict is **ship / G1 resolved** for the single GPU helper/capability typography correction at the original pictured-client scope. It does not expand CPU active/error visual certification, and it excludes the later telemetry/hash-lifetime implementation and real power transitions.

Titlebar, alternate DPI, dynamic high-contrast behavior and comprehensive accessibility remain unverified. No HTML/CSS detector ran on this native surface. Real standby/resume/reboot and longer sensor/idle runs remain separate work; existing HP services were untouched and the independent CPU transport does not establish complete OEM-controller replacement. Current test/review and release-memory results are in [VALIDATION.md](../../docs/VALIDATION.md), with no inherited memory figure here.
