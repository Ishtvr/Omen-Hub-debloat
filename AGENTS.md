# Omen Hub++

Product name: **Omen Hub++**. Technical Cargo names use `omen-hub-plus-plus` and `omen-hub-pp`. Preserve the user's Windows-first, native, small-footprint, offline requirements.

The user raised the tray-idle working-set budget to **50 MB** on 13 October 2026. Prioritize GPU wattage and temperature before profile controls, with CPU performance alongside it in the first view. CPU package power and temperature must remain unavailable until a verified hardware reader exists; Windows-reported MHz is not a measured boost clock. The UI workflow is code-first; use the confirmed product record in PRODUCT.md.

On 14 October 2026 the user clarified that the **open window may exceed 50 MB** when this improves the UI; the budget remains for total tray idle, including helpers after sensor activation. The requested appearance is **modern, clean and dark**, with clear fields and prominent actions.

Omen Hub++ must eventually replace the original OMEN Hub and its hardware-controller processes completely. Original OMEN Hub modules are reverse-engineering sources, not mandatory runtime dependencies for the target engine. Prove the necessary lifecycle and independent transport before claiming replacement support.

- Keep documentation truthful about implemented features versus plans. Never imply that a profile preview changed firmware.
- Hardware write support requires evidence for the exact board/BIOS, validated payloads and readback/restore behavior. Do not guess WMI opcodes, EC offsets or driver structures.
- Keep proprietary DLLs, drivers, decompiled sources and local diagnostic dumps in ignored `.local/` or `.tools/`. Commit only original code and concise interface observations.
- Do not stop/remove vendor services, install a kernel driver, flash firmware, or change power/voltage as part of ordinary verification.
- Keep core policies independent of Windows. No network calls, telemetry, updater or embedded browser in runtime code.
- App UI supports German and English. Use language keys for user-facing text and stable codes for control-flow signals, never translated prose. Keep the tone professional with occasional familiar PC terms (Boost, Undervolting, Preset, Idle); preserve units, exact numeric values and the distinction between selected and confirmed hardware state. Follow docs/LOCALIZATION.md.
- The TGP experiment must satisfy the user's 26 October requirement: user-mode application processes (elevation allowed), with TPM and Secure Boot enabled. Test-signing or disabled driver-signature enforcement is not an acceptable product path.
- Use public NVAPI/NVML contracts for telemetry. Optional FFI libraries must use exact ABI, validated lifetimes and explicit failure reporting.
- Run `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and a release build for implementation changes.
- For hardware support changes, update the model evidence and measured validation record. No multi-agent delegation is required.

Documentation entry point: [docs/README.md](docs/README.md). Keep the public README concise and in English. The hardware matrix owns current support, VALIDATION.md owns the current check summary, ROADMAP.md owns pending work, and docs/history/ preserves dated observations. Startup permission is distinct from measured lifecycle qualification; documentation edits must not silently change runtime permissions.
