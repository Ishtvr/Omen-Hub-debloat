# Getting started

Omen Hub++ is an experimental Windows x64 application. Start with the [hardware matrix](HARDWARE_MATRIX.md): current hardware-control evidence is specific to HP 8E35/F.13, Ryzen 9 8940HX and RTX 5070 Laptop. A different BIOS, GPU implementation or driver can make a control unavailable.

The current local source candidate is `0.9.0-beta.3`. The first public beta was withdrawn; a local candidate is not a published release. Follow [Building](BUILDING.md) and [Validation](VALIDATION.md) to identify the actual tested executable.

## Launch

From a source checkout after building:

```powershell
.\target\release\omen-hub-pp.exe
.\target\release\omen-hub-pp.exe --tray
.\target\release\omen-hub-pp.exe --show
```

The second command starts hidden. `--show` explicitly opens the dashboard even when **Start in the tray** is saved; desktop shortcuts should use it. Open the app from its tray icon; the tray menu also provides a full exit. Windows may put the icon in its overflow area. The app retries registration after Explorer restarts and keeps the window accessible if tray registration fails.

New app processes set Windows priority **High** and verify it. This improves scheduling of short UI/control work under CPU load; it does not increase CPU power limits or override firmware protection. Manual dashboard actions use a separate worker and wait briefly for an existing controller instead of sitting behind telemetry reads. Never use realtime priority for this app.

The dashboard puts GPU watts and temperature first, with CPU measurements beside them. Missing or stale readings show unavailable. CPU package watts and Tctl require the optional embedded module and an already running, exactly verified PawnIO 2.2.0 driver; see [CPU sensors](CPU_SENSORS.md). Windows-reported MHz is a separate OS value.

The first visible dashboard requests UAC for a session-bound CPU reader. Canceling does not grant access; **Refresh** is the explicit retry. Tray/suspend pauses the reader and releases hardware sessions. Returning begins with fresh measurement baselines.

## Performance profiles

The dashboard lists Quiet, Balanced, Performance and Unleashed defaults alongside saved custom presets. Selecting an entry does not apply it; **Apply** requests the relevant Windows approval.

Open **Performance profiles / Leistungsprofile** to create or edit a custom preset:

1. Select a preset or create a new one and enter its name.
2. Choose the GPU, CPU and Windows settings to include. An unchecked setting is omitted; it does not reset an existing override.
3. **Save** stores metadata. **Apply** changes supported settings through the transaction backend.
4. Optionally authorize the exact successfully applied revision for Windows sign-in. Saving or editing a preset alone does not authorize it.

Missing hardware support, invalid values or a different device block preflight. Normal Apply can return individual failed settings to their originals while retaining verified settings; this is labelled **only partially applied**, with the concrete failures. Complete current-preset verification fails if any requested target differs. A preset's HP shared budget (TPP) has no exact readback and is explicitly labelled unconfirmed. Limits such as 65 W Slow and 80 W PPT are ceilings, not a promise to consume those watts.

The optional **Preset hold** watches only targets confirmed by the last Apply. Manual Apply/Restore has precedence; changed revisions and pending recovery prevent old targets being rewritten. Repeated firmware conflicts cause a bounded pause and a visible notice, with details in `preset-hold.log`. The normal app does not silently stop the HP service; its ownership/state and startup qualification remain separate. Details: [Custom presets](CUSTOM_PRESETS.md), [HP control](HP_CONTROL.md).

The default HP-mode and dashboard MUX shortcuts currently have weaker recovery coverage than preset transactions. Their limitations are listed in [Recovery](RECOVERY.md) and the [project review](PROJECT_REVIEW.md).

## Curves, cooling and lighting

- **GPU V/F:** open **Performance profiles → GPU → GPU performance curve → Edit**. The chart works on an in-memory draft, including a new unnamed preset. Accept returns the draft; Save persists it; Apply is separate. [Gestures and shortcuts](CURVE_EDITOR.md).
- **Fans:** **Fan curve** edits the active curve used by the own background helper. Auto pauses own regulation, Curve uses the saved curve and Max requests full cooling. A saved choice or a quick-switch message is not yet a verified physical state. BIOS handover remains pending after own regulation. [Fan control](FAN_CONTROL.md).
- **Lighting:** choose one of four zones, confirm a color in the native color dialog, or use On/Off. Confirmed changes are applied immediately and read back; Cancel does not write. The window requests UAC once. [Lighting UI](KEYBOARD_UI.md).
- **Graphics:** the dashboard offers Hybrid and GPU only boot preferences. A change requires a restart to take effect. Physical switching and iGPU-only battery operation are not yet qualified. [Hardware matrix](HARDWARE_MATRIX.md).

## Administrator rights and single instance

The dashboard runs with administrator rights. Started without them, it asks once through UAC and restarts itself; declining keeps an unelevated dashboard that asks per action as before. Sub windows started from the dashboard inherit the rights, so hardware actions no longer prompt each time. Only one dashboard, preset editor and lighting editor runs per Windows session; a second launch brings the open window forward (from the tray if hidden). Smoke scripts that drive the dashboard must run in an elevated PowerShell, otherwise the restart replaces the process they started.

## Settings window

Every setting has a round **i** that explains it. Besides language, start and close behavior:

- **Disable HP OMEN Cap** takes effect at once (no Save) and stops the HP service HPOmenCap (`OmenCap.exe`) and sets its start type to Disabled. While it runs it overwrites CPU power limits, and presets with CPU PPT or Slow are refused. The previous start type is saved; turning the setting off restores it and starts the service. Details: [HP control](HP_CONTROL.md).
- **Preset on battery** (kept with **Save**) applies the chosen mode or preset when the charger is unplugged and returns to the mode or preset last applied on AC when it is plugged in again (Balanced if none is known). A preset applied by hand on battery stays. The app must be running, the tray is enough. The change is applied through the same elevated commands as **Apply**; it is not yet validated on hardware.

## Two different Windows-start options

**Start with Windows** creates a per-user logon task (`OmenHubPP-Tray-<SID>`, highest run level) that opens the dashboard, or only its tray icon with **Start in the tray**, with administrator rights and without a prompt. Windows does not start programs that need administrator rights from the Run key, so an entry from older builds is moved to the task the first time the elevated dashboard starts. It does not apply a hardware profile.

**Activate at Windows start** in a preset authorizes a particular applied revision and uses a protected scheduled task for sign-in application. Editing the profile or replacing the installed binary invalidates that approval. Full automatic recovery/reapplication after actual sleep and power changes is still pending.

## Installation and removal

A package produced by `scripts/build-release.ps1` contains `Install.cmd`. It requests UAC, installs the app under `%ProgramFiles%\OmenHubPlusPlus` and creates user shortcuts. It does not choose a profile or install a kernel driver. A prepared CPU module does not remove the separate PawnIO runtime prerequisite.

Profiles and journals stay under the Windows token's user-profile directory, normally `%USERPROFILE%\.omen-hub-pp`. Keep them until recovery is complete. Follow [Recovery](RECOVERY.md) before updating or uninstalling when settings or controller journals remain active.
