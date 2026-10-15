# GPU profiles and maximum cooling

Mode: Operate. Native Windows, code-first. Extend the approved dark system in DESIGN.md; no new visual world.

## Direction contract

THESIS: Save GPU tuning and an independent maximum-cooling override as explicit profile intent, then apply the entire saved profile through Windows approval.

OWN-WORLD: Existing Segoe UI, dark fields, restrained mint actions, native combo/button semantics. Inherit current spacing and geometry; secondary firmware facts use the existing 14px role.

STORY: Select a saved profile, edit GPU intent and Max-Fan intent, save metadata, then explicitly apply or restore. Max-Fan is not a temperature curve or a BIOS automatic-mode selector. Unsupported settings block the whole apply.

FIRST VIEWPORT: Keep the existing GPU name/current readings and profile/name fields. Add a labelled Max-Fan profile choice in the header: Nicht ändern / Ein / Aus. The choice shows profile intent, not current firmware state. Existing Apply/Restore actions stay in place.

FORM: Ordinary extension of native controls. No new seed or concept tournament; inherit dashboard direction7aa2845d. Changes mark drafts dirty; a saved Max-Fan setting is not eligible for automatic logon until its lifecycle is qualified.

QUALITY BAR: Labels distinguish draft from hardware. Save preserves unrelated settings and never writes firmware. Reload retains the exact boolean or its absence. Selecting Nicht ändern removes only the profile's Max-Fan instruction; it does not clear the current firmware override. Native keyboard/focus and capture evidence remain valid. No additional timer or runtime assets.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review and documentation.

## Implemented extension

`crates/app/src/gpu_ui.rs` places the three-way native selector in the existing header without changing the window geometry, theme or action positions. **Max-Fan im Profil** uses the shared 14-pixel supporting role; the choice retains the editor's Segoe UI and dark native COMBOBOX treatment. The selected value is saved profile intent, not a live firmware reading.

Changing the choice marks the draft dirty and disables Apply until a successful metadata save. Save/reload preserves absent, true and false distinctly and retains unrelated settings, including every point of an unedited 127-point custom Graphics curve. **Nicht ändern** removes only the profile instruction. **Aus** releases the Max-Fan override when explicitly applied and does not select a BIOS automatic mode. Whole-profile checks, Windows-UAC Apply/Restore and the startup-safety block remain in force. A saved unsupported fan curve still blocks the entire apply.

## Evidence and finish

- Real native own-client capture: `.impeccable/review/max-fan-profile.png`, 754×771 pixels, opened and visually checked. No approved mock, raster asset or browser runtime was introduced.
- Fresh finish-review disposition: **ship**, with no material fixes. The review confirms the pictured header choice, hierarchy, dark fields, actions and explicit intent wording within the inherited system. It excludes an open dropdown, titlebar, other DPI and comprehensive accessibility.
- GPU UI smoke, 15 October 2026 at 09:36:25 CEST: true/false/absent roundtrip, draft Apply gating, startup blocking and complete custom-curve preservation passed. This smoke saves metadata and performs no hardware writes.
- Separate hardware evidence: fixed fan GETs, the 09:17 Max-Fan trial and 09:30 persisted recovery are recorded in `docs/FAN_CONTROL.md`. Qualification is exact to 8E35/F.13, manual and AC-only, with table/topology/opaque-state capture before any SET. It does not prove settled maximum rated RPM, standby/reboot, software-curve safety or replacement of the still-running HP controllers.
- Native scope: no HTML/CSS detector ran; it has no verdict on this Win32 surface. `DESIGN.md` and `.impeccable/design.json` were compared with `theme.rs`, the GPU implementation and the capture, then preserved unchanged as an ordinary extension. No new design token or durable system rule was needed.

The context loader's existing Native Windows platform-enum warning and lack of source-target-to-brief resolution are recorded as tooling drift. They were not repaired during this extension.
