# Keyboard lighting protocol observations

Own interface observations for **8E35/F.13**, manufacturer family **Khalilah25C1**, with installed Hub 1101.2609.3.0 examined on 13–15 October 2026. Static sources were inspected with ILSpy and Ghidra/PyGhidra; proprietary inputs/decompilation remain ignored locally. Current usage: [Lighting UI](KEYBOARD_UI.md).

## Capability and fixed transport

Capability GET **0x20008 / type 0x2B**, empty input with size 0, returns exactly four bytes. The observed `[1,0,0,0]` means FourZoneWithNumpad. Vendor categories also include normal, no-numpad, per-key, one-zone and MCU variants; their presence in a table is not runtime write support.

The own reader validates transport, firmware return code 0, exact types/lengths and known capability. It avoids the vendor wrapper's potential zero/default fallback on failure. It does not infer zones from the ACPI keyboard or virtual VHF device name.

The native mapping is `ROOT\WMI`, `hpqBIntM`, output-size-specific `hpqBIOSInt4`/`hpqBIOSInt128`, embedded `hpqBDataIn` with `SECU`, Command, CommandType, Size and hpqBData. Embedded OutData carries `rwReturnCode` and Data. The zero-length input preserves `VT_ARRAY|VT_UI1`; it is not replaced with null/untyped data.

| Command / type | Operation | Input | Output |
| --- | --- | --- | --- |
| 0x20008 / 0x2B | Keyboard capability | Empty, size 0 | 4 bytes |
| 0x20009 / 2 | Colors GET | One zero byte | 128 bytes |
| 0x20009 / 3 | Colors SET | Complete captured 128-byte image | 4 bytes and status |
| 0x20009 / 4 | Brightness/mode GET | One zero byte | 128 bytes |
| 0x20009 / 5 | Mode SET | `[raw,0,0,0]` | 4 bytes and status |

Four RGB zones occupy bytes **25–36**. The own setter preserves all other bytes and validates the complete expected state before SET and both full blocks/capability afterwards. Only **0x64 / 0xE4** are qualified mode values. They mean Off / static On on this exact model; the raw byte is not a general brightness percentage.

## Native-source confirmation

`WMIService.dll` confirms direct BIOS-WMI transport. Its analyzed `ExecuteBiosWmiCommand` is at 0x18000DC60; input/output builders at 0x180011F10 / 0x180012410. These are static-image references, not addresses called by the app.

`WMISDK.dll` is itself an RPC path through HPSysInfoRpcEndpoint/WMIService; it is not adopted. The existing `OMENLighting.sys` VHF callback uses the same BIOS-WMI GUID **5FB7F034-2C63-45E9-BE91-3D44E2C707E4** and `SECU` envelope. The app neither bundles nor installs that driver and does not require its wrapper to issue the qualified direct color commands.

## Restore and lifecycle boundaries

Complete original images are stored durably. Recovery in a new process validates model, semantic/raw consistency and only original/own-target intermediate states; it does not write arbitrary opaque dump bytes. Separate-process restore was measured on 14 October. Direct-window transactions preserve the performance journal and startup selection.

The 15 October visible On/Off confirmation refined the old `firmware_default` name: it is a compatible Off alias, not proven firmware Auto. Current code permits profile sign-in authorization, following user preference; early statements that it is blocked are historical.

The vendor animation SET 0x20009/0x0B has a larger state, whereas GET 0x0C exposes only an effect value in the examined caller. That is not full restore. Some HID routes persist to flash; persistent and temporary operations must stay distinct. No animation/flash-write support is qualified here.

Fn-F4, profile changes, lock/unlock, display/standby and Windows Dynamic Lighting can involve competing controllers. Static zone success with HP services present does not prove their absence is harmless or that all lifecycle work has been replaced. Additional effects/per-key variants need their own capability and full original contract.

## Source identities

| File | Version | SHA-256 |
| --- | --- | --- |
| HP.Omen.Core.Model.Device.dll | 1101.2609.3.0 | FEB984522D08B627FE3140BF31A0B32A64D5A14AD0F933BF83532BC78CD8E4E9 |
| HP.Omen.Background.FourZone.dll | 1101.2609.3.0 | 77E387D307E89192406AE7FF896FBD0280BA35D0AC75B409132BC955181EAA11 |
| HP.Omen.Background.DojoLightingBg.dll | 1101.2609.3.0 | 7BDC40879B8E206DFA07963F9BA2261BE4A0B94649D0A48A943B0E33FA428336 |
| WMISDK.dll | 1.3.10 | 9680AECA5F3A192C19F94A26A3580B0C422D3D592C7BCE458ED1C1888B665AD3 |
| WMIService.dll | 1.87.4769.0 | A46829F999DA49080F62C570B5913EAEDE009DC419B91FB796ACB24BFE1D56F8 |
| HIDService.dll | 0.9.0.1 | 0A46C5547B4282BBFFE01CA85BCD7BC5559AF55FA7C8AC6D2413CEDA5CD45586 |
| OMENLighting.sys | 10.0.10011.16384 | 4162BD54DEB78114835156190EF05B73CDE17E9825F71DCA65CB5E17004E93BA |

The separate INF/PnP package version 18.39.5.0 is a different version field. One Device-module decompilation failed in WindowsLightingHelper.Initialize; only the successfully recovered model/capability methods were used. No fully error-free assembly analysis is claimed. Dated build/trial evidence: [Validation history](history/VALIDATION_THROUGH_2026-10-26.md).
