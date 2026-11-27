# HP mode observations: 8E35

Static source investigation of Hub **1101.2609.3.0**, recorded **16 October 2026**; current Rust behavior reviewed on 29 October. Sources: installed HP mode/common/background/AMD wrappers, `Khalilah_DRG_N22X4X6.json` and local vendor logs. Reading vendor IL/logs was not itself a hardware trial.

## Mode field

The notebook `SetFanMode` path builds **`[0xFF, mode, biosFanOwner, 0]`** for `0x20008/0x1A` with `doMap=false`. Do not apply the separate generic thermal-policy remapping to this route.

| Vendor meaning | Mode byte | Original observation |
| --- | --- | --- |
| Balanced center, L2 | **0x30** | Code and recorded logs |
| Balanced slider L0/L1/L3/L4 | 0x10 / 0x20 / 0x40 / 0x50 | Code |
| Performance default, L7 | **0x31** | Code and recorded logs |
| Performance L5/L6 | 0x11 / 0x21 | Code |
| Unleashed, L8/Extreme | **0x04** | Code/logs and separate own trial |
| Automatic Eco, enum 256 cast to byte | **0x00** | Code; not a dedicated UI button |

The vendor UI offered Balanced/Performance sliders and Unleashed. It did not expose a distinct notebook Quiet opcode. The own Quiet choice uses the measured Balanced-L0-style **0x10**, not an invented mode 3.

The ownership byte is separate from the performance byte. The examined AC route uses software ownership; the vendor BIOS branch has DC/thermal/model conditions. Full current ownership readback is missing. [Fan handover](../FAN_HANDOVER.md).

## Coupled writes

HP mode changes also update cTGP/PPAB through 0x22, shared budget and paired CPU limits through 0x29, and Max-Fan state. The paired CPU payload `[W,W,FF,FF]` is not an independently qualified STAPM interface.

| Own factory-style choice | Thermal byte | cTGP / PPAB on AC | CPU PL1 target | Firmware TDC / EDC |
| --- | --- | --- | --- | --- |
| Quiet | 0x10 | Off / Off | 55 W | 108 A / 160 A |
| Balanced (Standard) | 0x30 | Off / On | 55 W | 108 A / 160 A |
| Performance | 0x31 | On / On | 60 W | 108 A / 160 A |
| Unleashed | 0x04 | On / On | 80 W | 108 A / 160 A |

Live SMU PM-table verification (**25 November 2026**, `0x540208` cells 2/4/8/9/62/63 + fused GETs `0xDA/0xDB/0xDC`) and static inspection of `HP.Omen.Core.Common.PowerControl.JSON.Khalilah_DRG_N22X4X6.json` (`Version: 20250523`) confirm that neither HP BIOS `F.13` nor OMEN Gaming Hub changes TDC or EDC per mode on 8E35: all four modes share **108 A TDC** (fused `GetFusedTdc = 108000 mA`) and **160 A EDC** (`cell 62 = 160 A`), while live current draw scales with PL1 (Quiet ~40.2 A / 52.8 A, Standard/Balanced ~40.4 A / 51.8 A, Performance ~45.8 A / 54.3–68.8 A, Unleashed ~62.7–65.7 A / 71.5–104.3 A). Other boards/BIOS versions can discover their own per-mode TDC/EDC automatically via `device-defaults-detect` or Settings → *Detect device defaults*.

The source resource's default shared budget was 45; Unleashed uses the separately observed 72-W payload field. This shared-budget field is not an independently chosen GPU TGP. The original Hub can retain a user's Unleashed PL1, so its configured value need not equal the own factory-style 80-W target. Exact current payload construction is in `core/hp_mode.rs`; do not generalize it to other boards/BIOS.

Vendor Windows-power synchronization and AMD RPC tuning are separate optional paths. The HP AMD wrapper serializes requests to its service; it does not expose the actual native SMU messages. Omen Hub++ now has its own fixed qualified RSMU path rather than loading that RPC wrapper. CCD affinity is a Windows process policy, not a CPU firmware mode.

## Hub power sliders (decompiled, 20 November 2026)

OMEN Hub 1101.2611.2.0, read with ILSpy (the relevant modules are .NET; Ghidra is not needed for them), `HP.Omen.PerformanceControlModule` and the model table `PowerControl.JSON.Khalilah_DRG_N22X4X6` (version 20250523) for 8E35: the Unleashed panel's `PowerLimit1Slider` spans `UnleashedModeMinPL1`–`UnleashedModeMaxPL1` = **45–80 W**, tick 1 W, default `UnleashedModeDefaultPL1` = 80 W. On AMD the value is applied by `SetPL1` → `SetPowerLimit(pl1)` = command 0x20008 type 0x29 `[PL1, PL1, FF, FF]` (values above 254 W use the 128-byte 0x37 command, not relevant here). The `TppSlider` spans `TppMinValue`–`TppMaxValue` = **55–72 W**. Omen Hub++ shows the same ranges as sliders in the preset group *Dynamic Boost and CPU power* and writes PL1 through its existing qualified 0x29 path; the PM-table readback of Slow (cell 4) confirms it.

## Current implementation limits

The four own modes were exercised with GPU flag and available Slow/PL1 readback, ending in the user-specified Unleashed baseline. A complete active thermal-mode original cannot be read back. The dashboard path takes the shared write lease but has no complete preset-style write-ahead transaction. On a failed non-Balanced mode it attempts Balanced; that is a fallback, not restoration of the unknown previous mode.

Keep default-mode switching, Dynamic Boost flag and a future numeric GPU power limit distinct. The static vendor route is evidence for fixed contracts, not free CPU/GPU tuning or safe BIOS ownership. [System controls](../SYSTEM_CONTROLS.md), [Recovery](../RECOVERY.md), [Project review](../PROJECT_REVIEW.md).
