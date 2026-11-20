# Curve editor

The normal Rust/Slint editor and the legacy native GDI entry points share GPU V/F and fan editing rules. The GPU preset editor contains no hardware SET action. The user-provided MSI Afterburner view is an interaction reference; no MSI executable, driver, source or artwork is used by the runtime.

## Entry points and saving

- **Overview → + / New preset** opens a fresh driver-factory GPU curve immediately. Active driver offsets, a previous profile and cached draft/history are not inherited. Startup itself stays on Overview. Canceling an unsaved-change prompt preserves the old draft; Save/Discard followed by New opens the same fresh factory curve.
- **Overview → NVIDIA GPU → GPU performance curve → Edit curve** opens the current preset's in-memory chart. Use curve returns it to the preset draft; Back retains its editing session. Save in the parent persists the profile. Hardware changes require explicit profile Apply. Existing saved curves and unrelated draft fields are retained.
- `gpu-curve-ui [id]` opens the standalone saved GPU editor; `curve-ui [id]` remains the combined legacy entry.
- **Cooling → Edit curve** edits the selected named fan-preset draft. Use curve
  stages it, Save stores library metadata and Request applies the saved preference
  through the existing helper coordinator. `fan-ui` opens the same work area;
  legacy standalone editors keep their original active-curve behavior.

The Slint work area provides Back/Use curve, numeric editing, Undo/Redo and viewport tools. The graph takes keyboard focus when it opens. Legacy standalone views retain their own Cancel/Accept action rows.

The GPU toolbar also provides **Select range**, All, To end and Clear. In range
mode, plain dragging selects anchors without moving their frequencies. Numeric
From/To mV bounds include their end anchors, and a count/band identifies the range.
Enter the target MHz and choose **Equalize selection** to set only those points.
One action is one Undo step. Non-monotone/out-of-driver-range targets fail atomically;
an empty range is an error and disables tail flattening. Clear the range/mode to
use the separate **Flatten to end** action. **Cancel editing** restores the whole
currently loaded preset baseline; it differs from Back, Use curve and Reset.

Square nodes are the draft; the dotted line is the draft loaded at the start of this editing session. It can be a saved profile, not necessarily current firmware. Selected guides follow the editing selection; they are not a live GPU operating point. Fan templates are drafts, not hardware reads.

## GPU rules and gestures

On the reference device, all **127 actual Graphics voltage anchors** are fixed in count, voltage and order. Edit only frequency targets within the full driver bounds and nondecreasing frequency order. GPU anchors cannot be inserted, deleted or moved horizontally.

| Gesture | Draft change |
| --- | --- |
| Drag a point (within 15 px) | Move its frequency vertically; a point inside the selected band moves the whole band by one shared, clamped offset |
| Shift + drag point | Offset the entire curve/band using the loaded reference shape |
| Alt + drag point | Offset the entire curve/band while preserving the current draft shape |
| Ctrl + drag point | Tilt around the opposite fixed edge anchor |
| Shift + drag empty chart | Select a contiguous voltage range |
| Click on free chart space | Clear range selection |
| Double-click | Clear range selection; on fan curves, add a point on free space |
| Shift + Enter | Flatten selected range; otherwise plateau from the selected point |

A common offset is constrained consistently by all affected anchors/neighbors. Invalid tilts are rejected atomically. One completed drag is one undo step; Escape restores its starting state and adds no history. During a drag other keys are ignored; toolbar actions finish the drag first. A readout beside the dragged point shows voltage, frequency and the change since the drag started. There is no `L` shortcut: in Afterburner it locks a voltage, which this editor does not support.

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
| Ctrl + D | Restore the curve loaded at the start of this editing session, with undo support (Reset returns the GPU curve to the driver factory curve) |
| Ctrl + S | Save in the saved-profile editor; the in-memory preset chart returns through Accept |
| F1 | Show shortcut help |
| Wheel or +/− | Zoom X around the pointer (wheel) or the selected point (keys) |
| Ctrl + wheel or Ctrl with +/− | Zoom Y |
| Shift + wheel, sideways touchpad swipe | Pan horizontally |
| Ctrl + 0 | Fit the complete curve |
| Ctrl + Left/Right, fans | Move temperature 1 °C; Shift uses 10 °C |
| Shift + Up/Down, fans | Change fan value 10 percentage points |
| Insert/Delete, fans | Add/remove point |

Each curve keeps up to 64 undo entries. CPU/GPU fan histories remain independent. Viewport changes never drop points or alter data. GPU opens zoomed around the selected point near 950 mV; fan curves open fully fitted.

Fan curves retain 2–32 points, strictly ordered temperatures within 0–110 °C and nondecreasing 0–100% values. Metadata validity is not a complete thermal-safety proof.

## Hardware boundary

GPU frequency-at-voltage Apply/restore is qualified for the exact 8E35/F.13 / RTX 5070 Laptop / VBIOS with drivers **617.14 and 617.42**. Since 29 October 2026 any installed NVIDIA driver is accepted on this device; other versions are unmeasured and rely on the runtime ABI checks, readback and restore. Memory writes preserve coupled Graphics controls. Direct mV, hard voltage lock, Afterburner L lock and OC Scanner are not supported. A plateau is not a physical voltage ceiling or a load-stability result.

Fan curves run through the separate own helper. A shared CPU/GPU snapshot can now
be attached to a manual performance profile through a request-level coordinator.
The helper has no exact policy-digest acknowledgement: physical fan uptake,
full firmware rollback and BIOS handover remain unqualified. Linked snapshots
cannot gain sign-in authorization. [GPU contracts](GPU_PROFILES.md),
[fan control](FAN_CONTROL.md), [hardware matrix](HARDWARE_MATRIX.md).

## Implementation and validation

`core/curve_editor.rs` owns editing/history, `curve_input.rs` abstract input and `curve_view.rs` viewport/grid rules. `app/curve_widget.rs` owns GDI/native input; `curve_ui.rs` owns load/save/merge. Full curves and unrelated settings survive merge; conflicting concurrent edits fail.

Since 0.10.0-beta.8 the Slint editor implements these gestures itself (`app/slint_ui/curve.rs`: pick radius, press/drag/finish, wheel accumulation, key decoding); unit tests cover point picking, group drag as one undo step, Shift band selection, Escape without history and the key table. A mouse run of the Slint editor in `--ui-test` was not performed in this change.

Native smoke checks exercised chart opening from new/dirty presets, reference/current dragging, bands, flatten, undo/redo, fit, numeric alternatives and fan draft persistence without hardware Apply. Recorded client renders do not cover all DPI/titlebar/high-contrast/accessibility behavior. [Validation](VALIDATION.md).
