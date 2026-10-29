# Replacing original OMEN controllers

Omen Hub++ must eventually operate without original HP/OMEN user-mode hardware-controller processes. Their modules are research sources, not intended runtime dependencies. Windows firmware providers and properly installed device drivers remain platform foundations; removing them is not a replacement test.

## Current position

| Area | Own path | Work still needed |
| --- | --- | --- |
| Inventory / power source | Windows APIs/WMI | Broader devices and lifecycle acceptance |
| GPU sensors / offsets / V/F | Public telemetry and exact qualified own control adapters | Current helper/driver/power lifetime and load tests |
| CPU sensors / SMU controls | Fixed verified PawnIO modules, original Rust integration | Durable power-profile ownership, full range evidence and actual transitions |
| HP factory modes / Dynamic Boost | Model-derived BIOS-WMI calls already implemented | Complete original-mode readback/recovery and lifecycle |
| Keyboard | Direct BIOS-WMI four-zone RGB/static On-Off with full originals | Hotkeys/lock/unlock/resume/reboot without HP writers |
| Own cooling | Separate own curve/Max helper, fixed sensors/heartbeat/levels | BIOS ownership handover, durable independent safety and profile integration |
| MUX | BIOS boot preference already implemented | Physical reboot routing, iGPU-only/dGPU power and shortcut recovery |
| Presets and sign-in | Typed journaled transactions and protected exact-revision approval | Real reboot/resume/AC and durable controller integration |

These own paths do not load proprietary HP RPC/SDK wrappers. Most brief device trials left vendor services running; that does not prove full independent operation. Current availability is centralized in the [hardware matrix](HARDWARE_MATRIX.md).

## Original components and their roles

- **HPOmenCap:** hosts hardware/tuning capabilities. The own native migration adapter is fixed to its measured image/configuration; ordinary app flow does not automatically stop it.
- **HPSysInfoCap / WMIService:** RPC intermediary to Windows-WMI. Direct `ROOT\WMI/hpqBIntM` avoids that wrapper path.
- **HpReadHWData:** a separate hardware transport seen in the vendor code. It is not established as necessary for direct BIOS-WMI and is not distributed.
- **OMENLighting:** virtual HID/VHF lighting integration over BIOS-WMI. Direct static zone control and Windows lamp integration are distinct paths.
- **Other shared HP helpers:** installation alone proves neither dependency nor safe removability. Their shared responsibilities require separate attribution.

## Replacement sequence

1. Close the known shortcut-recovery, CPU range-evidence and fan-status gaps. Preserve working model-bound contracts.
2. Supervise the prepared own migration/guardian/cooling route with exact originals and bounded conditions. The temporary OEM return is a development fallback, not the final runtime design.
3. Establish readable coupled performance/fan ownership and safe BIOS handover. A silent controller, Max-Fan Off or saved Auto is insufficient.
4. Implement durable own ownership for ordinary operation and sign-in, including crash/driver-loss and full hardware recovery ordering.
5. Integrate necessary model-specific heartbeat, AC/DC, display, suspend/resume and keyboard hotkey behavior. Do not assume one 30-second schedule fits all functions/devices.
6. Prove real transitions, full profile consistency, cooling/thermal behavior, load stability and total resident memory without original HP writers.

## Completion evidence

Record the exact device/BIOS/driver/build, originals, actual target readbacks, restored state, controller processes, power conditions and failure behavior. A closed original GUI, one successful WMI call, brief service isolation or simulation is not full replacement acceptance.

Live hardware/service experiments remain explicit supervised work. Routine build/docs checks do not stop/remove vendor services, install a driver, flash firmware or change performance settings. [HP ownership](HP_CONTROL.md), [fan handover](FAN_HANDOVER.md), [power lifecycle](POWER_LIFECYCLE.md), [Roadmap](ROADMAP.md).
