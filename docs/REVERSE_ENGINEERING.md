# Reverse engineering and interface evidence

This project records original interface observations from installed software and public contracts. Vendor DLLs/drivers, decompiled source, raw captures and Ghidra projects remain ignored locally. Their presence is neither permission to redistribute them nor proof of a qualified hardware writer.

Current device status: [8E35 evidence](models/8e35.md), [hardware matrix](HARDWARE_MATRIX.md). The original source manifest/chronology is preserved in [Initial HP investigation](history/HP_INITIAL_INVESTIGATION.md).

## Method

1. Identify the exact model/BIOS, source image version/hash, API/ABI and selected device path.
2. Trace the vendor call through wrappers to the actual Windows/provider/driver contract. Do not use decompiler prototypes or a similar model's addresses as an ABI.
3. Separate static presence, successful query, writable control, complete readback and full original restoration.
4. Qualify fixed read paths before any planned SET. A named GET can still have side effects; HP fan-count/heartbeat 0x10 is the concrete example.
5. Before a write, establish all coupled originals, typed values, permissions, target bounds and failure/restore behavior. No blind adjacent-opcode, EC-address or IOCTL probing.
6. Keep device lifetime and ownership explicit, then test real transitions separately from one-session success.

## Findings now used by own code

| Area | Established interface observation | Current boundary |
| --- | --- | --- |
| BIOS envelope | Direct `ROOT\WMI/hpqBIntM`, `SECU`, typed embedded input/output and exact lengths | No proprietary HP RPC/SDK wrapper needed for these own paths |
| HP GPU flags | 0x20008/0x21 GET and 0x22 fixed cTGP/PPAB payload | Dynamic Boost flag is not numeric TGP; observed D-state 0 remains unknown |
| Factory modes | 0x1A plus coupled 0x22/0x29 model payloads | Thermal original GET and complete shortcut recovery missing |
| Cooling | Fixed topology/table/tachometer/Max-Fan plus 0x10 heartbeat and 0x2E levels | Stateful heartbeat, safe BIOS handover and durable ownership remain distinct |
| Keyboard | Capability, complete RGB/mode blocks and exact 0x64/0xE4 On-Off | No generic brightness, per-key or full effect/flash restore |
| NVIDIA | Public telemetry/offsets and exact private full Graphics V/F contracts | Only qualified driver images; numeric TGP still research |
| CPU | Fixed verified PawnIO modules with measured RAPL/Tctl/APERF and RSMU control readback | Broad allowed ranges/lifecycle require further evidence |

Initial HP GPU status readback was `[1,1,0,87]` under elevation. Static enum values did not explain D-state 0; the user's Unleashed report did not assign it a new meaning. Model resource 8E35→Khalilah25C1/DRG_N22X4X6 provided 30-second vendor heartbeat and PL1/TPP/IR context, not universal parameters.

HP's `SetTGPAsync` single-byte route and four-byte cTGP/PPAB route are different contracts. Shared budget/TPP in 0x29 is coupled policy, not a free GPU-watt field. Independent CPU Slow now uses measured RSMU 0xCB; old assumptions about independent HP STAPM are retired.

## Source records and tools

The original HP modules were examined with ILSpy 11.1.0.9782. Native WMI/lighting/NVIDIA and firmware observations used Ghidra 12.1.4 through PyGhidra MCP 0.2.7 and static DSDT analysis where documented. Importing an image for static analysis did not execute it. Partial decompilation errors are recorded rather than called complete success.

Specific source identities and contracts are retained in [Keyboard protocol](KEYBOARD_LIGHTING.md), [HP modes](models/8e35-hub-modes.md), [fan handover](FAN_HANDOVER.md), [NVIDIA 617.42](NVIDIA_61742.md), [CPU sensors](CPU_SENSORS.md) and [historical source notes](history/HP_INITIAL_INVESTIGATION.md).

## Active investigation

The [TGP plan](plans/NVPWRCONTROL_TGP_FORK_PLAN.md) compares existing NVIDIA transport, private NVAPI, NVML and HP/NVPCF approaches while retaining user-mode processes, TPM and Secure Boot. No alternate writer is established. Continue read-only transport/privilege/original-state research; implementation and new hardware writes remain deferred.

Separately, reliable HP thermal/fan ownership readback, complete independent lifecycle and source-to-actual-effect attribution remain necessary for full HP replacement. [Replacement plan](REPLACEMENT_PLAN.md).
