# Native dashboard

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Show GPU watts and temperature first with CPU measurements beside them. Keep the approved charcoal/mint/Segoe UI system, prominent Performance profiles / Fan curve / Lighting actions, explicit Apply and native fan/graphics choices. Preserve visible baseline/unavailable/provenance text. Rebuilding after tray must preserve selection without applying hardware. Total tray idle includes all helpers; the open window may exceed 50 MB.

## State and functional boundary

Actual watts, configured limits, Tctl, interval-effective clocks and Windows MHz must remain distinct. A saved choice is not execution confirmation; fan-helper acknowledgement is an open implementation item.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/GETTING_STARTED.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
