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

## Legacy Max-Fan instruction and separate control

The optional boolean stores On or Off. Unchecking removes only the instruction; it does not reset an override. Off releases that override and does not select a BIOS mode.

New custom presets expose no Max-Fan field. The separate Cooling Max choice stays
available; legacy files and their recovery images retain this decoder/restore path.

Capture checks board/BIOS, full flag, table hash, topology and design data before any SET. Complete originals are persisted, fresh expected state is checked and full readback follows. A new process can restore only matching original/own-target states; foreign or forged images are rejected. RPM values are observations, never restore targets.

The current controller permits Max-Fan sign-in authorization and has `requires_ac = false`; older AC-only/startup-blocked descriptions are obsolete. The hardware probes were bounded and do not qualify battery, actual reboot, sleep/resume or sustained maximum behavior.

## Cooling helper and quick choices

The own “fan service” is a protected elevated **scheduled-task helper**, not a kernel driver or an interactive Windows service UI. Its fixed task is `OmenHubPP-Fan-<SID>`, running `Omen Hub++ Fan.exe fan-service` from the installation directory.

| Choice | Meaning |
| --- | --- |
| Auto (`bios`) | Pause own software regulation after its conservative release; BIOS ownership is not confirmed |
| Curve (`curve`) | Use the saved active shared curve with CPU/GPU/IR protection |
| Max (`max`) | Request both levels 48, independent of CPU/GPU reader availability |

Absent preference defaults to Auto. Legacy `auto` meant “follow curve” and is read
compatibly. The active shared curve remains `active-fan-curve.json`.
[Curve editor](CURVE_EDITOR.md).

## Named fan presets and manual profile links

From 0.10.0-beta.7, Cooling has a separate named library (`fan-presets/`). Use curve
stages an editor draft; Save stores library metadata. Request uses the saved
revision, after checking its model, protected cooling duties and the installed
receipt-bound helper. A stale edited/deleted library revision is rejected before
helper resumption or preference writes. Delete archives metadata only.

A performance profile may copy a library curve into matching CPU/GPU fan-curve
instructions. The saved snapshot survives library edits/deletion. Unchanged omits
the pair and leaves the global fan choice alone. A custom saved snapshot remains
editable. Fan-only profiles use Cooling instead of the performance transaction.

The request-level coordinator captures exact `fan-mode.json` and
`active-fan-curve.json` originals, including absence, before helper resumption or
the HP prelude. Only a completely verified direct-control subset without skips
commits the curve/mode pair. Partial or failed direct results attempt direct
recovery and exact preference restoration; existing HP mode/TPP prelude limits
remain explicitly unconfirmed. File conflicts retain a pending duty.

`fan-profile-journal.json` records Prepared/Requested/Recovering/Restored ownership;
completed records go to `fan-policy-history/`. Direct checkpoint content and file
identity distinguish repeated identical revisions. An early aborted replacement
reattaches the prior matching owner atomically, retaining its original images.
Foreign choices and missing/corrupt archives are not silently overwritten.

This flow resumes/uses the **installed** helper and never replaces it merely for a
new UI build, including a profile with CCD selection. The current 0.9.0-beta.3
helper supplies no curve-digest acknowledgement. Success says cooling **requested**,
separately from verified direct settings; it does not prove physical curve uptake,
complete firmware rollback or BIOS ownership. Since 0.10.0-beta.15 (user decision,
22 November 2026) a preset with a linked curve can be the sign-in preset: the sign-in
task applies its direct controls and makes the same helper request as Apply. A
helper that cannot take the request at sign-in (for example still starting) does not
hold back the direct controls; `last-startup-result.json` then names the error under
`cooling_error`. Only a lone or differing CPU/GPU curve is still refused. Sign-in with
a linked curve is not yet tested across a real reboot.

An already configured AC/DC selection still uses the same runtime Apply route,
including this coordinator, if it references such a profile. No battery choice is
changed or enabled by creating a library/link. Actual AC/DC transitions with linked
curves remain unqualified.

The loop checks full known layout before first arm, uses a model-bound heartbeat at 20 seconds and level cycles at roughly two seconds, waits while the original Hub is detected and releases unused readers while paused. Curves use fixed CPU Tctl, public GPU temperature and IR/protection observations, with smoothing and emergency maximum. Max avoids loading the CPU/GPU reader libraries.

Since 0.10.0-beta.13 the GPU temperature is read so that a dGPU in hybrid mode can power down. Each cycle first reads the adapter's power state from the Windows device tree (`DEVPKEY_Device_PowerData`), which does not touch the device. A powered-down dGPU is not queried and is not a failed reader: the GPU fan follows the CPU temperature and the chassis sensor, without the emergency maximum. A GPU below 60 °C is read every 30 s with no driver session kept in between (the OMEN Hub's idle interval); at 60 °C and above it is read every cycle as before. The chassis sensor is read every 10 s. A failed read while the dGPU is awake still forces full cooling. Fan behavior with a resting or sleeping GPU is not yet measured on the device.

Write packets share the own hardware lease. Temporary contention reports `waiting_for_control`; renewed control establishes a fresh heartbeat/maximum. Status includes PID, process creation identity, timestamps and the last actual heartbeat. Unknown vendor enumeration is not treated as safe absence.

After own regulation, Auto, missing/invalid curves and normal stop request **48/48 once**, then cease heartbeat/level writes and report `handover_pending`. A failed last write reports failure. A confirmed original Hub prevents a competing final SET. [Handover evidence](FAN_HANDOVER.md).

The same helper enforces saved foreground CCD preferences and releases original masks. That policy is independent of fan-curve selection. [System controls](SYSTEM_CONTROLS.md).

## Evidence and remaining work

15 October Max-Fan trials verified full flags, separate-process recovery and preserved other controls. A 22 October conservative helper trial reached **4800/4800 firmware RPM**. The 23 October watchdog/FAMC observations did not qualify a direct BIOS-Auto switch. HP services were still present in these trials.

The dashboard describes Curve/Max as a selected request, without claiming measured
fan operation. The 17 November linking/recovery changes have isolated software
tests only, with no live curve request or new device write trial. Fresh exact-policy
helper acknowledgement remains the status-confirmation [review item](PROJECT_REVIEW.md).
Durable own cooling, safe BIOS return, actual power transitions, fully verified
profile integration and independent HP replacement remain open.
[Validation](VALIDATION.md), [HP control](HP_CONTROL.md).

`fan-read` and `cooling-read` are fixed diagnostics needing suitable permissions. `fan-validate-max`, `check-system-controls.ps1` and the opt-in migration trial perform changes; they are not ordinary status checks.
