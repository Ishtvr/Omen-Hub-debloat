# GPU-TGP investigation plan

Documentation reviewed **29 October 2026**. Technical observations below come from the **26 October** read-only investigation, Omen Hub++ source through `251fff6`. **Current scope is investigation and planning.** No new backend, hardware SET or reference GitHub fork has been produced. A local research worktree exists; it is not an upstream fork or implemented writer.

## Objective and hard requirements

Investigate whether an original, coherent and fully restorable GPU-TGP control can be reached through existing NVIDIA/OEM interfaces on **our HP OMEN**. Do not assume that a method for a 5070 Ti applies to the non-Ti GPU.

- App processes must run in user mode; UAC/admin is allowed. **TPM and Secure Boot remain enabled.**
- No test-signing, disabled signature enforcement, added test-certificate trust or security bypass as the product path.
- Prefer original Rust over an existing supported driver interface. A narrowly scoped licensed C/C++ user-mode shim is a fallback, not a way to turn kernel patching into user mode.
- Preserve offline/native runtime, resource budget, exact model/build binding and complete original/target/restore evidence.
- Do not install/start foreign driver modules, execute upstream tuners or issue guessed control packets during this investigation.

## Pinned upstream reference

[LevinAi-arch/rtx-5070ti-laptop-160w-power-limit](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit) was inspected at the following identities. These statements describe that inspection, not a fresh upstream check on every documentation edit.

| Reference | Observed result |
| --- | --- |
| `main` and `v2.3.0` | Commit **93f55e954e4dc3db1aefe5fb3bd4f0a1f8d283c3** |
| `v1.8.0` | Commit **ab8d6d0a3b05245dd6ad87afa29943337f06f2d5** |
| [v2.3.0 release](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit/releases/tag/v2.3.0) | Author claims user-mode control without their custom unsigned kernel driver, using a Lenovo/5070 Ti reference. No GitHub assets; refers to paid Pro. The examined public tree did not expose a separate new user-mode TGP backend. |
| Public v1.8 unified ZIP | SHA-256 **DB42DD704B5DF477CB309BFAAFB27CFA0A691D0747509D93FD6C1D183D82BA37**, includes source and prebuilt files |
| Root “v1.8.0 RTX 40/50” ZIP | SHA-256 **3CB163D54ADEB314A2EDFC5C2277CBFE21A473CE5070CE369C54FA3EB15095DD**, binary package without source |
| Version consistency | Release/archive/current source files differ; SYS blobs also differ. Do not assume every included binary is a reproducible build of one source set. |
| Reuse status | No explicit reuse license was found in the inspected tree/archive/source headers; repository metadata reported null license. A public fork does not by itself establish permission to copy/port code into this MIT runtime. |

The recent author claim is a research lead, not proof of compatibility. Access to Pro source is **not a prerequisite** for independent investigation. No purchase/contact/private download occurred. Reference source/archive copies stay ignored in `.local/research/nvpwrcontrol/`; only original concise observations belong in Git.

## What the old kernel path actually does

The examined v1.8 driver binds NVIDIA **616.92**, specific image signatures/layouts and kernel objects. It changes coupled base/cTGP, boost amount, UPPER/MAX and invokes an internal generator to establish current policy. It uses a fixed **25-W** boost split and session-specific originals. Driver unload explicitly does **not** restore automatically.

The pinned low-power non-Ti 5070 branch expects a coherent **115-W** baseline and accepts **120–140 W** in 5-W steps. The Ti branch has a different baseline/range; README tables conflict with parts of the pinned implementation. Neither range is our qualification. The separate XMG high-power contract is not our HP backend.

The shared `\\.\Nvpwr` interface has status v7 and SET v2, with status/set/restore operations and a 16-byte SET request. Its GUI ensures a kernel service, so running it for “status” is not assumed passive. The old driver was not loaded. Its addresses/pointers cannot be called directly from user mode or reused across driver sessions.

## Target and observed public values

| Property | Reference |
| --- | --- |
| Model | HP OMEN 16-ap0xxx, **8E35 / F.13** |
| GPU | RTX **5070 Laptop**, non-Ti, **10DE:2D18**, subsystem **103C:8E35** |
| NVIDIA / VBIOS | **617.42 / 98.06.2a.40.da** |
| Public power values | Default **80 W**, allowed **5–115 W**, separately effective **115 W** |
| Missing original | Configured/requested limit unavailable through current public queries |

Effective/default values are not the configured original and do not prove draw, a 25-W internal split or a 160-W capability. Actual TPM/Secure-Boot state was not newly qualified during the research; retaining them enabled is a required future acceptance condition.

## Candidate approaches

| Approach | Existing evidence | Next question | Position |
| --- | --- | --- | --- |
| **A. Trace existing NVIDIA control transport** | Exact 617.42 Ghidra path includes an administrative `\\.\NvAdminDevice` branch and `DeviceIoControl`; DOS device entry visible | Which policy operations select this branch, with what access rights, packet contract and matching handler? | Highest research priority; **not a proven TGP writer**. Device was not manually opened or sent a control packet. |
| **B. Private NVAPI policies** | Info/Status interfaces and setter statically present; validated GET frames callable | Does privilege/model selection or another offered policy class provide actual originals/bounds? What is the watt relationship? | Classic non-elevated v1/v2 calls returned empty policy availability, despite status 0. |
| **C. Modern public NVML** | v2 setter export present; some public limits/scopes readable | Is a correctly scoped configured original and full write/restore contract available? | Preferred documented API if complete; export presence alone does not solve missing original or extend OEM bounds. |
| **D. HP WMI / NVPCF budgets** | Own mode/cTGP/PPAB/shared-budget calls exist | Is there an actual numeric GPU-budget contract on 8E35, beyond fixed modes/preferences? | OEM fallback/complement; CPU limit, boost preference or AC/P-state change is not free TGP. |
| **E. New Nvpwr user-mode reference** | Author's v2.3 claim | Can legitimately available source/interface evidence explain its mechanism and applicability? | Optional comparison, no purchase/source dependency and no established HP support. |
| **F. Normally Windows-accepted signed kernel helper** | A user-mode app can technically call an approved backend | Additional kernel component, current signing/admission policy and independent lifetime/device evidence needed | Architectural reserve; **not pure user-mode reproduction**, not current implementation. Self-signing or Rust syntax does not satisfy it. |

A/B may converge on the same vendor path. Public [NVIDIA open kernel headers](https://github.com/NVIDIA/open-gpu-kernel-modules) are naming/semantic references, not proof of the Windows ABI. No examined public power header alone supplied our complete numeric TGP contract. [Microsoft user/kernel distinction](https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/user-mode-and-kernel-mode); [Windows driver policy](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy).

## Read-only results already obtained

- Resolved private power GETs belonged to implementation SHA-256 **C6A2014763C3662512B5D420DFC7632B777338881A13AEAB3F21D8033EAE6C71**. Live entrypoints matched the analyzed interface mapping.
- Info ID **0x34206D86**: v1 **184 bytes / 0x100B8**; v2 **2248 bytes / 0x208C8**. Status ID **0x70916171**: v1 **72 bytes / 0x10048**. Guard regions remained intact.
- As a **non-elevated user**, Info v1/v2 returned status 0 with no valid policy entries; Status v1 returned status 0/count 0. This supplied no original or usable target interval. Percent-mille fields cannot be relabeled watts.
- Setter ID **0xAD95F5ED** was analyzed statically, never invoked. Privilege/filter meanings are not inferred merely from a helper flag.
- Tracing showed a dispatcher/administrative transport branch, but did not prove our GET probes selected it or that old direct kernel effects are exposed through it.
- `NvAdminDevice` visibility was checked through the DOS device namespace only. The public NVML v2 setter was checked for export presence only. **No SET** was performed; no services or boot security settings were changed.

Raw GET evidence and native decompilation remain ignored locally. App hardware implementation was unchanged.

## Map effects to actual contracts

| Old kernel effect | Required user-mode evidence |
| --- | --- |
| Change base and dynamic amount | Actual originals, units, offered model bounds; no assumed fixed 25-W split |
| Change upper/MAX coherently | Valid coupled operation through an allowed interface, not direct kernel addresses |
| Commit/generate current policy | Proven eligibility/commit effect, not cached UI state |
| Verify current state | Distinguish configured/requested/effective/limiting components; empty lists/defaults are not snapshots |
| Restore completely | Same GPU/session identity and all own modified components; process exit/unload is not restore |

## Immediate next research

1. Finish transport/rights/handler attribution. Plan a standard-user versus administrator **read-only** comparison using only proven frames. No blind IOCTL fuzzing.
2. Examine offered policy classes and model/privilege filtering; the classic v1/v2 family is currently empty.
3. Resolve NVML scopes/units/original values and separate numeric GPU control from OEM performance preferences.
4. For each candidate record entrypoint, direction, privileges, device/build binding, snapshot, restore, boot compatibility and unknowns.
5. Decide on a prototype only after a readable original and plausibly corresponding allowed write/restore contract exist. Otherwise record a negative result without claiming a working slider.

## Later implementation, after that decision

1. Pin the independently evidenced contract and any required reuse permission. A fork and an available binary are not a code-integration license. [GitHub licensing guidance](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository).
2. If useful, create the authorized reference fork, preserve provenance and isolate actual integration in a `codex/` branch/worktree based on then-current app source. No upstream binaries go into normal packages.
3. Build a read-only Rust diagnostic for exact GPU/driver/session identity and all available original components. Verify with TPM/Secure Boot enabled before planning a write.
4. Keep core GPU-policy snapshot/validation separate from the narrow Windows transport/FFI. Use a small licensed C-ABI shim only if necessary. Keep the existing public NVML adapter separate. Never accept arbitrary raw driver commands from UI/profiles.
5. Test ABI/state fixtures first: bad versions, mixed state, lost replies, foreign changes and restore failure. Plan the first physical change **within the OEM envelope**, preferably a small decrease, with full immediate restore. The old above-115-W writer is unsuitable for that trial. Even an unchanged total is not a no-op if its base/boost split changes.
6. Only after successful full restore, integrate presets, sign-in, crash, sleep/resume and GPU/driver loss. Consider any above-OEM scope separately with supply/thermal/load evidence; missing VRM/VRAM/hotspot data must stay unknown. Repeat whole-helper tray acceptance and normal Cargo checks.

**Conclusion:** a useful research lead exists, but no compatible alternate writable backend has been established. Continue investigation; Rust/C++ implementation and new hardware writes remain deferred under the current user instruction.
