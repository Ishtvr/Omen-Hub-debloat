# Validation

Current software summary updated **20 November 2026**, version **0.10.0-beta.12**, Windows x64 / Rust 1.99.0. Source: beta.7 plus Apply-reliability, keep-preset and curve-editor work, identified by build-info.json. Earlier measurements below remain historical. No new hardware write/lifecycle qualification is implied.

Current support: [Hardware matrix](HARDWARE_MATRIX.md). Exact device evidence: [8E35](models/8e35.md). Pending work: [Roadmap](ROADMAP.md).

## Curve editor without bottom buttons, shortcut help — 20 November 2026

On top of the curve editor change below; software validation only, no hardware write.

- Format check, **353 unit tests** (170 core / 83 app / 100 platform), all-target
  Clippy with `-D warnings` and a locked release build passed; the release binary
  started in `--ui-test` and stayed up. New key regression: Ctrl + R decodes to Reset.
- `test-slint-ui.py` now drives the chart by value field and hotkeys instead of the
  removed buttons; the GPU-curve and new-profile modes passed. The full German run
  still stops at the preset-page PPT field (line 810), before its curve section.
- Screenshots checked the GPU and fan editors and the i help dialog.
- `.impeccable/` is no longer tracked (whole folder in `.gitignore`); local files stay.

## Curve editor layout, grid and hover — 20 November 2026

Software validation only, on top of beta.11; no hardware write.

- Format check, **353 unit tests** (170 core / 83 app / 100 platform), all-target
  Clippy with `-D warnings` and a locked release build passed.
- New regressions: grid ticks on round values using the point mapping, hover
  readouts, and GPU offsets against the driver default curve rather than the
  loaded preset.
- `test-slint-ui.py` (developer MCP build, software renderer) passed in German,
  English and the GPU-curve, new-profile and editor-extension modes on beta.8.
  On beta.11 the GPU-curve and new-profile modes pass; the full
  German/English runs and the editor-extension mode fail in preset-page steps
  exactly as without this change (regrouped fields), not in the curve editor.
  Screenshots checked the GPU and fan editors at 860×540, 820×500 and 1600×1000 logical px.

## Merged curve-page work and UI regression run — 20 November 2026

version **0.10.0-beta.12**. [Release notes](releases/0.10.0-beta.12.md). Format
check, **353 unit tests** (170 core / 83 app / 100 platform), 14 Guardian
scenarios, Clippy with `-D warnings` and a locked release build passed. The
developer-MCP build passed `scripts/test-slint-ui.py` in these modes: full DE
(software, 1.25) and full EN (FemtoVG, 2.0), editor extensions DE and EN,
new-profile curve DE and EN, GPU-curve enable and pending switch. The EN 2.0 run
exposed slider rows squeezing their labels in compact windows; fixed before the
release build. The release binary was started in `--ui-test` and stayed up.
## Startup fix — 20 November 2026

version **0.10.0-beta.12**. beta.10 panicked at start (group list count 7 versus 8 groups); reproduced with the beta.10 binary in `--ui-test` and fixed. The beta.11 release binary was started in `--ui-test` and stayed up. Unit tests and Clippy did not cover the window construction; this start check is now part of the release routine for UI catalog changes.

## Dynamic Boost group and Hub power sliders — 20 November 2026

version **0.10.0-beta.12**. [Release notes](releases/0.10.0-beta.10.md). UI
regrouping only; write paths unchanged. Slider bounds come from the decompiled
OMEN Hub panel for 8E35 ([evidence](models/8e35-hub-modes.md)). Format check,
workspace tests (350 unit tests, 14 Guardian scenarios), Clippy with `-D warnings`
and a locked release build passed; a new regression pins the group membership,
the Hub bounds and in-range seeds. The slider was not operated by mouse in this
change.
## Battery state machine and battery preset — 20 November 2026

version **0.10.0-beta.12**. [Release notes](releases/0.10.0-beta.9.md). Software
validation only; no hardware write and no battery run were made.

- Format check, **349 unit tests** (170 core / 79 app / 100 platform), **14 Guardian
  process scenarios**, all-target Clippy with `-D warnings` and a locked release
  build passed.
- New regressions: AC→DC and DC→AC switching, idempotent repeats, startup and
  wake sync on the current level, manual override until the charger returns,
  unknown source never switching, persisted state across a store reopen, the
  automatic queue submitter never replacing manual work, and the battery template
  (valid, no NVIDIA binding, Balanced mode, PL1 18 W, efficiency mode kept).
- Device facts read on 20 November 2026 (on AC): Windows battery WMI reports
  `PowerOnline`, charge/discharge rate and voltage, but no adapter wattage or type,
  so full charger and weak USB-C PD are not distinguishable; the logic stays AC/DC.
  No AMD PMF device and no DPTF service are present. Whether the 80 W PPT reset
  occurs on battery is unmeasured (it was only seen in Unleashed, which a battery
  preset never selects).
## Apply reliability, Unleashed PPT and curve editor — 20 November 2026

version **0.10.0-beta.12**. [Release notes](releases/0.10.0-beta.8.md). Software
validation only; no hardware write was made while preparing this change.

- Format check, **344 unit tests** (168 core / 77 app / 99 platform), **14 Guardian
  process scenarios**, all-target Clippy with `-D warnings` and a locked release
  build passed. One manual timing test remains ignored.
- Root cause of the reported "Fast-Limit (PPT) … Firmware hält 80 W" dialog: the
  Apply of "Mein preset" completed, then sign-in approval re-verified every
  requested value and failed on PPT, which the firmware had taken back to 80 W.
  Approval now accepts a value that reads back as its captured original.
- New regressions: late firmware revert skips only that value; Keep preset gives
  up after three rewrites until a new Apply and ignores an Apply from an earlier
  boot; event pacing (5 s for 60 s after a trigger, otherwise 30 s); curve point
  picking, group drag as one undo step, Shift band selection, Escape without
  history and the key table.
- Evidence for the Unleashed PPT reset and its scope: [8E35](models/8e35.md).
  CPU Tctl: the 11:43 Apply captured an 85 °C original from PM cell 10 (no write
  needed); no drift was logged afterwards; observed changes to 100 °C match cold
  boots (stock limit). No measured Tctl reached the limit, so enforcement under
  load remains unproven.
- Not performed: mouse run of the Slint curve editor (screen access declined;
  Python/Pillow for `test-slint-ui.py` absent) and the hardware acceptance runs
  (successful Apply with readback, real failure, two consecutive Applies, Restore).
  They need the installed build and UAC approvals on the device.
## Preset editing, cooling requests and lighting — 17 November 2026

Normal executable: **15,934,976 bytes**, SHA-256
**0AD1A1FA40D8242F8036D9C0B3D3ABF51D682B1BFE1E7310AAE233A9D1787463**.
Slint 1.18.1; software default; normal release excludes developer MCP.
[Release notes](releases/0.10.0-beta.7.md),
[direction and acceptance](plans/PRESET_AND_LIGHTING_REFINEMENT.md).

- Format check, **337 unit tests** (167 core / 71 app / 99 platform), **14 Guardian
  process/file scenarios**, all-target Clippy with `-D warnings` and locked normal
  release build passed. One manual timing test remains ignored. DE/EN catalogs
  contain **768 matching keys**.
- Core regressions verify selected-only equalization, unchanged outside anchors,
  one-step Undo and atomic rejection of empty/invalid ranges. Fan snapshots require
  a valid identical CPU/GPU pair, preserve unlinked device binding and cannot gain
  sign-in permission. A fan-only request uses the separate cooling route.
- Twelve isolated storage/request tests cover library revision conflicts and
  deletion archives, unchanged profile snapshots/permissions, exact original bytes
  and absence, interruption between curve/mode files, foreign preference/journal
  ownership, corrupt originals and durable Recovering barriers. They also cover
  Direct Applied before fan completion, byte-identical repeated checkpoints using
  file identity, and early aborted replacement retaining the prior owner's originals.
  Missing/corrupt history is retryable and never reported as successful reattachment.
- The existing hold regression now covers active → fan Prepared → paused → exact
  preference restore → active. Both initial and under-lease checks pause on a fan
  recovery duty. Linked+CCD uses the installed receipt-bound helper, not its updater.
- Both the extension and complete UI matrices passed DE/software/1.25,
  EN/software/1, DE/FemtoVG/1.5 and EN/FemtoVG/2 on the same developer executable.
  Each complete run covers 100 alternating wheel events, 15 resizes, four
  minimize/restore cycles, three tray returns, Unicode/Undo, dropdowns, curves,
  keyboard drafts and settings persistence across a process restart.
- Extension runs cover pointer/numeric range selection, selected-only equalization
  and Undo, empty-range rejection, cancel-to-saved name/CPU/per-core/curve/fan link,
  sign-in rejection before Save, centered top-telemetry overview, bounded HP/About
  actions at wide/minimum size and four independently editable large keyboard zones.
- The final source and rendered review found all six screenshot findings resolved,
  plus readable named-library Save/Request controls. This is UI/software evidence;
  selected frequency targets or zone tint do not establish physical behavior.
- Focused New-preset factory-curve and pending-recovery Save/switch regressions
  also passed in both languages. No live profile, recovery journal or tuning target
  was used by those fixtures.

Presented-pixel assertions precede any post-transition MCP snapshot. The owned
200% fixture is positioned on screen; its desktop pixels are copied in one BitBlt
before comparison, avoiding live per-pixel reads across compositor frames. Separate
caption double-click gestures wait for layout and the system click interval.
These harness corrections do not alter product repaint behavior or weaken expected
colors. Physical monitor-DPI transitions and all-pixel/frame-latency acceptance
remain outside this scope.

Named fan presets and performance links are **request-level software support**.
The existing 0.9.0-beta.3 helper has no exact curve-digest acknowledgement. Success
separates verified direct settings from cooling requested; restored preference
files do not prove physical fan uptake, firmware rollback or BIOS ownership.
Linked snapshots cannot gain sign-in authorization; rejection precedes profile/grant writes.
No live power/voltage/lighting SET, fan-policy request, service change, driver
installation or helper replacement was part of these checks. Sustained 85 °C
enforcement and HP-independent lifecycle remain separate device acceptance.
Earlier resource samples keep their original builds and scopes.

### Beta.7 handover and normal runtime sample

The controlled 17 November handover deployed source
**c81241f0400be7f5124911903d1d7c57a4fc60ee** to
`D:\Omen Gaming Hub Rewrite\dist\Omen Hub++.exe`. The Desktop link retains that
target, `--show` and the dist working directory. The normal GUI was observed visible
from this exact executable and returned visibly after a normal close-to-tray cycle.
The link's metadata and direct qualified target were verified; the `.lnk` itself
was not executed by automation.

All **47 live data files** were compared before/after; only the producing
`fan-service-status.json` changed. Profiles, preferences, sign-in selection and
recovery/control journals remained byte-identical. All five protected installation
files and all three own scheduled-task definitions were unchanged. The fan helper
retained PID 13628 and its 17 November 08:29:50 creation time. Protected main/fan
images remain **0.9.0-beta.3**; no helper was upgraded and no existing startup grant
was reapproved. HPOmenCap remained stopped/disabled; PawnIO remained running/Manual.

The clean-source **571-file** ZIP passed version/hash/module/license checks and was
published as [0.10.0-beta.7](https://github.com/Ishtvr/Omen-Hub-debloat/releases/tag/v0.10.0-beta.7).
ZIP size **9,906,706 bytes**, SHA-256
**1CF06670D189202C3F5BCE4FECC3F6713D53A34ED4C69424BBDA4E33D59ECA04**.
An unauthenticated public API read confirmed its source, asset size/digest and
published prerelease state. The [GitHub Windows checks](https://github.com/Ishtvr/Omen-Hub-debloat/actions/runs/44080120074)
also passed format, workspace tests, warnings-denied Clippy and release build.
The tagged artifact/build-info remains immutable; this paragraph is a later
documentation-only deployment record.

A warm normal-build sample included the main process, CPU sensor child and the
unchanged fan helper. Open aggregate working set was **90.28 MiB**, private bytes
**47.00 MiB**. After ten seconds in the tray, a **20.06-second** interval held at
**90.25 MiB** summed working sets and **46.95 MiB** private bytes. Combined CPU time
was **0.857% of one logical processor**; the CPU-reader child's CPU time did not
increase during that interval. The GUI returned visibly with the same main PID.
Working-set sums count shared mappings per process; private bytes are a commit
measure. This short sample does not establish sustained memory growth, physical
power use, transition safety or a new RAM ceiling. The low-background-resource
goal supersedes the historical fixed 50-MB budget.

## New preset factory curve — 16 November 2026

Normal executable: **15,533,056 bytes**, SHA-256 **8C20894C84706CFA168097923EFDD515F88FD1D8C32DA2FB18CA1E691C1500AC**. [Release notes](releases/0.10.0-beta.6.md).

Explicit New (+ or the selector entry) loads a fresh driver snapshot, discards
the accepted old editing session and opens the factory GPU curve without any
inherited target, offset or history. Cancel preserves the old session. Saving
before the switch and discarding before the switch both follow the same load
completion path. Startup/ordinary refresh does not open a chart automatically.
The graph receives focus after the selector/prompt finishes its own callback.
These are preview operations; hardware changes remain behind explicit Apply.

- Format check, **320 unit tests** (162 core / 71 app / 87 platform), **14 Guardian
  process/file scenarios**, all-target Clippy with `-D warnings` and locked normal
  release build passed. One manual timing test remains ignored.
- Focused UI tests passed DE/software/1.25, EN/software/1, DE/FemtoVG/1.5 and
  EN/FemtoVG/2. The fake driver carries an applied +60 MHz offset, the saved curve
  another target and a cached edit another value. Every explicit New starts at
  1900 MHz at the 950 mV fixture anchor, without inheriting any of them.
- The same sequences cover New button, New selector, repeated New, canceled
  switching with retained edits, approved discard, Save then New, retained saved
  targets and immediate arrow/Undo keyboard input. Fresh history is checked
  before the keyboard probe.
- Complete DE/software interaction and the focused pending-recovery Save-switch
  regressions in both languages passed with the new flow.

No driver reset/SET, live profile edit, startup reapproval or additional device
qualification was performed in these fixtures. Resource samples below retain
their original versions and scopes.

The controlled beta.6 handover deployed the exact executable above from source
**da2491d26e0b223c94292d9f3e71678b5cb84fab** to the existing dist start path. The
Desktop link retains that target and `--show`; the new GUI was observed visible.
All 47 live data files were compared before/after: only the producing fan status
file changed. Profiles, preferences, startup approval and recovery/control journals
were preserved. The fan helper retained PID 10400 and its 15 November creation
time; no protected helper was replaced or controller stopped. The package passed
the 571-file clean-source/version/hash/module/license check. This paragraph is a
post-deployment record; the immutable release tag/build-info identify the artifact.

## GPU curve opening — 16 November 2026

Normal executable: **15,529,472 bytes**, SHA-256 **79A1BB697A6401F9F543772358F8FF389A96BA3D60EA75802F72B0A618F11AD5**. [Release notes](releases/0.10.0-beta.5.md).

The beta.4 UI failure was reproduced with the actual sequence: check GPU V/F
curve before its first edit, then click Edit curve. The initial empty UI value
was passed as a complete target and failed with an anchor/order error. The
presentation adapter now treats that empty placeholder as a new factory-based
editor. Nonempty targets retain grid/range validation; the Apply contract still
rejects an empty curve. No profile or hardware value is silently replaced.

- Format check, **320 unit tests** (162 core / 71 app / 87 platform), **14 Guardian
  process/file scenarios**, Clippy all targets with `-D warnings` and locked normal
  release build passed. One manual timing test remains ignored.
- Focused real UI interaction passed DE/software/1.25, EN/software/1,
  DE/FemtoVG/1.5 and EN/FemtoVG/2: enable before opening, edit, Back/reopen,
  Undo/Redo, Use curve and reopen with the same draft. The preset name remains.
- The complete DE/software sequence also passed resize/minimize/tray, text and
  curve Undo, dropdowns, lighting drafts and settings process restart.
- Unit coverage starts from an applied plateau and verifies factory initialization,
  exact preservation of a saved curve, rejection of a partial nonempty curve and
  continued rejection of an empty Apply target.

These are isolated fixtures and presentation checks. No live tuning SET, vendor
service change, driver installation, startup reapproval or new hardware
qualification is part of this correction. Earlier beta.4 resource measurements
below are not new measurements of beta.5.

## Window lifecycle and manual Apply — 16 November 2026

Normal executable: **15,529,984 bytes**, SHA-256 **08B51F4584128E964E84710F27EEA25FCA56F71646494B278A1CE244864AFDEC**. Slint 1.18.1; software default; normal release omits developer MCP. [Release notes](releases/0.10.0-beta.4.md), [direction and acceptance](plans/WINDOW_AND_APPLY_REFINEMENT.md).

- Format check, **319 unit tests** (162 core / 70 app / 87 platform), **14 Guardian process/file scenarios**, all-target Clippy with warnings denied and locked normal release build passed. One manual timing test remains ignored. DE/EN catalogs have **705 matching keys**.
- Blocked-executor regressions run the actual serialized worker loop: old success and old failure both give way to the latest intent; intermediate targets never execute and obsolete results are not recorded as current. Queued Apply cannot start during an ordinary save/recovery operation.
- Mode-result regression keeps qualified Slow/PL1 readback separate from unqualified PM observations, preserving optional CPU-reader errors. The new notification describes a requested HP mode and its incomplete thermal-mode readback rather than claiming full confirmation.
- Focused DE/EN UI regressions simulate a pending recovery warning. Successful Save retains and follows the destination; invalid-name Save retains the draft/selection and clears that switch intent before retry. They do not open or modify a live recovery journal.
- The final complete matrix passed DE/software/1.25, EN/software/1, DE/FemtoVG/1.5 and EN/FemtoVG/2. Each exercised 100 alternating wheel events, 15 resizes, four minimize restores, three tray restores, exact units, native text/curve Undo, dropdowns, lighting drafts and settings process restart. Permanent client samples per transition ranged from 116 to 541 across sizes. The independent verdict scored all five listed fixes resolved at this UI scope; canonical design documentation also passed its source/token/JSON checks.
- Actual client-pixel comparisons occur after caption/native minimize and tray return, **before** further MCP screenshots. Slint 1.18.1 snapshots reset partial-render caches themselves and could mask discarded Windows buffers. The harness compares unobscured owned client canvas/surface samples, excluding legitimate control hover changes; this is sampled presentation evidence, not an all-pixel or frame-latency benchmark.
- Keyboard tests reach the read-only message by Tab, select it with Ctrl+A (before/after pixel assertion), cycle with Tab/Shift+Tab, close and reopen from returned Details focus, and Escape. System clipboard copying was not exercised because its owner-defined formats could not be safely preserved; the standard TextEdit copy implementation remains unchanged.

The new minimum is **820 × 500** logical px with a 40 px custom caption and 160 px sidebar. Native resize/maximize/minimize and the existing close/tray policy remain; bottom action bars are preserved. Physical multi-monitor DPI transitions and pointer-driven native OS dragging are not established by programmatic resize/double-click tests.

The software restore refresh is deferred once on return and drops its temporary snapshot immediately. It neither reconstructs the window/models nor adds an idle repaint loop. Manual latest-intent replacement waits for a running hardware call's safe completion; recovery failure preserves the journal and prevents the replacement target. Background/sign-in commands do not gain recovery authorization, and replacement recovery does not disable startup approval.

No live power/voltage/lighting SET, vendor-service change, kernel installation or new device/lifecycle qualification was performed for this refinement. Standard HP modes retain their partial recovery/readback limitations; sustained 85 °C enforcement and HP-independent cooling ownership remain separate device acceptance.

### Beta.4 normal-release resource samples

A warm owned UI fixture responded in **81 ms**. Open and tray working set were
**33,665,024 bytes**, private **12,533,760 bytes**; tray CPU did not advance over
**3031 ms**. No MCP listener appeared at the explicit test port. This excludes
activated sensors/helpers and is not a cold/UAC startup or total live-idle result.

The optimized owned wheel experiment repeated 300 native wheel events in five
seconds at each physical client size. CPU was **2890.625 ms** at 1280 × 800 and
**3156.25 ms** at 1920 × 1080; event-thread P95 **1.875 / 4.709 ms**. Steady render
counters were **144 FPS**. The UI/layout and system load differ from the earlier
samples below; foreground CPU is higher, so this does **not** establish a CPU
efficiency improvement. WM_NULL response and render counters are not direct
presentation latency. The idle gate still creates no periodic repaint loop.

### Controlled portable handover

After the user saved the open draft, the old GUI and its own reader exited
through the application's normal shutdown. The tested artifact from source
**1ccbae3f2f200f0b571fc47d26b2c418a703aeb0** was deployed to
`D:\Omen Gaming Hub Rewrite\dist\Omen Hub++.exe`; the Desktop link retains that
target, `--show` and its dist working directory. This deployment record is a
documentation follow-up; the release tag, package and build-info identify that
immutable artifact, with the executable hash above.

The normal deployed app additionally passed **three** native minimize/restore
cycles with **24** owned static client-background samples per cycle, without MCP
or a snapshot. Its process priority read back **High**. The integration probe
selects the actual visible client, avoiding Winit's internal event-target window.

With the activated own CPU reader and protected fan helper, a short tray sample
summed **93,237,248 bytes working set** and **47,132,672 private bytes**, with
**62.5 ms CPU over 3014 ms** across all three processes. Working-set sums can count
shared pages more than once; this is not long-run/sleep/reboot acceptance. The
window was brought back afterwards.

The fan helper retained PID 10400 and its 15 November start time throughout the
handover. Protected main/fan copies remain **0.9.0-beta.3**; they and the profile
sign-in task were not replaced or reapproved. The app tray task already points
to dist. HPOmenCap stayed stopped/disabled; PawnIO stayed running/Manual.

All **47** live data files were compared against the snapshot made after the
user's Save and before replacement. Only the producing `fan-service-status.json`
changed; profiles, preferences, startup approval and control/recovery journals
were preserved. The clean-source portable package passed its 571-file version,
hash/module/license check. Inno Setup was unavailable, so distribution is a ZIP
with installation scripts rather than a Setup executable.

## Color selection and scroll pacing — 16 November 2026

Normal executable: **15,376,896 bytes**, SHA-256 **90EB9AA24C06C97BB8F2279DAE45228F17E9F3B6BD7B33659338214071C19712**. Slint 1.18.1, software default; developer MCP absent. [Release notes](releases/0.10.0-beta.3.md), [extension/quality bar](plans/COLOR_SELECTION_AND_SCROLL.md).

- Format check, **314 unit tests** (160 core / 67 app / 87 platform), **14 Guardian process/file scenarios**, Clippy all targets with warnings denied and normal locked release build passed. One manual timing test remains ignored.
- Full actual UI interaction passed DE/software/1.25, EN/software/1, DE/FemtoVG/1.5 and EN/FemtoVG/2. Each exercised 100 alternating wheels, repeated resize/minimize/tray restore, native text/curve Undo, exact units, dropdowns, lighting drafts and settings restart.
- Shared HSV plane, hue, swatches and HEX were driven with pointer/keyboard. Cancel/focus return, incomplete HEX, draft acceptance and persistence passed. At 720 × 500, Tab reveals the focused HEX editor above persistent actions. Curves and their primary actions stay within the compact horizontal viewport after toolbar reflow.
- The independent review's compact-curve, picker-focus and fractional-timer findings were resolved by source inspection and all four recapture sets. Field widths are 276 logical px, ordinary text actions 128 px, compact actions 36 px; paired contextual numeric editors remain matched. Catalogs have **689 matching keys**.

### Optimized native wheel experiment

The owned --ui-test process received 300 native wheel events over five seconds at each physical size, scale1.25. No sensor/controller was activated. Slint console metrics count frames rendered; WM_NULL roundtrips measure event-thread response, **not presentation latency**. These short samples do not establish perceived smoothness for every workload.

| Physical client | Beta.2 CPU / event P95 | Corrected CPU / event P95 |
| --- | --- | --- |
| 1280 × 800 | 1875 ms / 2.054 ms | 2109.375 ms / 1.290 ms |
| 1920 × 1080 | 2500 ms / 2.962 ms | 2703.125 ms / 1.243 ms |

Beta.2 software rendering counted about 160–162 frames/s on the 144 Hz monitor. Corrected steady samples count **144 frames/s**. Source inspection found whole-ms period truncation in pinned Slint's Windows throttle; the local gate preserves fractional phase. A rejected trial used fractional Slint timer wakes, which its internal millisecond Instant truncates; final wakes round upward and cannot become zero-ms. Foreground CPU remains above the older UI baseline, so **no lower-CPU claim** is made. GPU pacing is unchanged. The gate schedules only pending visible work, stops on hide and creates no idle repaint loop.

One warm normal UI fixture responded in **59 ms**; tray working set **33,316,864 bytes**, private **12,066,816**, CPU **0 ms** over **3026 ms**. No explicit test-port MCP listener appeared. This fixture excludes activated helpers and is not a cold/elevated startup or total hardware-idle acceptance.

The shared chooser changes only local drafts. Keyboard Apply, exact device bindings, opaque RGB/mode originals, recovery and startup approval remain unchanged. No firmware master-brightness byte, physical LED-boundary measurement, sustained 85 °C enforcement or HP-independent lifecycle qualification was added.

## Native UI correction — 16 November 2026

Release executable: **15,572,992 bytes**, SHA-256 **902C8635501957096AA341BEE05815E23692B28AA6F4004A6536F5F4E8EB5A6A**. Normal software rendering; Slint 1.18.1; no developer MCP. [Release notes](releases/0.10.0-beta.2.md), [design](../DESIGN.md), [refinement record](plans/APPEARANCE_REFINEMENT.md).

| Check | Result |
| --- | --- |
| Format check | Passed |
| Workspace unit tests | 309 passed: 158 core / 64 app / 87 platform; one manual timing benchmark ignored |
| Guardian process/file fixtures | 14 passed, without real services/drivers |
| Clippy workspace/all targets, -D warnings | Passed |
| Locked normal release build | Passed; embedded pinned CPU modules retained |
| UI interaction | Passed DE/EN, software/FemtoVG, scales 1.25/1.0/1.5/2.0 |
| Actual compact geometry | 720 × 500 logical client; harness checks both physical dimensions with per-monitor-aware calls |
| Curves | Actual diagonal midpoint pixels, edit/Undo/gestures, fixed anchors and loading completion |
| Dropdowns | 24 long wrapped DE/EN names: late selection, repeated arrows, manual wheel, commit and reopening |
| Lighting drafts | Vendor zone order, mixed Rest values, RGB dimming, reset, numeric/slider synchronization, explicit memory-only Apply |
| Persistence/compatibility | Real isolated process restart; ui-settings.json separate from legacy app-settings; no startup grant |
| Pressed/focus states | Owned common-component held-pointer renders in Slate/Light; reviewed contrast pairs |
| Independent design review | Full captured-surface review; material dropdown and button-state findings resolved in scoped verdict passes; documentation gate passed |

Each full sequence includes 100 alternating wheel events, 15 resizes plus lighting-size captures, three minimize/restore and three tray/restore cycles, Unicode input/caret/text Undo, group collapse with numeric Undo, exact 55,125 mW roundtrip, preset switching, complete boost choices, curve history/reopen and four modifier gestures. The final common-button state correction was additionally exercised by complete DE/software and EN/FemtoVG sequences and held-pointer component renders. All fixtures are visibly marked and isolate profiles from live data. Persistence opts into a marked OS-temp directory; no hardware tuning, service changes or startup task writes are performed by the tests. Both catalogs contain **682 matching keys**.

One warm normal-release UI fixture responded after **252 ms**. Open working set was **43,851,776 bytes**, private bytes **11,866,112**. In tray: **44,265,472 bytes** working set, **11,866,112** private; **0 ms CPU** over **3034 ms**. An explicit MCP port produced no listener. This measures the UI fixture, without sensors/controllers; it is not total tray-idle hardware acceptance or a cold/elevated startup benchmark.

The normal deployed application was also sampled in tray with its activated CPU reader and the existing fan helper: **92,643,328 bytes** summed working set (**88.4 MiB**) and **46,723,072 bytes** private (**44.6 MiB**). Aggregate CPU time increased **46.875 ms** over a directly timed **3004 ms** interval (about **1.56% of one logical CPU**); the main contributed 15.625 ms, CPU reader 0 ms and fan helper 31.25 ms. Process identities were stable. Summed working sets can double-count shared pages. This is one short live sample, not long-run/sleep/reboot acceptance. The window was restored afterwards.

The controlled portable update verified the same executable hash and Desktop target with --show. The active beta.3 fan helper retained PID 10400 and its 15 November start time; protected main/fan binaries and startup tasks were not replaced or reapproved. HPOmenCap remained stopped/disabled; PawnIO remained running with Manual start. Package checking passed for 571 files with clean source, embedded CPU modules, correct version and disabled developer MCP. Inno Setup was absent, so the release is a portable ZIP with installation scripts rather than a Setup executable.

Protected user data: 47 files compared before/after; only the live fan-service-status.json producer changed. No profile, app preference, startup approval or control/recovery journal was changed by this work; no file was removed or added in the live store.

Hardware contracts, exact device binding, profile precedence, readback/restore and recovery remain. Corrected keyboard group labels and RGB-value dimming do not qualify firmware master brightness, exact visual zone boundaries, per-key control or an HP-independent lifecycle. The older protected helper/startup binaries are not replaced by updating the portable UI. Deployment identity, source and package checks are recorded in build-info.json and release checksums.

## Slint UI — 15 November 2026

Release executable: **15,384,576 bytes**, SHA-256 **`24FA1838A2C43E24D11F2F56F0E252581CDDEF8D5838FA7EFAE6FE43CB8009F0`**. The release defaults to software rendering; the explicit GPU renderer remains developer-testable. UI workspaces and exact scope: [migration](plans/SLINT_UI_MIGRATION.md), [release notes](releases/0.10.0-beta.1.md).

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| Workspace unit tests | 299 passed: 152 core / 61 app / 86 platform; one timing benchmark ignored |
| Guardian process fixtures | 14 scenarios passed; real services/drivers not addressed |
| Clippy workspace/all targets, `-D warnings` | Passed |
| Normal locked release build | Passed; embedded pinned CPU reader/SMU modules present |
| Release package contents, version, hash and notices | Passed (571 files); clean-source identity is required for publication |
| German, software renderer, scale 1.25 | Passed real interaction sequence |
| English, software renderer, scale 1.0 | Passed real interaction sequence |
| German, FemtoVG renderer, scale 1.5 | Passed real interaction sequence |
| English, FemtoVG renderer, scale 2.0 | Passed real interaction sequence |
| Normal release with `SLINT_MCP_PORT=9415` | No MCP listener; responding UI fixture opened |
| Protected user-data comparison | 47 files compared; only the live `fan-service-status.json` producer changed |

Each interaction sequence exercised 100 alternating wheel events, 15 resizes including compact dimensions, three minimize/restore and three real tray/restore cycles, actual tab transitions, Unicode name/caret input and text Undo/Redo, group collapse/reopen with numeric Undo, exact 55,125 mW roundtrip, all Windows boost choices, preset switching, curve numeric editing/history/reopen and four mouse/modifier gestures with immutable visible voltage anchors. About version/language and memory-only global settings save were checked separately from preset sign-in permission. Screenshots and element trees were inspected. These are marked **test fixtures**; no real tuning, manufacturer-service changes, startup-task writes or user-profile/settings writes are activated by the fixture.

Private evidence is under `.local/slint-ui-qa-20261115-182330-de-winit-software/`, `...182358-en-winit-software/`, `...182425-de-winit-femtovg/` and `...182454-en-winit-femtovg/`. The Windows display was not reconfigured: scale overrides exercise Slint rendering/layout, not physical multi-monitor DPI transitions. Native OS color picker/device actions and real standby/reboot/AC controller qualification remain outside this UI acceptance.

### Resource observations

A single warm **normal release, memory-only UI fixture** reached a responding window in about **249 ms**. After entering the tray its working set was **33,439,744 bytes**, private committed memory **12,087,296 bytes**; process CPU time did not advance during the observed approximately three-second interval. The fixture did not activate sensors or controllers, so this is **not** total tray-idle acceptance including helpers.

The renderer comparison found process-global GPU-driver allocations staying resident after hiding the GPU-rendered fixture (about 183 MB working set / 172 MB private in one observation). The software-rendered fixture used about 33 MB / 12 MB. This evidence motivated the default renderer change; executable size is not an optimization target. It is not a universal FPS/startup benchmark or a hardware-lifecycle claim.

During preparation the portable/installed main and protected fan helper were 0.9.0-beta.3; the fan helper remained running. Updating the portable UI must not silently overwrite protected executables or startup permissions. Check the deployed `dist/build-info.json` and private handover receipt for actual local identity; package construction supports `-SkipLocalDist` so a live image need not be touched while checks are running.


## Historical: control responsiveness and local update — 5 November 2026

The release executable has SHA-256 **`B00BC0B101D9F6F82F5B35587EA952DDB48D3F0989A77205852FDCA87A2E9639`**, source `2efca798ed58367323b4e562b39832126949f0ca` plus preserved/uncommitted work. The full private source and user-data snapshots are retained under ignored `.local/`. [Causes, changes and read-only evidence](history/RESPONSIVENESS_2026-11-05.md).

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --workspace --locked` | **295 passed**: 152 Core / 86 Platform / 57 App; one manual timing test ignored |
| Guardian real-process/file fixtures | **14 passed**, without real services/drivers |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo build --release --locked -p omen-hub-plus-plus` | Passed with both prepared CPU modules |
| New regressions | High priority readback; foreground/background lease contention; old/reapplied/pending/disabled hold revisions; exclusion of skipped targets; complete rather than partial verification; stale observations after coupled correction |
| PowerShell diagnostic/build scripts | Parser checks passed; package/launch checks recorded below |

The live beta.2 dashboard and CPU reader were explicitly set to **High** and read back as High. The new executable requests High for its own processes at startup. Neither requests realtime.

Three GET-only samples confirmed **85 °C Tctl limit / 65 W Slow / 80 W PPT**. Separately measured package power was **66.5 / 66.1 / 66.6 W**, Tctl **82.5 / 80.5 / 77.25 °C**. Effective clocks were unavailable because acquisition exceeded its timing bound. The historical full-load/1,700 MHz incident was not reproduced. HPOmenCap was already stopped/disabled; TPM and Secure Boot remained enabled. No Apply or hardware limit change was performed. This short observation does not qualify durable 85 °C enforcement or full HP-independent operation.

The actual desktop shortcut targeted the matching dist executable with `--show`. A single warm launch produced a responding main window after **1,029 ms** (including launch/elevation and process polling); the new native dashboard was inspected with live sensor data and the historical Apply result. The user stopped Computer Use with Escape before the remaining preset/scroll/resize acceptance. This is not a repeated cold-start benchmark. The total **50-MB tray-idle budget remains unaccepted**. Profiles, app preferences, startup authorization and the control/recovery journal stayed byte-identical; only the live fan status and GET diagnostic probe file changed. The installed beta.2 fan helper stayed running, and the protected beta.2 sign-in binary/approval were not replaced.

## German/English localization — 29 October 2026

The localization candidate, based on `b58d62c` plus this follow-up, is **3,160,576 bytes**, SHA-256 **`DC54030F4089BECB48BDBFCC200D0CD3BE4D454E7910333196530C81B0A26B19`**. Formatting, **242 unit tests** (130 Core / 75 Platform / 37 App), **14 process/file scenarios**, Clippy with denied warnings and the release build passed. All 37 App tests also passed with English explicitly selected.

Both catalogs have **543 matching keys**. New checks cover duplicate/empty entries, placeholder parity, literal placeholder-looking argument data, exact localized profile units, explicit language argument dispatch, structured preflight/rollback errors in both languages and direct native display literals. Hardware-busy handling uses a shared language-neutral code rather than display prose; all existing consumers were updated together.

**14 own native client renders** covered dashboard, presets, GPU V/F draft and synthetic keyboard lighting in both languages, plus expanded GPU/CPU/Windows groups at the 920×720 minimum preset size. These showed readable translated labels/actions; scrollable sections remain scrollable. CLI help was checked in German and English. Protected profile/recovery/startup/fan/CCD/graphics preference files remained byte-identical; the temporary metadata-only fixture was removed. No Apply, hardware SET, UAC activation, service action or installation occurred. Local images/results stay ignored in `.local/review/localization-20261029/`.

The app now propagates the selected language to separate control/UAC launches and localizes display decimals without changing stored driver/profile units. Expected profile/recovery/support failures use message keys; opaque native/provider details remain labeled original diagnostics. Windows-owned dialogs can retain the installed Windows resource language. [Localization policy](LOCALIZATION.md).

No real sign-in/reboot/sleep, hardware tuning or new memory acceptance is claimed. The installed 0.1.0 copy remains separate from this candidate.

## Previous software checks — source `251fff6`

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --workspace --locked` | 246 tests: 133 Core / 75 Platform / 38 App; one manual timing test ignored |
| Standalone guardian process fixtures | 14 scenarios passed with actual processes and file-simulated service/cooling state |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo build --release --locked` | Passed with both pinned CPU modules staged |
| Native curve regression fixtures | Seven passed: release coordinates, cancel/capture loss, GPU anchors/history, list contents/visibility, curve switching and live draft save; synthetic data only |
| Intel retirement/compatibility fixtures | Six passed; exact old serialization, no partial Apply, explicit cleanup, stable remaining control IDs and CLI rejection |
| Earlier curve release timing (`74B9662B…`) | Three final runs; [method and comparison](history/CURVE_PERFORMANCE_2026-10-29.md), not repeated after catalog cleanup |
| Native release previews | GPU and CPU presets at 1100 × 900, Windows section at 920 × 720; visually inspected with read-only GPU access |
| Catalog/capability query | 81 catalog entries, zero Intel entries; live availability/remaining blockers in the [cleanup record](history/CODE_CLEANUP_2026-10-29.md) |
| Changed preset smoke scripts | PowerShell parsing passed; interactive/hardware-adjacent flows not run |
| Earlier installer recovery guard fixtures (`251fff6`) | 60 passed; not rerun for this UI change |
| Earlier power-lifecycle evidence fixtures (`251fff6`) | 18 passed; not rerun for this UI change |
| [Earlier GitHub Rust checks for `251fff6`](https://github.com/Ishtvr/Omen-Hub-debloat/actions/runs/40361700174) | Passed; does not qualify the current working tree |

The current 29 October local candidate is **3,123,200 bytes**, SHA-256 **`C4E4DEA4D864827DF3C9E6A333756819B75611F5DB7C5D526EF5C4F3832A2B1A`**. It was not installed or used for hardware writes. No new package ZIP, full DPI/accessibility acceptance, total tray-memory measurement or physical power transition was performed.

The preceding curve-optimization candidate was **3,126,784 bytes**, SHA-256 `74B9662B0CBB7C953CFDC1F974E4B65C1BAF512CAC885FC223A0EBE546E6D27C`, with 240 tests. Its native curve timing/preview observations remain tied to that artifact.

The earlier 29 October candidate from `251fff6` was **3,125,760 bytes**, SHA-256 `6A23EABADA60C2077370E995DBD1A2543723F03764790AFC18520694B9720E89`. Its original check summary had 233 unit tests and no new native visual review. Do not mix its artifact identity with the optimized candidate.

The earlier documentation-only refresh (`b58d62c`) checked 47 Markdown files, 327 local links, fenced blocks, module hashes and documentation-only change scope. Three historical report bodies were preserved from `251fff6`, with only their relative links rebased and an archive notice added. That refresh did not change runtime source, scripts, permissions or hardware state.

The 26 October candidate with the same version had SHA-256 `AD74D4F6DCBF583EE47D5432290A194CA015B51297C3FDF2C28651967AB6786D`; its native metadata checks covered exact watt-to-milliwatt storage and preset-to-chart editing without Apply. The different hashes identify distinct artifacts; their measurements must not be mixed.

## Measured hardware evidence

| Date | Evidence | Boundary |
| --- | --- | --- |
| 13–15 October | Four-zone capability, full RGB/mode readback and original restore, including a new process | Existing HP services remained; visible On/Off separately confirmed on 15 October, color effect supported by readback |
| 15 and 22 October | Max-Fan flag and short own fan-helper curve/Max trials; firmware tachometers reached 4800/4800 RPM | No qualified BIOS ownership handover or full independent long run |
| 22 October | 617.42 full 127-point V/F, core/VRAM small offset trials and complete restore | No load stability, hard voltage ceiling or sustained full-TGP proof |
| 22 October | Independent CPU RSMU limits, CO around −20, PBO 1×; Windows AC/DC and target-plan restore | Brief control-path trials, not the full allowed numeric range |
| 23 October | Fan watchdog fallback after roughly two minutes; separate FAMC trial stopped at conservative thermal cutoff | Direct BIOS-Auto switch remains outside normal control; fallback time is not ownership readback |
| 24 October | Native CPU/tray cycles, fresh baselines and simulated power notifications | No actual standby/reboot; profile power limits subsequently reverted |
| 25 October | Explicit short HPOmenCap isolation: six reads retained 70 W Slow / 75 W PPT through about 12 seconds; full original restore | Supports service influence; does not prove sole cause, native takeover or durable independent operation |
| 26 October | Public NVML limits/default/effective reads; configured/requested original unavailable | No numeric GPU-power SET; limits and effective value do not substitute for original capture |

Detailed payloads, build hashes and timestamps remain in the [historical log](history/VALIDATION_THROUGH_2026-10-26.md) and topic/model pages. TGP read-only research is documented separately in the [investigation plan](plans/NVPWRCONTROL_TGP_FORK_PLAN.md).

## Memory and installation

On 24 October an earlier build plus its paused reader and existing active fan helper totaled **48.76 MB** tray working set after sensor activation/two returns. The short sample used no working-set trimming and counted shared pages per process. This is close to the **50-MB total resident target**, not a current-build or long-term acceptance.

At the 29 October read-only installation check, `%ProgramFiles%/OmenHubPlusPlus/Omen Hub++.exe` still reported version **0.1.0**, **2,753,536 bytes**, SHA-256 **`A0F4113A2FF0F1C3A2E4D02105FC1F2C56B51F94BC9E8F377FBD3A617C42834C`**. Building source does not update that copy, the fan helper or an existing protected sign-in approval.

## Release status and pending acceptance

`v0.9.0-beta.1` was published and then withdrawn at the user's request. Its source tag remains; no replacement release was present at the latest check. [Historical release notes](releases/0.9.0-beta.1.md).

Real reboot, standby/resume, AC/DC and MUX transitions; long-term CPU/GPU/fan stability; independent HP-controller replacement; full current helper memory; and broader devices remain pending. Code-level gaps in shortcut recovery, CPU range evidence and fan status confirmation are recorded in [Project review](PROJECT_REVIEW.md). Software fixtures do not replace these measurements.
