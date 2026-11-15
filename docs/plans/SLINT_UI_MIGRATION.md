# Slint UI migration

Started 15 November 2026 from `554bb55` (0.9.0-beta.3), on `codex/slint-ui-rework`. This is implementation tracking, not deployed support. [Design](../../DESIGN.md) owns the target composition.

| Existing operation | Source / preserved behavior |
| --- | --- |
| GPU and verified CPU measurements | app `ui.rs`, `telemetry_worker.rs`, `cpu_sensor_bridge.rs`; bounded reads, explicit unavailable/stale, suspend/hidden pause |
| Built-in modes/custom preset Apply | app `ui.rs`, `elevation.rs`; platform controller, foreground lease, journal, final readback and recovery |
| Preset create/edit/save/delete/recover/sign-in | app `preset_ui.rs`, `preset_fields.rs`; optimistic merge, unchanged unknown settings, explicit retired cleanup, separate exact-revision startup authorization |
| CPU/GPU/Windows fields | 25 catalog fields in 7 groups; per-core CO and GPU curve are structured values; no Intel or preset Max-Fan |
| GPU curve | core curve editor/input/viewport; immutable voltage anchors, Shift/Alt/Ctrl, range selection, point editing, history, fit/zoom |
| Cooling | shared fan curve; BIOS/Curve/Max request, helper compatibility, durable ownership and pending handover |
| Graphics preference | existing elevated MUX action, explicit restart-pending distinction |
| Lighting | direct four-zone read/apply including numpad and native picker; compare expected state and retain diagnostics |
| Global settings | existing settings transaction; System/DE/EN, app task, tray start/close, battery preset, preset hold, explicit HP-service action |
| Tray/power/single instance | existing own icon, Explorer recovery, foreground activation, power sampling generation, bounded exit |
| CLI/helpers/packaging | retain fixed command protocols, UAC language, CPU module pins, installer/recovery and license notices |

The brief's service-manager and snapshot examples do not establish implemented product functions. No such operations will be invented for the visual rewrite. Existing hardware qualification is unchanged.

Implementation sequence: specify → Slint components and model wiring → integrate existing operations → actual MCP interaction and render review → regression/build checks → document and deploy only the completed candidate. Preserve the current dist/desktop target throughout development.

## Implemented software result

The normal workspaces are connected to the Slint shell. The bounded form stays mounted to preserve edit history; a full GUI interaction matrix passed in both languages/renderers at four scale configurations. The default renderer was selected from measured tray behavior: software avoids persistent GPU-driver allocations in the main process. Exact checks and remaining device/lifecycle boundaries belong in [Validation](../VALIDATION.md). Legacy Win32 captures remain explicit diagnostics; the normal GUI entry points share one Slint instance.
