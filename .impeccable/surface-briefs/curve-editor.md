# Curve editor

Mode: Operate. Platform: native Windows x64, Rust/Win32/GDI. Build path: code.

## Direction contract

THESIS: A large editable V/F plot lets the user inspect and shape the complete frequency curve before saving a profile. The user's MSI Afterburner screenshot pins this surface's dark chart and interaction vocabulary.

OWN-WORLD: The plot retains dark neutral surfaces, fine major/minor grids, pale square draft nodes and a dotted loaded reference. Pale blue identifies the selected node and chart focus; a dark blue-gray band marks contiguous voltage selection. Surrounding native fields/actions now inherit the approved shared dark system in root DESIGN.md; this does not replace chart data semantics.

STORY: Select a point or voltage range, edit the draft, compare it with the loaded reference, undo or fit the complete curve, and save metadata. Hardware application is a separate approved operation in the GPU profile window.

FIRST VIEWPORT: Diagram-first window with GPU V/F, CPU fan and GPU fan selectors and save/history/help controls above the large plot. Selected coordinates are labelled Ausgewählt; units and loaded-reference legend remain visible. Status and zoom/fit controls sit below. Narrow layout gives save/history controls a second toolbar row. Profil / Werte … exposes native profile and numeric controls.

FORM: Preserve all 127 immutable GPU anchors; Shift uses the loaded baseline, Alt the current draft, Ctrl tilts around the opposite endpoint. Uniform bounded range offsets, selection flattening, 64-step history and view-only zoom/pan/fit use original Rust policies. Separate fan drafts remain editable and saveable, with hardware application unsupported.

QUALITY BAR: Read draft and loaded baseline separately, identify selected coordinates as editing state, keep the complete domain available through fit, and make save-only status explicit. F1 and a visible help button explain gestures and unsupported lock/OC Scanner/fan application.

## Persistent facts checked

- Chart values from `app/curve_widget.rs` are native COLORREFs, converted to RGB and recorded as scoped frontmatter tokens in DESIGN.md. Plot, reference/draft, rectangular grids, hover plate, selection color and band remain chart-specific; controls use `theme.rs`.
- Native curve controls retain Segoe UI GDI lfHeight -18, and chart labels use -14. Shared pushbuttons use the same primary-save/secondary/disabled/focus treatment as the other editors. No downloaded font, extra UI runtime or render timer is present.
- Square nodes, fine rectangular grids, dotted loaded reference and selected guides remain intact. Guides labelled Ausgewählt describe an editable point rather than a measured GPU operating point.
- `core/curve_editor.rs`, `curve_input.rs`, `curve_view.rs`, `app/curve_widget.rs` and `curve_ui.rs` remain behavior truth. The public [MSI guide](https://www.msi.com/blog/msi-afterburner-overclocking-undervolting-guide) is an interaction reference; no MSI binaries, drivers or assets are copied or required.
- `.impeccable/review/dark-curves.png` (1364×901) was opened and checked. It shows the actual full numerical client view with selected profile/name, point selector, numeric EDIT values, shared actions/status and unchanged chart hierarchy at the current DPI.
- `curve_widget.rs::render_window` paints the production parent then native children. Offscreen preview visibility makes EDIT text printable; selected COMBOBOX text is also present. The old hidden numeric-control capture limitation is superseded by this current evidence.
- Earlier own-client chart captures at large/small sizes and the explicitly unsaved/unapplied example remain historical scoped evidence. The earlier reviewer returned ship for those pictured charts/client controls. The current theme work does not change fixed voltage anchors, curve policies or hardware paths.

## Evidence boundary

The current full numerical view supports pictured client geometry, labels, native field text and shared control treatment. Titlebar, multiple DPI/display configurations, dynamic high-contrast behavior and comprehensive accessibility remain outside the capture. The chart palette is not a complete high-contrast certificate. No native HTML/CSS detector verdict is claimed.

The current dark finish verdict is **ship / G1 resolved**, scoring the single GPU helper/capability typography correction at the original pictured-client scope after re-opening all five valid captures. It does not broaden chart/DPI/accessibility certification or cover later telemetry/hash-lifetime changes and real power transitions. Current native smokes and build-scoped memory results are recorded in [VALIDATION.md](../../docs/VALIDATION.md). Visual approval does not prove hardware application, physical voltage ceilings, measured load stability or fan control. The plot remains an editor of profile drafts; editing/save changes metadata only.
