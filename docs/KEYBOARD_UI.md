# Keyboard lighting

Open **Lighting / Beleuchtung** in the main Rust/Slint window. The keyboard-ui
command selects that same work area. An original schematic keyboard supports
QWERTZ/QWERTY display and clicking keys to select their hardware group. Keyboard
users select the same groups with four zone buttons. The keyboard fills the
centered work area rather than using the former small fixed-width preview.

- **WASD** selects wire zone 3.
- **Left** selects wire zone 2; **Middle** independently selects wire zone 1.
- **NUM pad & right** selects wire zone 0. The right hardware area also includes
  adjacent main/arrow keys. It cannot be controlled as numpad-only or per-key RGB.

Choose a color in the shared in-app HSV/HEX editor, including sixteen RGB swatches and a keyboard route. Accept stages the chosen color; cancel changes nothing. Brightness adjusts the selected
color's RGB value from 0 to 100%; it does not write an unqualified master-brightness
byte. Changing brightness repeatedly uses the retained full-color base rather
than compounding rounding. Merely selecting a zone changes neither its color nor
another zone; the previous combined Rest group has been removed.

Color, brightness and On/Off are **drafts**. The keyboard previews them; **Apply**
writes through the existing guarded transaction and reads back the complete state.
Canceling the picker changes nothing. Reset restores the last read draft.
Unsaved lighting survives navigation/tray return and is covered by exit confirmation.
Refresh is available after resetting an edited draft, so it cannot silently discard edits.

## Contract and recovery

Writable support remains HP **8E35/F.13**, capability **[1,0,0,0]**, classic four-zone
keyboard with numpad. The existing direct BIOS-WMI transport loads no HP SDK/RPC wrapper.
Colors replace only bytes 25–36 of the retained 128-byte image. Mode values remain
**0xE4 = static On** and **0x64 = Off**; the entire rest of the brightness/mode image
and all reserved bytes are preserved.

DirectLighting uses its own keyboard-control store. Complete originals precede
SET; fresh expected-state checks reject outside changes, and readback/restore include
capability, colors, mode and reserved bytes. Pending performance recovery blocks
new direct lighting writes. Closing waits for an active own operation and never
terminates a cooling helper.

## Evidence and limits

The manufacturer UI assembly 1101.2609.3.0 maps indices 0/1/2/3 to right/middle/left/WASD.
Its Khalilah classic-with-numpad path selects the corresponding four-zone layout.
This corrects the old UI's misleading left/middle/right/numpad labels; saved RGB
arrays and recovery images are not reordered. The new diagram is schematic, not a
claim that every individual key has an independent LED or a newly measured boundary.

The four individual groups restored on 17 November use these same wire indices;
saved RGB arrays and recovery images are unchanged. The body no longer repeats the
schematic/readback explanations: a compact Preview badge identifies an edited
draft and Apply retains its readback description for accessibility.

The existing 15 October device trials qualified static RGB bytes and visible On/Off
with original restore. The 16 November work adds pure mapping/RGB-brightness tests
and isolated real-UI draft interactions. It performs no new physical lighting write.
General firmware-brightness bytes, animation, per-key controls, exact visual zone
boundaries and HP-independent hotkey/reboot/resume lifetime remain unqualified.
[Protocol](KEYBOARD_LIGHTING.md) · [Validation](VALIDATION.md).
