# Intel removal and unused-code review — 29 October 2026

Scope: `0.9.0-dev.2`, working tree based on `b58d62c`, including the preceding native curve optimization. The user removed Intel CPU tuning from the product and requested an unused-code/disabled-feature review. This record describes software changes and read-only observations, not new hardware write qualification.

## Intel removal and old files

- Removed the Intel group, its five remaining visible controls, all Intel field/help translations and the unused native Core-Ratios editor/parser.
- Removed all ten Intel keys from the current `Feature` enum variants and advertised catalog. The catalog now contains **81** entries; it still includes some other compatibility-only instructions, so this is not a working-adapter count.
- Isolated the ten old spellings in [the retired-input decoder](../../crates/core/src/features/retired.rs). Old profiles/journals retain their serialization and digest. New `profile-set` edits reject these keys; a profile containing them cannot apply any subset. Explicit Save in the preset editor removes the retired entries, while retaining unrelated data and checking concurrent edits.
- Preserved existing GPU/CPU/Windows field control IDs. Removed the empty group and adjusted the Windows section/group indices and affected smoke scripts.

The compatibility decoder and legacy `core_ratios` value validation are intentionally retained. Removing them would make old files unreadable and could prevent the profile list or recovery from opening. They expose no Intel controls and have no hardware writer.

## Unused and unreachable code

The review combined compiler/Clippy checks, CLI/UI call-site inspection, dependency-use inspection and a public-declaration reference scan. The final reference scan found no public declaration with only its definition in the Rust sources; this is a useful bounded check, not a proof that every possible execution path is reachable.

Removed:

- The unused `Feature::needs_ac` policy. Live requirements continue to come from adapter capabilities.
- The unused `ProfileStore::clear_active_fan_curve` production API; its test now constructs the absent-file fixture directly.
- The never-constructed `Access::ReadOnly` variant.
- Four permanently-true historical write-qualification switches and their unreachable disabled branches for measured Slow PPT, Max-Fan and keyboard controls.
- The ineffective Slow-PPT qualification state/constructor; the existing fixed qualification command now uses the same controller as other cases.
- Unreachable fallback messages for GPU power and Dynamic Boost, plus choice branches for options absent from the UI.

Calibration target restrictions, CLI qualification/recovery commands, the guardian, exact board/BIOS/driver checks and runtime ownership checks remain used code. Dependency entries all had build/runtime consumers. No speculative hardware transport was activated.

## Disabled-feature findings

The removed qualification switches were already `true`. Simplifying them **does not expand hardware or startup permissions**. The implemented controls are available through their current adapters; no extra functioning adapter was found behind a global off switch.

The release's read-only `gpu-capabilities` query reported 26 `read_write` catalog entries on this device, including the Dynamic Boost alias and the separate lighting controls. It also confirmed these specific limits:

| Item | Observation and next test/work |
| --- | --- |
| GPU/VRAM and V/F | VRAM offset and V/F are available. The global core-offset path is blocked while the current nonuniform V/F curve is active; the curve and global offset address the same domain. Use the documented original-restore path before testing the alternate mode. |
| Numeric GPU power | `NVML power limit GET: status 3`. Without a readable configured original the existing adapter cannot establish restore. Further transport/original-state work is required, not just a UI unlock. |
| CPU PPT/Slow | Adapter exists. Startup permission is false while HP-controller ownership requirements are unmet; normal writes also recheck the competing controller. Existing fixed qualification/migration commands retain their requirements. |
| Fan curves | The separate fan editor/helper remains available. Embedded CPU/GPU fan-curve profile transactions are unsupported because ownership/original restore is incomplete. |
| Other implemented paths | AMD CO/PBO/TDC/EDC/Tctl, Windows policies, MUX preference, Max-Fan and four-zone lighting retain their current permissions. Real reboot/resume, independent ownership and load stability still require the documented acceptance. |

Current support remains centralized in the [hardware matrix](../HARDWARE_MATRIX.md); pending implementation remains in the [roadmap](../ROADMAP.md). The application is not yet a fully qualified replacement for every HP controller.

## User test sequence

1. Open presets and verify the GPU / CPU / Windows groups, checkbox editing, Save/reopen and curve Undo/Redo. Intel controls must be absent. Do not select sign-in or Apply for a metadata-only check.
2. Test the already implemented hardware paths separately using a previously tested profile and its recorded originals. Check actual readback and restoration; a saved setting is not execution confirmation. [GPU controls](../GPU_PROFILES.md), [CPU/Windows controls](../SYSTEM_CONTROLS.md), [lighting](../KEYBOARD_UI.md).
3. Test the separate fan helper with its documented ownership/status checks. A selected curve/Max preference is not proof that the helper has acquired control. [Fan control](../FAN_CONTROL.md).
4. Perform the remaining sign-in, actual sleep/resume, AC/DC and longer load/total-tray-memory acceptance under [Power lifecycle](../POWER_LIFECYCLE.md) and the roadmap. Use the candidate's exact build identity; installing a new binary invalidates prior protected sign-in approval.

## Verification

Six new tests cover retired-key round trips, invalid legacy input, all-or-nothing rejection, new CLI rejection, UI catalog/groups/IDs and file-preserving explicit cleanup with concurrent edits. Together with the prior work: **246 passing tests**, one explicitly ignored manual timing fixture, and **14 guardian process scenarios**. Format, Clippy with denied warnings and release build passed.

Native GPU and CPU preset previews at 1100 × 900 and the Windows section at 920 × 720 were rendered and visually inspected. The catalog CLI returned 81 entries and zero Intel entries. Both changed smoke scripts pass PowerShell parsing; their interactive/hardware-adjacent workflows were not run in this review.

Candidate: **3,123,200 bytes**, SHA-256 **`C4E4DEA4D864827DF3C9E6A333756819B75611F5DB7C5D526EF5C4F3832A2B1A`**. No installation, hardware writes, vendor-service changes or physical power transitions were performed. Latest summary: [Validation](../VALIDATION.md).
