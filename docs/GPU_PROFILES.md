# GPU controls

The private-control reference is HP **8E35/F.13**, RTX **5070 Laptop**, VBIOS **98.06.2a.40.da**, with exact qualified NVIDIA **617.14** or **617.42** implementations. Another driver or readable ABI does not automatically grant writes. [617.42 evidence](NVIDIA_61742.md), [hardware matrix](HARDWARE_MATRIX.md).

## Offsets and V/F

- Public NVML core and memory offsets use the fixed 24-byte v1 clock-offset ABI, Graphics/Memory domains and P0. Small negative/positive probes verified exact target readback and complete restore.
- Private Graphics V/F uses full current masks, anchors, deltas and ranges. All **127 measured voltage anchors** stay immutable; only frequency targets change. Live implementation hashes and complete images are checked.
- A nonuniform or independent V/F curve blocks a global core offset. Profile intent cannot contradict itself.
- A memory offset SET can clear Graphics tuning on the measured driver. The controller captures the full coupled Graphics state, writes Memory first, then establishes the final Graphics target and checks the entire result.

The curve is useful for frequency-at-voltage tuning; it is not a direct mV offset, hard physical voltage ceiling or stability guarantee. [Curve editor](CURVE_EDITOR.md).

## Dynamic Boost and factory modes

The preset's **Dynamic Boost** controls HP's PPAB flag, with readback and original restore; cTGP stays as found. Legacy `gpu_unleashed` and `gpu_dynamic_boost` names are aliases, not two independently adjustable functions.

Quiet/L0, Balanced, Performance and Unleashed factory modes use fixed model-derived HP payloads. Their GPU flags and available CPU Slow/PL1 are read back. The full active thermal-mode GET remains missing, and factory shortcuts lack preset-equivalent journal recovery. A failed non-Balanced mode attempts Balanced; it does not restore an unknown original mode. See [HP mode observations](models/8e35-hub-modes.md) and [Recovery](RECOVERY.md).

The reference device separately reported an effective **115-W limit** with an appropriate HP state. This is not measured 115-W draw under load and not a free-TGP writer. PPAB alone is not a guarantee of the full OEM envelope.

## Numeric public power limit

`windows/gpu/power.rs` and `controller/public_power.rs` implement an optional public NVML adapter. It requires the actual configured current limit, live min/max bounds, setter presence, exact readback, durable originals and restore ownership checks. Simulated ABI cases cover missing originals, changed bounds, foreign values and errors after effect.

On the RTX 5070 Laptop / 617.42 read-only probe:

| Quantity | Result |
| --- | --- |
| Allowed public range | 5–115 W |
| Reported default | 80 W |
| Separately reported effective limit | 115 W |
| Configured original GET | NVML status 3, not supported |
| Requested-limit field | Not supported |
| GPU-base scope 2 query | Invalid argument |

Default/effective values are not substituted for a missing configured original. **No actual numeric power-limit write was qualified on this laptop.** The v2 setter export discovered during research was not called. Alternate transports remain in the [TGP investigation plan](plans/NVPWRCONTROL_TGP_FORK_PLAN.md); TPM and Secure Boot must remain enabled.

## Presets, sign-in and recovery

Save changes metadata; Apply invokes the elevated transaction path. Every original is captured before SET, dependencies are ordered and targets rechecked after the complete profile. Partial failures retain recovery if full original return cannot be confirmed.

The native preset chart returns an in-memory draft to its parent; its Accept button does not apply hardware. Sign-in requires a separately authorized applied revision, matching protected installation and current preflight. Real reboot/resume/AC acceptance is pending. [Custom presets](CUSTOM_PRESETS.md), [Power lifecycle](POWER_LIFECYCLE.md).

Read-only examples after building:

```powershell
.\target\release\omen-hub-pp.exe gpu-capabilities
.\target\release\omen-hub-pp.exe gpu-curve-read
.\target\release\omen-hub-pp.exe profile-plan <profile-id>
```

These may require device permissions but do not perform Apply. `profile-apply`, `profile-restore`, `gpu-validate*` and the hardware qualification scripts deliberately write/restore settings; they are not status checks. [Building](BUILDING.md), [Recovery](RECOVERY.md).

Direct GPU/VRAM voltage, FPS limiter and locked clocks remain absent. Current numeric power support is conditional as described above. Public contracts: [NVML queries](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html), [NVML commands](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceCommands.html). Private observations remain model/build-specific original documentation; proprietary binaries are not bundled.
