# Validation

Current software summary updated **15 November 2026**, version **0.10.0-beta.1**, Windows x64 / Rust 1.99.0. Source: the preserved `554bb55` baseline plus the Slint migration, identified by the release tag and packaged `build-info.json`. The older native measurements below remain historical. No new hardware write or HP-independent lifecycle qualification is implied.

Current support: [Hardware matrix](HARDWARE_MATRIX.md). Exact device evidence: [8E35](models/8e35.md). Pending work: [Roadmap](ROADMAP.md).

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
