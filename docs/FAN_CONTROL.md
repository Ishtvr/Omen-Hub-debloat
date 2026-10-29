# Fan control

Current implementation on **HP 8E35 / BIOS F.13** has two distinct controls: the journaled **Max-Fan firmware override**, and the separate **own cooling helper** for Curve/Max. Neither proves complete BIOS ownership handover or long-term operation without HP controllers. [Hardware matrix](HARDWARE_MATRIX.md).

## Firmware interface observations

The own adapter uses `ROOT\WMI/hpqBIntM`, the `SECU` envelope and command **0x20008**. Fixed contracts check instance, COM types, status and exact output lengths; there is no arbitrary-opcode interface.

| Operation | Type | Contract |
| --- | --- | --- |
| Max-Fan GET | 0x26 | 4-byte complete state; byte 0 is 0/1 |
| Max-Fan SET | 0x27 | Exactly one boolean input byte; 4-byte response, then full GET |
| Tachometers | 0x2D | 128 bytes; CPU/GPU firmware values in 100-RPM units |
| Topology | 0x2C | 128 bytes; seven mapped slots and preserved capability bytes |
| Coupled table | 0x2F | 128 bytes; paired CPU/GPU levels plus acoustic raw field |
| Design data | 0x28 | 128 bytes; model capabilities, not live wattage |
| IR selector 0 | 0x23 | 4 bytes; manufacturer uses °C on Khalilah25C1 |
| Heartbeat | 0x10 | Stateful OMCC effect; not a harmless recurring GET |
| Software levels | 0x2E | 128-byte block, CPU/GPU levels in first two bytes |

The measured topology is CPU/GPU with capability bytes 0/0 and thermal-policy version 1. Table SHA-256: **`A8E624C6573881F444E36D94A29CA0AF320248855377531F02A499437BEE57F2`**. Header `[2,12]` has eleven nonempty triples; byte 1 is not treated as a row count. CPU/GPU pairing varies through the table; the acoustic field is not calibrated dBA.

IR selector 0 has manufacturer Celsius semantics, not a calibrated physical location. Unknown/sentinel values, wrong reserved bytes or bad timing are errors. The diagnostic plausibility range is not a certified control-safety threshold.

## Max-Fan preset override

The optional boolean stores On or Off. Unchecking removes only the instruction; it does not reset an override. Off releases that override and does not select a BIOS mode.

Capture checks board/BIOS, full flag, table hash, topology and design data before any SET. Complete originals are persisted, fresh expected state is checked and full readback follows. A new process can restore only matching original/own-target states; foreign or forged images are rejected. RPM values are observations, never restore targets.

The current controller permits Max-Fan sign-in authorization and has `requires_ac = false`; older AC-only/startup-blocked descriptions are obsolete. The hardware probes were bounded and do not qualify battery, actual reboot, sleep/resume or sustained maximum behavior.

## Cooling helper and quick choices

The own “fan service” is a protected elevated **scheduled-task helper**, not a kernel driver or an interactive Windows service UI. Its fixed task is `OmenHubPP-Fan-<SID>`, running `Omen Hub++ Fan.exe fan-service` from the installation directory.

| Choice | Meaning |
| --- | --- |
| Auto (`bios`) | Pause own software regulation after its conservative release; BIOS ownership is not confirmed |
| Curve (`curve`) | Use the saved active shared curve with CPU/GPU/IR protection |
| Max (`max`) | Request both levels 48, independent of CPU/GPU reader availability |

Absent preference defaults to Auto. Legacy `auto` meant “follow curve” and is read compatibly. The fan editor stores the active shared curve in `active-fan-curve.json`; complete performance-profile fan-curve instructions remain unsupported. [Curve editor](CURVE_EDITOR.md).

The loop checks full known layout before first arm, uses a model-bound heartbeat at 20 seconds and level cycles at roughly two seconds, waits while the original Hub is detected and releases unused readers while paused. Curves use fixed CPU Tctl, public GPU temperature and IR/protection observations, with smoothing and emergency maximum. Max avoids loading the CPU/GPU reader libraries.

Write packets share the own hardware lease. Temporary contention reports `waiting_for_control`; renewed control establishes a fresh heartbeat/maximum. Status includes PID, process creation identity, timestamps and the last actual heartbeat. Unknown vendor enumeration is not treated as safe absence.

After own regulation, Auto, missing/invalid curves and normal stop request **48/48 once**, then cease heartbeat/level writes and report `handover_pending`. A failed last write reports failure. A confirmed original Hub prevents a competing final SET. [Handover evidence](FAN_HANDOVER.md).

The same helper enforces saved foreground CCD preferences and releases original masks. That policy is independent of fan-curve selection. [System controls](SYSTEM_CONTROLS.md).

## Evidence and remaining work

15 October Max-Fan trials verified full flags, separate-process recovery and preserved other controls. A 22 October conservative helper trial reached **4800/4800 firmware RPM**. The 23 October watchdog/FAMC observations did not qualify a direct BIOS-Auto switch. HP services were still present in these trials.

The dashboard currently can announce Curve/Max after saving a choice, without a fresh helper acknowledgement. Treat it as selection until the status-confirmation [review item](PROJECT_REVIEW.md) is implemented. Durable own cooling, safe BIOS return, actual power transitions, full profile integration and independent HP replacement remain open. [Validation](VALIDATION.md), [HP control](HP_CONTROL.md).

`fan-read` and `cooling-read` are fixed diagnostics needing suitable permissions. `fan-validate-max`, `check-system-controls.ps1` and the opt-in migration trial perform changes; they are not ordinary status checks.
