# Native dashboard

Mode: Operate. Code-first. User-pinned dark, modern, clean appearance. Preserve GPU-first / CPU-alongside priority, measured data, native Windows controls and offline Rust runtime. Open-window memory may exceed 50 MB; combined tray-idle remains capped at 50 MB, including a paused CPU reader.

## Revision — 22 October 2026

The CPU panel now shows the highest measured interval-effective clock alongside load, with Windows MHz on its own line and 32-logical-processor coverage/clock interval in the existing detail field. Clock baseline and unavailable states are explicit; successful package watts/Tctl stay visible on a clock-only fault. Native colors, font roles and sensor-panel geometry are retained. The latest main branch's Apply button, fan/graphics switches and startup/close-to-tray settings are preserved. The current source opens at 1140×780 outer dimensions, minimum 960×750. Real production client captures at 1124×741 and 944×711, 96 DPI, are `.local/review/cpu-clocks/cpu-clocks-1140.png` and `cpu-clocks-960.png`; both were checked for full status-line fit and action visibility by the implementing agent. There is no new independent Impeccable finish verdict. Current build, 152 passing tests, read-only hardware/UI verification and cold/combined-reader tray measurements belong to the 22 October entry in docs/VALIDATION.md. The earlier facts/captures/verdict below describe their earlier surfaces and binaries; they do not transfer as current dimensions or certification. Full replacement, actual standby/reboot, load stability, other DPI, titlebar, high contrast and comprehensive accessibility remain unproven by these captures.

## Direction contract

THESIS: A precise dark instrument panel makes GPU wattage and temperature immediately legible, with CPU readings beside them and deliberate hardware actions below.

OWN-WORLD: Charcoal ground, slightly lighter sensor surfaces, bright neutral text and restrained mint action ink. Segoe UI at explicit body, heading and measurement sizes. Flat surfaces, subtle boundaries, native fields and owner-drawn native buttons; no decorative graphs or continuous motion. The implemented shared world is recorded in root DESIGN.md and .impeccable/design.json.

STORY: Read both processors, distinguish measured values from unavailable/baseline states, activate CPU reading explicitly, then enter profiles, curves or lighting. A clearly marked HP preview remains separate from actual apply.

FIRST VIEWPORT: Header and power context, two sensor regions with paired power/temperature readings, supporting clocks/intervals, then one row of prominent tool actions. System identity and the HP preview are secondary. Native tab order follows the visual flow; widths reflow with the window.

FORM: Grounded candidate four, laboratory instrument front panel, seed `7aa2845d`. The pinned modern dark scene translates physical rails into aligned values and grouped actions. Other grounded systems considered: driver performance panel, Windows Settings, studio mixer, avionics, developer inspector and firmware EZ view. Catalog challengers' decorative materials, tiny single-size labels and geographic metaphors do not clarify this fixed hardware task. Keep their disciplines of consistent alignment, explicit state and density without importing their form.

QUALITY BAR: Real native controls preserve names, roles, keyboard activation and visible focus. Error and unavailable states stay readable and truthful. Uniform fields and action styles extend to profile and lighting windows. No new hardware operations or timers. Captures at default and minimum window sizes, function tests and current open/tray memory measurements establish the scoped result.

FINISH: unreviewed and undocumented is unfinished; finish review and build-scoped validation are recorded in docs/VALIDATION.md.

## Persistent facts checked

- `app/theme.rs` owns the shared palette, Segoe UI fonts and bounded GDI objects. `dashboard.rs` creates standard STATIC, EDIT, BUTTON and COMBOBOX controls, lays them out and paints the sensor panels; it never reads or changes hardware.
- GPU power/temperature lead the left panel; CPU package power/Tctl, busy time, explicitly qualified Windows MHz and sensor activation occupy the adjacent panel. Unavailable values remain explicit body-size text. Active package watts remain an energy mean with interval; the first baseline has no watts.
- The native default requests 1140×820 outer dimensions; WM_GETMINMAXINFO sets 960×790 minimum. Current own-client renders are `.impeccable/review/dark-dashboard.png` (1122×773) and `dark-dashboard-small.png` (942×743), inspected at 96 DPI. Both show the full sensor/action/system/preview hierarchy.
- Dashboard margins are 28 units, panel gap 20, sensor padding 24 and tool-action gap 16. Width determines two equal panel widths and three equal actions. Rounded sensor panels use 24-unit corner diameters; fields and actions stay rectangular.
- `ui.rs` releases child controls when hidden in tray and rebuilds them on return. The chosen HP-preview index is restored. CPU sampling and approval remain in the worker; tray/suspend release the CPU reader/baseline and return starts fresh.
- `app/telemetry_worker.rs` owns the read-only NVIDIA session on a disposable thread while visible. FFI handles/Libraries stay on their original thread; only owned sensor samples, warnings and lost-state cross channels. Sample waits are bounded to five seconds, stale data are discarded and the existing 30-second retry pause remains. Stop/drop lets a healthy thread exit and clean up on its original thread with a bounded roughly 200-ms wait. A vendor FFI hang can remain until process exit and is outside the normal tray evidence.
- All five dark captures were opened and checked, including GPU, lighting and full numeric curve controls. They are production parent/native-control WM_PRINT renders. A hidden preview is shown offscreen without keyboard activation so classic EDIT contents render, then destroyed. No mock, external desktop pixels, raster assets or browser runtime are used.

## Evidence boundary

The dashboard captures show CPU hardware sensors off, actual supplied NVIDIA state, selected HP preview text and native fields/actions. Active/error CPU visual certification, titlebar, alternative DPI/display configurations, dynamic high-contrast switching and comprehensive accessibility remain outside this evidence. The theme samples high contrast at creation and uses system colors for shared controls; DWM attributes request a dark frame but the client captures do not verify it.

The finish reviewer re-opened all five valid dark captures at their recorded sizes and returned disposition **ship**, scoring the single listed finding **G1 resolved**. Both GPU helper/capability statics use the shared 14-pixel font role; complete facts and action geometry remain intact. That verdict certifies this correction at the original pictured-client scope, rather than all active/error, titlebar, DPI/high-contrast/accessibility or real power-transition behavior. The later telemetry/hash-lifetime change is outside the visual verdict.

Five native smoke results and final cold/open/combined warm-tray measurements belong to the current binary in [VALIDATION.md](../../docs/VALIDATION.md). Do not inherit older release figures. No hardware write contract or transaction behavior changed; the later NVIDIA read-thread and CPU hash-buffer lifetimes are documented separately. HP profiles remain previews, CPU reads remain exactly qualified, and full OEM-controller replacement, real standby/reboot and load stability remain unproven. The native HTML/CSS detector does not apply and was not run.
