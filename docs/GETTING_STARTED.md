# Getting started

Omen Hub++ is an experimental Windows x64 application. Start with the [hardware matrix](HARDWARE_MATRIX.md): current hardware-control evidence is specific to HP 8E35/F.13, Ryzen 9 8940HX and RTX 5070 Laptop. A different BIOS, GPU implementation or driver can make a control unavailable.

The source version is `0.9.0-dev.2`. The first public beta was withdrawn. Follow [Building](BUILDING.md) to create a development build; do not treat an old local EXE as the current source version.

## Launch

From a source checkout after building:

```powershell
.\target\release\omen-hub-pp.exe
.\target\release\omen-hub-pp.exe --tray
```

The second command starts hidden. Open the app from its tray icon; the tray menu also provides a full exit. Windows may put the icon in its overflow area. The app retries registration after Explorer restarts and keeps the window accessible if tray registration fails.

The dashboard puts GPU watts and temperature first, with CPU measurements beside them. Missing or stale readings show unavailable. CPU package watts and Tctl require the optional embedded module and an already running, exactly verified PawnIO 2.2.0 driver; see [CPU sensors](CPU_SENSORS.md). Windows-reported MHz is a separate OS value.

The first visible dashboard requests UAC for a session-bound CPU reader. Canceling does not grant access; **Refresh** is the explicit retry. Tray/suspend pauses the reader and releases hardware sessions. Returning begins with fresh measurement baselines.

## Performance profiles

The dashboard lists Quiet, Balanced, Performance and Unleashed defaults alongside saved custom presets. Selecting an entry does not apply it; **Apply** requests the relevant Windows approval.

Open **Performance profiles / Leistungsprofile** to create or edit a custom preset:

1. Select a preset or create a new one and enter its name.
2. Choose the GPU, CPU and Windows settings to include. An unchecked setting is omitted; it does not reset an existing override.
3. **Save** stores metadata. **Apply** changes supported settings through the transaction backend.
4. Optionally authorize the exact successfully applied revision for Windows sign-in. Saving or editing a preset alone does not authorize it.

One unsupported instruction blocks the entire preset. In particular, PPT/Slow profiles are blocked while HPOmenCap is active or its state is unknown. New sign-in approval for these CPU power settings also requires the OEM service to be absent until startup handover is qualified. The normal app does not automatically stop it. Details: [Custom presets](CUSTOM_PRESETS.md), [HP control](HP_CONTROL.md).

The default HP-mode and dashboard MUX shortcuts currently have weaker recovery coverage than preset transactions. Their limitations are listed in [Recovery](RECOVERY.md) and the [project review](PROJECT_REVIEW.md).

## Curves, cooling and lighting

- **GPU V/F:** open **Performance profiles → GPU → GPU performance curve → Edit**. The chart works on an in-memory draft, including a new unnamed preset. Accept returns the draft; Save persists it; Apply is separate. [Gestures and shortcuts](CURVE_EDITOR.md).
- **Fans:** **Fan curve** edits the active curve used by the own background helper. Auto pauses own regulation, Curve uses the saved curve and Max requests full cooling. A saved choice or a quick-switch message is not yet a verified physical state. BIOS handover remains pending after own regulation. [Fan control](FAN_CONTROL.md).
- **Lighting:** choose one of four zones, confirm a color in the native color dialog, or use On/Off. Confirmed changes are applied immediately and read back; Cancel does not write. The window requests UAC once. [Lighting UI](KEYBOARD_UI.md).
- **Graphics:** the dashboard offers Hybrid and GPU only boot preferences. A change requires a restart to take effect. Physical switching and iGPU-only battery operation are not yet qualified. [Hardware matrix](HARDWARE_MATRIX.md).

## Two different Windows-start options

**Start with Windows** launches the unelevated tray UI through the user's Run entry. It does not apply a hardware profile.

**Activate at Windows start** in a preset authorizes a particular applied revision and uses a protected scheduled task for sign-in application. Editing the profile or replacing the installed binary invalidates that approval. Full automatic recovery/reapplication after actual sleep and power changes is still pending.

## Installation and removal

A package produced by `scripts/build-release.ps1` contains `Install.cmd`. It requests UAC, installs the app under `%ProgramFiles%\OmenHubPlusPlus` and creates user shortcuts. It does not choose a profile or install a kernel driver. A prepared CPU module does not remove the separate PawnIO runtime prerequisite.

Profiles and journals stay under the Windows token's user-profile directory, normally `%USERPROFILE%\.omen-hub-pp`. Keep them until recovery is complete. Follow [Recovery](RECOVERY.md) before updating or uninstalling when settings or controller journals remain active.
