# HP-Reverse-Engineering-Protokoll

Erster Durchlauf: 12. Oktober 2026. Analysiert wurden lokal installierte Dateien des OMEN Gaming Hub **1101.2609.3.0** mit `ilspycmd` **11.1.0.9782**. DLLs wurden nur statisch gelesen, nicht ausgeführt. Die Ergebnisse sind Beobachtungen des installierten Codes und keine Bestätigung ihrer Wirkung auf Board 8E35.

## Quellenmanifest

| Datei | SHA-256 |
| --- | --- |
| `HP.Omen.Background.PerformanceControl.dll` | `CB10CB067F219CABC3A20D3C1192F3B7511621F392A85950C00321CE839B07B2` |
| `HP.Omen.Background.FanControlBg.dll` | `7FCF8AB8BCCD747F0A10A0923D2D39C776AF9A0C4BD20A550D89F1AB317B34A4` |
| `HP.Omen.PerformanceControlModule.dll` | `6BF7173F78333E2C44731CFD1385E68E549E4646589A81F4F807AF6527FF0EB1` |

Zusätzlich wurden `HP.Omen.Core.Common.dll` und `HP.Omen.CommonData.dll` untersucht. Die Dekompilierung von `Core.Common` meldet einen Fehler in `Utility.ShowToast`; die hier genannten untersuchten WMI-Wrapper wurden ausgegeben. Das ist keine vollständig fehlerfreie Dekompilierung des gesamten Moduls.

Die lokale Decompilation liegt in `.local/research/<DLL>/`. Code aus proprietären Dateien bleibt außerhalb von Git. Dieses Dokument beschreibt Schnittstellenbefunde in eigenen Worten.

## Statische Befunde zur GPU-Leistung

`PowerControlBase.GetGpuStatus` ruft den HSA-Client mit Command **131080 (`0x20008`)**, CommandType **33 (`0x21`)**, vier Nullbytes Input und vier Bytes Output auf.

`PowerControlBase.SetTgpPpabAsync` nutzt denselben Command und CommandType **34 (`0x22`)**. Seine vier Felder sind: cTGP-Schalter, PPAB-Schalter, D-State und Temperaturgrenze. In diesem Pfad setzt HP die Grenze auf **87 °C**. Danach folgt ein zusätzlicher Concurrent-TDP-Schritt. Dieser Wert ist ein beobachtetes Feld im HP-Pfad, keine allgemeine Empfehlung für Temperaturgrenzen.

`SetTGPAsync` verwendet dagegen nur ein Byte. Ein einheitlich angenommenes vier-Byte-Layout für alle Modellpfade wäre daher falsch. `SetGpuStatus` kann alle vier Werte aus dem aktuellen Plattformkontext liefern.

`UpdateTgpPpab` differenziert Extreme/Unleashed, Performance, Balanced/Eco, AC/DC, cTGP-Support, TPP-Support und Max-Q. Für bestimmte Max-Q-Pfade wird die CPU-Leistung mit 35 W verglichen. Daraus folgt keine Aussage, welcher Zweig auf der RTX 5070 dieses Rechners aktiv ist.

`SetConcurrentTdp` prüft Performance-/Extreme-Modus, AC, TPP-Support und einen Plattform-Schalter zur Umstellung auf Dynamic Boost. Der beobachtete Pfad verwendet CommandType **41 (`0x29`)** und modifiziert das vierte Byte eines Payloads; der Offset enthält einen modellbezogenen Mindestwert. Dieser zweite Schritt muss zusammen mit den Plattformdaten verfolgt werden.

## Heartbeat

`RunHeartbeatLoop` verwendet `PowerContextSettings.IntervalHeartbeat`; ohne Settings steht im Code ein Rückfallwert von **30000 ms**. Der Thread ruft wiederholt `GetNumOfFan` auf. `PowerControlBase.GetNumOfFan` delegiert an `_omenHsaClient.GetNumOfFan()` und prüft Fehler des HP-Service-Hosts. Der Code führt außerdem einen zusätzlichen Heartbeat nach Resume aus.

Nachuntersuchung am 13. Oktober: `OmenHsaClient.GetNumOfFan` in `HP.Omen.Core.Common.dll` verwendet `BiosWmiCmd_GetSync(131080, 16, ...)`, also **`0x20008 / 0x10`**, mit vier Nullbytes Input und vier Bytes Output. Das zweite Antwortbyte wird auf BIOS-Protection-/OCP-/OTP-Flags geprüft. Die End-to-End-Wirkung dieses Heartbeats auf Board 8E35 bleibt unvalidiert; er wird vom Prototyp nicht ausgeführt. Dass der Aufruf als Heartbeat bezeichnet wird, belegt noch keine konkrete Ursache des beobachteten 80–90-W-Verhaltens.

`ExecuteBiosWmiCommandThruDriver` bevorzugt bei vorhandenem kompatiblem HP-Treiber einen 4116-Byte-Buffer: `SECU` am Anfang, Command/CommandType/Input-Größe als Little-Endian-Werte, Daten ab Offset 16 und Output-Klasse am Ende. Der Pfad ruft `ReadHWData.Open` und `ReadHWData.Rdmsr` auf. Bei erfolglosem Treiberpfad wird an `WMIHelper.ExecuteBiosWmiCommand` delegiert. Der darunterliegende native IOCTL/RPC-Transport und mögliche EC-Nebenwirkungen bleiben offen. Es wurde kein HP-Treiber geladen oder nachgebaut.

## Auf dem Gerät vorhandene WMI-Metadaten

In `ROOT\WMI` sind `hpqBIntM`, `hpqBDataIn` und die Output-Klassen für 0/4/128/1024/4096 Bytes vorhanden. `hpqBIntM` enthält die Methoden `hpqBIOSInt0`, `hpqBIOSInt4`, `hpqBIOSInt128`, `hpqBIOSInt1024` und `hpqBIOSInt4096`. Eingabefelder umfassen `Command`, `CommandType`, `Sign`, `Size` und `hpqBData`.

Das bestätigt die Existenz der Schnittstelle, nicht die modellbezogene Unterstützung beliebiger Befehle. Standarddiagnose und Dashboard fragen nur die Klasse ab. Der separate Rust-Befehl `hp-read` konstruiert ausschließlich den statisch belegten GPU-Status-GET (`0x20008 / 0x21`) mit eingebettetem `hpqBDataIn`, vier `SECU`-Bytes und vier Nullbytes. Er erwartet `hpqBDataOut4`, Returncode 0 und genau vier Datenbytes; unbekannte Feldlayouts werden abgewiesen. Das rohe Temperaturfeld bleibt erhalten; ein Nullwert wird nicht als belegte Firmwarefunktion ausgegeben.

Ein unelevierter Probeaufruf am 13. Oktober scheiterte mit **0x80041003**. Erhöht lieferte der eigene Transport Code 0 und **[1,1,0,87]**. Der statische `GpuDState` benennt 1–5; der Benutzer bestätigt zugleich Unleashed im Original-Hub. Nach diesem Modellabgleich bleibt 0 im Decoder ausdrücklich unbekannt, während die belegten Bool-Schalter gelesen werden. Die anfangs strengere Abweisung war ein früherer Zwischenstand. Daraus folgt noch keine HP-Leistungs-/Write-Freigabe. Die Envelope entspricht der [OmenHwCtl-Untersuchung](https://github.com/GeographicCone/OmenHwCtl/blob/master/OmenHwCtl.ps1).

## Modellressource und frische Capabilities

Die Zuordnung in der installierten Gerätebibliothek lautet 8E35 → Khalilah25C1, SKU DRG_N22X4X6, Dragon Range und NV BD2 80W GN22-X4/X6. `Core.Common` enthält die Ressource `Khalilah_DRG_N22X4X6`, Version 20250523, mit Unleashed-Support, 30000-ms-Heartbeat, PPAB in Performance und CPU-PL1-/TPP-/IR-Parametern. TPP bezeichnet einen gekoppelten Parameter und darf nicht als alleiniger GPU-TGP übernommen werden.

Der originale SystemDesignData-Pfad verwendet **0x20008/0x28**, vier Nullbytes Input, 128 Outputbytes. Der neue feste eigene GET bestätigt frisch 230 W Shipping-Adapter, Rohbyte 0x0B, Software-Fan-/Extreme-Support und Default-Shared-Budget-Rohwert 45. Die Registry-Version dieser Daten ist ein Cache, kein Live-Readback. Das nicht gesetzte Extreme-Unlock-Bit wird nicht als universelle Sperre des beobachteten Notebook-UI-Modus ausgegeben.

Ein Fan-/Performance-SET nutzt **0x20008/0x1A**, einschließlich eines Thermal-Policy-abhängigen Mode-Mappings. **0x20008/0x29** kann je nach Aufruf sowohl CPU-PL1/PL2-Felder als auch den vierten Shared-Budget-Wert betreffen. Genau deshalb wäre ein isolierter Bool-Setter unter der Bezeichnung „Unleashed“ unvollständig. Aktueller vollständiger Zustand, Sensorsicherheit, gekoppeltes Restore und unabhängiger Controller-Lifecycle fehlen noch.

## NVIDIA-Takt- und Voltage-Untersuchung

Die öffentlich dokumentierte NVML-Clock-Offset-ABI ist als eigener Rust-Adapter implementiert: 24-Byte-v1-Struktur, Versionbits 24–31, Graphics=0/Memory=2, P0. Negative und positive kleine Offsets wurden nativ zurückgelesen und vollständig wiederhergestellt. Memory −25 wurde vom Treiber als −24 gelesen und führte korrekt zur Fehler-/Restore-Behandlung. Eigene persistierte Profile und Recovery verwenden dieselbe Transaktion; Quellen und Messungen stehen in [GPU_PROFILES.md](GPU_PROFILES.md) und [VALIDATION.md](VALIDATION.md).

UXTU wurde ausschließlich als lokale Referenz für Funktionsgruppen und weitere private NVIDIA-Verträge untersucht, Commit `eeb43042883bc9a80847999b2957b6ded86938a9`. Die MHz-Offset-Implementierung ist keine mV-Steuerung. Pauschales Zurücksetzen privater Kurven-Deltas auf null würde vorhandene Benutzertuning-Werte verlieren und wird nicht als Restore übernommen.

Ghidra untersucht zusätzlich den installierten NVIDIA-Loader und dessen Implementierung 617.14. Der Loader aus System32 vermittelt `nvapi_QueryInterface` an `nvapi64_impl.dll` im NVIDIA-DriverStore. Die proprietären Programme, Ergebnisse und ausführlichen Dumps verbleiben im ignorierten Analyseprojekt; daraus wird kein Runtime-Treiber oder DLL-Paket übernommen.

Native Schnittstellenbeobachtungen am 13. Oktober, ohne übernommenen dekompilierten Code: Loader SHA-256 `3AEDC98E990F6AD7CDF625526662923288DEB18E4DCF8F0E46BA8B1D90E2E274`, Implementierung `FF1527CEA533C75F89033F7C0D9CDBAA6EC84BCAC26C1DB082227D68EFD6A16E`. Für die v1-Blöcke sind Mask/Curve/Control/Ranges **6188/7208/9248/2344 Bytes** bestätigt, DWORD-ausgerichtet. Der Control-Header umfasst 17 DWORDs; die 255 Datensätze haben 36 Bytes, und das mutable Frequenz-Delta liegt bei **88 + Index × 36**. Curve-Datensätze haben 28 Bytes; die Maske umfasst acht DWORDs nach der Version. Es wird eine frische GET-Vorlage verwendet und ausschließlich die Graphics-Maske sowie deren Frequenzwörter geändert.

GET-IDs: Mask `0x507B4B59`, Curve `0x21537AD4`, Control `0x23F1B133`, Ranges `0x64B43A6A`. Der gesondert untersuchte Frequenz-SET ist `0x0733E009`. Native GETs um 17:22 lieferten Status 0, 127 mutable Graphics-Punkte und fünf Memory-Punkte. Graphics spannt 450000…1240000 µV und ±1000000 kHz Delta; Memory-Indizes 127…131 haben teils immutable Datensätze. Diese Memory-Koordinaten werden nicht als DRAM-Rail-Voltage ausgelegt. Der erste vollständige Graphics-SET-/Readback-/Restore-Nachweis erfolgte um 18:02 mit allen Deltas −15000 kHz.

UXTUs lokale Kurven-SET-Rezeptur und ältere Wrapper-Strukturbezeichnungen wurden nicht als ABI übernommen: Offset 0x20/Stride 0x48 passt nicht zum Control-Pfad dieses Treibers. Der Rust-Adapter basiert auf den eigenen nativen Beobachtungen, pinnt das tatsächlich geladene Implementierungsmodul und restauriert alle Original-Deltas. Ein späterer kombinierter Versuch zeigte außerdem, dass NVML-Memory-SET eine individuelle Graphics-Kurve löscht. Die Prüfung erkannte den Verlust und stellte vollständig zurück; die Engine erhält nun den Graphics-Zustand und setzt den endgültigen Graphics-Wunsch nach Memory. Ghidras getrenntes lokales Projekt mit fünf Programmen wurde gespeichert und geschlossen; die temporäre Laufwerkszuordnung ist entfernt.

## Tastatur und nativer Transport — 13. Oktober 2026

Die installierten Beleuchtungs-, Modell- und Hintergrundmodule wurden statisch untersucht; drei native Dateien wurden mit Ghidra 12.1.4 / PyGhidra-MCP 0.2.7 in einem separaten, ignorierten Projekt analysiert: `WMIService.dll`, `HIDService.dll` und `OMENLighting.sys`. Die nativen Funktionen wurden nicht ausgeführt. Programmdateien und vorhandene Geräte-/Dienst-/WMI-Metadaten waren die Quellen; Rohmaterial verbleibt lokal.

Der Tastaturbefund enthält die genaue Modellzuordnung 8E35/Khalilah25C1, den Laufzeit-GET 0x20008/0x2B sowie klassische Farben-/Helligkeits-GETs und -SETs. Ghidra bestätigt die Abbildung auf `ROOT\WMI` / `hpqBIntM` mit eingebettetem `InData`/`OutData`. HPs `WMISDK` ist ein RPC-Zwischenweg, nicht die unabhängige Windows-Anbindung. Der lokale Reader stoppt im unelevierten Prozess vor BIOS-Methodenaufrufen. Erhöhte PowerShell- und Rust-Proben bestätigten anschließend **FourZoneWithNumpad** sowie byteweise identische vollständige Farb-/Helligkeitsblöcke, mit Firmwarecode 0 und Größen 4/128/128. Die native leere Capability-Eingabe ist am Provider validiert. Am 13. Oktober bestand dadurch ausschließlich GET-Evidenz; HP-Dienste liefen weiter.

Am **14. Oktober** folgten begrenzte eigene RGB-SETs und der exakte eigene-Farben/Firmware-Modusschalter **0xE4/0x64** nach dem untersuchten `SetFnF4StatusAsync`-/Update-Ablauf. Vollständiger Soll-Readback und Original-Restore bestanden, zuletzt auch für alle vier Zonen mit einem gespeicherten Profil und Recovery in einem neuen Prozess aus isoliertem dauerhaftem Journal. Die manuellen Adapter sind auf 8E35/F.13 und Capability [1,0,0,0] freigegeben. Animationen, allgemeine Helligkeit, automatischer Lifecycle und vollständiger Ersatz ohne HP-Controller bleiben ungeprüft; die Dienste wurden nicht verändert.

Ausführliche eigene Schnittstellenbeobachtungen und Quellenhashes: [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md). Die Benutzerpräzisierung zum vollständigen Ersatz samt notwendigen Hintergrundaufgaben steht in [REPLACEMENT_PLAN.md](REPLACEMENT_PLAN.md).

## Lüfterpfad — 15. Oktober 2026

Die statische Managed-Analyse trennt jetzt den unabhängigen **Max-Fan-Override** vom gekoppelten Leistungs-/Lüftermodus. Max-Fan liest mit **0x20008/0x26**, vier Nullbytes Input und vier Outputbytes; SET **0x27** verwendet dagegen genau ein Bool-Byte Input und vier Outputbytes. Der eigene feste Transport ist auf 8E35/F.13 zusätzlich durch Tabelle/Topologie und vollständige reservierte Schalterbytes begrenzt. ON-/gehaltener Readback-/Original-OFF-Restore und später eine separate persistierte Recovery sind real bestätigt. Quellenhashes und präzise Bedingungen: [FAN_CONTROL.md](FAN_CONTROL.md), Messungen: [VALIDATION.md](VALIDATION.md).

Drehzahlen **0x2D**, Topologie **0x2C** und Tabelle **0x2F** liefern jeweils 128 Bytes. CPU/GPU werden in 100-RPM-Firmwareeinheiten beobachtet; unbekannte Sentinelwerte werden nicht als positive RPM ausgegeben. Tabellenbyte 1 ist im Herstellerparser uninterpretiert und wird nicht als Zeilenzahl verwendet. Die weitere Untersuchung verfolgt Selektor 0 von **0x23** auf Khalilah25C1 bis `GetIRSensorValue` und zur expliziten Celsius/Fahrenheit-Ausgabe des Monitors. Der eigene Decoder gibt nun Firmware-IR in °C mit erhaltenen Rohbytes aus; physischer Messort und Sicherheitsgrenzen bleiben unqualifiziert. Der reine Tabellen-Paarungshelfer ist keine Freigabe eines Softwarekurven-SETs.

`GetThermalControl`/`GetPowerModeNB` lesen gespeicherte Profilabsicht, teilweise mit Standardwert-Schreibzugriffen. In der installierten Version führt die Profilfabrik zum Publisher-Cache `OMEN/profiles.json`; diese Helfer wurden nicht ausgeführt. `IsNBBiosFanControl` liefert im AC-Pfad false, und der Notebook-Controller führt für Auto die Softwarekurvenregelung aus. Die interne Zuständigkeitsvariable entsteht aus Setter-Erfolg. Ein vollständiger Firmware-GET für diese Zuständigkeit/den Leistungsmodus wurde in den untersuchten Pfaden nicht gefunden. Die eigene einmalige `cooling-read`-Diagnose hält diese Felder daher unbekannt und prüft ausschließlich reale Temperatur-/Lüfterleser und deren Alter. Der Leser ist keine Verbindung zum Originalcontroller und liest dessen Profilcache nicht zur Steuerung.

**0x1A** ändert Leistung und BIOS-Lüfterzuständigkeit gemeinsam; die alte Oberfläche für Quiet/Normal/Turbo darf daher nicht als isolierter Fan-Setter nachgebaut werden. Der ursprüngliche Softwarekurvenablauf koordiniert CPU-, GPU- und zusätzliche IR-/Oberflächenanforderung einschließlich Glättung, Hysterese und Modellgrenzen. Vollständiger Modus-Readback/Restore, Zusatzsensorbedeutung und unabhängige Lifecycle-Sicherheit bleiben offen. Level-SET **0x2E** und Heartbeat **0x10** wurden nicht ausgeführt. Max-Fan qualifiziert diese anderen Pfade nicht mit.

## Nächste Untersuchung

1. HSA/CAP-RPC, `GetNumOfFan` und `ExecuteBiosWmiCommand` bis zum nativen Transport verfolgen.
2. Die inzwischen gelesene Lüfterzuordnung/-tabelle durch einen vollständigen gekoppelten Modus-Readback und Restore ergänzen; Zusatzsensor 0 und CPU-/GPU-/IR-Sicherheitsgrenzen qualifizieren. cTGP/PPAB, Dynamic Boost und ihre Wirkung bleiben getrennte Untersuchungen.
3. Lesende, begrenzte Get-Operationen separat integrieren und Rückgabecodes/Längen validieren.
4. Gaming-Hub-Zustände und GPU-Watt unter kontrollierter Last vergleichen. Gleiche Versorgung und Kühlung verwenden; CPU-Leistung und Akkudrain mit erfassen.
5. Erst mit belegter Modellspezifikation Apply/Readback/Restore und Keep-Alive implementieren.
6. Nach der bestätigten Vier-Zonen-Schreib-/Restore-Transaktion den Beleuchtungs-Lifecycle bei Hotkeys, Entsperren und Power-Ereignissen sowie den Betrieb ohne ursprüngliche HP-Omen-Controller prüfen. Andere Beleuchtungstypen erfordern eigene vollständige Zustandsverträge.

Öffentliche Vergleichsquellen: [OmenMon CLI](https://omenmon.github.io/cli), [OmenMon GPU-Presets](https://omenmon.github.io/gui), [OmenHwCtl](https://github.com/GeographicCone/OmenHwCtl). Diese dokumentieren cTGP/PPAB und HP-WMI-Funktionen, ersetzen aber nicht die Untersuchung der installierten Version und dieses Boards.
