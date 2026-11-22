# Custom presets

Open **Performance profiles / Leistungsprofile** or run `omen-hub-pp.exe presets-ui [id]`. `gpu-ui` remains an alias. The editor groups settings; it does not make every catalog entry writable. Current availability: [Hardware matrix](HARDWARE_MATRIX.md).

## Workflow

1. Select a saved preset or create a new one and enter its name. A new preset offers only **Reset** and **Save**; Apply, Delete and the sign-in option appear once it is saved (a pending hardware restore always shows **Restore**). **Reset** returns the draft to the system defaults: no setting selected and every editor at its default value. It changes only the draft.

Unticked fields show this device's own values. Volatile firmware and driver values (CPU PPT/Slow/TDC/EDC/Tctl, PBO, all-core CO, GPU offsets) are read when the editor opens while no Omen Hub++ control has run since boot, and stored per board/BIOS in `device-defaults.json`; fields without such a reading show a fixed seed. Reset uses the same values and the driver's factory V/F curve (all custom deltas removed).
2. Expand GPU, CPU or Windows groups. Only checked settings belong to the preset. Omitting a setting does not reset an existing hardware override. On/off fields (cTGP, Dynamic Boost) have no separate selector: the tick is the value (ticked = on, unticked = off), they always belong to the preset and Apply always writes their state (user decision, 30 October 2026). New and reset presets start with cTGP on and every other on/off field off. A saved preset without such a field shows its default and marks the draft unsaved; Save stores it, Apply follows Save.
3. **Save** persists the draft. **Apply** uses a fixed elevated child and preflight of a saved preset. Since 30 October 2026 (user rule: the app overwrites, it does not block), only a different device (board/BIOS/CPU, GPU identity for NVIDIA settings), a value beyond the reported range or missing hardware/driver support stop the whole preset. Pending validation, AC, sign-in use and unknown ranges no longer block. A value that changed outside the app before its write becomes the restore original (journal updated, reported as adopted) and is then overwritten. A value whose write or readback fails after one more attempt is returned to its original alone and reported ("Preset applied, except: …"); the others stay applied, and "Keep preset" retries it with back-off. An unfinished earlier journal is moved to `control-history` instead of blocking. Restore writes every original back even when a value changed meanwhile, reports what it could not return and closes the entry. Validation trials stay all-or-nothing. Saving a changed sign-in preset applies and approves it again.
4. **Activate at Windows start** requires the exact successfully applied revision, live verification and protected installation/approval. An explicit startup choice during Save may request Apply followed by authorization; ordinary Save alone does not do so. An update installed with the setup moves an existing approval of this user to the new build when the approved build was the one installed before; the approved preset revision and device binding stay unchanged.
5. **Delete** archives the original JSON bytes and hash, removes the preset from selection and disables a matching sign-in selection. It keeps hardware recovery data; deletion is not restoration.

Declining to discard unsaved edits keeps the previous selection, draft and save target. Saves merge against fresh stored data and report conflicts instead of silently overwriting another editor.

**Cancel editing** is available beside Reset and in the GPU editor. It restores
the currently loaded preset's last saved baseline, including name, per-core values,
curve, sign-in checkbox draft and fan snapshot. It does not reset defaults, apply
hardware, save files or alter the existing sign-in grant. Reset continues to mean
defaults. An unnamed new preset returns to its initial loaded draft.

## Groups

| Section | Current fields |
| --- | --- |
| GPU | Core/VRAM offsets, full V/F curve |
| Dynamic Boost and CPU power | Dynamic Boost, cTGP, TPP slider (55–72 W), CPU power slider = Slow/PL1 (45–80 W with Dynamic Boost, otherwise 15–160 W) |
| CPU | Tctl limit, PPT, TDC/EDC, PBO, all-core or per-core Curve Optimizer |
| Windows | Processor boost, min/max, EPP, core parking, CCD preference, power plan/mode, refresh rate and graphics boot preference |

Three independent GPU power fields (user decisions, 30 October 2026): **cTGP** (`0x22` byte 0, on by default for new and reset presets) and **Dynamic Boost (PPAB)** (`0x22` byte 1) are transactional HP firmware flags (WMI, no NVIDIA identity in the preset binding and no wake of the discrete GPU); each write keeps the other flag as read (read-modify-write, fixed D-state 1 and 87 °C byte), reads both back and restores both captured flags. **TPP** (`0x29` [FF, FF, FF, watts], shared CPU+GPU budget, factory 45 W, Unleashed 72 W, the firmware accepted up to 115 W) has no readback: it is written before the transaction, is never reported as confirmed and is not restored. Ticking Dynamic Boost in the editor proposes cTGP on and TPP 72 W.

**Dynamic Boost switches the HP mode** (user decision, 30 October 2026; [load measurement and NPCF table](models/8e35.md#gpu-evidence)). Before the transaction, Apply runs the Hub's full mode sequence: Dynamic Boost on selects Unleashed (`0x04`; GPU limit about 100–112 W under load), on battery Performance (`0x31`) because the Hub offers no Unleashed there; off selects Balanced (`0x30`; 80 W, where Dynamic Boost would enforce 65 W and cTGP/TPP have no effect). Then the preset's TPP (without one, the mode's budget remains, 72 W in Unleashed), then the transaction with the preset's flags, V/F curve, offsets, CPU limits and Windows values. Every preset value therefore applies; CPU values it leaves out follow the mode. The mode also changes the fan behaviour, has no GET and is not undone by Restore. A preset without a Dynamic Boost field leaves the mode unchanged. With **Keep preset** on, a disposable child process of the dashboard reads the NVML enforced limit every 5 s (30 s once stable; not on battery or while the dGPU is powered down); two locked readings under load (limit ≤ 80 W, draw within 5 W of it) write the mode payload and the preset's TPP again, at most three times per applied revision, each logged in `preset-hold.log`. The dashboard shows the last applied preset instead of a mode. The HP performance mode is not a preset field; legacy entries are discarded on Apply and retired on Save. The dashboard modes Balanced, Performance and Unleashed set cTGP on, Quiet sets it off (Balanced deviates from the Hub, which leaves cTGP off). On 29 October Apex ran at a 65 W enforced GPU limit with cTGP off and about 107–109 W with Unleashed. Both legacy names `gpu_unleashed` and `gpu_dynamic_boost` represent the same instruction; if their values differ, `gpu_dynamic_boost` wins. It is separate from the factory Unleashed mode and a numeric GPU watt limit.

The public NVML power field is hidden from the editor: the driver refuses its GET/SET on the reference RTX 5070 Laptop (status 3, not supported). Legacy entries stay readable and are retired on explicit Save. Max-Fan is not a preset field; it stays a dashboard switch.

APU/iGPU menus, direct GPU/VRAM voltage, FPS limiting and fixed CPU clock/voltage were intentionally removed. Legacy data remains readable; explicit Save retires the removed instructions. Other hidden settings and complete curves survive unrelated edits. The current 81-key catalog is not a list of implemented adapters. Intel controls have been removed; a separate decoder preserves ten retired keys in old files until explicit Save. Opening an old file does not rewrite it or silently authorize the remaining subset. Apply is blocked until the retired entries are removed.

## Curve editing

**GPU performance curve → Edit** opens the native in-memory chart for a new unnamed preset or an unsaved draft. Accept returns the full curve; Cancel discards only that chart session. Name, VRAM and other draft values remain intact. Neither action writes a profile or hardware; Save in the parent persists it.

Overlapping instructions do not block a preset (user decision, 29 October 2026). Apply uses a fixed precedence: a full V/F curve wins over a global core offset, per-core Curve Optimizer over all-core, and a non-Balanced power plan over the Windows power mode. A power mode without a plan selects Balanced, whose original is captured and restored like any other setting. The saved preset keeps every value; only the applied set skips the overridden ones. Numeric limits are the driver's or adapter's reported ranges, for example NVML core −1000…+1000 MHz and VRAM −2000…+6000 MHz on the reference RTX 5070 Laptop. Memory/Graphics coupling is handled during Apply, not by destroying the saved curve. [Curve controls](CURVE_EDITOR.md), [GPU contracts](GPU_PROFILES.md).

Under **Cooling**, save a named fan preset, then choose it in the performance
editor's **Fan preset** field. Saving the performance profile copies the points
into its existing CPU/GPU fan-curve fields. Later library edits or deletion do not
change that snapshot. **Unchanged** leaves the current global cooling choice alone.
An existing unmatched snapshot remains selectable as a saved curve.

Linked profiles use a separate request-level coordinator: capture exact cooling
preference originals before the HP prelude, verify the complete direct subset,
then request the shared curve from the installed helper. Failure retains/recovers
originals and reports the direct and cooling outcomes separately. No exact helper
curve-digest acknowledgement exists, so this is not a fully verified physical
fan-curve transaction. For linked profiles, enabling sign-in is rejected
before saving or changing permissions. Fan-only curves are requested in Cooling.
The separate Max action remains available there. [Fan control](FAN_CONTROL.md).

## Authorization, recovery and validation

Sign-in permission is tied to the applied revision and installed binary hash. Editing/replacing either invalidates approval. Lighting/Max-Fan can currently be authorized, but real lifecycle qualification is pending. Preset values take precedence over other apps (user rule, 30 October 2026): CPU PPT/Slow are written even while HPOmenCap runs, and "Keep preset" writes them again when another controller or the firmware changes them. See [HP control](HP_CONTROL.md).

An unresolved transaction changes the preset action to **Restore**. Preset writes journal complete originals and verify full targets; interrupted recovery stays visible. The dashboard factory-mode/MUX shortcuts currently have separate recovery gaps. [Recovery](RECOVERY.md).

The native layout has a fixed name/selection/footer and a scrollable settings area. Fields/checkboxes retain native keyboard roles and focus scrolling. Recorded client checks at 96 DPI do not establish comprehensive DPI/accessibility support. Metadata/chart checks are separate from hardware probes: [Validation](VALIDATION.md).
