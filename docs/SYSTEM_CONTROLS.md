# CPU and Windows controls

Current adapter contracts for the reference **8E35/F.13 / Ryzen 9 8940HX**, SMU **0x546F00**, PM table **0x540208**. [Hardware matrix](HARDWARE_MATRIX.md) separates implementation from measured acceptance.

## CPU tuning

The original Rust SMU transport uses the pinned signed **RyzenSMU 0.2.11** module through the already running verified PawnIO driver. Module SHA-256: **`301D9CA397108E09F31BFBD5AC4C9BB4F352A5DE68532C32DB3BA7DDCDE93450`**. It accepts fixed Raphael/Dragon Range messages, not caller-chosen opcodes.

| Setting | Fixed RSMU message | Readback |
| --- | --- | --- |
| PPT | 0x56 | PM-table cell 2 |
| Slow PPT | 0xCB | Cell 4 |
| TDC | 0x57 | Cell 8 |
| EDC | 0x58 | Cell 62 |
| Tctl ceiling | 0x59 | Cell 10 |
| PBO scalar | SET 0x5B / GET 0x6D | Full scalar bit image |
| Curve Optimizer | All-core 0x07 / per-core 0x06 / GET 0xD5 | Full 16-core margin image |

CO margins are signed 16-bit steps, with CCD/core masks from the exact module contract. All-core and per-core instructions cannot coexist in a preset. Their step size is not a fixed mV conversion.

Independent STAPM was removed after the HP path could not preserve the paired limits independently. New limit journals use `rsmu_limit` plus the exact table version. Old HP limit images remain decodable but cannot select the new transport or invent a restore. The corrected PM-table version is **0x540208**, not the earlier erroneous 0x540108.

After a limit SET, bounded fixed GETs must agree for at least 100 ms within two seconds. A successful API return alone is insufficient; the SET is not repeatedly forced. Each original and the whole final profile are checked, including coupled control images.

### Accepted values are not a measured stability envelope

Current constants allow PPT/Slow **15–160 W**, TDC **30–108 A**, EDC **30–160 A**, Tctl **60–95 °C**, CO **−50…+30** and PBO **1–10×**. Recorded trials used small decreases, CO around −20 and PBO 1×. They do not prove the complete allowed range stable or suitable. Raw fuse GETs are diagnostics, not currently a dynamic verified cap. This remains a [high-priority review finding](PROJECT_REVIEW.md).

New PPT/Slow applications are blocked before capture/SET while HPOmenCap runs or its state is unknown. Writes recheck this condition. Startup needs the OEM service absent until own startup handover is qualified; a temporary stop is insufficient. Existing original recovery remains possible. [HP control](HP_CONTROL.md).

Intel PL1/PL2/PL4 and related preset fields have no integrated backend. There is no available Intel test device. Fixed CPU clock/voltage and the removed APU controls are outside the present UI.

## Windows settings

Windows processor boost, min/max, EPP and core parking preserve and verify **both AC and DC**. An already correct AC value must not hide a different DC value.

When a preset includes a power plan and processor policies, the controller captures policies of the target plan, activates that plan first and then writes policies. Restore verifies even an inactive plan's original policies without activating it as a side effect.

Power plan and Windows energy/overlay mode are separate. The energy mode requires Balanced, which can be requested in the same preset; incompatible combinations fail before SET. Configured original overlay comes from `PowerGetActualOverlayScheme`; the effective overlay is an additional observation. A legacy effective-only image cannot restore a user's configured selection.

Display refresh uses supported Windows modes with readback/restore. The MUX setting is an HP BIOS boot preference, not an immediately observed display-routing change. [Power lifecycle](POWER_LIFECYCLE.md).

## CCD and cooling helpers

CCD selection is a Windows process-affinity preference consumed by the own helper, not a firmware switch. An owned-process probe verified masks 0xFFFF, 0xFFFF0000 and exact original 0xFFFFFFFF return. This did not rearrange other user programs. The helper releases original masks when appropriate or on normal shutdown.

Own fan curves and helper Max use separate control, temperature-only GPU telemetry and fixed model checks. A short conservative probe reached 4800/4800 firmware RPM; it does not establish BIOS handover or durable HP independence. [Fan control](FAN_CONTROL.md).

## Verification scope

`scripts/check-system-controls.ps1` is an **explicit hardware-writing developer qualification**, not routine diagnosis. It requires an expected binary hash and suitable conditions, checks for competing tuners and performs small writes with full original restore. Optional `-CoolCpu` changes/restores Windows boost too. Do not invoke it as an ordinary documentation or build check.

Exact dated results and old memory samples are preserved in [history](history/VALIDATION_THROUGH_2026-10-26.md). Latest software checks and unfinished physical work: [Validation](VALIDATION.md), [Roadmap](ROADMAP.md).
