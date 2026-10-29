# Project review

Reviewed **29 October 2026**, source **251fff6987966ca0b04f9c92c8b1e1f068b53511**, version **0.9.0-dev.2**. This is a code/documentation/build assessment, not new hardware qualification. Latest test identities/results are in [Validation](VALIDATION.md); current support in [Hardware matrix](HARDWARE_MATRIX.md).

## Findings still requiring implementation

| Priority | Finding | Next concrete action |
| --- | --- | --- |
| High | HP-mode/MUX shortcuts do not use the preset-equivalent pending-journal/write-ahead path | Block new conflicting writes during unresolved recovery; persist MUX original/intent first; establish actual HP-mode restore contract |
| High | CPU allowed ranges exceed the measured safe trial envelope | Separate protocol bounds, model evidence and tested operating settings; correct fuse/stability implications |
| Medium, before everyday acceptance | Fan quick-switch confirms selection, without a fresh actual helper acknowledgement | Match fresh helper process/status/mode and show waiting/error/pending separately from confirmed regulation |

### Shortcut recovery

`mode-ui-apply` and `mux-ui-apply` in [profile_commands.rs](../crates/app/src/profile_commands.rs) take the own control lease and call the direct HP functions. They do not first check the full pending performance journal or use its durable transaction. MUX preference metadata is saved after the BIOS write. A crash or prior unresolved Apply can therefore leave weaker recovery coverage than a preset.

[hp.rs](../crates/platform/src/windows/hp.rs) cannot fully GET the thermal mode. The non-Balanced error fallback attempts Balanced, not the actual captured original. Closing this gap must not invent a mode image. Tests should cover pending recovery, error after effect, missing result and interruption between steps before another physical trial.

### CPU ranges

[cpu_tuning.rs](../crates/platform/src/windows/cpu_tuning.rs) accepts PPT/Slow up to **160 W**, CO **−50…+30** and PBO **10×**. Recorded device trials used small power/current/temperature decreases, CO around **−20** and PBO **1×**. Diagnostic fuse reads do not dynamically limit these constants. A working SET/readback contract does not establish all combinations stable or thermally suitable. No numeric policy was changed by the review/docs refresh.

### Fan status

The `Request::FanMode` branch in [ui.rs](../crates/app/src/ui.rs) saves preferences and checks installation. The localization follow-up corrected its wording to confirm selection only. It still does not require fresh matching helper acknowledgement: the helper can be waiting for the original Hub or report a fault. Existing process identity, timestamps, modes and `waiting_for_hub` / `waiting_for_control` / `handover_pending` should inform execution confirmation.

## Larger remaining work

1. Qualify the prepared native HPOmenCap/guardian/cooling migration; then design and integrate durable own responsibility without original HP controllers.
2. Establish safe BIOS fan handover and complete fan-curve profile transactions.
3. Connect authorized profile revalidation/reapplication to real resume/AC events. Existing dashboard power handling renews sensor state, not full profiles.
4. Test real reboot, sleep/resume, source change, driver/owner failure, full restore and longer load stability.
5. Continue the [TGP investigation](plans/NVPWRCONTROL_TGP_FORK_PLAN.md). No alternate writer is yet established; TPM and Secure Boot remain requirements.
6. Verify physical MUX/iGPU-only battery behavior and independent keyboard hotkey/power lifetime.
7. Obtain appropriate hardware for Intel/additional model support; none is currently available for Intel.
8. Measure current total resident helpers against 50 MB, finish DPI/accessibility review, then install/package one accepted version and reauthorize exact sign-in revision.

The local installed 0.1.0 copy was not updated by the source build; [Validation](VALIDATION.md) identifies both artifacts. The ordinary app still has no automatic HP takeover.

## Documentation follow-up completed

The documentation refresh now centralizes current support, records startup permission separately from physical qualification, distinguishes own helper curves from embedded profile instructions, includes both qualified NVIDIA implementations and current CPU sensor semantics, and preserves dated logs separately. The short public README is English. These documentation corrections do not resolve the runtime findings above.

Recommended order: recovery/range/status gaps; independent cooling/controller ownership; full profile/power lifecycle; physical acceptance and memory; scoped packaging/release. TGP research remains a separate investigation strand. The [Roadmap](ROADMAP.md) is the maintained checklist.
