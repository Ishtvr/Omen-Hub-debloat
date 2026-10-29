# BIOS cooling handover

Evidence from **23 October 2026**, HP **8E35/F.13**, Ryzen 9 8940HX / RTX 5070 Laptop; original Hub 1101.2609.11.0. Reviewed for documentation on 29 October; no new experiment was performed.

## Firmware observations

The read-only DSDT capture had a valid ACPI checksum and identified HPQOEM/8E35, SHA-256 **`08558148F65C7FBF88AD8D61FF7D94330E6569A29A3D9D6FA44452539DECD7EE`**. ACPICA iASL 20240927 was used for static analysis; raw/disassembled vendor material remains ignored locally.

- `0x20008/0x10` sets **OMCC** in addition to returning fan-related bytes. Repeating it changes watchdog state.
- `0x1A` input byte 1 sets performance **HPCM**, byte 2 sets **FAMC**. It is not an independent Auto switch; the original performance mode cannot be guessed.
- `0x2E` uses the first two bytes of the known 128-byte block for **SRP1/SRP2**. Successful status and tachometers are not full ownership readback.
- `0x2D` tachometers are rounded firmware 100-RPM observations.

The manufacturer still uses software cooling on AC in the examined Hub version. Its BIOS-Auto branch requires DC, thermal Auto and model conditions. A saved Auto choice is not proof of hardware ownership. Public [hp-wmi](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/hp/hp-wmi.c) is a semantic reference; its separate zero-level route is not assumed valid for this exact Windows payload.

## Bounded device probes

| Probe | Observation | What it establishes |
| --- | --- | --- |
| 22:36–22:39 CEST, watchdog | After a 48/48 period, 150 seconds without further heartbeat/levels; RPM fell after roughly 122 seconds from 4800/4800 toward 2300/2100 | Autonomous fallback, not immediate safe handover or unchanged full power policy |
| 22:48 CEST, FAMC | Known Unleashed byte 04 with FAMC 1 reduced RPM despite continued heartbeat/48 targets; at 86 °C Tctl the preset 85-°C abort condition stopped the trial | Real effect, **no unattended BIOS-Auto qualification** |

The FAMC trial restored the known software payload with maximum cooling and restarted the original Hub. GPU curve/offsets, CO, metadata and vendor configuration were preserved. CPU Slow was later 66 W; the exact source/timing of that change was not inferred. HP Windows services remained active. A bounded EC-read attempt returned no timely answer and was abandoned; it is not a runtime path.

## Current behavior

The normal app does not expose the experimental direct FAMC=1 switch. After own regulation, a regular pause/stop requests maximum once, stops further commands and retains explicit **`handover_pending`**. Elapsed time alone never changes this to confirmed BIOS ownership.

An identified original controller preempts own writes. Restarting own regulation rechecks layout and establishes a fresh heartbeat. Failure to send the conservative release remains a failure, not a successful handover.

## Needed before completion

Obtain reliable current performance/FAMC ownership state and a thermally qualified direct handover preserving own power settings. Verify real AC/DC, standby/resume, reboot and failure cases without HP user-mode controllers. Integrate this with durable own-controller recovery and total-profile transactions. [Fan control](FAN_CONTROL.md), [replacement plan](REPLACEMENT_PLAN.md), [validation history](history/VALIDATION_THROUGH_2026-10-26.md).
