---
name: Omen Hub++
description: A native Windows tuning workbench with clear measured state, cohesive controls and explicit hardware actions.
colors:
  canvas: "#202127"
  surface: "#27282E"
  control: "#2E2F34"
  hover: "#37383E"
  line: "#424348"
  ink: "#F2F3F5"
  muted: "#B3B4B7"
  quiet: "#747579"
  accent: "#7C87F8"
  on-accent: "#000000"
  good: "#71D5A6"
  warning: "#E5C18A"
  error: "#F3A1AF"
  good-light: "#147347"
  warning-light: "#765000"
  error-light: "#A21635"
  scrim: "#00000099"
  graphite-canvas: "#12151B"
  graphite-ink: "#ECEEF2"
  graphite-accent: "#7EB9DF"
  vesper-canvas: "#101010"
  vesper-ink: "#ECE6E6"
  vesper-accent: "#FFC799"
  light-canvas: "#F5F6FA"
  light-ink: "#202127"
  light-accent: "#4D57C8"
typography:
  body:
    fontFamily: Segoe UI
    fontSize: 13px
  section:
    fontFamily: Segoe UI
    fontSize: 15px
    fontWeight: 600
  caption:
    fontFamily: Segoe UI
    fontSize: 12px
  utility:
    fontFamily: Segoe UI
    fontSize: 11px
  product-mark:
    fontFamily: Segoe UI
    fontSize: 12px
    fontWeight: 600
    letterSpacing: 0.8px
  metric:
    fontFamily: Consolas
    fontSize: 24px
  metric-unavailable:
    fontFamily: Consolas
    fontSize: 13px
  chart-label:
    fontFamily: Consolas
    fontSize: 11px
  diagnostic:
    fontFamily: Consolas
    fontSize: 12px
  keycap:
    fontFamily: Segoe UI
    fontSize: 10px
rounded:
  small: 4px
  control: 6px
  keyboard: 8px
spacing:
  small: 8px
  regular: 12px
  inset: 16px
  panel: 20px
  group: 24px
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    height: 36px
  button-secondary:
    backgroundColor: "{colors.control}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    height: 36px
  button-primary-pressed:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    rounded: "{rounded.control}"
    height: 36px
  button-secondary-pressed:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: 36px
  button-secondary-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: 36px
  button-disabled:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.quiet}"
    rounded: "{rounded.control}"
    height: 36px
  field:
    backgroundColor: "{colors.control}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    height: 36px
  value-field:
    backgroundColor: "{colors.control}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: 36px
    width: 276px
  navigation-selected:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: 36px
  section-header:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    height: 36px
  curve-canvas:
    backgroundColor: "{colors.surface}"
  keyboard-preview:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.keyboard}"
---

# Design System: Omen Hub++

## Overview

**Creative North Star: "The Native Tuning Workbench"**

The implemented Rust/Slint system is a modern, clean Windows utility for frequent
hardware checks and careful editing. Discord-inspired Slate neutrals, a restrained
indigo accent and Antigravity-style appearance choices replace the initial palette.
The user's pinned direction is recorded in [the appearance brief](docs/plans/APPEARANCE_REFINEMENT.md).
This record describes the implementation refined on 16 November 2026 for 0.10.0-beta.2.

Measured GPU watts and temperature lead, with verified CPU telemetry beside them.
Editable presets follow in the same work area. Information density supports scanning;
alignment, readable fields and explicit actions carry the hierarchy. Missing readings
stay unavailable, and Windows-reported MHz never becomes a measured boost clock.

The system is native and offline: Slint 1.18.1 owns layout and input, with a Windows
event/tray owner. The shared palette covers the main window and About. Product and
device contracts remain in [PRODUCT.md](PRODUCT.md) and the
[hardware matrix](docs/HARDWARE_MATRIX.md); a visual change grants no hardware,
startup or lifecycle qualification.

**Key Characteristics:**

- GPU/CPU instruments before tuning controls.
- Common field and action geometry, with one aligned value column.
- Complete palette choices and validated local appearance previews.
- Distinct selected, edited, saved and confirmed hardware states.
- Connected curve plots and a schematic keyboard drawn from original geometry.
- Event-driven rendering and low background resource use.

## Colors

Slate is the default: cool dark neutrals with one indigo action and focus accent.
The frontmatter records its derived tokens, the three other preset base palettes
and the theme-dependent semantic colors. Custom appearance accepts opaque sRGB
background, foreground and accent values.

### Primary

**Indigo action** (`accent`) identifies primary actions, focus, draft curves and the
selected keyboard group. **On-accent ink** (`on-accent`) is whichever of black or
white gives the higher contrast against the active accent.

### Neutral

**Canvas**, **surface**, **control**, **hover** and **line** separate the work area,
section headers, fields, interaction feedback and boundaries. Runtime appearance
derives surface/control/hover/line by mixing foreground into canvas at
3.5/6.5/11/16%, with channel rounding. These are reused tonal roles, not shadows.

**Ink** carries essential content. **Muted** is a 70% foreground mix when it meets
the control-text contrast check, otherwise full ink. **Quiet** is a 40% mix reserved
for disabled or subordinate state. The dialog scrim dims the covered work area.

Slate, Graphite, Vesper and Light are complete presets. Custom uses the same derived
roles, so a palette change reaches fields, buttons, plots and About together. Standard
Slint attribution and diagnostic controls use the matching light/dark scheme.

### Semantic

Good, warning and error have separate dark/light colors and accompany readable
wording. Neither an accent nor a colored status dot proves a hardware operation.

**The Shared Palette Rule.** Reuse semantic theme roles throughout the native UI.
Do not introduce a separate chart, popup or settings palette.

**The Valid Preview Rule.** Foreground on the hovered control must meet 4.5:1 and
accent against canvas must meet 3:1. Valid edits preview immediately; partial or
low-contrast input retains the last valid preview and gives inline recovery guidance.
Save persists the appearance. These validation thresholds do not establish full
Windows high-contrast or accessibility coverage.

## Typography

Segoe UI is the native text face. Consolas is restricted to measured values,
coordinates and diagnostics. The frontmatter records the observed logical-pixel
roles; there is no decorative display face or downloaded font.

Section titles use the semibold section role. Body text carries controls and ordinary
labels; caption and utility roles cover descriptions, privilege and footer details.
The product wordmark uses its own small semibold role and measured letter spacing.
Keyboard labels use the compact keycap role within the schematic.

Available instrument readings use the larger metric role; unavailable readings use
the smaller metric-unavailable role. Long explanations wrap. Control text may elide
inside its fixed field, while wrapped popup choices expose the full selection.

**The Exact Value Rule.** Preserve units, exact numeric values and the distinction
between configured, requested, effective and measured state. Essential descriptions
must remain readable in both German and English.

## Layout

One resizable native window retains the system caption, menu, minimize/maximize and
resize behavior. Default client size is 1000 × 720 logical px; minimum is 720 × 500.
Compact work areas scroll within their bounds instead of increasing the window's
minimum to fit every expanded section.

The command bar is 44 px high with 16 px side insets, an original geometric mark and
device privilege details in the expandable device area. Overview, Cooling and Lighting occupy three equal targets
within the navigation strip; Settings remains a separate action. The footer is
28 px high, with bounded selectable details for long diagnostics.

Content panels use the shared 20 px inset and mainly 8/12 px spacing. Ordinary
inputs, selectors and buttons share 36 px height and six-pixel corners. Preset fields
and settings, including preset selector/name, use one fixed 276 px value column.
Ordinary text actions share 128 px; compact actions use 36 px. Per-core fields, chart controls and keyboard keys retain their
purpose-specific widths.

**The One Work Area Rule.** Keep Overview's instruments and preset editor in one
mounted scroll area. Selection, drafts and Undo survive scrolling and navigation.
Save, Apply/Recovery, Delete and sign-in approval remain explicit below that area.

Slint-scale checks at 100/125/150/200% are layout evidence. Physical Windows DPI
transitions require their own observed validation; scale overrides do not prove them.

## Elevation & Depth

The system uses opaque tonal layering, hairline boundaries and focus outlines,
without drop shadows, glass or decorative gradients. Section surfaces and popups
remain flat. Confirmation uses the scrim and a bordered surface rather than a
shadow vocabulary.

Action state feedback is immediate, avoiding animation churn during scrolling.
Software redraws follow the monitor's fractional refresh period. Pending redraws
coalesce; timer wakes round upward to whole milliseconds to avoid a zero-delay
loop in Slint 1.18. The display phase survives rounding, late frames do not create
catch-up bursts, and there is no idle repaint timer. Rendering remains event-driven
and dashboard polling pauses when hidden or minimized.

**The State Before Color Rule.** Name busy, error, draft, saved, unavailable and
confirmed states in text. Color reinforces the wording.

## Shapes

Controls and confirmation surfaces share the gently rounded control shape. Popup
rows, checkboxes and keycaps use the smaller corner; the keyboard's enclosing
surface uses the larger keyboard corner. One-pixel borders and original Path
geometry supply the remaining structure.

Charts use actual connected paths. Rotated thin rectangles are not an alternative:
transforms are unavailable in the supported software renderer. Path, markers and
pointer input must share the same plot viewport, with axes outside that clipped area.

## Components

### Buttons and navigation

Primary actions use accent/on-accent; ordinary actions use control/ink with tonal
hover feedback. Pressed primary actions retain accent/on-accent with a contrasting
on-accent boundary; pressed ordinary actions retain hover/ink with an accent boundary.
Disabled actions retain their labels with subdued ink and surface.
Keyboard focus uses the on-accent boundary for primary actions and accent for
ordinary actions, following the same geometry as the interactive target.
Space and Enter activate buttons. Equal navigation targets keep selected fill,
keyboard outline and hit area together.

### Inputs and selectors

Shared fields have inset native Slint text input, a visible accent focus boundary,
caret, selection, IME, clipboard/context actions and Undo. Do not replace the editor
with painted text. Checkboxes and the brightness slider expose keyboard interaction
and semantic roles.

Selectors use the same closed-field geometry. The popup is bounded and scrollable;
choices grow to wrap long names. Opening starts at the current choice. Arrows,
Home and End move the pending highlight and reveal its actual variable-height row;
Enter/click commits, Escape cancels. A highlighted open choice is not yet a selected
preset.

### Overview and presets

GPU and CPU instruments precede the selected preset's name and directly editable
GPU, CPU and Windows groups. Device, provenance, mode and graphics details expand
on demand. The last recorded AC preset loads if it still exists; stored intent and
historical Apply never imply a current hardware readback.

Help is disclosed on demand. Save, Apply/Recovery, Delete and a preset's explicit
sign-in permission keep their separate meanings. Rust owns stable draft/history
models; telemetry must not rebuild the editor or release another worker's busy state.

### Curves and cooling

BIOS/Curve/Max cooling remains separate from custom presets. GPU curves belong to
preset drafts. Both editors use connected Path geometry, draft/reference legends
and readable axes. Plot clipping, point markers and input use the same viewport.
The MSI Afterburner reference remains scoped to curve interaction/composition.

### Keyboard lighting

An original schematic QWERTZ/QWERTY keyboard previews four hardware zones through
three selectable groups: WASD selects wire zone 3; Rest links zones 1 and 2; NUM pad
and right selects zone 0, including adjacent keys. Clicking a key selects its group;
the three group buttons provide the keyboard route. The diagram does not claim
per-key RGB or measured physical LED boundaries.

The shared color field opens the same in-app HSV/HEX dialog used by appearance.
Hue, saturation/value, sixteen RGB swatches and exact HEX have pointer and keyboard
routes. Before/selection previews distinguish changes; invalid HEX cannot accept.
Tab skips disabled controls and reveals its target in the clipped body, including
at minimum height. Escape/Cancel restore the invoking field's focus without edits.
Color acceptance, On/Off and the compound brightness field edit a draft. Brightness adjusts
RGB color value from the retained full-color base, never an unqualified firmware
master-brightness byte. A mixed Rest group can keep its separate hues during a
brightness edit. Canceling the picker changes nothing.

Apply uses the existing guarded transaction and complete readback. Reset restores
the last read draft; refresh cannot silently discard edits. Outside changes are
rejected by the expected-state check. Unsaved lighting survives navigation/tray
return and joins preset, curve and settings exit protection. The
[keyboard guide](docs/KEYBOARD_UI.md) owns the detailed mapping and limits.

### Settings, About and status

Five equally sized theme tiles preview actual canvas/text/accent combinations.
Three common color fields open the staged editor; appearance acceptance previews
through the existing format/contrast guard, and Save persists it.

Settings groups appearance, language, app autostart, tray/close behavior, battery
choice and hold. App startup differs from a preset's sign-in permission. The existing
explicit HP-service action is separate from normal settings saves.

Appearance lives in a separate versioned `ui-settings.json`, preserving the
`app-settings.json` schema for older protected main/fan/startup binaries. Failed
normal saves restore prior appearance and the Run entry with concurrent-change
checks. A palette preview grants or rewrites no profile, startup or recovery record.

German and English use stable language keys and professional wording. Language
stays consistent for the process and UAC actions; a saved language change requires
restart without discarding drafts. Footer wording distinguishes busy/error/saved/
confirmed state; selectable details expose long diagnostics.

## Do's and Don'ts

### Do

- **Do** retain native Windows window behavior, native Slint text editing and visible
  keyboard focus.
- **Do** keep selected intent, draft edits, metadata saves and hardware readback
  distinct in wording and actions.
- **Do** preserve bounded telemetry, controller leases, expected-state checks,
  readback and recovery. Closing waits for an active own write; a UI update never
  terminates an active cooling helper.
- **Do** verify both languages, compact/enlarged clients, popup focus/scrolling,
  drafts/Undo, profile switches, exact units, connected curves and palette recovery
  in both supported renderers. Exercise real wheel input, true compact client
  geometry, repeated resize/minimize/tray restore and settings persistence through
  a real process restart.
- **Do** keep developer-only MCP and marked UI fixtures isolated. Fixture files are
  restricted to a marked OS-temp directory and never write the live user store.
  Normal releases have no MCP server.
- **Do** measure total tray resources including resident helpers in the actual
  configuration. The low-background-resource goal supersedes the historic fixed
  50 MB ceiling; responsiveness and correctness take priority over executable size.
- **Do** publish only a tested, identified build with source/version/SHA-256 and
  truthful device/lifecycle limits. Current checks belong in
  [Validation](docs/VALIDATION.md).

### Don't

- **Don't** infer sensor readings, claim CPU package power/temperature without a
  verified reader, or present Windows MHz as measured boost.
- **Don't** present a preset preview, metadata save, palette change or UI fixture as
  a firmware operation.
- **Don't** describe a keyboard group as per-key RGB or numpad-only control.
- **Don't** add a browser runtime, raster decoration, downloaded fonts, decorative
  motion or a continuous renderer to this native system.
- **Don't** claim new hardware support, independent HP-controller ownership,
  sustained 85 °C enforcement, physical DPI transitions or complete accessibility
  coverage from these visual corrections. These require their own evidence.

Current extension and quality bar: [Color selection and scrolling](docs/plans/COLOR_SELECTION_AND_SCROLL.md). Curve numeric, history and viewport controls use bounded rows; chart and primary action stay inside the compact horizontal viewport.
