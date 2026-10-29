# GPU profiles and maximum cooling

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

A named preset contains optional supported instructions and explicit hardware Apply/sign-in authorization. Preserve full Graphics curves when editing names/VRAM, preserve dependency order and show Max-Fan as a distinct optional override.

## State and functional boundary

A nonuniform V/F curve conflicts with global core offset. Max-Fan Off releases the override, not proven BIOS ownership. Sign-in permission exists in current code but does not establish real reboot acceptance or durable HP independence.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/CUSTOM_PRESETS.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
