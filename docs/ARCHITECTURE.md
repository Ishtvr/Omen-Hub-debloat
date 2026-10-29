# Architecture

Reviewed against `0.9.0-dev.2`. [Hardware support](HARDWARE_MATRIX.md) and [Validation](VALIDATION.md) define what the architecture has actually demonstrated.

## Crates

| Crate | Responsibility |
| --- | --- |
| `omen-core` | Platform-independent typed settings, editing, preflight, transactions, recovery and ownership policies |
| `omen-platform` | Windows hardware/API adapters, persistent stores, fixed helper/service/task inspection |
| `omen-hub-plus-plus` | Native UI/tray, CLI, bounded read processes, UAC entry points, localization |

Core policies contain no Windows handles, private driver structures or network calls. Platform contracts do not accept user-supplied opcodes, kernel addresses or arbitrary modules.

## Processes and reading

The native dashboard runs a blocking Win32 message loop. Its worker polls visible sensors approximately every three seconds. Hiding it releases dashboard child controls; reopening rebuilds them without applying a profile. Power notifications independently track suspend, display state and source changes.

Inventory and general NVIDIA telemetry use the current executable as short-lived read children in a private job. Commands are fixed, output bounded to 64 KiB and the child deadline is five seconds. The serializing GPU worker permits six seconds for delivery. A read-only child can be terminated at its deadline; hardware transaction children are not killed using this mechanism.

The CPU reader is a separate elevated, session-bound process. A temporary UAC launcher reports its process ID through a private pipe. The local channel binds user/SYSTEM access, both peer identities, nonce, schema, operation and sequence; replies are bounded. Fixed preparation/sample/pause/stop operations accept no profile, register address or arbitrary function. Pause releases driver/module sessions and both energy/clock baselines. Preparation has its own inventory/open budget. See [CPU sensors](CPU_SENSORS.md).

Public NVAPI/NVML provide GPU telemetry. Windows provides CPU load and explicitly labeled OS MHz. Exact PawnIO/module identities permit the fixed energy/Tctl/APERF reader on the reference device. Invalid or stale measurements remain unavailable.

## Preset transaction

`core::transaction` validates the whole profile, device binding, capabilities, values and dependencies before writing. `Backend` supplies typed originals, live checks, write order, readback and restore. `Journal` saves originals before the first SET and retains unresolved recovery.

The Windows controller owns optional NVIDIA/V/F, HP, CPU-SMU and Windows adapters. Public GPU power control is a separate adapter. GPU memory writes precede the final Graphics setting because the measured memory setter can clear custom V/F deltas. Even memory-only transactions preserve coupled Graphics state.

`profile_store.rs` uses bounded JSON, durable atomic replacement and revision checks in the token's user-profile directory. Legacy migration preserves old files and pending barriers. Deleting a preset archives exact bytes and revokes its sign-in selection; it does not restore hardware.

The global own-process control lease serializes writing sessions. It does not exclude HP/UXTU/Afterburner writers. Hardware originals, helper responsibility and OEM-controller responsibility have distinct journals.

## Native editors

- `preset_fields.rs` declares groups/units and stable control slots; `preset_ui.rs` handles draft state, merge, Save/Apply/Delete and sign-in requests.
- `curve_editor`, `curve_input` and `curve_view` separate immutable anchors, editing policy, history and viewport. The Win32 canvas renders on events. Accepting an in-memory chart draft does not write a file or hardware.
- `keyboard_ui.rs` owns one elevated direct-lighting worker. `DirectLighting` uses a separate transaction store, full state checks and the shared lease. A confirmed color applies immediately; Cancel does not write.
- `theme.rs` supplies the shared dark native appearance. `.impeccable/` is design documentation, not app-loaded HTML/CSS. See [Design](../DESIGN.md).

`i18n.rs` owns the immutable per-process German/English choice, with explicit `--lang` propagation to separate UI/UAC processes, display-number formatting and language-neutral error rendering. Known JSON preflight/rollback messages are decoded before display; opaque provider details remain labeled diagnostics. Persistent values and fixed task/IPC identifiers are not translated. [Localization](LOCALIZATION.md).

## Sign-in, cooling and ownership

The ordinary Windows-start checkbox uses the user's Run entry to launch the unelevated tray. It does not apply settings.

Profile sign-in uses a protected fixed binary/action, task/principal verification, binary digest and approval of an exact applied profile revision. It performs full preflight again. Binary replacement or profile edits invalidate approval. Lighting and Max-Fan are currently allowed by capability policy; their full physical lifecycle is not yet accepted. CPU PPT/Slow have an additional HP-controller gate.

The separate elevated fan/CCD helper follows saved choices, checks vendor presence, reads CPU/GPU/IR for curves, sends fixed model-bound heartbeat/levels and reports explicit waiting/failure/handover state. Max does not need CPU/GPU reader libraries. After own regulation, normal pause/stop first requests full cooling once and stops sending; this does not confirm BIOS takeover.

The prepared HPOmenCap migration route adds durable owner/cooling records and an independently bound START-only return guardian. Its bounded qualification is separate from ordinary UI/sign-in; it has not yet been physically accepted as the current native takeover. [HP control](HP_CONTROL.md).

## Known gaps

- Dashboard HP-mode and MUX shortcut writes do not yet have preset-equivalent pending-journal and write-ahead protection. The HP thermal mode is not fully readable; Balanced fallback is not original restore.
- The fan quick-switch message can precede a fresh actual helper acknowledgement.
- CPU accepted ranges are wider than the measured test envelope.
- Dashboard power events reset sensor sessions; the general `core::supervisor` policy is not wired to a full production profile-resume controller. Sign-in application is a separate path.
- Custom fan curves run in the helper but remain unsupported inside complete performance-profile transactions.
- Direct diagnostic/provider calls and writing children can still block. Real provider/driver hangs, power transitions and full independent lifetime need acceptance.
- Multiple GPUs, broader hardware, DPI/high-contrast/accessibility and the total current helper memory remain incomplete qualifications.

These are recorded as pending work, not fixed by this documentation refresh. [Project review](PROJECT_REVIEW.md), [Roadmap](ROADMAP.md).
