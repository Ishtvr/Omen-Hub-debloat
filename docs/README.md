# Documentation

Reviewed **29 October 2026** against source `251fff6`, version `0.9.0-dev.2`. This refresh changes documentation only. Hardware observations remain tied to their recorded device, date and build.

## Start here

| Goal | Read |
| --- | --- |
| Run the app and understand the controls | [Getting started](GETTING_STARTED.md) |
| Check what works on a particular configuration | [Hardware matrix](HARDWARE_MATRIX.md) |
| Restore settings or uninstall | [Recovery](RECOVERY.md) |
| Build, prepare CPU modules or make a package | [Building](BUILDING.md) |
| Find pending implementation and testing | [Roadmap](ROADMAP.md) |
| Inspect the latest software checks and measured limits | [Validation](VALIDATION.md) |

The **hardware matrix** is the current feature-status reference. The **validation summary** identifies the checks behind it. The **roadmap** is the ordered work list. A feature catalog entry, saved preset or successful API return is not a completed hardware qualification.

## Controls

| Topic | Usage and implementation |
| --- | --- |
| Custom presets and sign-in application | [Custom presets](CUSTOM_PRESETS.md) |
| GPU offsets, V/F and power limits | [GPU controls](GPU_PROFILES.md), [curve editor](CURVE_EDITOR.md) |
| CPU measurements | [CPU sensors](CPU_SENSORS.md) |
| CPU tuning and Windows policies | [System controls](SYSTEM_CONTROLS.md) |
| Cooling | [Fan control](FAN_CONTROL.md), [BIOS handover](FAN_HANDOVER.md) |
| Keyboard | [Lighting UI](KEYBOARD_UI.md), [lighting protocol](KEYBOARD_LIGHTING.md) |
| Power transitions | [Power lifecycle](POWER_LIFECYCLE.md) |

## Development and research

- [Architecture](ARCHITECTURE.md), [product requirements](../PRODUCT.md), [design system](../DESIGN.md), [localization](LOCALIZATION.md).
- [Project review](PROJECT_REVIEW.md) and [development/release status](UNLOCKED_STATUS.md).
- [Replacing HP controllers](REPLACEMENT_PLAN.md) and [controlled HP migration](HP_CONTROL.md).
- [Reverse-engineering method](REVERSE_ENGINEERING.md), [8E35 evidence](models/8e35.md), [HP mode observations](models/8e35-hub-modes.md), [NVIDIA 617.42 qualification](NVIDIA_61742.md).
- [TGP investigation plan](plans/NVPWRCONTROL_TGP_FORK_PLAN.md). Investigation is authorized; the proposed new backend and hardware writes remain deferred.
- [Dependencies and licenses](../THIRD_PARTY.md).

## Historical records

[History](history/README.md) preserves the earlier measurement log and source observations. These entries may describe old restrictions, old UI or unpublished development binaries. They do not override current status. The [withdrawn beta notes](releases/0.9.0-beta.1.md) are historical too.

Raw dumps, proprietary binaries, decompilation and local tools stay in ignored `.local/` and `.tools/`. They are not distributed as project documentation or runtime dependencies.
