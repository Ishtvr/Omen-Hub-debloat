# Power and profile lifecycle

Reviewed **22 November 2026**. **Sensor power handling and the automatic AC/battery preset change are implemented; neither is qualified across real transitions on the device.** Sign-in application is a separate existing path. [Hardware matrix](HARDWARE_MATRIX.md).

## Automatic AC/battery change

Each preset names its battery behavior ([Getting started](GETTING_STARTED.md)). The dashboard decides on the current level, never on an event alone: Windows reports the source (`GetSystemPowerStatus`; unknown never switches), the policy in `core::battery` compares it with the persisted state, and the apply queue runs the switch behind any manual choice.

- **Events.** `GUID_ACDC_POWER_SOURCE`, suspend/resume, display and power-plan notifications. `PBT_APMPOWERSTATUSCHANGE` also arrives for every battery percent; it counts only when the source differs from the one seen last.
- **Waits.** 1 s after unplugging, 2 s on AC and after resume, as in the OMEN Hub; then the source is read again. One more check runs 20 s after the dashboard starts, after the sign-in task has finished.
- **Order.** The preset being left is undone from its journal when it is still the last completed Apply; then the selection is applied. The firmware mode has no readback and is not in a journal, so a battery preset without a Dynamic Boost field first writes Balanced.
- **Sign-in.** On battery, a sign-in preset with a battery behavior is not applied; the task records it as the preset in use on AC and the dashboard applies the behavior. On AC it is applied as before.
- **Boot.** `power-switch.json` carries the boot time. A restart resets volatile firmware values, so a battery session or manual override of an earlier boot ends.
- **Failure.** Three retries after 5, 15 and 60 s, then rest until the next power event. A crashed dashboard is restarted by its logon task (three times, one minute apart).

The journal keeps one generation of originals. After a restart on battery the battery preset is applied on top of its own persistent Windows values, so values the AC preset does not name (for example 60 Hz) can remain after the return. None of this is measured on battery yet.

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
