# Power and profile lifecycle

Reviewed **29 October 2026**. **Sensor power handling is implemented; complete automatic profile recovery/reapplication across real transitions is not yet integrated and qualified.** Sign-in application is a separate existing path. [Hardware matrix](HARDWARE_MATRIX.md).

## Existing behavior

The reference system reported Modern Standby **S0 connected** and hibernation. The dashboard registers source changes, session display state and suspend/resume through public Windows notifications. Screen-off and suspend independently pause dashboard polling; automatic wake does not override a dark session.

Source changes and resume invalidate measurement baselines. Results spanning a transition are discarded. A pending CPU response is matched/drained before explicit pause and cannot acknowledge a later operation. Hardware sessions and energy/clock baselines are released; reopening starts fresh. This does not establish the sleep depth of the whole computer or fan/keyboard ownership.

The `Request::Power` worker branch refreshes sensor state, not a full performance profile. The general `core::supervisor` policy is not wired to a production profile-resume controller. Adding authorized identity checks, originals/target reconciliation and ordered reapplication remains work.

## Sign-in and read-only readiness

```powershell
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-startup-check
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-verify-current <profile-id>
```

The first inspects the selected revision, completed application, protected installation/approval and current full startup plan. It enables/disables nothing. The second needs appropriate hardware-read permissions and verifies the exact applied profile's full live targets without repairing a mismatch. A different build or edited profile is not a valid sign-in acceptance.

`profile-startup-apply` **writes settings** and records execution time/PID. It is the protected sign-in action, not a readiness query. `profile-apply-trace` likewise applies a profile and only adds CPU readback observations; it is not read-only.

PPT/Slow conflict/startup gates remain active until own controller handover is qualified. A readable source state or a temporarily stopped OEM service is not durable startup ownership. [HP control](HP_CONTROL.md).

## Evidence checkpoints

`scripts/check-power-lifecycle.ps1` records local read-only checkpoints; it does not apply a profile, authorize sign-in, suspend, reboot or change services/drivers. It needs a currently verified applied profile, matching installed build/approval, one unambiguous German dashboard and all required sensors.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\check-power-lifecycle.ps1 -Phase Prepare
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\check-power-lifecycle.ps1 -Phase AfterStandby -RunDirectory '<directory-from-Prepare>'
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\check-power-lifecycle.ps1 -Phase AfterReboot -RunDirectory '<directory-from-Prepare>'
```

Actual sleep/reboot must be separately coordinated. The script itself does not trigger either.

Standby comparison requires the same boot/app/reader identities, new dashboard power generation, fresh sensors/profile targets and a correctly ordered new Kernel-Power **506/507** S0 event pair according to the installed event definitions. Reboot comparison requires a new boot and matching fresh task/profile execution evidence. Old success files or simulated window messages cannot pass as a real transition. DRIPS/deep-idle residency is not measured by this classification.

`test-power-lifecycle-evidence.ps1` has 18 artificial evidence cases, including old/future/out-of-order events, missing readers, changed builds/revisions and stale/failed startup results. Passing them proves evidence classification, not actual sleep/reboot.

## Recorded power-limit conflict

On 24 October, the user's unchanged profile could briefly read back **70 W Slow / 75 W PPT** and later revert toward **66/80 W**. UXTU was configured to reapply every 15 seconds but was absent during the isolated trials; it was not proven to cause those particular resets. Pausing own cooling also did not establish the cause. No periodic SET loop was added to overrule another controller.

On 25 October, a separately authorized brief HPOmenCap isolation retained 70/75 W through six reads over roughly 12 seconds. Full originals/metadata, service, own helper and original Hub were restored. This supports service/lifecycle influence without identifying a sole setter or qualifying later native takeover, long-term stability or real power transitions.

Detailed chronological results and build identities: [historical validation](history/VALIDATION_THROUGH_2026-10-26.md). Next work: durable own cooling/ownership, full profile controller, actual transitions and longer independent operation. [Roadmap](ROADMAP.md).
