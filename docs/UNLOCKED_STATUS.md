# Development and release status

Reviewed **29 October 2026**, source version **`0.9.0-dev.2`**. The first beta release and assets were withdrawn; its source tag remains historical. No replacement release was present at the latest repository check.

The request for an “unlocked” release includes implementing missing hardware paths. Removing a UI check cannot supply a missing adapter, original-state query or restore contract. Use the [hardware matrix](HARDWARE_MATRIX.md) for current availability and the [roadmap](ROADMAP.md) for the work order.

## Main unfinished work

- Shared recovery protection for HP-mode/MUX shortcuts and truthful fan-helper status confirmation.
- Evidence-based CPU range policy; allowed protocol values exceed the measured test envelope.
- Durable cooling/controller ownership without original HP services, then full profile lifecycle.
- Real reboot, sleep/resume, AC/DC, failure recovery and longer stability/RAM acceptance.
- A verified numeric GPU-TGP path. The public NVML adapter exists, but its configured original cannot be read on this laptop. The alternate [TGP investigation](plans/NVPWRCONTROL_TGP_FORK_PLAN.md) remains research only.
- Physical MUX/iGPU-only operation and independent keyboard/hotkey lifecycle.
- Intel backend and additional models, each with suitable hardware evidence.

Qualified offsets/V/F, static lighting, short fan trials and CPU/Windows controls retain their documented scope. Intentionally removed voltage/FPS/APU controls are not pending accidental unlocks. Latest checks and their limits: [Validation](VALIDATION.md). Detailed code findings: [Project review](PROJECT_REVIEW.md).
