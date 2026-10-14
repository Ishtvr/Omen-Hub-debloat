# Keyboard editor

Mode: Operate. Platform: native Windows x64, Rust/Win32/GDI. Build path: code. Scope: ordinary extension of the incumbent native application.

## Direction contract

THESIS: Verified four-zone colors in a native save/check/apply/restore workflow. The surface makes saved RGB intent, explicitly read firmware state and verified transaction results distinguishable.

OWN-WORLD: Inherit the existing Win32 controls, Segoe UI and Windows system colors. This extension adds no dashboard redesign or global visual world; the dark curve workbench keeps its separate identity.

STORY: Choose or create a profile, edit RGB and static-versus-firmware mode metadata, read actual firmware after Windows approval, apply the whole saved profile through a fixed UAC child, and restore the same profile's recorded original transaction.

FIRST VIEWPORT: Profile selection and name above four equal zone rows. Each row contains a hex field, draft swatch, Windows color picker and explicit firmware provenance. Mode selection precedes check/copy/save actions; whole-profile apply and original restore sit separately above a persistent status field.

FORM: Standard Win32 controls and RGB hex plus picker swatches. Drafts remain independent of firmware GET snapshots. Metadata save preserves unrelated settings. Dirty or unchecked drafts, open recovery and unsupported whole-profile settings block apply. Hardware actions clear the live snapshot before dispatch.

QUALITY BAR: Recognize a draft versus an explicitly read firmware color; save without hardware change; inspect the entire profile before a separate UAC apply; restore the selected profile's complete recorded original. Static/custom versus firmware preset is the exact supported mode, not a generic animation or brightness control.

## Persistent facts checked

- `crates/app/src/keyboard_ui.rs` uses Segoe UI via GDI `lfHeight = -18` and standard `BUTTON`, `EDIT`, `STATIC` and `COMBOBOX` controls. The common Windows color dialog is `ChooseColorW`. No additional font/runtime assets are introduced.
- The window background uses the system `COLOR_BTNFACE` role. Owner-drawn draft swatches use typed RGB field values and `COLOR_WINDOWTEXT` frames. These dynamic values establish no fixed global accent palette, shadow vocabulary, radius scale or type hierarchy.
- The four zone rows repeat at a 58-unit vertical interval with a 20-unit left inset in the current fixed native layout. The outer window requests 780×780; the reviewed own-client render is 764×741. These are observed implementation dimensions, not a tested DPI/responsive system.
- `keyboard_ui.rs` supplies metadata save, explicit GET/profile check, apply and scoped restore. `app/elevation.rs` owns fixed current-binary UAC actions and nonce/result verification; `app/profile_commands.rs` checks selected profile ID again during elevated restore.
- `core/keyboard.rs`, `platform/windows/hp.rs` and `controller.rs` are the state/transport/transaction truth. Write support is exact to 8E35/F.13 and capability `[1,0,0,0]`. RGB replaces only bytes 25–36 in the complete color image; the supported mode raw states are 0x64/0xE4. Both lighting features remain unsafe for automatic startup.
- The native image `.impeccable/review/keyboard-editor.png` was opened and checked for the intended layout, labels, buttons and draft swatches. Firmware was intentionally not read or applied for this capture. Runtime code and measured hardware records govern behavior, not the image.

## Evidence boundary and existing context gap

The own-client renderer does not fully paint text in hidden `EDIT`/`COMBOBOX` controls. Their contents, titlebar, other DPI settings and comprehensive accessibility certification are outside this visual evidence. No HTML/CSS detector ran for the native surface. Hardware readback/restore qualification does not establish reboot/resume/hotkey behavior, physical animation or service-independent OEM replacement.

The root still has no global `DESIGN.md` or design sidecar. `PRODUCT.md` leaves dashboard identity open, and the curve brief scopes its dark reference to that workbench. This ordinary extension documents observed incumbent rules and preserves those boundaries; it does not repair the pre-existing global design-context gap.
