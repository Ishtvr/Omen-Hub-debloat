# Keyboard lighting

Open **Lighting / Beleuchtung** or `omen-hub-pp.exe keyboard-ui`. This is a separate elevated native window; the dashboard/tray remain unelevated. It shows four clickable zones including the numpad, **On/Off**, and a short status line.

1. On enables the stored static zone colors. Off disables lighting while retaining RGB values.
2. With lighting on, click a zone or reach it by keyboard to open the native Windows color picker, initialized from its read color.
3. Confirm to apply and fully read back the change. Cancel writes nothing. No separate Save/Apply button is needed.

The keyboard picture represents four zones, not individual keys. Off disables zone editing. Invalid/external state requires a fresh read; a rejected action is not retried automatically.

## Contract and recovery

Writable support is **HP 8E35/F.13, capability `[1,0,0,0]`, FourZoneWithNumpad**. On this device, reading the BIOS provider also requires administrator access. The own Rust path uses Windows BIOS-WMI, without loading HP RPC/SDK wrappers.

Colors replace only bytes 25–36 of the complete 128-byte state. The supported mode bytes are **0xE4 = static On** and **0x64 = Off**. Legacy `firmware_default` is retained as a serialization alias for Off on this model, not a third mode or percentage brightness.

`DirectLighting` uses the typed transaction engine with its own `%USERPROFILE%\.omen-hub-pp\keyboard-control` store. Complete originals precede SET; capture/readback includes capability, colors, mode and reserved bytes. Fresh expected-state checks reject foreign changes. Pending performance recovery blocks new direct lighting writes; the shared lease serializes own writers only.

Reopening can recover a valid unresolved own lighting transaction before reading a new state. Closing waits for an in-flight hardware operation; it does not kill the writing worker. The direct window does not replace the performance journal or sign-in selection. [Recovery](RECOVERY.md), [protocol details](KEYBOARD_LIGHTING.md).

## Evidence and limits

On 15 October, the real UI check verified On/Off, prefilled picker, Cancel without SET, a red left-zone target with other zones preserved and complete original restore. Color evidence was full firmware readback, not observed visual color acceptance. A separate user-requested four-second **On → Off → On** probe was visibly confirmed and restored.

The current controller permits sign-in authorization for lighting settings in a supported applied profile. That permission does **not** qualify actual reboot/resume, Fn-F4, lock/unlock, lasting ownership or operation without HP writers. Brightness percentages, animations, per-key control and other keyboard variants remain unsupported.

Native synthetic renders test On/Off layouts without UAC or firmware reads; real `check-lighting-ui.ps1` tests deliberately write/restore lighting. The two are distinct. Recorded client sizes were 984×551 and 744×441 at 96 DPI; titlebars, color-dialog presentation, broader DPI/high contrast and assistive technology remain separate work. [Validation](VALIDATION.md), [hardware matrix](HARDWARE_MATRIX.md).
