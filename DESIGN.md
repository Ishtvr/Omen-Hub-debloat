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

## Overview

**Creative North Star: "The Dark Instrument Panel"**

The user approved a modern, clean dark application on 14 October 2026. Charcoal surfaces, clear value fields and restrained mint actions give the native Windows interface a consistent visual language. Information density serves frequent hardware checks; typography and alignment carry the hierarchy.

Measured state, editable drafts and hardware actions keep distinct labels and places. The shared system covers the dashboard and native GPU, lighting and curve-editor controls. The curve plot retains its user-approved MSI Afterburner reference and its own data palette. The client UI requires no raster assets, downloaded fonts, animation or browser runtime; the only raster asset is the application icon embedded as a Windows resource.

**Key Characteristics:**

- Opaque dark surfaces and bright, legible text.
- Prominent measured values, explicit unavailable and baseline states.
- Flat rectangular native fields and actions, with gently curved sensor panels.
- Mint for a primary action; pale blue for curve editing selection.
- Event-driven rendering and bounded native resource lifetimes.

## Colors

The shared palette combines cool charcoal neutrals with a clear mint accent. Frontmatter values are extracted from `crates/app/src/theme.rs`; the scoped chart values come from `curve_widget.rs`.

### Primary

- **Mint action** (`accent`): the dashboard's GPU-profile entry and the editors' metadata-save buttons. The deeper ink color carries text and the inset focus frame on this fill.
- **Mint hover and pressed**: immediate native button feedback, without animation.

### Secondary

- **Pale blue selection** (`chart-selection`): curve points, selected guides and chart focus only. It represents an editing selection rather than a measured operating point.

### Neutral

- **Charcoal ground** (`background`): shared client background.
- **Sensor surface** (`panel`): dashboard sensor groups and disabled button fills.
- **Field slate** (`field`): editable/read-only fields, selectors and secondary actions. `field-hover` marks a hot secondary button.
- **Boundary gray** (`border`): rectangular secondary action and selector boundaries.
- **Bright neutral** (`foreground`): values, control labels and primary reading text. **Muted neutral** (`muted`) carries supporting explanations and disabled button text.
- **Soft red** (`error`): CPU cause/recovery text in an error state. The message still names the cause and next step.
- **Chart neutrals**: the plot ground, major/minor grids, dotted loaded reference, square draft nodes and range band are scoped to the curve canvas. They do not replace shared field or button colors.

**The State Before Color Rule.** Name measured, unavailable, draft, selected and preview states in text. Color reinforces that distinction.

Calculated contrast is 7.77:1 for muted text on the sensor panel, 6.56:1 on a field, 12.70:1 for bright text on a field, 10.46:1 for ink on the default mint action, and 3.13:1 for the shared boundary against a field. These pairs support the pictured controls; they are not comprehensive accessibility certification.

## Typography

Segoe UI is the native face for every role. The dashboard uses small supporting text, body labels, section headings, a product title and large measured values. Missing measurements use body size rather than a large fabricated value. GPU, lighting and numeric-curve controls retain their existing editor-body size; chart labels use the small size. The GPU curve helper and unsupported-capability explanation both use the shared small role, with their complete factual text and action geometry preserved.

The sizes in frontmatter correspond to the implementation's negative GDI `lfHeight` values. Dashboard body/small use weight 400; heading/title/metric use weight 600. Native control default metrics provide line layout. The current renders were inspected at 96 DPI; this scale is not a verified multi-DPI system.

## Layout

Use alignment and stable groups to help repeated scanning. Put a field beside its label, keep supporting provenance close to its value, and keep each deliberate action visibly distinct. Dashboard insets, panel padding and column/action gaps are the observed spacing tokens; editor surfaces retain their current native layouts.

The dashboard has two parallel sensor regions, followed by three equal tool actions and a secondary system/HP-preview row. Its current outer default is 1140×780, with minimum 960×750. Width changes distribute the sensor columns and action widths; this is a native desktop layout, with no mobile breakpoint. Exact composition and observed client captures belong in `.impeccable/surface-briefs/dashboard.md`.

## Elevation & Depth

Surfaces are opaque and flat. Tonal separation between the ground, sensor panels and fields supplies depth; there are no shadows, gradients, glass layers or animated transitions. Native input behavior and immediate hover/pressed/focus feedback remain the interaction cues. Rendering responds to Windows messages; it adds no continuous render timer.

## Shapes

Sensor panels use a GDI rounded rectangle with 24-unit corner diameters, corresponding to the panel radius in frontmatter. Buttons, text fields, selectors and chart geometry remain rectangular. A shared secondary button has a one-unit border. Focused owner-drawn buttons have an inset two-unit frame; a focused selector changes its boundary to mint. Native text fields retain their standard focus and caret behavior.

## Components

### Buttons

Actions are clear and direct. The native `BUTTON` role, labels, keyboard activation and enablement remain in place; push buttons are owner drawn through the shared theme. The primary uses mint with dark ink; secondary buttons use field slate, bright text and a visible boundary. Hover and pressed states use the extracted colors. Disabled buttons use sensor surface and muted text. The CPU approval button is disabled while Windows approval is starting, and its label describes that state.

### Cards / Containers

The dashboard's two sensor panels are flat tonal containers with gently curved corners and aligned internal values. Their measured values use metric typography; source, interval, clocks and limits use smaller supporting roles. Since 22 October 2026 the CPU panel combines the highest measured effective clock with load, keeps Windows-reported MHz on a separate line, and uses its existing detail line for logical-processor coverage and the longest measurement interval. Baseline and clock-only failure states use explicit text. The palette, type scale and panel geometry remain unchanged. Other windows use the shared client ground and field surfaces without adding decorative containers.

### Position switches

The dashboard's fan (**Auto / Kurve / Max**) and graphics (**Hybrid / GPU only**) switches are custom native controls in the shared palette. A `field` pill with a one-unit `border` rim holds a `border` rail with small `muted` stops; the selected stop is a larger `accent` knob. The option names sit underneath in the small role, the selected one in `foreground`. Hover enlarges and brightens a stop, keyboard focus turns the rim `accent`, and arrow/Home/End keys move the selection. Shapes are drawn at four times the size and halftone-scaled, so the circles stay smooth without anti-aliased GDI. Captions sit right-aligned beside each switch; **Neustart nötig** appears under the graphics caption while a chosen mode waits for a restart.

### Inputs / Fields

Standard Win32 `EDIT` and `COMBOBOX` controls retain their native roles and interaction. Both use field slate and bright text. The selector paints its selected text, chevron and boundary in the shared palette; its native dropdown remains the selection mechanism. Read-only status and inventory fields use the same surface vocabulary. Disabled values remain disabled controls, rather than becoming draft values.

### Draft color swatches

Lighting swatches show the typed RGB draft. Their colors are data, not system accents. The Windows `ChooseColorW` dialog supplies color selection. Firmware labels remain independent and state whether an explicit firmware GET has occurred.

### Curve canvas

The canvas keeps the scoped chart palette, rectangular grid, pale square draft nodes, dotted loaded reference, selected guides labelled **Ausgewählt**, and contiguous selection band. The editor's surrounding fields and actions use the shared theme. Fixed GPU voltage anchors, save-only behavior and unsupported hardware paths are functional contracts, not theme variations.

Windows high contrast is sampled when `Theme` is created and selects system color roles for shared surfaces, text and buttons. DWM dark-frame/caption/text attributes are requested in ordinary mode. The client captures do not verify titlebars, dynamic high-contrast switching or comprehensive assistive-technology behavior. The chart's scoped palette also remains outside a complete high-contrast certificate.

The design sidecar contains self-contained HTML/CSS reference tiles for the documentation panel. They translate these native primitives for inspection; the application does not load or execute them. Its synthesized tonal ramps are display metadata derived from each token's hue/chroma, rather than additional runtime palette steps.

### Application icon

The icon is a flat `panel` tile with a one-unit `border`-family rim and the panel's rounded corners. A mint gauge arc opens toward the bottom, a dim mint track marks the remaining range, and a pale mint needle ends in a bright `foreground` hub. Sizes from 48 pixels add the word **Hub** below the gauge, drawn as rounded `foreground` strokes in the gauge's line language rather than set in a font; at the user's request on 16 October 2026 it replaced an earlier **++** label; 32 and 40 pixels drop the label, and 16–24 pixels show only a thicker arc and hub. Every size is drawn separately by `scripts/generate-icon.py`, not downscaled, and the same `.ico` serves Explorer, shortcuts, taskbar, title bar and tray. It contains no HP or OMEN mark.

## Do's and Don'ts

### Do:

- **Do** use the shared dark theme for application controls and retain the chart's scoped data colors.
- **Do** keep native control roles, keyboard activation, readable labels and visible focus.
- **Do** show measured values, unavailable states, drafts and previews with explicit wording.
- **Do** preserve immediate event-driven feedback and release native view resources when the dashboard returns to tray.
- **Do** keep hardware qualification, transaction status and memory results tied to their evidence records.

### Don't:

- **Don't** invent sensor values or present Windows MHz as measured boost/effective clock.
- **Don't** present a metadata save or HP profile preview as a firmware change.
- **Don't** recolor the curve selection as a general hardware-success signal.
- **Don't** add a browser runtime, raster decoration or continuous render loop to this native visual system.
- **Don't** claim titlebar, multiple-DPI, dynamic high-contrast or full accessibility coverage from the inspected client renders.
