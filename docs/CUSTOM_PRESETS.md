# Custom presets

Open **Performance profiles / Leistungsprofile** or run `omen-hub-pp.exe presets-ui [id]`. `gpu-ui` remains an alias. The editor groups settings; it does not make every catalog entry writable. Current availability: [Hardware matrix](HARDWARE_MATRIX.md).

## Workflow

1. Select a saved preset or create a new one and enter its name.
2. Expand GPU, CPU or Windows groups. Only checked settings belong to the preset. Omitting a setting does not reset an existing hardware override.
3. **Save** persists the draft. **Apply** uses a fixed elevated child and complete preflight of a saved preset. Unsupported or conflicting instructions block the whole Apply.
4. **Activate at Windows start** requires the exact successfully applied revision, live verification and protected installation/approval. An explicit startup choice during Save may request Apply followed by authorization; ordinary Save alone does not do so.
5. **Delete** archives the original JSON bytes and hash, removes the preset from selection and disables a matching sign-in selection. It keeps hardware recovery data; deletion is not restoration.

Declining to discard unsaved edits keeps the previous selection, draft and save target. Saves merge against fresh stored data and report conflicts instead of silently overwriting another editor.

## Groups

| Section | Current fields |
| --- | --- |
| GPU | Core/VRAM offsets, public numeric power limit, full V/F curve, Max-Fan, Dynamic Boost |
| CPU | Tctl limit, Slow/PPT, TDC/EDC, PBO, all-core or per-core Curve Optimizer; Intel power fields remain unsupported |
| Windows | Processor boost, min/max, EPP, core parking, CCD preference, power plan/mode, refresh rate and graphics boot preference |

Dynamic Boost uses the qualified HP PPAB flag and preserves cTGP. Both legacy names `gpu_unleashed` and `gpu_dynamic_boost` represent the same instruction; conflicting values block Apply. It is separate from the factory Unleashed mode and a numeric GPU watt limit.

The public power field stores exact milliwatts: **90.125 W → 90125 mW**, without floating-point rounding. It needs actual configured-original readback, live constraints and a setter. Those requirements currently fail on the reference laptop; storing its field is not a successful GPU-power write.

APU/iGPU menus, direct GPU/VRAM voltage, FPS limiting and fixed CPU clock/voltage were intentionally removed. Legacy data remains readable; explicit Save retires the removed instructions. Other hidden settings and complete curves survive unrelated edits. The 91-key schema is a compatibility catalog, not a list of implemented adapters.

## Curve editing

**GPU performance curve → Edit** opens the native in-memory chart for a new unnamed preset or an unsaved draft. Accept returns the full curve; Cancel discards only that chart session. Name, VRAM and other draft values remain intact. Neither action writes a profile or hardware; Save in the parent persists it.

An individual V/F curve and global core offset cannot control the same domain together. Memory/Graphics coupling is handled during Apply, not by destroying the saved curve. [Curve controls](CURVE_EDITOR.md), [GPU contracts](GPU_PROFILES.md).

The separate **Fan curve** editor saves `active-fan-curve.json` for the cooling helper. Full fan-curve instructions remain unsupported in performance-profile transactions. A Max-Fan boolean is a different, supported override.

## Authorization, recovery and validation

Sign-in permission is tied to the applied revision and installed binary hash. Editing/replacing either invalidates approval. Lighting/Max-Fan can currently be authorized, but real lifecycle qualification is pending. CPU PPT/Slow has the additional HPOmenCap ownership/startup gate. See [HP control](HP_CONTROL.md).

An unresolved transaction changes the preset action to **Restore**. Preset writes journal complete originals and verify full targets; interrupted recovery stays visible. The dashboard factory-mode/MUX shortcuts currently have separate recovery gaps. [Recovery](RECOVERY.md).

The native layout has a fixed name/selection/footer and a scrollable settings area. Fields/checkboxes retain native keyboard roles and focus scrolling. Recorded client checks at 96 DPI do not establish comprehensive DPI/accessibility support. Metadata/chart checks are separate from hardware probes: [Validation](VALIDATION.md).
