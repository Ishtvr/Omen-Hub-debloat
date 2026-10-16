# OMEN Hub 1101.2609.3.0: Leistungsmodi auf 8E35 (statische Analyse)

Stand 16. Oktober 2026. Quelle: IL der installierten Assemblies (`HP.Omen.Core.Model.Device.dll`, `HP.Omen.Background.PerformanceControl.dll`, `HP.Omen.Core.Common.dll`, `AMDRyzenSDK.dll`), die eingebettete Gerätedatei `Khalilah_DRG_N22X4X6.json` und die Hub-Logs auf diesem PC (3.–13. Oktober). Nichts wurde ausgeführt oder an das BIOS gesendet. „Belegt“ heißt: im Code gelesen und im Log bestätigt; „Code“ heißt: nur im Code gelesen.

## 1. 0x1A-Payload je Modus

`PerformanceControlHelper.SetFanMode(mode, doMap, biosFanOwner)` baut `[0xFF, (byte)mode, (byte)biosFanOwner, 0x00]` und sendet `0x20008/0x1A`. Notebooks rufen es aus `Entry.ApplySettings` mit `doMap = false` auf; das Mapping für ThermalPolicy v1 (Default→0x30, Performance→0x31, Cool→0x50) gilt also **nicht** für 8E35. Byte 1 ist der rohe `PerformanceMode`-Wert:

| Hub-Modus (UI) | PerformanceMode | Byte 1 | Beleg |
|---|---|---|---|
| Balanced (Slider-Mitte, Standard) | L2 | **0x30** | belegt (Log „mode = L2“, 9×) |
| Balanced-Slider 0 / 25 / 75 / 100 % | L0 / L1 / L3 / L4 | 0x10 / 0x20 / 0x40 / 0x50 | Code (`GetMappedMode`, Slider `[0,25,50,75,100]`) |
| Performance (Standard = Slider 100 %) | L7 | **0x31** | belegt (Log „mode = L7“, 13×; `CurrentMappedDefaultValue` → 49) |
| Performance-Slider 50 / 75 % | L5 / L6 | 0x11 / 0x21 | Code (Slider `[50,75,100]` bei SwFanControl, sonst `[0,50,100]`) |
| Unleashed | L8 = Extreme | **0x04** | belegt (Log „mode = L8“, 27×; deckt sich mit unserem Unleashed-Test) |
| Eco (automatisch, siehe unten) | 256 | **0x00** (`(byte)256`) | Code |
| Cool (2), Quiet (3) | — | — | Code: `GetMappedMode` bildet beide auf L2 ab; im V2-UI gibt es keinen eigenen Knopf |

**Byte 2** = Rückgabe von `Entry.IsNBBiosFanControl()`. Laut [FAN_CONTROL.md](../FAN_CONTROL.md) ist das im Netzbetrieb immer `false` (0); nur im Akkubetrieb mit Thermal-Auto, Plattformfreigabe und passender BIOS-Version kann es 1 werden. Pro Modus unterscheidet es sich nicht. **Byte 3** ist immer 0.

Ein Modus „Cool“ oder „Quiet“ mit eigenem 0x1A-Wert existiert auf 8E35 also nicht. Am nächsten kommt der Balanced-Slider ganz links (L0 = 0x10). Die eingebettete Gerätedatei hat keine Modusliste; das UI ergibt sich aus `UnleashedModeSupport: true` und dem V2-Slider-Modell.

## 2. Was der Hub dem UI anbietet

- **Balanced** mit 5-stufigem Slider (L0…L4), **Performance** mit 3-stufigem Slider (L5…L7), **Unleashed** (Extreme, nur mit Netzteil).
- **Eco** (Modus 256) ist kein Knopf. `IsInEcoMode` wird wahr, wenn `ShouldAutoSwitchToEcoMode` greift (Einstellung „AutoSwitchToEcoMode“, im Log meist `False`). Dann sendet ApplySettings 0x1A mit Byte 1 = 0x00.
- Quiet (3) sendet auf diesem Gerät nie einen eigenen Wert.

## 3. Weitere Schreibzugriffe beim Moduswechsel

- **CPU-PL1 (`0x29`)**: `PowerControlBase.SetPL1DefaultValue` setzt den Zielwert, `ResetParameters(needResetPL1)` lässt ihn neu schreiben (über `SetPowerLimit` → `[W, W, FF, FF]`, Byte 0 = Slow, Byte 1 = STAPM laut eigener Messung). Werte aus Log und JSON:
  - Unleashed: gespeicherter Nutzerwert `UnleashedModePowerLimit1` (bei Flo **75 W**, Bereich 45…80, Default 80).
  - Performance (L5–L7): `NbPL1UpperBoundPerformance` = **60 W** (mit BIOS-OC ebenfalls 60).
  - Balanced/Eco: `PL1DefaultValue` = **55 W**.
  - `IsDynamicPowerLimitSupport = False`, `IsPowerLimit2/4Support = False`: kein dynamischer PL1-Algorithmus, kein PL2/PL4.
- **TPP (`0x29` Byte 3)**: Unleashed 72 W, Performance 45 W (siehe Dynamic-Boost-Analyse); Balanced sendet nichts.
- **0x22 (cTGP/PPAB)**: siehe Dynamic-Boost-Analyse, je Modus unterschiedlich.
- **Windows-Energiemodus**: Nur wenn die Hub-Einstellung `AutoSwitchToWin11PowerMode` an ist (`IsWin11PowerModeSyncSupport = True`): Performance/Unleashed → „Beste Leistung“, Eco → „Beste Energieeffizienz“, sonst „Keine Überlagerung“, getrennt für AC und Akku (`PowerSetUserConfigured{AC,DC}PowerMode`). Energiesparplan, EPP oder Prozessor-Boost schreibt der Hub dabei nicht.
- **PBO**: `AMDSDKHelper.SetPBOStatus(IsPerformanceMode)` läuft nur, wenn `IsSystemSupportedPBOScalar`; auf 8E35 laut Log `False`, also kein Schreibzugriff.
- **Max-Fan**: `OmenHsaClient.SetMaxFan(CurrentMaxFan)` bei jedem ApplySettings.

## 4. AMD-CPU-Übertaktung im Hub

Vorhanden, aber nicht über eigene SMU-Aufrufe in der App:

- `HP.Omen.OverclockingAMDModule` (UI) → `AMDSDKHelper` (`EnableOverclocking`, `SetCurrentFID`, `SetCurrentVoltage`, `SetCurveOptimizerForAllCores/EachCore/PerCore`, `SetPPTLimit`, `SetTDCLimit`, `SetEDCLimit`, `SetPBOStatus`) → `AMDRyzenSDK.dll` (`AmdRyzenHelper`).
- `AmdRyzenHelper` serialisiert die Parameter als JSON und schickt sie per RPC an den HP-Dienst **„AmdRyzenOverclockingService“** (`RpcClientUtil.RunCustomCapabilityCommandAsync`, Befehlsnummern: EnableOverclocking 11, SetCurrentFID 2, SetCurrentVoltage 4); PBO-Scalar geht über `SetOmenCapFunctionAsync` Nr. 37. Weitere Einträge im Wrapper: `SetOverclockFreqAllCores`, `SetOverclockFreqPerCore`, `SetOverclockCPUVID`, `SetOverclockCPUVoltage`, `SetSOCVoltage`, `SetAutoOverclockingCPU`, `GetFIDbyFrequency`, `GetVIDbyVoltage`.
- Die eigentlichen SMU-Nachrichten liegen im nativen HP-Dienst außerhalb des App-Pakets und sind hier **nicht** sichtbar. Für eigene Implementierungen bleiben die bekannten Raphael-RSMU-Nachrichten (ZenStates-Core) die Quelle; sie sind auf 8E35 ungetestet.
- Auf 8E35 laut Log: Overclocking-Seite mit „CPU_AMD“-Ansicht wird geladen, `IsBiosDefinedOcSupport = True`, `DeviceJsonHasCurveOptimizer = False`, der Hintergrunddienst `OverclockingAMDBg` wird **nicht** geladen.
- **CCD-/Kern-Affinität**: Nur im Game Booster (`GameBoosterLib.CoreAffinity.CoreAffinityUtility.SetCoreAffinity_ByProcessObj`) für einzelne Spielprozesse, also Prozess-Affinität per Windows, kein Firmware- oder SMU-Schalter. Fester Takt/feste Spannung/Spannungs-Offset gibt es nur über den RPC-Pfad oben.

## Folgerung für Omen Hub++

- Performance = `[FF, 0x31, 0, 0]`, danach PL1 60 W (`0x29 [3C, 3C, FF, FF]`), 0x22/TPP wie Dynamic-Boost-Analyse.
- Ein „Quiet“-Modus entspricht am ehesten Balanced-L0 = `[FF, 0x10, 0, 0]` mit PL1 55 W; Eco wäre `[FF, 0x00, 0, 0]`. Beides ist vom Hub auf diesem Gerät nicht als eigener Modus vorgesehen und muss vor Freigabe auf der Hardware gemessen werden.
