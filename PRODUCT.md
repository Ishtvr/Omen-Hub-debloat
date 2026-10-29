# Omen Hub++

Confirmed product requirements, reviewed **29 October 2026**. Product name: **Omen Hub++**; Cargo package/binary names: `omen-hub-plus-plus` / `omen-hub-pp`.

Current implementation and support belong in the [hardware matrix](docs/HARDWARE_MATRIX.md), measurements in [Validation](docs/VALIDATION.md), and pending work in the [Roadmap](docs/ROADMAP.md). This file records durable requirements, not a build-by-build changelog.

## Purpose

A native, lightweight Windows application that brings practical OMEN hardware control and relevant UXTU-style tuning into one place. It should eventually replace original OMEN user-mode hardware controllers completely, with no ads, overlays, account requirement, telemetry or background updater.

The target user wants to inspect GPU power and temperature first, see CPU performance beside them, then select or edit profiles. The interface is modern, clean and dark. The workflow is code-first in Rust with real native controls and visual review.

## Platform and resource requirements

- Windows x64 first. The current UI is Win32/GDI; no Electron, UWP shell or embedded browser runtime.
- One standalone application binary may also run fixed-purpose helpers. UI and tray stay in the user session; privileged work uses explicit UAC/fixed helpers where required.
- **50 MB total tray-idle working set**, including resident helpers after sensors have been activated. The open window may exceed 50 MB to improve the UI. These user decisions date to 13–14 October 2026.
- Event-driven drawing, suspended dashboard polling while hidden/asleep, bounded readers and explicit stale/unavailable states.
- Runtime is offline. Build preparation and developer tools may require downloads.
- The GPU-TGP investigation requires user-mode application processes, optionally elevated, with **TPM and Secure Boot enabled**. Test-signing, disabling signature enforcement and adding test trust are not acceptable product paths.

## Product surfaces

| Surface | Intended workflow |
| --- | --- |
| Dashboard | GPU watts/temperature and CPU measurements first; performance-mode selection and Apply; three control entries; fan/graphics quick choices |
| Performance profiles | Select/create/name a custom preset; separate GPU, CPU and Windows sections; Save / Apply / Delete and explicit sign-in authorization |
| GPU curve | Interactive full V/F chart within performance profiles, with Shift/Alt/Ctrl editing, history, zoom and numeric alternatives |
| Fan curve | Native CPU/GPU curve editor for the separate cooling helper; confirmed execution state must remain distinct from saved intent |
| Lighting | Four clickable zones including numpad, native color picker, On/Off and a short state line; immediate verified application |
| Tray | Reliable own icon after minimize/close/Explorer restart, open/exit actions and optional unelevated Windows start |

The MSI Afterburner chart is a scoped interaction/composition reference; the surrounding app keeps its own design. UXTU's custom-preset grouping is a workflow reference. No vendor code, bitmap assets or proprietary runtime wrappers are copied into the product.

## Profiles and hardware behavior

- A saved preview is metadata. Hardware Apply and sign-in authorization are separate actions.
- A custom preset can contain GPU, CPU and Windows settings together. One unsupported instruction blocks the whole transaction; it must not appear as partial success.
- Originals must be captured durably before a write, targets read back, and valid original state restored on failure. A process mutex does not replace a journal or exclude another vendor's writer.
- Device/BIOS/driver evidence is specific. A feature name, public setter export, default limit or inferred opcode does not establish support.
- Configured, requested, effective and measured values are different. Windows MHz is not measured boost; package watts need an energy interval; Tctl is not a per-core temperature.
- GPU V/F edits change frequency targets at immutable driver-read anchors. They do not establish a hard voltage ceiling. Curve Optimizer steps are not a fixed mV offset.
- The factory Unleashed mode and **Dynamic Boost** flag are separate from a future numeric TGP control. Full OEM sustained power still needs load evidence.
- Startup permission for lighting and Max-Fan is allowed in the current code following the user request; real lifecycle acceptance remains pending. New CPU PPT/Slow startup approval remains gated by HP-controller ownership.
- Actual sleep/resume, reboot, AC/DC, controller failure and independent cooling behavior need their own acceptance. Do not call successful message simulation a physical transition test.

## Scope decisions

Preserve separate GPU / CPU / Windows sections. Direct GPU/VRAM mV controls, FPS limiter, fixed CPU clock/voltage and APU/iGPU tuning menus were removed at the user's request. Legacy keys remain readable where needed for compatibility; the 91-key schema is not a promise of 91 working adapters.

Intel and additional device support remain goals requiring actual backend work and test hardware. Four-zone static lighting is the current target; extra effects/per-key RGB depend on real capabilities and separate restore contracts.

## Independence and safety

Original HP components are reverse-engineering sources, not target runtime dependencies. Existing regular Windows/firmware/device drivers remain platform infrastructure. Full replacement requires necessary controller lifecycle and cooling ownership, not merely closing the original GUI or uninstalling services.

Routine verification must not change power/voltage, stop vendor services, install kernel drivers or flash firmware. Hardware experiments require a concrete, evidenced scope and preserved originals. The broad CPU ranges now accepted by code are not fully measured safe ranges; this remains an explicit [review finding](docs/PROJECT_REVIEW.md).

## Documentation contract

Keep the public [README](README.md) short and in English. Put usage, building and recovery in topic guides. Maintain one current support table, a current validation summary and an ordered roadmap. Preserve dated measurement history separately with build identities; never let old restrictions or old RAM numbers override current support.
