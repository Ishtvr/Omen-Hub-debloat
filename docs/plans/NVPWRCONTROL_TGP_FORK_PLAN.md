# GPU-TGP investigation plan

Documentation reviewed **29 October 2026**. Technical observations below come from the **26 October** read-only investigation, Omen Hub++ source through `251fff6`. **Current scope is investigation and planning.** No new backend, hardware SET or reference GitHub fork has been produced. A local research worktree exists; it is not an upstream fork or implemented writer.

## Objective and hard requirements

Investigate whether an original, coherent and fully restorable GPU-TGP control can be reached through existing NVIDIA/OEM interfaces on **our HP OMEN**. Do not assume that a method for a 5070 Ti applies to the non-Ti GPU.

- App processes must run in user mode; UAC/admin is allowed. **TPM and Secure Boot remain enabled.**
- No test-signing, disabled signature enforcement, added test-certificate trust or security bypass as the product path.
- Prefer original Rust over an existing supported driver interface. A narrowly scoped licensed C/C++ user-mode shim is a fallback, not a way to turn kernel patching into user mode.
- Preserve offline/native runtime, resource budget, exact model/build binding and complete original/target/restore evidence.
- Do not install/start foreign driver modules, execute upstream tuners or issue guessed control packets during this investigation.

## Pinned upstream reference

[LevinAi-arch/rtx-5070ti-laptop-160w-power-limit](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit) was inspected at the following identities. These statements describe that inspection, not a fresh upstream check on every documentation edit.

| Reference | Observed result |
| --- | --- |
| `main` and `v2.3.0` | Commit **93f55e954e4dc3db1aefe5fb3bd4f0a1f8d283c3** |
| `v1.8.0` | Commit **ab8d6d0a3b05245dd6ad87afa29943337f06f2d5** |
| [v2.3.0 release](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit/releases/tag/v2.3.0) | Author claims user-mode control without their custom unsigned kernel driver, using a Lenovo/5070 Ti reference. No GitHub assets; refers to paid Pro. The examined public tree did not expose a separate new user-mode TGP backend. |
| Public v1.8 unified ZIP | SHA-256 **DB42DD704B5DF477CB309BFAAFB27CFA0A691D0747509D93FD6C1D183D82BA37**, includes source and prebuilt files |
| Root “v1.8.0 RTX 40/50” ZIP | SHA-256 **3CB163D54ADEB314A2EDFC5C2277CBFE21A473CE5070CE369C54FA3EB15095DD**, binary package without source |
| Version consistency | Release/archive/current source files differ; SYS blobs also differ. Do not assume every included binary is a reproducible build of one source set. |
| Reuse status | No explicit reuse license was found in the inspected tree/archive/source headers; repository metadata reported null license. A public fork does not by itself establish permission to copy/port code into this MIT runtime. |

The recent author claim is a research lead, not proof of compatibility. Access to Pro source is **not a prerequisite** for independent investigation. No purchase/contact/private download occurred. Reference source/archive copies stay ignored in `.local/research/nvpwrcontrol/`; only original concise observations belong in Git.

## What the old kernel path actually does

The examined v1.8 driver binds NVIDIA **616.92**, specific image signatures/layouts and kernel objects. It changes coupled base/cTGP, boost amount, UPPER/MAX and invokes an internal generator to establish current policy. It uses a fixed **25-W** boost split and session-specific originals. Driver unload explicitly does **not** restore automatically.

The pinned low-power non-Ti 5070 branch expects a coherent **115-W** baseline and accepts **120–140 W** in 5-W steps. The Ti branch has a different baseline/range; README tables conflict with parts of the pinned implementation. Neither range is our qualification. The separate XMG high-power contract is not our HP backend.

The shared `\\.\Nvpwr` interface has status v7 and SET v2, with status/set/restore operations and a 16-byte SET request. Its GUI ensures a kernel service, so running it for “status” is not assumed passive. The old driver was not loaded. Its addresses/pointers cannot be called directly from user mode or reused across driver sessions.

## Target and observed public values

| Property | Reference |
| --- | --- |
| Model | HP OMEN 16-ap0xxx, **8E35 / F.13** |
| GPU | RTX **5070 Laptop**, non-Ti, **10DE:2D18**, subsystem **103C:8E35** |
| NVIDIA / VBIOS | **617.42 / 98.06.2a.40.da** |
| Public power values | Default **80 W**, allowed **5–115 W**, separately effective **115 W** |
| Missing original | Configured/requested limit unavailable through current public queries |

Effective/default values are not the configured original and do not prove draw, a 25-W internal split or a 160-W capability. Actual TPM/Secure-Boot state was not newly qualified during the research; retaining them enabled is a required future acceptance condition.

## Candidate approaches

| Approach | Existing evidence | Next question | Position |
| --- | --- | --- | --- |
| **A. Trace existing NVIDIA control transport** | Exact 617.42 Ghidra path includes an administrative `\\.\NvAdminDevice` branch and `DeviceIoControl`; DOS device entry visible | Which policy operations select this branch, with what access rights, packet contract and matching handler? | Highest research priority; **not a proven TGP writer**. Device was not manually opened or sent a control packet. |
| **B. Private NVAPI policies** | Info/Status interfaces and setter statically present; validated GET frames callable | Does privilege/model selection or another offered policy class provide actual originals/bounds? What is the watt relationship? | Classic non-elevated v1/v2 calls returned empty policy availability, despite status 0. |
| **C. Modern public NVML** | v2 setter export present; some public limits/scopes readable | Is a correctly scoped configured original and full write/restore contract available? | Preferred documented API if complete; export presence alone does not solve missing original or extend OEM bounds. |
| **D. HP WMI / NVPCF budgets** | Own mode/cTGP/PPAB/shared-budget calls exist | Is there an actual numeric GPU-budget contract on 8E35, beyond fixed modes/preferences? | OEM fallback/complement; CPU limit, boost preference or AC/P-state change is not free TGP. |
| **E. New Nvpwr user-mode reference** | Author's v2.3 claim | Can legitimately available source/interface evidence explain its mechanism and applicability? | Optional comparison, no purchase/source dependency and no established HP support. |
| **F. Normally Windows-accepted signed kernel helper** | A user-mode app can technically call an approved backend | Additional kernel component, current signing/admission policy and independent lifetime/device evidence needed | Architectural reserve; **not pure user-mode reproduction**, not current implementation. Self-signing or Rust syntax does not satisfy it. |

A/B may converge on the same vendor path. Public [NVIDIA open kernel headers](https://github.com/NVIDIA/open-gpu-kernel-modules) are naming/semantic references, not proof of the Windows ABI. No examined public power header alone supplied our complete numeric TGP contract. [Microsoft user/kernel distinction](https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/user-mode-and-kernel-mode); [Windows driver policy](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy).

## Read-only results already obtained

- Resolved private power GETs belonged to implementation SHA-256 **C6A2014763C3662512B5D420DFC7632B777338881A13AEAB3F21D8033EAE6C71**. Live entrypoints matched the analyzed interface mapping.
- Info ID **0x34206D86**: v1 **184 bytes / 0x100B8**; v2 **2248 bytes / 0x208C8**. Status ID **0x70916171**: v1 **72 bytes / 0x10048**. Guard regions remained intact.
- As a **non-elevated user**, Info v1/v2 returned status 0 with no valid policy entries; Status v1 returned status 0/count 0. This supplied no original or usable target interval. Percent-mille fields cannot be relabeled watts.
- Setter ID **0xAD95F5ED** was analyzed statically, never invoked. Privilege/filter meanings are not inferred merely from a helper flag.
- Tracing showed a dispatcher/administrative transport branch, but did not prove our GET probes selected it or that old direct kernel effects are exposed through it.
- `NvAdminDevice` visibility was checked through the DOS device namespace only. The public NVML v2 setter was checked for export presence only. **No SET** was performed; no services or boot security settings were changed.

Raw GET evidence and native decompilation remain ignored locally. App hardware implementation was unchanged.

## Map effects to actual contracts

| Old kernel effect | Required user-mode evidence |
| --- | --- |
| Change base and dynamic amount | Actual originals, units, offered model bounds; no assumed fixed 25-W split |
| Change upper/MAX coherently | Valid coupled operation through an allowed interface, not direct kernel addresses |
| Commit/generate current policy | Proven eligibility/commit effect, not cached UI state |
| Verify current state | Distinguish configured/requested/effective/limiting components; empty lists/defaults are not snapshots |
| Restore completely | Same GPU/session identity and all own modified components; process exit/unload is not restore |

## Immediate next research

1. Finish transport/rights/handler attribution. Plan a standard-user versus administrator **read-only** comparison using only proven frames. No blind IOCTL fuzzing.
2. Examine offered policy classes and model/privilege filtering; the classic v1/v2 family is currently empty.
3. Resolve NVML scopes/units/original values and separate numeric GPU control from OEM performance preferences.
4. For each candidate record entrypoint, direction, privileges, device/build binding, snapshot, restore, boot compatibility and unknowns.
5. Decide on a prototype only after a readable original and plausibly corresponding allowed write/restore contract exist. Otherwise record a negative result without claiming a working slider.

## Later implementation, after that decision

1. Pin the independently evidenced contract and any required reuse permission. A fork and an available binary are not a code-integration license. [GitHub licensing guidance](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository).
2. If useful, create the authorized reference fork, preserve provenance and isolate actual integration in a `codex/` branch/worktree based on then-current app source. No upstream binaries go into normal packages.
3. Build a read-only Rust diagnostic for exact GPU/driver/session identity and all available original components. Verify with TPM/Secure Boot enabled before planning a write.
4. Keep core GPU-policy snapshot/validation separate from the narrow Windows transport/FFI. Use a small licensed C-ABI shim only if necessary. Keep the existing public NVML adapter separate. Never accept arbitrary raw driver commands from UI/profiles.
5. Test ABI/state fixtures first: bad versions, mixed state, lost replies, foreign changes and restore failure. Plan the first physical change **within the OEM envelope**, preferably a small decrease, with full immediate restore. The old above-115-W writer is unsuitable for that trial. Even an unchanged total is not a no-op if its base/boost split changes.
6. Only after successful full restore, integrate presets, sign-in, crash, sleep/resume and GPU/driver loss. Consider any above-OEM scope separately with supply/thermal/load evidence; missing VRM/VRAM/hotspot data must stay unknown. Repeat whole-helper tray acceptance and normal Cargo checks.

**Conclusion (updated 25 November 2026):** a clean, Defender-safe, Secure-Boot-compatible, flashless user-mode path **exceeding 115 W** has now been identified and empirically verified on **8E35 / F.13** (`RTX 5070 Laptop`, driver `621.14`, VBIOS `98.06.2a.40.da`).

## Verified finding — 25 November 2026 (`nvlddmkm.sys` registry VBIOS override, 125 W live trial)

### Why free vs paid external projects differ on Windows Defender

- **Legacy unsigned-driver approach (`NvpwrControl` free v1.8.0):** loads an unsigned kernel driver (`Nvpwr.sys`) that patches `nvlddmkm.sys` memory at runtime. Blocked by Windows DSE / Secure Boot and flagged by Windows Defender.
- **Defender-safe, driverless approach (used in principle by open flashless loaders such as `Panchovix/nvidia-windows-vbios-vix-loader` [GPL-3.0-or-later, research reference only — no code reused], `b00nz/mVolt+`, and modern user-mode tools):** uses `nvlddmkm.sys`'s own built-in registry VBIOS override (`HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0000\vbios`, `REG_BINARY`) plus an official GPU PnP restart (`pnputil /restart-device` or `CM_Disable_DevNode`/`CM_Enable_DevNode`). Zero custom kernel drivers, zero memory patching, zero EEPROM flashing, Secure Boot stays ON, standard signed boot stays ON, and Windows Defender stays 100% enabled and untouched.

### Ruled-out user-mode paths on `621.14`

- **Public NVML (`nvidia-smi -pl`):** strictly clamped to VBIOS `[5 W, 115 W]` (and reports `Changing power management limit is not supported in current scope`).
- **Private NVAPI (`0x34206D86` / `0x70916171`):** disassembled `nvapi64_impl.dll` (`621.14`) and probed read-only; returns `status=0, valid=0, count=0` on Dynamic Boost notebooks by RM design (`0x2080A619`/`0x2080A61A`).
- **`nvpcf.sys` alone:** `nvlddmkm.sys` F7 generator (`0x1404F4560`) clamps `CtgpTarget + PpabAmount` to `UpperBoundary` (`115,000 mW`) initialized from VBIOS `Policy[2]` (`0x1404F450A..0x1404F4523`).

### Read-only disassembly & VBIOS layout (`nvlddmkm.sys 621.14`, VBIOS `98.06.2a.40.da`)

- On GB206 GeForce (`arch 0x1B`), `nvlddmkm.sys 621.14` runs **Monolithic RM** by default (`EnableGpuFirmware` not needed).
- When persistent `HKLM\...\0000\vbios` exists, `0x1405BBA3C` sets `PDB_PROP_GPU_VBIOS_FROM_REGISTRY` (`pGpu+0xAE4 = 1`) and `0x140186A78 -> 0x1405BC0C0` loads the image before firmware/cache sources (`vbiosSource = 2`). Validation (`0x140185330` / `0x140185D60` / `0x1401851F0` / `0x1405FF010`) checks PCI Option ROM headers, BIT structure, and magic `0x4EDCBDCB`, with no RSA/ECDSA check rejecting Power Policy edits in Monolithic RM.
- Stock ROM is already cached verbatim by `nvlddmkm` in volatile `HKLM\...\0000\Session\vbios` (readable without elevation): `465,664` bytes (`0x71B00`), SHA-256 `C7082786D9E4A063EC249BEA8FE8ECAF69054C0EF2CFEB0B46F3C393F44E11CB`.
- BIT `'P'` (`0xF36`) -> `P[+0x2C] = 0x3FA17` -> file offset `0x57A17` (Power Policy Table v4.0, 20 entries × 102 bytes, `tgpPolicyIdx = 2`).
- **`Policy[2]` (`0x57B1B`, `PWR_POLICY_TOTAL_GPU`, channel 0)** holds all board-power limits in mW (and `0x14090DAA0` enforces monotonicity `inflection[0] >= inflection[1] >= ... >= inflection[4]`):
  - `0x57B2C` (`+0x11`): `limitRated` (Boot Default Board Power) = `80,000 mW` (`80 W`)
  - `0x57B30` (`+0x15`): `limitMax` (Board Power Ceiling) = `115,000 mW` (`115 W`)
  - `0x57B48` (`+0x2D`): `inflection[0]` = `100,000 mW` (`100 W`; followed by `60k, 20k, 17k, 0`)
  - `0x57B64` (`+0x49`): `tgpRated` (Notebook Base TGP, `TGP_IFACE+0x28`) = `100,000 mW` (`100 W`)
- `Policy[13]` (`0x57F7D`, NVVDD OCP) is already `180,000 mA` (`180 A`). `ROM[2]` (`0x27A00..0x71B00`) has no 8-bit checksum (`sum = 0xE7`).

### Live hardware verification (`115 W -> 125 W`, auto-restoring trial)

- **Patch (+10 W coherent shift, 8 bytes changed across 4 uint32 fields in `Policy[2]`):** `limitRated 80 -> 90 W`, `limitMax 115 -> 125 W`, `inflection[0] 100 -> 110 W`, `tgpRated 100 -> 110 W`.
- **Environment verified before & after:** `SecureBoot = True`, `bcdedit` standard signed boot (no `testsigning`/`nointegritychecks`), Windows Defender `AM=True / AV=True / RTP=True / BM=True`, **0 Defender threats/alerts**.
- **Critical HP OMEN (`8E35`) post-restart finding:** restarting `PCI\VEN_10DE&DEV_2D18...` (`pnputil /restart-device`) succeeds cleanly (`PnP=OK / CM_PROB_NONE`, stock Monolithic RM, no `EnableGpuFirmware` change), immediately updating `Default = 90.00 W, Max = 125.00 W`. However, `nvlddmkm` wakes up with cTGP inactive (`enforced = limitRated - 15 W`, i.e., `65 W` stock / `75 W` at +10 W) until HP WMI **Unleashed + cTGP/PPAB + TPP** (`0x1A [FF,04,00,00]`, `0x22 [01,01,01,57]`, `0x29 [FF,FF,FF,48]`) is re-applied. Re-sending those three WMI calls immediately re-engages Dynamic Boost (`90 + 20 + 15 = 125.00 W`).
- **Real load comparison (500 ms `nvidia-smi` polling, `SecureBoot = True`, Defender 100% ON with 0 alerts):**
  - **Stock 115 W baseline (`gpu_burn3` 4K MRT / `gpu_burn4` 4K Aniso-16×+MRT):** `Enforced = 115.00 W`, steady-state average draw **`114.75 W` / `114.11 W`** (max `115.50 W`), throttled on every steady sample by `0x04` (`SW Power Cap`), max temp `65.9–67.3 °C`.
  - **Unlocked 125 W trial (+10 W shift: `90 / 125 / 110 / 110 W`, `gpu_burn3`):** `Default = 90.00 W`, `Max = 125.00 W`, `Enforced` ramped dynamically to **`125.00 W`** (`SteadyAvgEnf = 122.46 W`), steady-state average draw **`119.12 W`** (15/17 steady samples > 115 W, peaks **`125.72–131.16 W`**), throttle shifted from `0x04` (`SW Power Cap`) to `0x400` (`Vrel`), max temp **`73.4 °C`**.
  - **Unlocked 135 W trial (+20 W shift: `100 / 135 / 120 / 120 W`, `gpu_burn4` 15 s):** `Default = 100.00 W`, `Max = 135.00 W`, `Enforced` ramped dynamically to **`135.00 W`** (`SteadyAvgEnf = 129.74 W`), steady-state average draw **`124.52 W`** across 23 steady samples (+10.4 W sustained above stock `114.11 W` on the identical load) with repeated bursts to **`140.30 W`, `142.04 W`, and `140.98 W`** (+26.5 W over stock peak), throttle predominantly `0x400` (`Vrel` voltage ceiling at `2640–2647 MHz` — no longer power-capped), max temp **`71.7 °C`**.
- **Rollback verified (after both 125 W and 135 W trials):** removing `0000\vbios`, restarting the dGPU, and re-applying HP Unleashed returned exact stock (`Default = 80.00 W, Max = 115.00 W, Enforced = 102.97 W` idle / dynamic up to `115 W`, `0000\vbios exists = False`, `EnableGpuFirmware exists = False`, Defender 100% active with 0 alerts).

### Historical integration proposal

The following items describe the research proposal. The [subsequent crash investigation](../VALIDATION.md#gpu-power-apply-incident--25-november-2026) records the observed dashboard failure and the behavior of the current worktree. Release qualification still requires an actual transition between different targets. A confirmed, already-loaded ceiling only establishes readback.

1. **Dynamic driver key & fresh stock extraction:** never hardcode `0000` or bundle ROM bytes; resolve the live GPU PnP `DriverKey`, read verbatim stock ROM from `Session\vbios` (verifying `98.06.2a.40.da` / BIT / `Policy[2]` invariants before patching), and persist a SHA-256-verified backup + recovery journal.
2. **Post-restart re-apply sequence:** after `CM_Disable_DevNode`/`CM_Enable_DevNode` (or `pnputil /restart-device`), automatically re-apply HP WMI `0x1A/0x22/0x29` and any active NVAPI V/F curve / core / VRAM offsets (which clear on GPU restart). Auto-rollback immediately if `PnP Status != OK`.
3. **Thermal / VRM / 230 W PSU safety bounds:** hardware is designed for 115 W GPU on a 230 W brick; cap any future UI slider conservatively (e.g., `125 W` / max `+10..+15 W`) and pair with CPU Slow PPT caps under combined load.
4. **Driver updates & anti-cheat:** NVIDIA driver updates clear/replace the display class key (requiring clean re-apply from fresh `Session\vbios`). While Secure Boot, DSE, and Defender stay 100% intact, one-click Restore (`remove vbios + restart GPU`) should always remain immediately available.

## Limiting peaks with the 135 W override — 25 November 2026

The target has a **±5 W tolerance** around the requested value. For peak control,
the upper side applies across abrupt load changes. A 135 W request allows peaks
up to **140 W**. Draw can fall below the target as the workload drops. Retain the
established path that makes 135 W available. A lower
operating limit can serve as a peak reserve if it works in a controlled comparison.
Its performance cost must be measured separately.

### Evidence on the reference laptop

- **8E35 / F.13, RTX 5070 Laptop, 621.14, VBIOS 98.06.2a.40.da.** NVIDIA still
  reports a 100 W default and a 135 W maximum, with the configured request
  unavailable through NVML. The live registry image matches Omen Hub++'s saved
  hash. The stock backup also matches its saved hash. A byte comparison found
  changes only in the four already documented total-GPU fields. The separate
  protection policy was untouched.
- The D3D11 load probe reported **151.70 W** via `power.draw.instant` against a
  135 W maximum and a 131.31 W enforced limit in the same query. It stopped at
  the first reading over 146 W. This reading exceeds the new 140 W criterion
  even at the low end of the stated ±5 W measurement accuracy. The earlier
  125 W and 135 W trials reported maxima
  of 131.16 W and 142.04 W through the legacy `power.draw` alias, which the
  installed NVIDIA utility describes as a one-second average on newer GPUs.
  Keep those historical samples separate from explicitly instantaneous readings.
  [Load probe and driver readback](../VALIDATION.md#gpu-power-apply-incident--25-november-2026).
- Power Smoothing and Workload Power Profiles returned unsupported. Adaptive TGP
  adjusted power, EDPp and the configured NVML power request returned unavailable.
  NVIDIA documents Fast Trigger as one possible source of a hardware-slowdown
  status bit. That status alone establishes neither an adjustable threshold nor
  a board-wide peak setting on this laptop. No writable fast peak control has
  been verified for this device. [NVIDIA clock event reasons](https://docs.nvidia.com/deploy/nvml-api/620/api/group__nvmlClocksEventReasons.html).
- A second NVML read-only probe returned no usable module or GPU-base limits.
  The power-history API offered a buffer size but returned no samples on the
  actual fetch. In six seconds at the desktop, 300 polls at 20 ms intervals
  produced only 23 changes in the instantaneous value, with a median of about
  250 ms between value changes. The driver supplied new CPU timestamps on
  each poll. This demonstrates duplicate readings in this run. It does not
  establish the sensor's sampling rate under load or capture shorter peaks.
  [NVML sample queries](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html).
- PowerMizer readback on 621.14 reported Adaptive, Maximum Performance and
  Auto. Its base-clock Consistent Performance mode was absent. The reported
  P0 clock range does not establish a writable peak control. The energy
  counter can support an interval average, but cannot resolve the maximum
  within that interval.

### Candidate for a controlled trial

Use the separate HP cTGP flag as a coarse operating reserve. The already loaded
135 W image stays in place. Elevated readback on this device shows cTGP and PPAB
enabled and the existing 87 °C temperature byte. The [8E35 platform evidence](../models/8e35.md#gpu-evidence)
describes a +20 W cTGP contribution and up to +15 W from Dynamic Boost in
Unleashed. Holding the mode and PPAB constant while turning cTGP off therefore
suggests an operating ceiling near `100 + 0 + 15 = 115 W`. This estimate must
be read back on the modified table before a load trial. A 20 W reserve can reduce
available sustained performance even when the stored 135 W maximum remains
available. A finer reserve that retains the full operating budget would require
an independently qualified control.

The HP shared budget (TPP) is a finer secondary candidate. Earlier stock-ceiling
measurements with both HP GPU flags on showed lower enforced GPU values at
55 W TPP than at 72 W TPP under load. That evidence concerns a shared operating
budget and depends on the workload split. Reuse would require an exact
current-TPP readback and restoration path, which the existing adapter does not
provide. [8E35 power observations](../models/8e35.md#cpu-and-windows-evidence).

For a future approved trial, capture the original HP flags and driver values.
Change only cTGP using the existing device-bound contract and confirm both flags
by readback. Proceed only if the live operating ceiling actually falls. Use AC,
known cooling and short, bounded workloads covering idle-to-load and repeated
load changes. Verify the original flags and 135 W image again after restoring.
Collect instantaneous draw, one-second average, effective ceiling, temperature,
clocks and sample times separately. Reusing the same cached sample after stopping
load does not provide a second peak observation.

HWiNFO64 8.60 is installed on the reference laptop. For the comparison trial,
log its NVIDIA GPU board-power channel (`GPU Leistung [W]` on this device) in
parallel with NVML using the Sensors CSV option. Keep chip or VRM rail readings
in separate columns and compare the actual timestamps and polling intervals.
Agreement is useful evidence of the reported draw. It does not provide an
independent measurement if both tools read the same NVIDIA telemetry.
[HWiNFO sensor logging](https://www.hwinfo.com/forum/threads/is-it-possible-to-log-the-values-of-a-given-item-over-a-period-of-time-and-then-export-them-into-a-spreadsheet-that-could-be-made-into-a-graph.7693/) · [NVIDIA sensor sources in HWiNFO](https://www.hwinfo.com/forum/threads/understanding-nvidia-gpu-power-sensor-readings.7601/).

The user started the HWiNFO CSV at 17:42. The first 58 desktop samples logged
25.47–31.67 W at 1–17% GPU core load and a median 2.01 s interval. HWiNFO
reported a 100 W nominal value and 135 W maximum. During the overlapping
read-only NVIDIA run, all six pairs with timestamps less than 200 ms apart
agreed within 0.98 W on board draw. Both tools reported the same 135 W maximum.
This checks channel selection during light load. The current two-second HWiNFO
interval is too slow to establish the maximum of a sub-second burst. Before a
controlled load trial, choose a shorter logging interval and check the interval
actually achieved in the exported log. The user-started log came after the
151.70 W event, so there is no simultaneous HWiNFO sample for that event.

The repeated D3D11 trial at 17:48 used the original shader with staged load
before reaching the full 4K draw loop. The workload completed eight seconds and
344 frames with 100% peak GPU utilization. The 135 W maximum stayed loaded.
NVML reported a **139.041 W instantaneous maximum** and a **134.455 W
one-second-average maximum**. No sample crossed the 140 W test stop threshold.
HWiNFO's configured 20 ms polling produced a median log interval of 70 ms.
Its `GPU Leistung [W]` maximum was **134.455 W**. Of 119 timestamp pairs
less than 30 ms apart, 79 matched the NVML one-second value exactly in mW,
and none matched the instantaneous value exactly. At the HWiNFO maximum,
the NVIDIA average matched within a 5 ms pairing gap. This supports using
the HWiNFO channel as an average cross-check for this configuration.

The staged run stayed within 140 W in the observations and raised no new
display-driver event. It did not apply a different power setting. Its different
load transition and averaging evidence leave the prior abrupt-load peak
unresolved. The first 151.70 W report cannot be classified as a sensor error
from this comparison. Use separate instantaneous and averaged channels for
further qualification.

The ±5 W acceptance tolerance is separate from the NVIDIA utility's stated ±5 W
accuracy for instantaneous draw. For a conservative 140 W physical acceptance
check at a sampled instant, the reported value would need to stay at or below
135 W. Even that criterion leaves between-sample peaks unverified.
Certifying a physical peak bound would also
require faster, independently validated board-power measurement and an evidenced
fast enforcement path. A Windows loop that reacts after reading a spike cannot
prevent that first spike. [NVIDIA power readings](https://docs.nvidia.com/deploy/nvidia-smi/index.html#gpu-power-readings).

Status for this follow-up is read-only investigation. The candidate is recorded
for later testing. No app code, GPU setting or firmware image was changed, and
the +5 W peak bound remains unverified.
