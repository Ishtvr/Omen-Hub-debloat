# Omen Hub++ — Precision Hardware Utility

Slint migration specification, 15 November 2026. This describes the new UI being implemented on `codex/slint-ui-rework`; it is not a claim that the deployed beta.3 already uses Slint. Confirmed functions and device contracts remain authoritative in PRODUCT.md and docs/HARDWARE_MATRIX.md.

## Product and operating context

A local Windows hardware utility used alongside demanding games/development workloads, often in a dim room. The dark graphite surface reduces glare. Its first job is to show GPU watts/temperature with CPU performance beside it, then offer deliberate tuning and cooling controls. Mode: Operate. All existing tuning, recovery, startup, cooling and lighting workflows survive the visual replacement.

The approved brief replaces the Win32/GDI visual world. It does not request a browser runtime, a debloat service manager or new snapshot operations. Do not copy the brief's generic three-tab feature examples. Keep the backend flat and reuse existing core/platform policies and sleeping workers. System calls never execute in Slint callbacks on the event thread.

## Composition and geometry

Use one resizable native Windows window with the system caption, window menu, minimize/maximize and resize behavior. Default client size 760 × 540 logical px, minimum 720 × 500. Validate rather than assume readability at 100/125/150/200% scale. The window can grow; at compact sizes each work area scrolls within its own bounds. No controls move outside their viewport or over the command/status bars.

- Top command bar: 44 px, 16 px horizontal inset, original geometric app mark, Omen Hub++ label, quiet actual privilege status. Flat segmented navigation: Overview, Presets, Cooling, Lighting. A separate Settings action stays visible. No wide sidebar or hero.
- Main work area: 20 px horizontal inset, 16 px vertical inset, 8 px row rhythm and 20 px group separation. Headers 32 px; controls at least 32 px, primary actions 34 px. Label/value columns share edges.
- Overview: two open instrument regions separated by one hairline, GPU first and CPU beside it; concise measured values and provenance, no symmetric KPI-card row. Power source and device identity are secondary. Mode/preset selection and Apply are distinct. Fan/graphics requests show saved preference separately from confirmed state.
- Presets: pinned selection/New/name row; independent scrolling groups GPU, CPU limits/PBO/CO/per-core and Windows processor/energy/display. Name input is plain left-aligned and vertically centered. Save, Apply/Recovery, Delete and sign-in permission remain explicit. Long choice text wraps in the open chooser/detail area rather than being silently cut off. Help opens on demand, never a repetitive explanation under every row.
- Curves: GPU curve belongs to the preset draft. Cooling owns the shared fan curve and BIOS/Curve/Max choices. A chart takes the available work area; a compact numeric point editor, Undo/Redo, fit/zoom, selection and modifier editing remain accessible. Closing/applying the curve draft does not itself write GPU hardware.
- Lighting: four keyboard zones including numpad, On/Off, native color picker and real readback/error state. Color picker cancellation changes nothing. Opening the view only reads.
- Settings: language System/Deutsch/English, app autostart, start in tray, close behavior, battery selection and preset hold. Preserve the explicit existing HP-service action, separate from ordinary settings saves. App startup and a preset's sign-in approval are different contracts.
- Footer: 28 px high, one actionable status line. Busy, failed, saved and confirmed/partial Apply use text, not color alone. Clicking details expands a bounded diagnostic area; long failure details remain selectable/readable.

## Tokens

| Role | Value | Use |
| --- | --- | --- |
| surface-0 | #0A0B0E | window canvas |
| surface-1 | #111318 | command bar, fields |
| surface-2 | #171A21 | hover/selected segment |
| hairline | #1E222B | one-pixel separators |
| text-high | #ECEEF2 | primary text/values |
| text-mid | #8A93A2 | secondary labels |
| text-mono | #5E6673 | nonessential metadata only; essential text uses readable higher contrast |
| signal-crimson | #E11D48 | single primary action/focus, at most two concurrent accents |
| signal-clean | #10B981 | small confirmed-status point only |
| warning | #E5B76B | explicit degraded/pending state |
| error | #F3A1AF | readable error text |

Segoe UI Variable/Segoe UI for controls; Consolas for aligned measurements and technical identifiers only. Type scale 11 px nonessential metadata, 12 px secondary, 13 px body/control, 15 px group heading, 24 px measurement. Preserve native caret, text selection, keyboard navigation and visible focus. Never put essential explanations at the low-contrast mono token.

Use 4 px field/button radius and 6 px segment radius. No nested bordered panels, glass, gradients, decorative shadows, KPI tiles, colored pill stacks, emoji icons or continuous decoration. Draw the original app mark and action icons as geometry; do not ship vendor logos/fonts or downloaded artwork.

Motion is limited to hover/segment changes at 120–160 ms, ease-out cubic-bezier(0.16,1,0.3,1). No entrance delays, bouncing, animated telemetry numbers or redraw loop at rest. The signature interaction is the precise curve/draft selection guide, driven by actual editing state.

## State, threading and lifecycle

Rust owns state and editing history. Slint owns layout, clipping, rendering and input. Keep stable models and update changed rows without recreating the whole form on each telemetry tick. Virtualize long lists. The bounded preset form stays mounted inside a clipped ScrollView, including collapsed groups, so input selection and native text Undo survive scrolling and group toggles. Group visibility, scroll position and viewport geometry never mutate profile contents or Undo history.

Reuse existing bounded telemetry children, CPU reader, controller lease, Apply/preflight/readback/recovery, cooling helper and persistent stores. Manual controls do not queue behind telemetry. Hold/battery operations retain their existing independent completion paths. Controls remain busy until their own operation finishes; a telemetry or automatic result cannot unlock them.

A hidden Win32 event/tray owner can retain power notifications, single-instance activation and Explorer recreation while Slint supplies the visible windows. Hiding/minimizing pauses sensor polling and rendering. Closing during a write waits for that write's completion. Exiting must preserve cooling ownership and journals; never kill active controllers as part of UI tests.

Settings save atomically through the existing settings contract and detects concurrent edits. Language stays process-consistent; a saved language change clearly requires restart. Do not discard drafts or restart automatically. New subprocess/UAC actions carry the current language. Do not silently change startup permissions.

A dedicated developer test mode isolates UI interaction from hardware writes, user profiles/settings, startup tasks and cooling helpers. It is visibly labeled. MCP exists only in explicit developer builds; normal release builds have no localhost server or network client.

## Acceptance and truthfulness

Rapid alternating scroll, repeated resize, minimize/restore, group toggles, dropdowns and keyboard/name input must be tested in the running app. Verify drafts, selection, native text Undo and curve history survive. Test both languages, compact dimensions and DPI; inspect screenshots and actual element geometry.

Show requested limit, successful limit readback and measured Tctl as different data. Missing telemetry stays unavailable; do not cap the temperature display or guess power/clock values. Stored requests and historical successful Apply are not current hardware proof. Intel controls stay removed and legacy cleanup remains explicit on Save. Max-Fan stays outside custom presets.

The latest brief removes an extreme fixed RAM limit; low tray-idle CPU/RAM including helpers remains a measured goal. Executable size is not a target. Measure startup and interaction latency and renderer behavior on actual views. Update validation with measured scope and limits, not blanket claims.

Run format, workspace tests, Clippy with warnings denied and a release build, plus targeted regressions. Do not replace dist, redirect shortcuts or publish a partially migrated UI. Record exact source/version/SHA-256 when a complete tested build is deployed.
