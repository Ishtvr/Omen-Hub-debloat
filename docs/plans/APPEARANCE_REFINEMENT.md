# Appearance and curve correction — 16 November 2026

Scope: native Slint Operate UI. The user's screenshots are incumbent evidence,
especially the horizontal curve fragments and unequal selected-tab outline.
Hardware transports, profile precedence and recovery permissions stay authoritative.

## Direction contract

THESIS: one workbench for measured GPU/CPU status and the selected preset's editable
settings. Selection, saved intent and confirmed hardware state remain distinct.

OWN-WORLD: Discord-inspired Slate neutrals with a quiet indigo accent, common
36 logical-pixel controls, six-pixel corners and one aligned value column. Graphite,
Vesper, Light and custom background/text/accent support the user's appearance choice.
Text input remains native Slint text input, including caret, selection, IME and Undo.

STORY: read watts and temperature, select a preset, inspect and edit its included
settings, then explicitly save/apply. Cooling and GPU curves are connected geometric
paths in the same coordinates used for point interaction. Appearance previews only
valid colors and persists through the existing atomic settings transaction.

FIRST VIEWPORT: GPU and CPU side by side at the top of the overview's scroll area,
then preset selector/name and tuning groups. Secondary device/mode details expand
on demand. Save/Apply and sign-in approval remain visible below the scroll area.
Overview, Cooling and Lighting share equal navigation targets; Settings is separate.

FORM: seed: none (pinned). User: "alle Felder gleich groß und das Design einheitlich",
"Farbpalette … wie … Discord", customizable as in the supplied Antigravity screenshot.
Code-led refinement of the existing native utility; no image comp or web runtime.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Reference observations

- [WinUI Gallery](https://github.com/microsoft/WinUI-Gallery): familiar Windows
  command/navigation patterns, common control geometry and separated content groups.
- [Windows settings guidance](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings):
  keep global preferences in one place, with grouped rows and immediate feedback.
- [G-Helper](https://github.com/seerge/g-helper): compact hardware tuning and numeric
  editing alongside fan curves. This is visual reference only, not an HP device contract.
- [Slint 1.18.1 transforms](https://releases.slint.dev/1.18.1/docs/slint/reference/common/#transforms):
  transforms are unavailable in the software renderer. Replace rotated one-pixel
  rectangles with actual Path geometry, retaining the low-background-cost renderer.

## Acceptance

Both languages; compact and enlarged window; rapid scrolling, resize, minimize/tray
restore; stable drafts and Undo; popup/focus geometry; actual diagonal curve pixels
in software and GPU renderers; valid/partial/unreadable custom colors; persistent
preferences with unchanged startup approval; required Rust checks and release build.
Normal verification performs no new voltage/power/fan writes or vendor-service changes.

## Keyboard addition and compatibility

The user's later request adds an original schematic QWERTZ/QWERTY keyboard with
WASD, Rest and NUM pad/right selection, color and RGB-value brightness. The
manufacturer UI's right/middle/left/WASD wire order replaces the misleading old
labels. Right is a coupled hardware area, not a numpad-only/per-key promise.
Edits preview until explicit Apply; reset, mixed values and exit protection remain.
Appearance uses a separate ui-settings.json so older protected helpers retain a
readable app-settings.json. All ordinary save failures retain the rollback path.
