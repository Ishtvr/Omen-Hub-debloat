# Hardware support

Current status, reviewed **29 October 2026** against `0.9.0-dev.2`. This is the authoritative feature-status table. [Validation](VALIDATION.md) records the tests; [Roadmap](ROADMAP.md) records unfinished work. **Implemented**, **briefly tested on hardware** and **qualified for a full lifecycle** are different claims.

## Reference configuration

HP OMEN 16-ap0xxx, **board 8E35 / BIOS F.13**, AMD Ryzen 9 **8940HX**, SMU **0x546F00**, PM table **0x540208**, NVIDIA **RTX 5070 Laptop** (not Ti), VBIOS **98.06.2a.40.da**.

Private NVIDIA write contracts have separate entries for drivers **617.14** and **617.42**, with exact loaded-implementation hashes. Profiles bind device identity and GPU UUID where applicable. Other boards, firmware and GPU implementations do not inherit these private-control permissions. Public Windows/NVIDIA API availability is queried independently; it does not certify an untested system.

## Feature status

| Feature | Current implementation and evidence | Remaining limit |
| --- | --- | --- |
| GPU watts, temperature and clocks | Public NVML/NVAPI; bounded isolated reader | Driver availability; reported limit is not measured draw |
| CPU load / Windows MHz | Windows APIs | OS-reported MHz is not measured boost |
| CPU package watts / Tctl / effective clocks | Verified PawnIO reader; package energy and all 32 logical CPUs measured | Exact driver/module prerequisites; real power transitions and long runs pending |
| GPU / VRAM offsets | Small −15/+15 and −24/+24 MHz trials, exact readback and full restore | Global core offset conflicts with an independent V/F curve; no stability certification |
| GPU V/F curve | All 127 Graphics anchors/deltas read, edited, applied and restored | Frequency targets only; voltage anchors immutable; no hard voltage ceiling |
| HP default modes | Quiet/L0, Balanced, Performance and Unleashed fixed SET paths exercised | Thermal mode lacks full GET; shortcut lacks preset-equivalent journal recovery; Balanced fallback is not original restore |
| Dynamic Boost | PPAB On/Off/readback/restore; cTGP preserved | Needs the appropriate HP performance state for the full OEM envelope; sustained 115 W not load-proven |
| Numeric GPU power limit | Public NVML adapter with original capture, bounds, journal and restore; simulated ABI tests | Configured original GET unsupported on this laptop, so no actual write qualification |
| CPU PPT / Slow / TDC / EDC / Tctl | Fixed RSMU messages, small decreases, complete original readback/restore | PPT/Slow conflict with active HPOmenCap; broad allowed ranges are not measured safe envelopes |
| CPU Curve Optimizer / PBO | Full 16-core images around −20; PBO 1× trial | CO steps are not fixed mV; allowed −50…+30 and PBO up to 10× are not fully stability-tested |
| Windows processor policies / power mode / plan / refresh | Native SET/GET/restore, AC/DC and target-plan originals | Balanced plan required for the overlay mode; real power/display transitions pending |
| CCD affinity | Saved preference and actual owned test process, both CCDs and original-mask release | Own helper needed; other user programs and real power transitions not qualified |
| MUX | Hybrid/discrete boot preference set, read back and restored | Physical reboot switch and iGPU-only/dGPU-off battery operation unproven; dashboard shortcut has separate recovery limits |
| Manual Max-Fan override | Full flag, table/topology checks, readback and persisted restore | Off releases the override only; it does not prove BIOS ownership |
| Own fan curve / helper Max | Separate helper with CPU/GPU/IR protection; short trial reached 4800/4800 firmware RPM | Helper waits for original Hub; BIOS handover pending; embedded fan curves remain unsupported in profile transactions |
| Four-zone static lighting / On-Off | Complete RGB/mode snapshots, readback/restore; visible On/Off confirmed by user | No brightness scale, animation, per-key support or independent hotkey/power lifecycle qualification |
| Profile sign-in | Protected task and exact applied-revision approval implemented | Actual reboot and full independent operation pending; editing/binary replacement invalidates approval |
| Intel tuning | Format/UI fields only | No integrated Intel backend or available Intel test device |

## Sign-in permission versus measured lifecycle

The current controller **permits** sign-in authorization for supported lighting and Max-Fan settings, following the user's request. Older `startup_safe = false` descriptions are obsolete. This code permission does not establish successful real reboot, resume, hotkey or independent HP-controller operation.

New CPU PPT/Slow sign-in approvals require HPOmenCap to be absent until startup handover is qualified. A temporary stop is insufficient. The public GPU power adapter is offered only with a readable configured original, valid live bounds and setter presence; those conditions currently fail on the reference laptop.

## Scope and removed features

Full replacement of HP user-mode controllers, durable own ownership, safe BIOS cooling handover, automatic profile reapplication after resume/AC changes and broad model support remain unfinished. The TGP experiment must retain TPM and Secure Boot with user-mode app processes; see its [investigation plan](plans/NVPWRCONTROL_TGP_FORK_PLAN.md).

Direct GPU/VRAM mV controls, FPS limiter, fixed CPU clocks/voltages and the removed APU menus are intentionally absent. The 91-key data catalog preserves compatibility; it is not a count of working controls.

The tray working-set target is **50 MB total across all own resident helpers after sensor activation**. Open-window memory may exceed it. Previous short measurements do not qualify the current/future full helper stack. See [Validation](VALIDATION.md).
