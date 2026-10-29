# NVIDIA 617.42 qualification

Recorded **22 October 2026** on **HP 8E35/F.13**, RTX **5070 Laptop**, VBIOS **98.06.2a.40.da**. Reviewed during documentation cleanup on 29 October; no new GPU trial is claimed.

## Exact implementation

The installed implementation `nvapi64_impl.dll` was **5,688,768 bytes**, SHA-256 **`C6A2014763C3662512B5D420DFC7632B777338881A13AEAB3F21D8033EAE6C71`**. The System32 loader hash was **`912AC885975DA8D8DF80AA0D1C04704E7DD2CA42FA2A02D341AE70237890E9BB`**.

The runtime checks resolved private function ownership against the version-specific implementation hash. Ghidra/PyGhidra compared the five function regions and alternate RID entries to 617.14; these regions were byte-identical, while whole sections were not. Interface-table/QueryInterface mapping was also checked. This is not a blanket compatibility claim for other versions.

| Function | Interface ID | Analyzed image address | v1 contract |
| --- | --- | --- | --- |
| Mask GET | 0x507B4B59 | 0x180206CC0 | 0x182C bytes, 255 × 24-byte entries |
| Frequency/anchor GET | 0x21537AD4 | 0x1802075A0 | 0x1C28 bytes, 255 × 28 |
| Delta control GET | 0x23F1B133 | 0x1802062F0 | 0x2420 bytes, 255 × 36 |
| Domain ranges GET | 0x64B43A6A | 0x180205C90 | 0x928 bytes, 32 × 72 |
| Frequency delta SET | 0x0733E009 | 0x180208220 | Selected Graphics frequency words in a fresh control template |

Point blocks start after 68 bytes; the mutable control delta is at `0x58 + index × 36`. The native read found **127 Graphics anchors, 450–1240 mV**. The app accepts no caller-defined buffer, address or opcode. These addresses identify the analyzed image only.

## Measured trials and originals

After user-authorized temporary closure of Afterburner, its original individual curve was preserved. Each fixed qualification used durable originals and full final comparison:

| Trial | Target | Result |
| --- | --- | --- |
| Full curve | All frequencies −15 MHz from captured state | All 127 deltas verified and restored |
| Memory after curve | First Graphics point −15 MHz, then VRAM −24 MHz | Graphics target preserved, complete restore |
| Negative offsets | Core −15 MHz / VRAM −24 MHz | Exact readback; original curve and offsets restored |
| Positive offsets | Core +15 MHz / VRAM +24 MHz | Exact readback; original curve and offsets restored |

The **2,657,792-byte** qualification binary, SHA-256 **`FB924A420FBF0211A2A24608C36579E5E685F08D69F4468087F2A9C074F2F80F`**, repeated its three CLI paths at **03:20:45–03:20:48 CEST**. Independent readers confirmed complete anchors/deltas and offsets. Prior user journal/startup/fan/UI data remained unchanged. An unresolved trial would keep its recovery barrier instead of replacing it with the earlier completed checkpoint.

Afterburner was restarted and the original curve/offsets remained intact at 03:21:20. The native preset-to-chart metadata smoke passed at 03:21:24 without hardware Apply. Field-retirement counts in early logs describe earlier editors; current retired-option behavior is in [Custom presets](CUSTOM_PRESETS.md).

## Boundary and reproduction

These are control/readback/restore tests, not load-stability, direct voltage, hard voltage ceiling, real reboot/resume or sustained 115-W acceptance. Profiles remain bound to their saved driver; changing driver does not silently migrate or reauthorize them.

`scripts/check-gpu-driver.ps1 -ExpectedBinarySha256 '<exact-built-hash>'` is an elevated **hardware-writing** developer check with small offsets/curves and restore. It refuses competing Afterburner/global-offset conditions rather than closing programs or installing drivers itself. Do not run it as routine status verification.

Current source accepts both exact **617.14 and 617.42** entries. Public telemetry/offset contracts: [NVML queries](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html), [commands](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceCommands.html). Private contracts are limited by this own image analysis and measured evidence. [GPU controls](GPU_PROFILES.md), [Validation](VALIDATION.md).
