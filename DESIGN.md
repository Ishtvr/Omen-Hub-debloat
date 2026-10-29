---
name: Omen Hub++
description: A clean dark native instrument panel for measured hardware state and deliberate controls.
colors:
  background: "#101215"
  panel: "#191d22"
  field: "#242b33"
  border: "#657888"
  foreground: "#edf2f7"
  muted: "#a6b1bc"
  accent: "#68dbbf"
  accent-ink: "#0b1c17"
  error: "#f7a7a7"
  accent-hover: "#85e9d0"
  accent-pressed: "#4cb699"
  field-hover: "#303943"
  chart-background: "#1c1f22"
  chart-plot: "#1c1d1f"
  chart-surface: "#272a2d"
  chart-band: "#273140"
  chart-foreground: "#dee2e5"
  chart-grid: "#34383c"
  chart-grid-minor: "#26292c"
  chart-reference: "#848d97"
  chart-draft: "#c7cdd3"
  chart-selection: "#85cdf4"
typography:
  small:
    fontFamily: "Segoe UI"
    fontSize: "14px"
    fontWeight: 400
  body:
    fontFamily: "Segoe UI"
    fontSize: "16px"
    fontWeight: 400
  editor-body:
    fontFamily: "Segoe UI"
    fontSize: "18px"
    fontWeight: 400
  heading:
    fontFamily: "Segoe UI"
    fontSize: "20px"
    fontWeight: 600
  title:
    fontFamily: "Segoe UI"
    fontSize: "28px"
    fontWeight: 600
  metric:
    fontFamily: "Segoe UI"
    fontSize: "36px"
    fontWeight: 600
rounded:
  control: "0px"
  panel: "12px"
spacing:
  action-gap: "16px"
  column-gap: "20px"
  panel-inset: "24px"
  dashboard-inset: "28px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.control}"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
    textColor: "{colors.accent-ink}"
  button-primary-pressed:
    backgroundColor: "{colors.accent-pressed}"
    textColor: "{colors.accent-ink}"
  button-secondary:
    backgroundColor: "{colors.field}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.control}"
  button-secondary-hover:
    backgroundColor: "{colors.field-hover}"
    textColor: "{colors.foreground}"
  button-secondary-pressed:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.foreground}"
  button-disabled:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
  field:
    backgroundColor: "{colors.field}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.control}"
  sensor-panel:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.panel}"
    padding: "{spacing.panel-inset}"
---


# Design System: Omen Hub++

Reviewed **29 October 2026**. The approved direction is a modern, clean dark native instrument panel. Frontmatter preserves the shared palette/typography/spacing reference; runtime implementation is in `theme.rs` and the scoped chart palette in `curve_widget.rs`. This document defines intent and observed native composition, not blanket hardware/DPI/accessibility acceptance.

## Information hierarchy

GPU watts and temperature come first; CPU package watts, Tctl, load and interval-effective clocks sit alongside. Inventory and detailed controls follow. Measured, baseline, unavailable, selected, draft and confirmed-applied states must be named in text before color is used.

The dashboard has two sensor regions, three prominent tool entries—Performance profiles, Fan curve, Lighting—and explicit profile Apply plus fan/graphics choices. GPU V/F belongs under performance profiles. User custom presets separate GPU, CPU and Windows controls. Avoid long instruction walls on operational surfaces.

The current outer dashboard reference is 1140×780 with minimum 960×750. Width distributes sensor/action space; it is a native desktop layout, not a mobile breakpoint design. Open-window memory may exceed 50 MB; total resident tray idle including activated helpers must target 50 MB.

## Visual language

- Cool charcoal ground, slightly raised opaque panels and slate fields; bright neutral values and restrained muted support text.
- Mint primary actions with dark ink; clear hover, pressed, disabled and keyboard focus states.
- Flat rectangular native buttons/inputs; gently rounded sensor panels. No glass, animated gradients or continuous decorative rendering.
- Segoe UI with a deliberate value/heading/body hierarchy. Missing measurements use readable body text rather than a large fabricated number.
- Pale blue is the chart selection color. Zone colors, curve nodes and bands are data, not replacements for the app accent.

Use the established frontmatter tokens instead of introducing competing palettes. Earlier calculated contrast checks apply to the specified pairs, not comprehensive accessibility certification. Native EDIT/COMBOBOX behavior, labels, caret, selection and keyboard operation remain intact under custom drawing.

## Surface contracts

| Surface | Required distinction |
| --- | --- |
| Dashboard sensors | Package-energy watts versus limit; Tctl versus per-core temperature; effective intervals versus OS MHz |
| Presets | Optional included instruction versus observed value; Save versus Apply versus sign-in approval |
| GPU chart | Loaded-reference/draft curves and selected guides; no claim that the selection is a live operating point |
| Fans | Chosen preference versus fresh helper-confirmed regulation; pending BIOS handover remains explicit |
| Lighting | Real read colors and On/Off, four clickable zones, native picker; confirmed color applies immediately |
| Graphics | Requested BIOS preference versus active routing; show restart pending without claiming a physical switch was verified |

The current fan quick-switch copy does not yet fully satisfy its confirmation requirement; this is tracked in [Project review](docs/PROJECT_REVIEW.md). Documenting the design does not claim that bug fixed.

## Curves and keyboard

The curve canvas follows the user-provided Afterburner composition: large grid, square draft nodes, dotted loaded reference, selected guides, contiguous range and immutable GPU voltage anchors. Keep Shift/Alt/Ctrl editing, undo/redo, fit/zoom and numeric/keyboard alternatives. surrounding fields/actions use the shared app theme. [Curve behavior](docs/CURVE_EDITOR.md).

Lighting is a simple four-zone keyboard including numpad, On/Off and a short status line. It does not use a profile/hex/save/apply form or per-key promises. Zone controls are keyboard reachable; color picker cancellation leaves hardware unchanged. [Lighting UI](docs/KEYBOARD_UI.md).

## Native lifecycle and assets

Render in response to Windows messages; release/rebuild dashboard controls around tray visibility. Preserve native roles, readable focus and bounded GDI object lifetimes. High contrast uses sampled system roles where implemented; dynamic changes and the chart require additional review.

The original app icon is generated by `scripts/generate-icon.py` and embedded as a Windows resource. It is not an HP/OMEN/MSI logo. Raster mockups, downloaded fonts, WebViews and vendor assets are not required for the client UI. The documentation sidecar's HTML/CSS tiles are references only; the app never loads them.

## Review scope

Existing 96-DPI client renders and native smoke checks cover specific recorded builds/surfaces. They do not certify titlebars, every system dialog, multiple monitors/DPI, dynamic high contrast or comprehensive assistive technology. Keep those limits in [Validation](docs/VALIDATION.md) and the [Roadmap](docs/ROADMAP.md).

The versioned [surface briefs](.impeccable/surface-briefs/dashboard.md) record design requirements. Current support is always the [hardware matrix](docs/HARDWARE_MATRIX.md), not an old design brief or screenshot.
