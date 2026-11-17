# Preset editing, cooling links and lighting

Native Windows / Slint 1.18.1, Operate surface, 17 November 2026.
Extends the [window and Apply contract](WINDOW_AND_APPLY_REFINEMENT.md).
The user supplied screenshots showing left-aligned bounded content, an overflowing
HP action, an undersized keyboard and unnecessary explanatory text. They requested
GPU range/equalize controls, cancel-to-saved preset editing and linked fan presets.

## Direction

Preserve the existing Native Tuning Workbench, Slate/Graphite/Vesper/Light palettes,
40 px caption and 160 px sidebar. Center the bounded work area beside the sidebar
at up to 1280 logical px. Keep GPU watts/temperature and CPU telemetry first.
Fields and selectors share 276 × 36 px geometry; 44 px field rows and group
padding keep their edges clear of section headers. Long actions get enough width
and an inset, wrapped label. Compact action bars wrap into two rows when needed.

Cancel editing restores the currently loaded preset baseline, including its name,
per-core values, curve and fan snapshot. Reset remains a different action returning
to defaults. Cancel neither writes storage/hardware nor changes startup grants.

The GPU plot keeps actual driver anchors. A visible range-selection mode supports
plain dragging, inclusive numeric mV bounds, all/to-end/clear and selected-count
feedback. Equalize uses the numeric MHz target for selected anchors only, in one
undoable operation. An empty range never falls through to tail flattening. Driver
ranges and monotonicity remain checked; invalid equalization is atomic.

Cooling adds a named fan-preset library. Editing/saving metadata and requesting a
saved curve are separate actions. Performance presets copy a shared CPU/GPU curve
snapshot; library edits/deletion cannot change that saved profile silently.

Lighting uses the full work-area width for its original schematic keyboard.
Four independent controls map WASD/left/middle/right to wire indices 3/2/1/0.
The right group includes the numpad and adjacent keys; no per-key claim. Remove
the explanatory body paragraphs, keep compact Preview feedback and accessible
readback detail. Natural content height prevents large gaps and stretched controls.

No new shipping raster assets or new visual world are introduced.

## Cooling request and recovery boundary

- Only the existing qualified 8E35/F.13 shared-helper transport is used. Inspect or
  resume the installed receipt-bound helper; a linked preset must not replace it,
  including when CCD selection is also present.
- Library files are separate from performance-profile lists. Save/delete/request
  compare the expected revision under the short fan-preference lock.
- Persist exact original preference bytes/absence before any helper resumption,
  HP prelude or direct write. Commit active curve then mode only after the entire
  direct subset is verified without skipped controls. A partial direct result
  fails the combined request and attempts direct recovery and preference restore.
- The request journal has Prepared/Requested/Recovering/Restored states. Ownership
  and current images are checked; outside changes retain a visible pending duty.
  Bind a pending request to the actual direct checkpoint transition using content
  and file identity, without changing the legacy core-journal format.
  On Windows this uses the documented [FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/desktop/api/winbase/ns-winbase-file_id_info)
  from the same open handle as the bytes and creation time.
- An early failed replacement retains the previous owner's original images through
  a hash-checked archive and atomic final metadata reattachment. Missing/corrupt
  archives remain pending; unrelated global fan choices are never reclaimed.
- The installed helper has no exact curve-digest acknowledgement. UI/CLI success
  means direct values verified plus cooling **requested**, not physical policy
  confirmation, complete firmware rollback or BIOS ownership. The HP mode/TPP
  prelude retains its documented incomplete readback/restore boundary.
- Linked curves cannot gain sign-in authorization. Reject it before profile or
  permission writes. Unlinked profiles, existing grants and background startup
  permissions keep their previous contracts. Fan-only requests use Cooling.

## Acceptance

Exercise DE/EN, software/FemtoVG, minimum/wide clients, true scrolling, pointer
range selection, equalize/Undo, empty-range guard and cancel-to-saved values.
Inspect the shown client after minimize/tray restore before screenshot cache reset.
Check four independent lighting drafts, reachable actions and settings restart.
Use isolated stores for library conflicts, interrupted paired-file restore, exact
absence, checkpoint identity, stale revisions and previous-owner recovery.

Run format, workspace tests, warnings-denied Clippy and a normal locked release.
Final source review, rendered UI review and canonical design records are required.
Ordinary QA changes no live tuning, service state, startup grant or helper image.
Physical fan uptake, sustained 85 °C control, BIOS handover and HP-independent
lifecycle remain separate device acceptance. Record exact artifact identity and
the controlled dist/Desktop handover in Validation after deployment.
