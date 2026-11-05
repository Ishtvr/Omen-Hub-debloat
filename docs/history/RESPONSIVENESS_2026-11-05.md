# CPU performance and control responsiveness — 5 November 2026

This record concerns the local working tree based on `2efca798ed58367323b4e562b39832126949f0ca`, preserving the existing uncommitted native UI, tuning, localization, battery, installer and controller work. The new local candidate is `0.9.0-beta.3`. Nothing was reset, and no profile or recovery original was edited for this investigation.

## Observations

The previously running dist copy was `0.9.0-beta.2`, SHA-256 `F6F6C92191118549755E710697DC67EB1BBA25F5A30590620AB23964AF4D9441`. Both installed executables also reported beta.2. The current-user desktop shortcut already targeted dist but had no `--show` argument, so a first launch could honor the saved tray preference instead of opening the dashboard.

The last manual Apply result identified `preset-18e32c78c2f704f02f50`, digest `8e81a4a89e4d3d947f147795776b65f46e1f2ad3a943aaf91f9c67bcbef3d2f1`, and reported successful readback of 65 W Slow, 80 W PPT, 85 °C Tctl limit and Windows Efficient Enabled boost. An 80 W PPT request is not an 80 W sustained-demand guarantee. The saved preset also contains the separate unconfirmed HP shared budget and a Windows power mode superseded by its High Performance power-plan choice.

A read-only probe with that exact beta.2 reader took three samples at 23:42:08–17 UTC on 4 November (00:42 on 5 November in Berlin). All three PM tables were version `0x540208`, cell 2 = 80 W PPT, cell 4 = 65 W Slow, cell 10 = 85 °C Tctl limit. Separately measured AMD package power was 66.5 / 66.1 / 66.6 W and Tctl was 82.5 / 80.5 / 77.25 °C. This is a short current-state observation, not proof of sustained enforcement or reproduction of the reported 1,700 MHz under full load. APERF/MPERF acquisition exceeded its timing bound in these standalone reads, so effective clocks were unavailable. Windows-reported MHz must not substitute for them.

The active Windows plan was High Performance, AC boost index 3 (Efficient Enabled), AC maximum processor state 100%, AC EPP 0 and maximum-frequency policy 0 (no configured MHz cap). HPOmenCap was already Stopped/Disabled; this investigation did not change its state. TPM was present/ready/enabled and Secure Boot enabled. No Apply, power/voltage change, service stop, driver installation or firmware flash was performed. The two running dist processes were explicitly changed to Windows **High** priority and read back as High, as requested.

## Corrected causes

- Main-window startup synchronously queried Task Scheduler merely to obtain local display preferences; legacy-startup migration also ran before window creation. Startup now reads the local preferences and moves migration off the window thread. Explicit `--show` overrides tray startup.
- Dashboard controls shared their worker with inventory, CPU and GPU reads. A bounded-but-slow sensor call could delay Apply. Manual controls now have a separate sleeping worker and result mailbox; battery results cannot unlock a pending manual control.
- The hardware lease used only a zero-timeout acquisition. A short background rewrite could reject a manual Apply. Explicit commands now wait up to 15 seconds per mutex on their worker, and a named foreground intent makes the preset keeper yield. No in-flight writer is terminated.
- The preset keeper read the old journal/revision before acquiring its write lease. It now rechecks the result nonce, digest, journal, preference and live target under the same lease as the write. Only the Apply result's verified settings can be held; skipped recovery requests cannot become background write targets. Repeated conflicts remain bounded and are surfaced in the dashboard as well as the existing log.
- `verify_applied` filtered out failed target reads and could return success with only a partial matching set. Verification now fails on any missing or mismatching target. Apply also performs a final read-only verification after corrections, includes late correction writes in rollback accounting and verifies originals of skipped settings.
- The HP mode/TPP prelude ran before pending-recovery and profile preflight checks. Both manual and sign-in application now check first and reuse the controller's fresh inventory. TPP still has no exact hardware readback/restore contract; new results and UI state explicitly identify it as unconfirmed.
- Every new app process requests and reads back `HIGH_PRIORITY_CLASS`. Realtime is never requested. This changes scheduling, not CPU thermal/power limits or boost policy.

## Validation boundary

The full software checks and local deployment identity are recorded in [Validation](../VALIDATION.md). Native GUI checks do not apply a hardware preset. No new full-load, reboot, cooling-ownership or HP-independent lifecycle qualification is implied. The protected installed fan helper and preset sign-in authorization are separate from the dist dashboard and must not be overwritten as part of an ordinary GUI update.
