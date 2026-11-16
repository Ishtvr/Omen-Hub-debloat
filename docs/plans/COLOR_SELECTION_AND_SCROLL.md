# Color selection and scrolling — 16 November 2026

Ordinary extension of the existing native Windows Operate workbench. The user
requests uniformly sized fields, better app/keyboard color selection and smoother
up/down scrolling. Retain the Slate/Graphite/Vesper/Light/Custom world.

## Direction contract and quality bar

- Normal form inputs, selectors and color fields share 276 × 36 logical px.
  Text actions share 128 px; compact controls use 36 px. Chart coordinates and
  per-core arrays use matched contextual widths. No row may widen a compact page.
- Five equal theme previews show their real canvas, ink and accent. A common
  color field opens the same staged HSV/HEX editor for app and keyboard colors.
- Pointer and keyboard can choose hue/saturation/value or sixteen RGB swatches;
  incomplete HEX never accepts. Escape/Cancel change no draft. Acceptance changes
  a UI draft; keyboard firmware still requires the existing explicit Apply.
- Modal Tab stays within enabled controls and reveals the focused control even at
  720 × 500. Curves, header actions and tools fit the horizontal viewport.
- Fix measured software frame pacing without a permanent repaint loop or changes
  to telemetry/control ownership. Keep GPU pacing and hardware contracts intact.
- Preserve native caret/selection/Undo, drafts across navigation/resize/tray return,
  both languages, profile/recovery/startup bytes and controller processes.

The quality bar is a precise native tool: actual interaction, clear
selected/draft/readback distinctions, accessible compact controls, honest timing
and no new hardware-qualification claims. Code-led; no raster comp or separately
supplied Discord quality card. Source/captures, this contract and DESIGN.md are
the review evidence.
