# Controlled HP migration and ownership

The product target is complete operation without original HP/OMEN user-mode controllers. Current ordinary UI/sign-in **does not automatically stop HPOmenCap or take over its lifetime**; the explicit dashboard setting below disables it on request. A prepared, explicitly opted-in native migration qualification is separate; it is not yet physically accepted for the current native owner/guardian/cooling combination.

## User setting: Disable HP OMEN Cap

The dashboard setting **Disable HP OMEN Cap** is the normal way to run without the HP controller. It runs the fixed elevated command `hp-service-ui disable|restore <nonce>`, which only accepts the `HPOmenCap` user-process service whose image is `OmenCap.exe` from an `hpomencustomcapcomp.inf_amd64_*` DriverStore package (any package version, so a Windows Update of the component keeps working). Disabling saves the current start type, sets it to Disabled and stops the service. Restoring sets the saved start type back (Automatic or Manual, otherwise Automatic) and starts it; it is refused while a hardware transaction still needs restoring. An open developer handover session blocks both. No other service, driver or configuration field is touched.

While the service is stopped and Disabled, CPU PPT/Slow presets apply and may also be approved for sign-in, because Windows will not start the service at logon. The BIOS keeps its own thermal protection; the own fan helper controls the fans when Curve or Max is chosen. A Windows Update of the HP component can enable the service again; the checkbox then shows it as not disabled. This setting has not yet been run on the device.

## Normal profile gates

A preset containing independent CPU **PPT or Slow** fails before capture/SET if HPOmenCap is running, transitioning or unreadable. Each power write rechecks the condition. A missing service is accepted only for the explicit Windows “service does not exist” result; the app does not install/start an absent OEM service.

New PPT/Slow sign-in approval requires the OEM service to be absent, or stopped with start type Disabled, until startup takeover is qualified. A temporarily stopped auto-start service is insufficient. Existing hardware-original recovery remains available and must finish before an owned OEM return. Read-only status/preflight changes no selection, service or hardware.

## Fixed native qualification

The developer-only `hp-control-qualify --confirm-service-test` path binds an identically installed protected app and own cooling helper. It can stop only the measured **HPOmenCap user-process service**, not arbitrary services or drivers. The model is **8E35/F.13/8940HX**, image SHA-256 **`9EECCBDD7BEAA9C33930551853A572AB1E56E2E9A72F6453DC53E190284FE0C8`**. Exact fixed DriverStore path, service type, account, auto-start configuration, dependencies, file digest and live process creation identity are checked.

Qualification requires known competing tuners/original Hub closed, AC, fresh CPU Tctl below 80 °C, GPU below 75 °C, IR below 50 °C and both fans at least 4000 RPM after own Max is established. It reads six bounded cooling/source samples and restores original responsibility; it does not tune CPU/GPU voltage/power or change service start types. This is an actual service/cooling experiment, not a routine verification command.

The session is limited to 60 seconds with an independent return guardian. This deadline is not a supported long-term ownership policy. Windows synchronous service control can itself block longer than the subsequent status-observation budget; the ten-second polling budget is not a ten-second action guarantee.

## Durable records and return ordering

The protected installation keeps distinct `controller-ownership.json` and `controller-cooling.json` records. Before choosing own Max, cooling responsibility records exact original preference bytes plus owner, controller, helper hash/PID/creation identity. Existing unresolved/corrupt records block another takeover. Completed records are archived.

Readiness requires a live exact elevated fan helper, valid receipt/task/action/user/session, fresh Max output and an actual heartbeat after preparation. A saved status file, reused PID or normal dashboard is insufficient. The helper's existing task must allow the intended lifetime and battery continuation. Its hardware packets remain serialized; contention is not reported as fresh maximum output.

Return first revokes further acquisition, then confirms the original service state, then restores the original cooling preference. A matching existing own helper task may be resumed after exact verification; it must confirm maximum before cooling is lowered. A valid external choice is preserved as superseding the original. Unresolved hardware/service recovery does not authorize lowering cooling. Both responsibilities must finish before the guardian exits.

Installers block update/removal while ownership or cooling recovery is active/unknown. The fan helper must end normally; there is no forced-upgrade fallback that silently removes responsibility.

## Independent return guardian

The own guardian is bound to exact executable, creator PID/creation time, user, session nonce and controller identity. It acknowledges readiness before STOP and is checked immediately before/after STOP. Its channel has fixed bounded frames and lifetime semantics.

It can request **START only** for the fixed original service. It cannot stop services, select another target, install a driver or alter configuration. Owner death, lost lifetime channel or qualification expiry can return only the matching owned session. Foreign/new sessions and unknown identity remain untouched. Pending hardware recovery prevents a competing OEM start; errors and original records remain visible.

Job independence uses the explicit Windows process/job contract. A prohibited independent start refuses takeover; the product has no WMI launch fallback. Only the separate file-simulation test executable uses a local WMI escape from the restrictive test job. `controller-guardian-last.json` is a historical outcome report, not live service state.

## Inspection and recovery

```powershell
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" hp-control-status
```

This is read-only. Unelevated full identity audit can correctly return access denied. It must not be labeled successful.

For an actually interrupted own session, the explicit elevated `hp-control-recover` checks the protected same-build installation, valid records and owner death before return. It may finish cooling return after service return. Unknown/active owner, foreign identity or unresolved hardware state blocks it. See [Recovery](RECOVERY.md) before running it.

## Evidence and remaining work

14 process/file scenarios and installer guards test owner loss before STOP, loss after service return, failed return, helper failure and external preference changes. They simulate service/cooling state; they do not prove physical RPM or service behavior.

The separately authorized 25 October PowerShell isolation retained the user's 70/75-W targets for roughly 12 seconds, then restored complete originals and metadata. It supports service influence but does not qualify this later native takeover or demonstrate sole cause/durable independence.

The next supervised native qualification, durable owner lifetime, UI/sign-in integration, safe BIOS return, real power transitions and total memory remain open. [Replacement plan](REPLACEMENT_PLAN.md), [Validation](VALIDATION.md), [Roadmap](ROADMAP.md).
