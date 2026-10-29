# Direct keyboard lighting

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

The current lighting surface is four zones, native color picker, On/Off and short status—not the earlier save/check/apply/restore profile form. Keep real named keyboard-focusable buttons and the shared dark native controls.

## State and functional boundary

The exact supported capability is FourZoneWithNumpad on 8E35/F.13. On/Off was visibly confirmed; general brightness, animation/per-key control and independent power/hotkey lifetime remain unqualified. Dedicated durable recovery preserves the performance store.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/KEYBOARD_UI.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
