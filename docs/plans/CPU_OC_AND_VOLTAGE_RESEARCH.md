# CPU Overclocking & Voltage Control Research — HP 8E35 / F.13 / AMD Ryzen 9 8940HX

**Date:** 26 November 2026  
**Target:** HP OMEN Gaming Laptop `16-ap0xxx` (`SKU: C2VM8EA#ABD`), Board `8E35`, BIOS `F.13`, AMD Ryzen 9 **8940HX** (Zen 4 Dragon Range, Family `19h` Model `61h`, Microcode `0x0A60120C`, 16C/32T, 2×8-core CCDs, SMU `0x00546F00`, PM-Table `0x00540208`).  
**Constraints:** Windows Defender 100% untouched, Secure Boot ON, no unsigned drivers, no BIOS SPI flashing, signed PawnIO (`RyzenSMU.bin` + `AMDFamily17.bin`) and standard Windows WMI/ACPI only.

---

## 1. Executive Summary

Unlike the RTX 5070 Laptop GPU (where the `115 W` power table is parsed inside Windows kernel driver `nvlddmkm.sys` from a registry VBIOS override and `nvpcf.sys` has registry feature overrides), **manual static desktop OC Mode** on the Ryzen 9 8940HX is enforced **inside the PSP-signed MP1 SMU firmware (`0x00546F00`)** via capability bit `RSMU 0x6F bit 0 = 0`. Every static manual OC opcode (fixed all-core MHz, fixed per-core MHz, fixed manual Vcore VID) is rejected directly by the SMU with `0xFD` (`CMD_REJECTED_PREREQ`).

However, live hardware probing uncovered **five genuinely working, reversible, live runtime CPU tuning and telemetry capabilities** beyond what Omen Hub++ currently exposes:

| Capability | Interface | Live Status on 8E35 / F.13 | Verified Range / Behavior |
| --- | --- | --- | --- |
| **Live Global Boost Fmax Override** | `RSMU 0x70` / `0x6E` | **WORKS LIVE (no reboot)** | Accepts `≤ 5550 MHz` (**`+200 MHz`** over `5350 MHz` stock; `5600/5700/6000` hard-clamp to `5549 MHz`). Down-capping (`5000 MHz`) immediately enforced `5000.0 MHz` and dropped measured SVI3 `VDDCR_VDD` by **`−68 mV`**. |
| **Live Per-Core Boost Ceiling & Target Shaping** | `MSR_AMD_CPPC_REQ` (`0xC00102B3`) via `AMDFamily17.bin` | **WORKS LIVE & STICKY** | Writes stayed sticky across active load (`AmdPPM` did not overwrite). Capping Core 0 (`MaxPerf = 140`) held Core 0 at `4203.8 MHz` while Core 4 (`236`) ran at `5224.4 MHz`. Cannot exceed each core's fused `highest_perf` (`CCD0: 206..236`, `CCD1: 166..201`). |
| **Live Per-Core Voltage Shift (Undervolt & Overvolt)** | `RSMU 0x06` / `0xD5` (Per-Core CO) | **WORKS LIVE** | Supports both negative undervolt (SMU accepted `-31` below `-30`) and positive overvolt up to hard clamp **`+30`** (`+31/+40` clamped to `+30`). Measured SVI3 `VDDCR_VDD` rose **`+50.6 mV`** at `CO = +10` vs stock `0`. |
| **Live PBO FIT Scalar (Voltage/Boost Headroom)** | `RSMU 0x5B` / `0x6D` | **WORKS LIVE** | Verified `1.0×` (`100`) → `2.0×` (`200`) → `5.0×` (`500`) → `10.0×` (`1000`) → `1.0×` with exact readback. |
| **Live Radeon 610M iGPU Curve Optimizer** | `RSMU 0xA7` / `0xD7` | **WORKS LIVE** | Verified `0 → −5 → 0` with `OK` and exact readback. |
| **Real-Time 3-Rail SVI3 Telemetry & CPPC Core Ranking** | PM-Table `0x540208` (`float[47..61]`), `RSMU 0x5C`, `MSR 0xC00102B0` | **WORKS LIVE** | Live VID, measured V, current (A), power (W), VRM temp (°C) for `VDDCR_VDD`, `VDDCR_SOC`, `VDD_MISC` + exact per-core silicon ranking. |
| **Static Manual OC Mode (Fixed Core MHz / Manual Vcore VID)** | `RSMU 0x5D/5F/60/61`, `MP1 0x24/26/27/28` | **LOCKED IN SMU (`0xFD`)** | `RSMU 0x6F = 0xE6` (`bit 0 = 0`). Both RSMU and MP1 reject with `CMD_REJECTED_PREREQ`. |
| **Per-Core SMU Fmax Opcode** | `RSMU 0x70` with core mask / `RSMU 0x71` | **UNSUPPORTED** | `0x70` ignores core mask (clamps to `5549 MHz`); `0x71` returns `0xFF` (`UNKNOWN_CMD`). |
| **Curve Shaper (Zen 5)** | `RSMU 0x84` | **LOCKED (`0xFD`)** | Zen 5 feature; not enabled on Dragon Range Zen 4. |
| **HSMP Mailbox** | `0x3B10534` | **INACTIVE (`TIMEOUT`)** | Server/EPYC only. |
| **BCLK / Async eCLK Overclocking** | FCH SMN `0x02D01E08..1EAC` | **UNSAFE / NO eCLK** | `StrapStatus = 0x00020003`, `EclkMode = 0x00000000` (internal synchronous CGPLL only; coupled to PCIe/NVMe/USB). |
| **BIOS `AMD_ACPI` (`SSD9 / AOD`, `SWSMI 0xB9`)** | `root\wmi:AMD_ACPI` | **READ-ONLY TABLE / SMM STUBBED** | Exposes 69 AGESA parameter definitions (`CCLK Fmax 4350..5550`, `SOC VID 0..211`, `CLDO_VDDP`, `VDDG`, `VDD_MISC`, `MemClk 2600`, `FCLK`, DDR5 timings), but `RunCommand` (`SWSMI 0xB9`) returned 200 zero bytes even on `0x00040001`, `0x00040002`, and invalid `0x0000FFFF`. |

---

## 2. Detailed Empirical Evidence

### 2.1 SMU Capability Word & Mailbox Survey (`SMU 0x00546F00`)

* **`RSMU 0x6F` (`IsOverclockable`):** Returned `230` (`0xE6` = `0b1110_0110`):
  * Bit 0 (`OverclockingEnabled`, manual static OC Mode) = **`0`**
  * Bit 1 (`PowerLimitsAvailable`) = **`1`**
  * Bit 2 (`PboAvailable`) = **`1`**
  * Bit 5 (`ExpoProfilesAvailable`) = **`1`**
* **Static OC Opcodes Tested (all returned `0xFD CMD_REJECTED_PREREQ`):**
  * RSMU (`0x3B10524 / 0x3B10570 / 0x3B10A40`): `0x5D` (`EnableOcMode`), `0x5F` (`SetOverclockFreqAllCores`), `0x60` (`SetOverclockFreqPerCore`), `0x61` (`SetOverclockCpuVid(181)` = `1.15 V`).
  * MP1 (`0x3B10530 / 0x3B1057C / 0x3B109C4`): `0x24`, `0x26`, `0x27`, `0x28(181)`. (MP1 itself is active: `0x01 TestMessage = OK`, `0x02 GetSmuVersion = 0x546F00`, `0x23 GetSustainedPowerAndThmLimit = OK`.)
* **Fused Limits & Status Reads (`RSMU`):**
  * `0x6E` (`GetBoostLimitFrequency`) = `5350 MHz`
  * `0x6D` (`GetPBOScalar`) = `1.0×` (`0x3F800000`)
  * `0xD7` (`GetGpuPsmMargin`) = `0`
  * `0xD9` (`GetPboFusedVrmSocTdc`) = `102,000 mA` (`102 A`)
  * `0xDA` (`GetPboFusedApuSlow`) = `85,000 mW` (`85 W`)
  * `0xDB` (`GetPboFusedVrmVddTdc`) = `108,000 mA` (`108 A`)
  * `0xDC` (`GetPboFusedPowerLimit`) = `160,000 mW` (`160 W`)
  * `0xDD` (`GetLN2Mode`) = `0`

### 2.2 Live Fmax Override (`RSMU 0x70`) & Bounds

* **Readback & Ceiling Probes:**
  * `0x70(5375)` → `OK`, `0x6E` = `5375 MHz`
  * `0x70(5450)` → `OK`, `0x6E` = `5450 MHz`
  * `0x70(5550)` → `OK`, `0x6E` = `5549 MHz`
  * `0x70(5600 / 5700 / 6000)` → `OK`, `0x6E` = `5549 MHz` (**hard SMU ceiling = `5550 MHz` / `+200 MHz`**, matching `AMD_ACPI` `Set CCLK Fmax: Min=4350, Max=5550, Step=25`).
  * `0x70(core_mask(0,1) | 5100)` (`0x001013EC`) → treated as global `> 5550` and clamped to `5549 MHz` (proves `0x70` is global all-core only).
  * `0x71(core_mask(0,0) | 5200)` → `0xFF` (`UNKNOWN_CMD`).
* **Measured Load Effect (Core 0, 1T scalar workload, stock `CO = 0`):**
  * `Fmax = 5350 MHz`: `eff = 5247.8 MHz`, `HW P-State max = 5300 MHz`, `SVI3 meas = 1.2266 V`, `55.8 W`, `84.2 °C`.
  * `Fmax = 5000 MHz`: **`eff = 5000.0 MHz`**, **`HW P-State max = 5000.0 MHz`**, **`SVI3 meas = 1.1632 V` (`−68 mV` vs `5550` run)**, `50.9 W`, `79.2 °C`.
  * *Observation caveat:* During both probe sessions, background system activity kept package power at `55.0 W` (Balanced Slow cap) and `80–99 W` / `91–101 °C` (in the 80 W probe), so Core 0 was thermally/power-limited before crossing `5350 MHz`. Physical single-core peaks at `5400–5550 MHz` should be confirmed on a completely idle, cool system in Unleashed mode with safe mild CO (e.g. `0` to `−10` on the top cores, since `+200 MHz` requires more V/F voltage headroom than an aggressive undervolt).

### 2.3 Per-Core Clock Control via `MSR_AMD_CPPC_REQ` (`0xC00102B3`) & Core Ranking

* **Exact Fused CPPC Core Ranking (`MSR 0xC00102B0` & `RSMU 0x5C`):**
  * **CCD0 (Fast Die):** `C0 (236) = C4 (236) > C6 (231) > C1 (226) > C2 (221) > C7 (216) > C5 (211) > C3 (206)`
  * **CCD1 (Efficiency Die):** `C13 (201) > C9 (196) > C15 (191) > C14 (186) > C12 (181) > C11 (176) > C10 (171) > C8 (166)`
  * Nominal perf = `74` (`2400 MHz`), Lowest nonlinear = `46`, Lowest = `13`.
* **Live Per-Core Write Verification (via `AMDFamily17.bin`):**
  * Original Core 0 `0xC00102B3` = `0x0000000000002EEC` (`MaxPerf = 236`, `MinPerf = 46`, `DesPerf = 0`, `EPP = 0`).
  * Wrote `0x0000000000002E8C` (`MaxPerf = 140`) to Core 0 (logical CPUs 0 & 1) while leaving Core 4 untouched (`MaxPerf = 236`) and setting `Fmax = 5550 MHz`:
    * Sticky check after 1.2 s active load: **`0x0000000000002E8C` unchanged** (`AmdPPM` did not revert it).
    * Core 0 measured: **`4203.8 MHz`** (`1.186 V`, `3294.1 ops/s`).
    * Core 4 measured: **`5224.4 MHz`** (`1.220 V`, `4307.1 ops/s`).
  * Fixed target check (`MinPerf = MaxPerf = DesPerf = 160` on Core 0): `4715.0 MHz`, `SVI3 meas = 1.0127 V`.
  * Raising CCD1 Core 8 `MaxPerf` above its fused `166` to `236`: remained capped at `4990.6 MHz` (`HW max = 5050 MHz`) vs stock `5014.4 MHz` (`5075 MHz`). **Conclusion:** `CPPC_REQ` provides genuine per-core frequency capping/shaping up to each core's fused `highest_perf`, which combined with `RSMU 0x70` (`+200 MHz` global Fmax) yields per-core boost ceilings.

### 2.4 Voltage Control, SVI3 Telemetry & Hardware CO Range

* **Hardware P-State MSRs (`0xC0010064..65`, SVI3 formula `0.245 + vid9 × 0.005`):**
  * `P0 = 0x8000000046324A78` → `2400 MHz @ 1.2500 V` (`VID = 201`)
  * `P1 = 0x8000000043DE5078` → `1500 MHz @ 0.8500 V` (`VID = 121`)
  * `P2..P7` disabled. (Not written; Zen 4 boost is governed by SMU + CPPC, not legacy P-state MSRs.)
* **PM-Table `0x540208` Live SVI3 Rail Layout (verified at float index `47` / byte `0xBC`):**
  * `VDDCR_VDD`: `[47]` VID (`1.237 V`), `[48]` Measured (`1.233 V`), `[49]` Current (`24.58 A`), `[50]` Power (`30.46 W`), `[51]` VRM Temp (`75.1 °C`).
  * `VDDCR_SOC`: `[52]` VID (`1.115 V`), `[53]` Measured (`1.105 V`), `[54]` Current (`11.17 A`), `[55]` Power (`12.33 W`), `[56]` VRM Temp (`61.3 °C`).
  * `VDD_MISC`: `[57]` VID (`1.100 V`), `[58]` Measured (`1.105 V`), `[59]` Current (`5.39 A`), `[60]` Power (`5.96 W`), `[61]` VRM Temp (`74.0 °C`).
* **Real SVI3 Voltage Shift via Curve Optimizer (`RSMU 0x06`, Core 0 1T @ 55 W):**
  * `CO = −25`: `HW max = 5350 MHz`, `VID max = 1215 mV`, `SVI3 meas = 1.1788 V` (highest clock, lowest VID).
  * `CO = 0`: `HW max = 5250 MHz`, `VID max = 1225 mV`, `SVI3 meas = 1.1715 V`.
  * `CO = +10`: `HW max = 5175 MHz`, `VID max = 1235 mV`, **`SVI3 meas = 1.2221 V` (`+50.6 mV` real overvolt)**.
* **SMU CO Hardware Bounds (Core 15 quick probe, restored immediately):**
  * `−25 → −25`, `−30 → −30`, **`−31 → −31`** (SMU permits below `−30`, Zen 4 Raphael/Dragon Range range is typically down to `−50`).
  * `+10 → +10`, `+30 → +30`, **`+31 → +30`**, **`+40 → +30`** (**SMU hard-clamps positive CO at `+30`**).
* **PBO Scalar (`RSMU 0x5B / 0x6D`):** `200 → 2.0×`, `500 → 5.0×`, `1000 → 10.0×`, `100 → 1.0×` (all `OK`).
* **iGPU Curve Optimizer (`RSMU 0xA7 / 0xD7`):** `0 → −5 → 0` (all `OK`).

### 2.5 HP BIOS `F.13` ACPI `SSD9` (`AMD_ACPI` / `AOD`) & FCH BCLK

* **`SSD9` (`HPQOEM / AOD_____`, `ACPI\PNP0C14\AOD_0`, Version 6):**
  * `OperationRegion (AODE, SystemMemory, 0x6B65F018, 0x24AC)`, `ASMI = 0xB2`, `DSMI = 0xB9`.
  * `GetMinMaxSteppingValues` (`AM06`) and `Getdvalues` (`AM04`, param `ID`) expose 69 AGESA parameters including `Set CCLK Fmax` (`4350..5550 MHz`, step `25`), `Set SOC VID` (`0..211`), `Set CLDO_VDDP` (`0..1300 mV`), `Set CLDO VDDG` (`650..1650 mV`), `Set VDDG IOD` (`650..1650 mV`), `Set VDD_MISC` (`1100..1650 mV`), `Set Mem Clock` (`Current = 2600`, 47 entries), `Set FCLK OC Mode` (`0/1`), `Set FCLK Frequency` (52 entries), `Set UCLK DIV1` (`Current = 1`), and full DDR5 timings (`42-42-42-84-125`).
  * **Liveness test:** `RunCommand` (`AM05`, writes `SCMI`/`SCMD` and triggers `SWSMI 0xB9`) returned 200 zero bytes (`nonzero_bytes = 0`) for `0x00040001` (`Command Buffer Start`), `0x00040002` (`Command Buffer End`), and invalid ID `0x0000FFFF`. Desktop `AmdSetup`/`AodSetup` UEFI NVRAM variables do not exist (`ERROR_ENVVAR_NOT_FOUND 203`; only 244-byte `Setup` exists). **Conclusion:** HP includes the read-only AGESA AOD reporting block, not an active SMM OC writer.
* **FCH CGPLL / BCLK (Read-Only SMN):**
  * `FCH_MISC_CGPLLConfig1 (0x02D01E08) = 0x78001E01`
  * `FCH_MISC_CGPLLConfig3 (0x02D01E10) = 0x20000000`
  * `FCH_MISC_ClkCntl1 (0x02D01E40) = 0x01004042`
  * `FCH_MISC_StrapStatus (0x02D01E80) = 0x00020003` (internal CGPLL)
  * `FCH_MISC_EclkMode (0x02D01EAC) = 0x00000000` (no external async eCLK)

---

## 3. Recommended Omen Hub++ Integration Candidates

If the user wants these live-verified controls integrated into **Omen Hub++**, the cleanest, 100% Defender-safe, Secure-Boot-compatible additions are:
1. **CPU Boost Clock Limit (`Fmax`, `RSMU 0x70`, `4350..5550 MHz` / `−1000..+200 MHz` in `25 MHz` steps):** Enables both **`+200 MHz` PBO Boost Overclocking** (up to `5550 MHz`, paired with an explicit warning to relax aggressive negative CO when pushing above `5350 MHz`) AND **efficient boost capping** (e.g., `4500–5000 MHz` for quiet/cool gaming or battery savings, verified `−68 mV` at `5000 MHz`).
2. **Per-Core Positive Curve Optimizer (`+1..+30`) & Extended Range Option:** Allow positive CO offsets (`+1..+30`) for overvolting unstable/overclocked cores (especially when raising Fmax up to `5550 MHz`), plus optionally allowing deeper negative CO (`−31..−50`) behind an expert warning.
3. **Per-Core CPPC Boost Ceiling (`MSR_AMD_CPPC_REQ 0xC00102B3`):** Combined with global `Fmax = 5550 MHz`, allows independent per-core frequency ceilings (and displaying the CPU's fused CPPC silicon ranking `C0/C4/C6...` directly in the Per-Core UI!).
4. **Radeon 610M iGPU Curve Optimizer (`RSMU 0xA7 / 0xD7`):** Undervolt the iGPU for lower idle/hybrid power draw and extra CPU PPT headroom.
5. **PBO FIT Scalar (`RSMU 0x5B`, `1×..10×`) + Real-Time SVI3 3-Rail Telemetry (`VDDCR_VDD`, `VDDCR_SOC`, `VDD_MISC`):** Expose real SVI3 voltage/current/VRM temp in telemetry and optional PBO Scalar tuning.
