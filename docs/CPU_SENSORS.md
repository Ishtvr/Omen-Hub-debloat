# CPU sensors

Measured hardware telemetry is implemented for **HP 8E35 / F.13 / Ryzen 9 8940HX** using an already running, exactly verified **PawnIO 2.2.0** driver and the embedded signed **AMDFamily17 0.2.11** module. It does not install/start a driver or load AMD's proprietary SDK. [Hardware support](HARDWARE_MATRIX.md).

## What the values mean

| Value | Source and interpretation |
| --- | --- |
| CPU load | Two Windows `GetSystemTimes` samples, excluding idle |
| Windows MHz | OS-reported processor information; not measured boost |
| Package watts | Delta of AMD package-energy counts over a measured monotonic interval |
| Tctl | Labeled CPU control temperature; not a per-core or ACPI-zone average |
| Effective max | Highest interval-effective clock across all 32 logical processors, including idle time in the denominator |

The first energy/clock baseline has no watts/effective-clock value. Missing, reset, partial or stale counters do not become zero or estimated readings. A configured PPT limit is never shown as measured draw.

Fixed reads use energy-unit/package MSRs **0xC0010299 / 0xC001029B**, read-only MPERF/APERF **0xC00000E7 / 0xC00000E8**, and Tctl SMN **0x59800**. SMN address-window selection is serialized through `Global\Access_PCI`; it is not a power/voltage setting.

Effective clocks count APERF cycles over wall time. MPERF progress is a plausibility requirement, not a request to change counters. The own thread temporarily visits each logical CPU and restores its original affinity. The reader checks EffFreqRO, one processor group, complete coverage, per-read/batch timing and 200–10000-ms intervals. HWCR/counter-enable/reset are not modified. A clock-only fault can leave valid package watts/Tctl available. Active boost peaks and per-core/CCD temperatures remain unimplemented.

## Exact prerequisites

| Component | Qualified identity |
| --- | --- |
| PawnIO.sys | 2.2.0, SHA-256 `FCA6E7D58B0CF38DBB913A2B9E532F48629145D395F454B16A9F58E97B8D3940` |
| AMDFamily17.bin | Signed 0.2.11, 10652 bytes, SHA-256 `DAE74615761B78BDF064DFB3E136252DDCC6FC727D88F14738D0E5800D427A91` |
| Module sources | `52a7e536dff3e53c96917a28caac5e0fa6510696` |
| CPU | CPUID signature 10882834, Family 19h / Model 61h / Stepping 2, Package 1 |

`cpu_hardware.rs` queries service identity/status, file version/hash and module identity. Missing permissions, absent modules, version mismatches and invalid data are explicit errors. The SMU tuning module is a separate component described in [System controls](SYSTEM_CONTROLS.md).

## UAC, helper and pause behavior

The first visible dashboard requests one session-bound CPU-reader approval. Cancel/error retries only via Refresh. A temporary launcher owns the UAC call; the separate reader exposes fixed preparation/sample/pause/stop operations through a user/SYSTEM-restricted local pipe with peer PID, nonce, schema, sequence and bounded replies. It accepts no tuning parameters or arbitrary register/function names.

Preparation has a separate ten-second budget covering bounded inventory and reader opening; sample/pause use five-second budgets. The helper caches only the validated inventory identity for its process/boot session. Reopening still checks driver/module identities. An operation spanning a power transition is discarded.

Tray, suspend and relevant display events pause samples, release hardware sessions and reset energy/clock baselines. Reopening, refresh, resume or source change starts fresh. Stale data is cleared. The last technical failure is saved separately as `last-cpu-error.json`; `cpu-sensor-status` reports that historical failure, not current live reader health.

## Build and evidence

Run `scripts/prepare-cpu-module.ps1` before building; distribute the module's corresponding LGPL source and notice with binaries containing it. The preparation does not install PawnIO. Full instructions: [Building](BUILDING.md), [Third-party components](../THIRD_PARTY.md).

`cpu-hardware-read` is a fixed elevated diagnostic: baseline, at least roughly 1100 ms wait, then a second sample. Native sensor/tray probes and simulated suspend/resume verified fresh baselines and complete 32-CPU coverage on recorded builds. Actual sleep/reboot, longer runs and additional devices remain pending; old RAM samples do not qualify later builds. [Validation](VALIDATION.md), [Power lifecycle](POWER_LIFECYCLE.md).

Interface references: [PawnIO user-mode protocol](https://github.com/namazso/PawnIO/blob/9d52965895588b0ffa3703b72eec75ba19f4ccc0/PawnIO/include/pawnio_um.h), [AMDFamily17 source](https://github.com/namazso/PawnIO.Modules/blob/52a7e536dff3e53c96917a28caac5e0fa6510696/AMDFamily17.p), [RAPL](https://github.com/torvalds/linux/blob/master/arch/x86/events/rapl.c), [k10temp](https://github.com/torvalds/linux/blob/master/drivers/hwmon/k10temp.c). Register conversions and lifecycle code in this repository are original Rust.
