# Lighting and custom presets

Design intent reviewed **29 October 2026**. Native Windows x64 / Rust / Win32, code-first. This is a surface brief, not a hardware support or validation record. Earlier implementation directions are superseded by the current product/workflow decisions below.

## Direction

Replace text-heavy lighting with four native clickable zones including numpad, On/Off and a short state line. Confirmed picker choice writes/readbacks immediately. Custom presets select/create/name, visually separate GPU/CPU/Windows and offer Save/Apply/Delete plus explicit sign-in choice.

## State and functional boundary

Picker Cancel writes nothing; preset Save is metadata. Show stale/external state and pending/failed action honestly. Native dark controls and keyboard roles remain; no vendor artwork/code is included.

## Visual and review contract

Preserve the shared [design system](../../DESIGN.md), clear native labels, keyboard focus and explicit unavailable/pending/confirmed states. Use event-driven rendering without a browser runtime, copied vendor assets or decorative polling. Existing screenshots/native smokes cover their recorded builds and client geometry only; DPI/display/high-contrast/accessibility acceptance stays separate.

Current behavior: [topic guide](../../docs/KEYBOARD_UI.md). Current support: [hardware matrix](../../docs/HARDWARE_MATRIX.md). Evidence: [Validation](../../docs/VALIDATION.md). Product requirements: [PRODUCT.md](../../PRODUCT.md).
