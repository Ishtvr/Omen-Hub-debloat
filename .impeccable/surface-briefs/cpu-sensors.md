# CPU sensors

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Measured CPU package-energy mean watts, labeled Tctl and interval-effective clocks join the GPU-first view. Preserve the native dark panel hierarchy, source/coverage/interval and readable first-baseline/error states. UAC belongs to the session-bound reader; cancel retries explicitly via Refresh.

## State and functional boundary

Keep OS MHz separate from measured effective clocks and active boost peaks. CPU-only clock faults may leave valid watts/Tctl. Pause releases reader sessions/baselines. Real power transitions and total current helper memory remain separate acceptance.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/CPU_SENSORS.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
