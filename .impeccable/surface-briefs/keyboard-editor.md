# Keyboard editor

Mode: Operate. Platform: native Windows x64, Rust/Win32/GDI. Build path: code. The workflow remains the qualified four-zone extension; its current controls inherit the approved shared dark visual system.

## Direction contract

THESIS: Verified four-zone colors in a native save/check/apply/restore workflow. The surface makes saved RGB intent, explicitly read firmware state and verified transaction results distinguishable.

OWN-WORLD: Inherit root DESIGN.md, shared charcoal ground/slate fields, Segoe UI and native rectangular actions. Mint identifies metadata save. Draft swatch colors remain user data. The curve chart keeps its separate data palette.

STORY: Choose or create a profile, edit RGB and static-versus-firmware mode metadata, read actual firmware after Windows approval, apply the whole saved profile through a fixed UAC child, and restore the same profile's recorded original transaction.

FIRST VIEWPORT: Profile selection/name above four equal zone rows. Each row contains a hex field, draft swatch, Windows color picker and explicit firmware provenance. Mode selection precedes check/copy/save actions; whole-profile apply and original restore sit separately above a persistent status field.

FORM: Standard Win32 controls, shared dark field/action rendering, RGB hex and picker swatches. Drafts remain independent of firmware GET snapshots. Metadata save preserves unrelated settings. Dirty or unchecked drafts, open recovery and unsupported whole-profile settings block apply. Hardware actions clear the live snapshot before dispatch.

QUALITY BAR: Recognize a draft versus an explicitly read firmware color; save without hardware change; inspect the entire profile before a separate UAC apply; restore the selected profile's complete recorded original. Static/custom versus firmware preset is the exact supported mode, not a generic animation or brightness control.

## Persistent facts checked

- `app/keyboard_ui.rs` uses Segoe UI at existing GDI lfHeight -18 and standard BUTTON, EDIT, STATIC and COMBOBOX controls. `theme.rs` supplies common dark colors and pushbutton states. ChooseColorW remains the Windows color dialog; no font/runtime assets are added.
- Draft swatches use typed RGB field values and a Windows system text-color frame. They establish no fixed accent color. Shared fields/background/actions follow DESIGN.md; save is mint and native disabled actions remain visibly unavailable.
- Four zone rows repeat at a 58-unit interval with 20-unit left inset. The outer window requests 780×780; the inspected own-client render is 764×741 at the current DPI. These are observed dimensions, not a verified multi-DPI/responsive system.
- `keyboard_ui.rs` supplies metadata save, explicit GET/profile check, apply and scoped restore. `app/elevation.rs` owns fixed current-binary UAC actions and nonce/result verification; profile_commands.rs rechecks selected profile ID during elevated restore.
- `core/keyboard.rs`, `platform/windows/hp.rs` and `controller.rs` remain state/transport/transaction truth. Write support is exact to 8E35/F.13 and capability [1,0,0,0]. RGB replaces only bytes 25–36 of the complete color image; supported raw modes are 0x64/0xE4. Both lighting features remain unsafe for automatic startup.
- `.impeccable/review/dark-keyboard.png` (764×741) was opened and checked. It contains actual selected profile/name, EDIT hex and COMBOBOX mode text, four draft swatches, action labels and status. It used temporary saved UI-Vorschau metadata, subsequently removed. Firmware was not read or applied for the capture.
- The own-client renderer prints production parent/child controls. Preview visibility is activated offscreen without keyboard activation so EDIT text prints correctly. The prior hidden-field capture limitation does not describe this current evidence.

## Evidence boundary

The capture supports pictured client layout and current native field contents, not titlebar, other DPI settings, dynamic high-contrast switching or comprehensive accessibility certification. No HTML/CSS detector ran for the native surface. The current dark finish verdict is **ship / G1 resolved**, scoring only the GPU helper/capability typography correction at the original pictured-client scope; the reviewer re-opened all five valid captures. It does not certify the later telemetry/hash-lifetime implementation or real power transitions. Current binary-specific smoke/memory results belong in [VALIDATION.md](../../docs/VALIDATION.md); the earlier stale-GET fix verdict remains scoped historical evidence.

The theme work does not alter the qualified BIOS contracts or whole-profile transactions. Hardware readback/restore qualification does not establish reboot/resume/hotkey behavior, physical animation or service-independent OEM replacement. Brightness percentages, off/animated/per-key writes and automatic lighting reapplication remain unsupported.
