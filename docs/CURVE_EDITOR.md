# Curve editor

The native GDI editor shares editing rules across GPU V/F and fan curves. It contains no hardware SET action. The user-provided MSI Afterburner view is an interaction reference; no MSI executable, driver, source or artwork is used by the runtime.

## Entry points and saving

- **Performance profiles → GPU → GPU performance curve → Edit** opens an in-memory chart, including new unnamed or unsaved presets. Accept returns the draft; Cancel discards only that chart session. Save in the parent persists it. Other unsaved fields stay intact.
- `gpu-curve-ui [id]` opens the standalone saved GPU editor; `curve-ui [id]` remains the combined legacy entry.
- **Fan curve** / `fan-ui` edits the active CPU/GPU curve for the separate helper. Saving a fan curve does not prove the helper has applied it.

The preset chart shows a compact Cancel/Accept/Undo/Shortcuts action row; numeric editing, redo and zoom remain accessible from keyboard/mouse. Other views offer chart and profile/value layouts.

Square nodes are the draft; the dotted line is the draft loaded at the start of this editing session. It can be a saved profile, not necessarily current firmware. Selected guides follow the editing selection; they are not a live GPU operating point. Fan templates are drafts, not hardware reads.

## GPU rules and gestures

On the reference device, all **127 actual Graphics voltage anchors** are fixed in count, voltage and order. Edit only frequency targets within the full driver bounds and nondecreasing frequency order. GPU anchors cannot be inserted, deleted or moved horizontally.

| Gesture | Draft change |
| --- | --- |
| Drag a point | Move its frequency vertically, or move its selected band together |
| Shift + drag point | Offset the entire curve/band using the loaded reference shape |
| Alt + drag point | Offset the entire curve/band while preserving the current draft shape |
| Ctrl + drag point | Tilt around the opposite fixed edge anchor |
| Shift + drag empty chart | Select a contiguous voltage range |
| Double-click | Clear range selection |
| Shift + Enter | Flatten selected range; otherwise plateau from the selected point |

A common offset is constrained consistently by all affected anchors/neighbors. Invalid tilts are rejected atomically. One completed drag is one undo step; Escape restores its starting state. The opposite tilt anchor remains fixed.

## Keyboard and viewport

Commands apply with chart focus unless noted. Native numeric fields provide an alternative to dragging.

| Input | Action |
| --- | --- |
| Left/Right, Home/End | Select point / first / last |
| Up/Down | Change frequency by 1 MHz or fan value by 1 percentage point |
| Ctrl + Up/Down | Change by 10 steps |
| Shift or Alt + Up/Down, GPU | Same reference/current-curve offset as the corresponding drag; Ctrl uses 10-MHz steps |
| Shift + Left/Right/Home/End | Extend contiguous GPU selection |
| Ctrl + A | Select all GPU anchors |
| Enter | Open numeric/profile view and focus the selected Y value |
| Ctrl + Z / Ctrl + Y / Ctrl + Shift + Z | Undo / redo |
| Escape | Cancel drag, otherwise clear range |
| Ctrl + D | Restore the active loaded draft, with undo support |
| Ctrl + S | Save in the saved-profile editor; the in-memory preset chart returns through Accept |
| F1 | Show native shortcut help |
| Wheel or +/− | Zoom X around pointer/selection |
| Ctrl + wheel or Ctrl with +/− | Zoom Y |
| Shift + wheel | Pan horizontally |
| Ctrl + 0 | Fit the complete curve |
| Ctrl + Left/Right, fans | Move temperature 1 °C; Shift uses 10 °C |
| Shift + Up/Down, fans | Change fan value 10 percentage points |
| Insert/Delete, fans | Add/remove point |

Each curve keeps up to 64 undo entries. CPU/GPU fan histories remain independent. Viewport changes never drop points or alter data. GPU opens zoomed around the selected point near 950 mV; fan curves open fully fitted.

Fan curves retain 2–32 points, strictly ordered temperatures within 0–110 °C and nondecreasing 0–100% values. Metadata validity is not a complete thermal-safety proof.

## Hardware boundary

GPU frequency-at-voltage Apply/restore is qualified for the exact 8E35/F.13 / RTX 5070 Laptop / VBIOS and **617.14 or 617.42** implementations. Memory writes preserve coupled Graphics controls. Direct mV, hard voltage lock, Afterburner L lock and OC Scanner are not supported. A plateau is not a physical voltage ceiling or a load-stability result.

Fan curves do run through the separate own helper. They remain unsupported as `CpuFanCurve`/`GpuFanCurve` instructions inside a complete performance-profile transaction, and BIOS ownership handover is not qualified. [GPU contracts](GPU_PROFILES.md), [fan control](FAN_CONTROL.md), [hardware matrix](HARDWARE_MATRIX.md).

## Implementation and validation

`core/curve_editor.rs` owns editing/history, `curve_input.rs` abstract input and `curve_view.rs` viewport/grid rules. `app/curve_widget.rs` owns GDI/native input; `curve_ui.rs` owns load/save/merge. Full curves and unrelated settings survive merge; conflicting concurrent edits fail.

Native smoke checks exercised chart opening from new/dirty presets, reference/current dragging, bands, flatten, undo/redo, fit, numeric alternatives and fan draft persistence without hardware Apply. Recorded client renders do not cover all DPI/titlebar/high-contrast/accessibility behavior. [Validation](VALIDATION.md).
