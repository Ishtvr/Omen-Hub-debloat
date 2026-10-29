# Roadmap

Reviewed **29 October 2026**, `0.9.0-dev.2`. The goal is an independent native replacement for OMEN hardware-control processes, with relevant UXTU-style tuning. The [hardware matrix](HARDWARE_MATRIX.md) owns current support claims; historical completed work is in [Validation](VALIDATION.md), not an ever-growing checklist here.

## 1. Close recovery and status gaps

- [ ] Check all pending hardware/ownership journals before dashboard HP-mode or MUX writes.
- [ ] Persist the MUX shortcut's original and intent before SET; add abort/error-after-effect fixtures.
- [ ] Establish the full HP-mode original/readback contract. Keep Balanced fallback distinct from actual original restoration.
- [ ] Separate CPU protocol ranges, model evidence and stability-tested settings; document the source of each bound. Current PPT/Slow 160 W, CO −50…+30 and PBO 10× exceed the measured trial envelope.
- [ ] Confirm fan-mode execution using fresh, matching helper status; distinguish selected, waiting, failed, active and handover-pending.

Acceptance: model/file/process tests cover interrupted operations and foreign changes; the UI never reports a saved preference as confirmed firmware state. Details: [Project review](PROJECT_REVIEW.md).

## 2. Own cooling and controller responsibility

- [ ] Supervise the prepared native HPOmenCap/cooling qualification on the actual device, including the independent return guardian.
- [ ] Obtain reliable performance/fan-ownership readback and a safe BIOS handover contract.
- [ ] Prove sustained own cooling and CPU-profile persistence without original HP writers.
- [ ] Integrate durable ownership into normal app/sign-in control only after its recovery path is qualified.
- [ ] Integrate fan curves into complete profile transactions with coupled CPU/GPU/IR protection and preserved originals.

Acceptance: native controller/cooling recovery, helper loss and aborted ownership retain a safe accountable state. The temporary OEM return used by migration trials is not a target runtime dependency. [Replacement plan](REPLACEMENT_PLAN.md), [HP control](HP_CONTROL.md), [fan handover](FAN_HANDOVER.md).

## 3. Complete the profile and power lifecycle

- [ ] Connect authorized profile revalidation/reapplication to resume and AC/DC transitions. Existing dashboard notifications renew sensor baselines; they are not a full profile controller.
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

- [ ] Implement and qualify Intel PL1/PL2/PL4 and related controls when a test device is available; none is currently available.
- [ ] Qualify other boards, BIOS/SMU versions and NVIDIA implementations separately.
- [ ] Measure physical undervolting effect and longer CPU/GPU/fan stability under controlled load.
- [ ] Measure total resident tray memory/CPU after sensor activation, including owner/guardian/fan helpers; target 50 MB. Do not transfer the earlier 48.76-MB sample to new builds.
- [ ] Review multiple DPI/display configurations, keyboard navigation, high contrast and accessibility.
- [ ] Update installed app, helper and protected sign-in approval to one qualified build; validate the package, licenses, corresponding sources and recovery before publishing a scoped beta.

## Documentation

- [x] Replace the long GitHub README with a concise English entry point.
- [x] Centralize current support, validation and pending work; move dated logs to history.
- [x] Correct obsolete startup, driver, fan-curve and sensor descriptions without changing runtime permissions.
- [x] Audit German/English UI, help and common errors; keep professional tuning terminology, exact localized display units and explicit selection-versus-confirmation wording.

Direct GPU/VRAM voltage, FPS limiting, fixed CPU clocks/voltages and removed APU menus stay outside the current requested UI. Additional lighting effects require actual device capability and a separate restore contract; they are not required for the simple four-zone interface.
