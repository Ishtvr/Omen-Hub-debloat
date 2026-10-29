# Recovery and removal

This guide describes current behavior, including its gaps. Support is model/build-specific; use [Hardware support](HARDWARE_MATRIX.md). **Preserve the journals until actual recovery is complete.** Reboot, process exit, deleting a preset and stopping software commands are not proven substitutes for restoration.

## What has a durable original

| Path | Current recovery coverage |
| --- | --- |
| Supported custom-preset settings | Complete original images are saved before SET, then targets/restore are verified; unresolved failures retain the journal |
| Direct keyboard window | Separate lighting transaction store; complete RGB/mode recovery on reconnect |
| Controlled migration qualification | Separate protected service/cooling responsibility records plus return guardian |
| Dashboard HP default-mode shortcut | No full mode-original journal; thermal mode lacks GET; a failed non-Balanced mode attempts Balanced |
| Dashboard MUX shortcut | Reads/writes the boot preference, but saves its own record after the BIOS write; preset-equivalent recovery remains unfinished |
| Fan Auto/Curve/Max preference | Saved intent for the helper; after own regulation normal pause/stop requests maximum once and retains handover-pending status |

The shortcut gaps are open work, not fixed by this guide. A shared mutex serializes own writes but does not block a foreign tuner or supply a missing original.

## Data locations

User data follows the Windows token's actual user profile, normally `%USERPROFILE%\.omen-hub-pp`:

- `profiles/`, `deleted-profiles/`, `control-journal.json`, `control-history/`, `startup.json`, `last-*.json`;
- `keyboard-control/` for direct lighting;
- `active-fan-curve.json`, `fan-mode.json`, `ccd-affinity.json`, `graphics-mode.json` for preferences/status-related state.

The protected `%ProgramFiles%\OmenHubPlusPlus` installation contains the app, optional own `Omen Hub++ Fan.exe`, receipts and exact-revision/binary sign-in approval. Migration adds `controller-ownership.json`, `controller-cooling.json` and a separate last-guardian report. Task names are `OmenHubPP-Profile-<SID>` and `OmenHubPP-Fan-<SID>`.

The current original is the state before the relevant Apply, not necessarily factory defaults. Historical completed checkpoints are not automatically applied. Removing the data directory can destroy the only original-state record.

## Restore a custom preset

If the preset UI offers **Restore / Wiederherstellen** after an unresolved transaction, use it to recover the recorded original. Alternatively, in an elevated Windows PowerShell for the same user and matching installation:

```powershell
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-restore
```

This explicitly changes hardware back to recorded originals and disables the profile's automatic sign-in selection first. Identity changes, foreign values, invalid originals or incomplete recovery remain errors. Do not delete the record to bypass them.

New CPU limit images use the measured RSMU/table contract. Old HP limit images cannot select it. An old effective-only Windows overlay record cannot restore the user's configured overlay. Such legacy failures retain the journal for diagnosis rather than guessing values. [System controls](SYSTEM_CONTROLS.md).

## HP modes, MUX and cooling

Choosing Balanced writes a known factory-style state; it does **not** reconstruct an unreadable previous thermal mode. Choosing another known mode is a new hardware action. The dashboard HP/MUX paths currently lack complete preset-journal protection.

A MUX boot-preference change takes effect only after restart. To return, choose the **known original** preference and coordinate the restart; Hybrid is not assumed to have been every user's original. BIOS preference readback alone does not prove actual GPU routing or dGPU-off state.

Max-Fan Off releases the independent override only. Auto in the own cooling helper pauses own regulation after the conservative one-time maximum request; neither confirms immediate BIOS ownership. Removing a helper likewise does not certify safe BIOS takeover. Resolve cooling responsibility before treating removal as complete. [Fan handover](FAN_HANDOVER.md).

## Lighting and owned migration recovery

The lighting window checks its own pending transaction when reconnecting and restores only valid matching own originals before a fresh read. Resolve pending performance recovery first when it blocks lighting. On/Off retains RGB; there is no separate arbitrary firmware-reset command.

For an interrupted explicitly owned HP migration, inspect first:

```powershell
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" hp-control-status
```

Only an actually pending own session calls for elevated `hp-control-recover`. It requires a dead matching owner, valid protected same-build installation and clear hardware-recovery ordering. It can complete cooling return after the service has returned. A running original service alone does not close both journals. [HP control](HP_CONTROL.md).

## Disable automatic launches

```powershell
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-startup-disable
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-startup-status
```

The preset's startup checkbox controls hardware-profile sign-in. The separate dashboard **Start with Windows** setting controls the unelevated tray Run entry; turn it off too when removing the app. Disabling sign-in does not restore current hardware values.

## Remove or update

First complete hardware and controller/cooling recovery. The installers refuse changes while protected ownership/cooling records or an active session remain unresolved. They do not forcibly kill a responsible cooling helper to make an update succeed.

From the repository in Windows PowerShell as the intended user, elevated where required:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-fan-service.ps1 -Mode Remove
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-startup.ps1 -Mode Remove
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-app.ps1 -Mode Remove
```

In an extracted package the scripts sit beside the EXE, so omit `scripts\`. This removes own task/approval/helper/app components as permitted by their guards. App removal can retain the executable if a profile task still uses it. Profiles/journals are preserved; inspect any failure instead of bypassing the guards.

Replacing the installed binary invalidates old protected sign-in approval. A successfully compiled newer file is not an installed update. Real sleep/reboot and full independent lifetime remain pending; [Validation](VALIDATION.md) identifies what was measured.
