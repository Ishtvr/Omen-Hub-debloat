# Roadmap

Reviewed **29 October 2026**, `0.9.0-dev.2`. The goal is an independent native replacement for OMEN hardware-control processes, with relevant UXTU-style tuning. The [hardware matrix](HARDWARE_MATRIX.md) owns current support claims; historical completed work is in [Validation](VALIDATION.md), not an ever-growing checklist here.

## Slint software migration — 15 November 2026

The normal workspaces have moved to Rust/Slint on the 0.10.0-beta.1 branch. Software interaction acceptance is recorded separately in [Validation](VALIDATION.md). Keep physical multi-monitor/DPI changes, actual color-picker/device writes, sleep/reboot/AC lifecycle and total tray-idle measurement including activated helpers as device acceptance tasks. The UI rewrite does not close the hardware ownership/recovery items below.


## 1. Close recovery and status gaps

- [ ] Complete pending ownership-journal checks for MUX and other shortcuts. Manual HP-mode replacement now verifies pending profile recovery first; its complete original/readback contract remains below.
- [ ] Persist the MUX shortcut's original and intent before SET; add abort/error-after-effect fixtures.
- [ ] Establish the full HP-mode original/readback contract. Keep Balanced fallback distinct from actual original restoration.
- [ ] Separate CPU protocol ranges, model evidence and stability-tested settings; document the source of each bound. Current PPT/Slow 160 W, CO −50…+30 and PBO 10× exceed the measured trial envelope.
- [ ] Confirm fan-mode execution using fresh, matching helper status; distinguish selected, waiting, failed, active and handover-pending.
- [ ] Capture Slow/STAPM/PPT and the GPU flags **before** the HP mode prelude, so Restore returns to the state before Apply (today the originals are read after the prelude; found 20 November 2026).
- [ ] Route the AC/battery automatic preset switch through the latest-intent queue (`Replace`, serial wait) instead of separate non-replace commands.
- [ ] Persist nonce, digest, verified/skipped/requested lists and boot time in the journal completion instead of the shared `last-control-result.json`.
- [ ] Decide the TPP bound: presets can store up to 254 W, the measured acceptance is ≤115 W.
- [ ] Identify the trigger of the Unleashed PPT reset with read-only sampling (PM cells 0–5 at 250 ms, GPU power, WMI-Activity ETW), then a user-approved single 75 W write with Dynamic Boost off/Balanced/fan helper paused. Only a qualified path may write PPT in Unleashed again.
- [ ] Run the Slint curve editor through `scripts/test-slint-ui.py` (developer MCP build) with the beta.8 gestures added; Python/Pillow were not installed on 20 November 2026.

Acceptance: model/file/process tests cover interrupted operations and foreign changes; the UI never reports a saved preference as confirmed firmware state. Details: [Project review](PROJECT_REVIEW.md).

## 2. Own cooling and controller responsibility

- [ ] Supervise the prepared native HPOmenCap/cooling qualification on the actual device, including the independent return guardian.
- [ ] Obtain reliable performance/fan-ownership readback and a safe BIOS handover contract.
- [ ] Prove sustained own cooling and CPU-profile persistence without original HP writers.
- [ ] Integrate durable ownership into normal app/sign-in control only after its recovery path is qualified.
- [ ] Qualify physical fan-curve uptake and lifecycle integration using exact helper
  policy acknowledgements. Manual named snapshots now have request-level preference
  journaling and preserved originals; full physical transaction/sign-in acceptance
  remains pending.

Acceptance: native controller/cooling recovery, helper loss and aborted ownership retain a safe accountable state. The temporary OEM return used by migration trials is not a target runtime dependency. [Replacement plan](REPLACEMENT_PLAN.md), [HP control](HP_CONTROL.md), [fan handover](FAN_HANDOVER.md).

## 3. Complete the profile and power lifecycle

- [ ] Connect authorized profile revalidation/reapplication to resume and AC/DC transitions. Existing dashboard notifications renew sensor baselines; they are not a full profile controller. The Settings option **Preset on battery** applies a chosen mode/preset on unplug and returns on AC; validate it on the device.
- [ ] Run **Disable HP OMEN Cap** and its restore on the device, then apply a CPU PPT/Slow preset and a sign-in approval without HPOmenCap.
- [ ] Check the elevated self-restart, single instance and the tray logon task on the device, including migration of an old Run entry.
- [ ] Test actual sign-in after reboot with an exact authorized profile revision.
- [ ] Test actual standby/resume, display/power events, driver loss/restart and controller failure, then verify full originals/targets.
- [ ] Prove keyboard behavior across hotkeys, lock/unlock, resume and reboot without HP controllers.
- [ ] Verify the physical MUX switch after reboot and the dGPU state for an iGPU-only battery profile.
- [ ] Check that monitoring/editor activity does not defeat dGPU power saving.

Acceptance: current boot/event evidence plus live profile readback, never just simulated Windows messages. A reboot or timeout is not assumed to restore settings. [Power lifecycle](POWER_LIFECYCLE.md).

## 4. Continue GPU-TGP research

- [ ] Map existing NVIDIA control transports, privilege requirements and matching kernel handlers without guessed calls.
- [ ] Compare proven read-only queries as standard user and administrator; investigate offered policy classes and NVML scopes.
- [ ] Identify configured original, units, bounds, coherent write and complete restore for the reference GPU.
- [ ] Decide whether a separate prototype is justified; only then implement the Rust backend and safe small-decrease trial.

Current scope is investigation/planning. TPM and Secure Boot remain enabled; no test-signing or driver-enforcement bypass. Author claims, an exported setter and a visible device endpoint are not proof of a usable writer. The detailed [TGP plan](plans/NVPWRCONTROL_TGP_FORK_PLAN.md) remains the source for this work.

## 5. Broader support and release acceptance

- [ ] Qualify other boards, BIOS/SMU versions and NVIDIA implementations separately.
- [ ] Measure physical undervolting effect and longer CPU/GPU/fan stability under controlled load.
- [ ] Measure sustained total resident tray memory/CPU after sensor activation, including owner/guardian/fan helpers; low background use supersedes the old fixed 50 MB ceiling. Do not transfer earlier short samples to new builds or a full lifecycle.
- [ ] Review multiple DPI/display configurations, keyboard navigation, high contrast and accessibility.
- [ ] Update installed app, helper and protected sign-in approval to one qualified build; validate the package, licenses, corresponding sources and recovery before publishing a scoped beta.

## Documentation

- [x] Replace the long GitHub README with a concise English entry point.
- [x] Centralize current support, validation and pending work; move dated logs to history.
- [x] Correct obsolete startup, driver, fan-curve and sensor descriptions without changing runtime permissions.
- [x] Audit German/English UI, help and common errors; keep professional tuning terminology, exact localized display units and explicit selection-versus-confirmation wording.

Direct GPU/VRAM voltage, FPS limiting, fixed CPU clocks/voltages and removed APU menus stay outside the current requested UI. Additional lighting effects require actual device capability and a separate restore contract; they are not required for the simple four-zone interface.
