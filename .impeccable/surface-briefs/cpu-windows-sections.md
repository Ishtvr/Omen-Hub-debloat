# CPU and Windows sections

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Use clear GPU / CPU / Windows boundaries over declarative collapsible groups. CPU contains Tctl, Slow/PPT, TDC/EDC, PBO and CO. Windows contains boost, processor percentages, EPP/core parking, CCD, energy/display/graphics choices. Keep name/selection/footer stable with native focus and scrolling.

## State and functional boundary

Removed APU/iGPU menus and fixed CPU clock/voltage stay absent. Intel fields are not an implemented Intel backend. Keep supported Dynamic Boost in GPU; do not infer safety from broad input ranges.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/CUSTOM_PRESETS.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
