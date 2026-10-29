# Usable laptop GPU controls

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Offer qualified core/VRAM offsets, full V/F editing, Max-Fan, Dynamic Boost and the conditional public GPU power field. The curve action must open from new unnamed and dirty presets without losing other draft fields. Keep native stable control IDs independent of layout.

## State and functional boundary

Direct GPU/VRAM mV, FPS limiter and locked clocks stay absent. Public watt control needs a configured original, which is unavailable on the reference laptop. Earlier removal of Dynamic Boost/power fields is superseded by current product decisions.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/GPU_PROFILES.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
