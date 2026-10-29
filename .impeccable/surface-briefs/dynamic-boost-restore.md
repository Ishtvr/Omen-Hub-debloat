# Dynamic Boost

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Retain the user's required Dynamic Boost option in GPU with a simple included On/Off instruction. Preserve legacy gpu_unleashed and gpu_dynamic_boost aliases as one logical setting, with conflict-aware merge.

## State and functional boundary

The current PPAB flag has readback/restore and preserves cTGP. It is distinct from factory Unleashed and free TGP. A reported effective 115-W limit is not sustained measured draw. Sign-in and model checks still apply.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/GPU_PROFILES.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
