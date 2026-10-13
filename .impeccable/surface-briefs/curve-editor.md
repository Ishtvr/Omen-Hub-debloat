# Curve editor

Mode: Operate. Platform: native Windows x64, Rust/Win32/GDI. Build path: code.

## Direction contract

THESIS: A large editable V/F plot lets the user inspect and shape the complete frequency curve before saving a profile. The user's MSI Afterburner screenshot pins this surface's dark chart and interaction vocabulary.

OWN-WORLD: The implemented plot uses dark neutral surfaces, fine major/minor grids, pale square draft nodes and a dotted loaded reference. Pale blue identifies the selected node, contiguous voltage band and keyboard focus. This is a local extension and establishes no product-wide visual identity.

STORY: Select a point or voltage range, edit the draft, compare it with the loaded reference, undo or fit the complete curve, and save metadata. Hardware application is a separate approved operation in the GPU profile window.

FIRST VIEWPORT: Diagram-first window with GPU V/F, CPU fan and GPU fan selectors and save/history/help controls above the large plot. Selected coordinates are labelled “Ausgewählt”; units and loaded-reference legend remain visible. Status and zoom/fit controls sit below. Narrow layout gives the save/history controls a second toolbar row. “Profil / Werte …” exposes native profile and numeric controls.

FORM: Preserve all 127 immutable GPU anchors; Shift uses the loaded baseline, Alt the current draft, Ctrl tilts around the opposite endpoint. Uniform bounded range offsets, selection flattening, 64-step history and view-only zoom/pan/fit use original Rust policies. Separate fan drafts remain editable and saveable, with hardware application unsupported.

QUALITY BAR: Read the draft and loaded baseline separately, identify selected coordinates as editing state, keep the complete domain available through fit, and make save-only status explicit. F1 and a visible help button explain gestures and unsupported lock/OC Scanner/fan application.

## Persistent facts checked

- Palette from `crates/app/src/curve_widget.rs`: background `#1C1F22`, plot `#1C1D1F`, controls `#272A2D`, foreground `#DEE2E5`, draft `#C7CDD3`, reference `#848D97`, selection/focus `#85CDF4`. These are converted from native BGR COLORREF constants and scoped to this surface.
- Segoe UI uses GDI `lfHeight = -18` for native controls and `-14` for the plot. No display-font hierarchy, downloaded font or extra UI runtime is present.
- The plot uses square nodes, fine rectangular grids, a dotted loaded reference and selected guides, plus a filled range band. It labels the guides “Ausgewählt”; they represent an editable point rather than a measured GPU operating point.
- `core/curve_editor.rs`, `curve_input.rs`, `curve_view.rs`, `app/curve_widget.rs` and `curve_ui.rs` are the behavioral source of truth. The public [MSI guide](https://www.msi.com/blog/msi-afterburner-overclocking-undervolting-guide) is an interaction reference; no MSI binaries, drivers or assets are required or copied.
- Reviewed own-client GDI/WM_PRINT renders: `.impeccable/review/gpu-window.png`, `gpu-small.png`, `cpu-fan-window.png` and the explicitly unsaved/unapplied `gpu-example.png`. The finish reviewer returned SHIP for their charts and pictured client controls. Drawing, palette and layout are unchanged since those captures; a later GPU-profile label correction and band-selection cancellation fix affect behavior only.

## Evidence boundary and existing context gap

The review excludes titlebar, the full numerical view, multiple DPI settings and comprehensive accessibility certification. `gpu-controls.png` is excluded because the own-client renderer leaves hidden EDIT/COMBOBOX text incomplete. No native HTML/CSS detector verdict is claimed. Hardware application, physical voltage ceilings and load stability are outside the visual verdict.

The root has no `DESIGN.md` or global design sidecar. `PRODUCT.md` still leaves dashboard identity open. This ordinary curve extension preserves that scope and does not repair or invent a global visual system. No generic chart-skill dataset is persisted.
