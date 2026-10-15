# Direct lighting and custom presets

Mode: Operate. Native Windows / Win32, Rust, code-first. User screenshots on 15 October 2026 are the composition/interaction reference. Preserve the approved charcoal/mint/Segoe UI system; replace the old text-heavy lighting and GPU-only profile workflows.

## Direction contract

THESIS: Choose a zone and a color, or choose a preset and the settings it owns. Make the hardware result visible without an instruction wall.

OWN-WORLD: Existing opaque dark surfaces, native controls and focus, restrained mint primary actions. Keyboard keycaps are data visualization drawn in GDI; no copied HP bitmap, logo, decompiled source or proprietary assets enter runtime.

STORY: Dashboard offers Leistungsprofile / Lüfterkurve / Beleuchtung. Its Leistungsmodus selector lists Quiet (Default), Balanced (Default), Performance (Default) and saved custom profiles. The former standalone Unleashed choice moves to GPU options under the requested label Dynamic Boost; the underlying unqualified HP power contract remains blocked.

FIRST VIEWPORT: Lighting has a clear title, one Ein/Aus control, a wide four-zone keyboard including numpad, and one short state/error line. Clicking any zone opens the native color picker; confirmation applies and reads back immediately. No profile selector, RGB text form, separate firmware-check or apply/save/restore controls. A dedicated elevated lighting window supplies the required Windows access once per window, while the dashboard stays unelevated.

FIRST VIEWPORT: Leistungsprofile has a selected Custom Presets tab, saved-preset selector plus Neu action, name, separated GPU and CPU und System regions with eleven expandable tuning groups, Windows-start selection and anchored Anwenden / Speichern / Löschen actions. GPU-Leistungskurve is nested in GPU settings; the separate Lüfterkurve entry offers CPU/GPU fan drafts only. Exclude Super Resolution and Radeon Graphics Options without deleting stored metadata. Saved fields remain typed; unsupported adapters are concise states and block entire hardware apply. Persistent recovery and exact startup approval remain enforced.

FORM: Consistent native fields/buttons, keyboard-operable zone buttons and group disclosures, logical tab order, footer outside the scrollable settings pane, responsive desktop bounds. Windows color dialog protects focus. Instant pressed/selected/loading feedback; no decorative animation or continuous render timer.

QUALITY BAR: No metadata save is called a hardware change. Lighting compares the complete expected state and preserves unrelated GPU journals/startup choices using a separate durable lighting journal with a shared write lease. Unknown or externally changed hardware does not get silently overwritten. Name editing and partial option changes preserve full GPU curves and unrelated settings. Delete preserves recovery originals. Startup requires the successfully applied exact supported revision. Short errors explain recovery; technical detail remains in local diagnostics.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Finished artifact and evidence

The shipped work is an ordinary extension of the approved native system. No palette, typography or visual-world change was approved. Comparison of `theme.rs`, `native_ui.rs`, dashboard, preset fields/editor, keyboard and curve controls with DESIGN.md and `.impeccable/design.json` confirms the charcoal/mint/Segoe UI tokens, flat fields/actions, focus treatment and scoped chart palette. The system files are preserved. Native checkbox painting adds the shared dark treatment while retaining checkbox names, types, auto-toggle and keyboard behavior. The settings track supports wheel, page/start/end, drag and focused-field autoscroll; it is not a certified accessible scrollbar.

Production own-client WM_PRINT captures under `.impeccable/review/controls-redesign/` were validated after the correction:

| Captures | Client size at 96 DPI |
| --- | --- |
| `dashboard.png` / `dashboard-min.png` | 1122×773 / 942×743 |
| `lighting-on.png`, `lighting-off.png` | 984×551 |
| `lighting-on-min.png`, `lighting-off-min.png` | 744×441 |
| `presets-overview.png`, `presets-gpu.png`, `presets-cpu.png` | 1084×861 |
| `presets-min.png` | 904×681 |

Lighting captures are synthetic metadata previews, not hardware readings. No HTML/CSS detector ran for Win32. The fresh finish reviewer first returned **fix** for preset identity after a declined dirty-selection change. The correction preserves preset A's selection, draft and Save target when B is chosen and discard is declined. The actual regression also saved A and checked B's unchanged hash. The verdict pass accepted all ten recaptures and returned **ship**, scoring its one listed finding **F1 resolved**. This is the scope of the verdict; it does not certify full lifecycle, titlebars, other DPI or accessibility.

Final release SHA-256: `505B721BE79887D550C2C71126F7FF3600A8907ED3EA71B27B87921B5A18239B` (2,044,928 bytes). Format, 87 workspace tests, Clippy and release passed. Six native smokes passed on that binary: presets, navigation, dashboard, curves, read-only CPU UAC and lighting previews. These smokes wrote no hardware. Actual picker/cancel/red-zone/readback/original-restore evidence at 20:57 CEST and the separately requested, visibly confirmed four-second on/off/on sequence at 21:08:02 CEST on 15 October 2026 are recorded in `docs/KEYBOARD_UI.md` and hardware validation records. They do not establish physical color acceptance, reboot/resume, lasting stability or independence from HP controllers.

The context loader's existing Native Windows platform-enum warning and source-target-to-brief resolution gap remain tooling drift, with no repair or plugin update. Legacy surface-specific lighting/selector examples in the incumbent system documentation were not promoted into the new workflow or silently regenerated. Current behavior is recorded in PRODUCT.md, `docs/CUSTOM_PRESETS.md` and `docs/KEYBOARD_UI.md`.
