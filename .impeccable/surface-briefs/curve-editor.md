# Curve editor

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Keep a large native GDI V/F canvas using the user's Afterburner composition, square draft nodes, dotted loaded reference, selected guides, band selection, Shift/Alt/Ctrl gestures, history, zoom and numeric alternatives. Surrounding controls follow the shared theme.

## State and functional boundary

GPU voltage anchors stay immutable. The preset chart edits an in-memory draft; Accept/Cancel do not Apply hardware. Fan drafts and active helper curves are separate from unsupported embedded performance-profile curves. No L lock or OC Scanner support is implied.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/CURVE_EDITOR.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
