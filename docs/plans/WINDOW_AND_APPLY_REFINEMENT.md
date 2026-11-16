# Window lifecycle, notifications and manual Apply

Native Windows / Slint 1.18.1, Operate surface, 16 November 2026.
Extends the existing [color/scroll contract](COLOR_SELECTION_AND_SCROLL.md).
The user requested a frameless window, reorganized navigation, popup status,
restored HP modes, newer Apply intent taking precedence, and a fix for missing
paint after minimize/restore. Their clarification preserves the bottom action
buttons; only the status/error strip is replaced.

## Direction contract

**Thesis:** Keep one persistent work area with reliable window return and visible,
truthful results. Preserve the existing neutral Slate world and shared fields.

**Own world:** Forty-pixel caption, native move/resize behavior and clear caption
actions; a 160-pixel sidebar replaces the top tabs. Normal value fields remain
276 × 36, text actions 128 × 36. The minimum client becomes 820 × 500 so the
sidebar and complete curve tools fit without horizontal clipping.

**Story:** Overview shows GPU/CPU telemetry, directly accessible standard HP modes
and custom presets. Cooling, lighting and settings keep their own work areas.
Selected, saved and hardware-confirmed values remain distinct.

**First viewport:** Quiet custom caption above a left navigation column; telemetry
first, HP mode selection next, then the custom editor. Keep existing action bars.
Success messages expire; errors persist with selectable details and dismissal.
Switching away from a preset draft offers Save, Discard and Cancel.

**Form:** Code-led native refinement; seed: none (pinned). User instructions pin
"Nur die Status-/Fehlerzeile durch Popups ersetzen" and removal of the native
frame. No image comp or separate Discord quality card was supplied.

**Finish:** unreviewed and undocumented is unfinished; this build ends with the
finish review, the verdict, DESIGN.md, and every shipping raster carrying its
provenance. No new shipping rasters are introduced.

## Acceptance and safety boundaries

- Verify real shown client pixels after minimize/restore and tray return, before
  requesting any MCP screenshot. Slint's snapshot path resets software partial
  render caches and can mask this bug. Compare only owned, unobscured client
  samples; do not treat a model snapshot as proof of presentation.
- One deferred cache reset on software-window return; no periodic full redraw,
  window/model reconstruction or permanent repaint timer. Preserve drafts,
  selection, caret and Undo through window lifecycle changes.
- Latest manual intent replaces waiting targets; only one executor runs. An
  in-flight hardware operation reaches its safe completion boundary, never a
  process kill or mid-write cancellation. Ignore obsolete UI completion results.
- Manual replacement restores and verifies an existing pending checkpoint before
  a new target/prelude. Failure preserves the journal and prevents the new write.
  Background/sign-in commands do not silently gain recovery permission.
- Manual recovery for replacement preserves startup selection/approval files;
  explicit user Restore retains its existing separate startup-disable contract.
- Test DE/EN, compact/wide, software/FemtoVG, controls, dialogs, keyboard and
  persistence. Test worker supersession with a blocked fake executor. Ordinary
  verification does not write hardware, replace helpers or stop vendor services.
- Record checks, source/hash, foreground/tray resource samples and deployment.
  No sustained 85 °C limit or HP-independent lifecycle claim follows from UI QA.
