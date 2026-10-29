# Validation

Current summary reviewed **29 October 2026**. Source tested: **`251fff6987966ca0b04f9c92c8b1e1f068b53511`**, version **`0.9.0-dev.2`**, Windows x64 / Rust-Cargo 1.99.0. The documentation refresh introduces no new hardware qualification.

Current support: [Hardware matrix](HARDWARE_MATRIX.md). Exact device evidence: [8E35](models/8e35.md). Pending work: [Roadmap](ROADMAP.md).

## Latest software checks

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --workspace --locked` | 233 unit tests: 130 Core / 74 Platform / 29 App |
| Standalone guardian process fixtures | 14 scenarios passed with actual processes and file-simulated service/cooling state |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo build --release --locked` | Passed with both pinned CPU modules staged |
| Installer recovery guard fixtures | 60 passed, no real installer/service action |
| Power-lifecycle evidence fixtures | 18 passed, no standby/reboot requested |
| [GitHub Rust checks for `251fff6`](https://github.com/Ishtvr/Omen-Hub-debloat/actions/runs/40361700174) | Passed |

The 29 October local candidate is **3,125,760 bytes**, SHA-256 **`6A23EABADA60C2077370E995DBD1A2543723F03764790AFC18520694B9720E89`**. It was not installed or used for hardware writes. No new package ZIP, visual/DPI acceptance, memory measurement or physical power transition was performed in this review.

The documentation-only refresh checked 47 Markdown files, 327 local links, fenced blocks, module hashes and documentation-only change scope. Three historical report bodies were preserved from `251fff6`, with only their relative links rebased and an archive notice added. No runtime source, scripts, permissions or hardware state changed.

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
