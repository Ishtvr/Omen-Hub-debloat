# Tastaturbeleuchtung: Schnittstellenbefund

Stand: 15. Oktober 2026. Referenzgerät OMEN 16-ap0xxx, Board 8E35, BIOS F.13, installierter Hub 1101.2609.3.0. ILSpy 11.1.0.9782 und Ghidra 12.1.4 über PyGhidra-MCP 0.2.7 bestätigen den Transport. Auf GETs vom 13. Oktober folgten am 14. Oktober RGB-/Modusproben mit vollständigem Restore. Der erneute Ghidra-Abgleich und die sichtbare Ein/Aus-Bestätigung am 15. Oktober präzisieren die frühere Bezeichnung Firmware-Voreinstellung: 0x64 schaltet die Beleuchtung aus. Die neue UI bietet Zonenwahl und Ein/Aus; [Bedienung](KEYBOARD_UI.md).

Das Ziel ist ein vollständiger Ersatz des originalen Hubs. Die HP-Module sind Analysequellen; eine Abhängigkeit von deren Hintergrundprozessen oder RPC-Diensten ist kein Ziel für die eigene Beleuchtungsengine. Direkte GETs und die unten begrenzten SETs ohne Laden der HP-RPC-/SDK-Wrapper sind bestätigt. Die HP-Dienste liefen währenddessen weiter; unabhängiger Betrieb ohne diese Controller und der vollständige Lifecycle sind weiterhin ungemessen.

## Modell und tatsächliche Tastaturvariante

Die eingebettete Herstellertabelle führt 8E35 unter **Khalilah / 25C1**. `FourZone` und `DojoLighting` sind dort Featurekandidaten. Der Hub filtert diese anschließend nach Laufzeitfähigkeiten. Die Modulnamen bestimmen daher weder die physischen Zonen noch gleichzeitig aktive Funktionen. Die Zyklusauswertung von 25C1 ergibt 251; der gesonderte spätere Modellzweig `>260` wird dadurch nicht aktiviert.

`StarmadeKeyboardHelper.GetKeyboardTypeAsync` liest den Tastaturtyp mit Command **0x20008**, Type **0x2B**, Eingabegröße **0**, Ausgabegröße **4**. Byte 0 enthält den Typ:

| Byte | Herstellerkategorie |
| ---: | --- |
| 0 | Normal |
| 1 / 2 | Vier Zonen mit / ohne Nummernblock |
| 3 | RGB pro Taste |
| 4 / 5 | Eine Zone mit / ohne Nummernblock |
| 6 / 7 | Vier Zonen mit MCU, mit / ohne Nummernblock |

Der ursprüngliche Typhelfer prüft den Rückgabecode nicht zuverlässig. Ein fehlendes Ergebnis kann im darunterliegenden Wrapper als Code 255 plus Nullbytes erscheinen und dadurch fälschlich als Typ 0 interpretiert werden. Unser Reader verlangt erfolgreichen Transport, Firmwarecode 0, passende Datentypen, genau vier Bytes und einen bekannten Typ.

Windows meldet die interne Tastatur als **ACPI HPQ8001 / i8042prt** und außerdem einen virtuellen Beleuchtungspfad **ROOT OMENLighting / VHF VID 0461, PID 0000**. Die Herstellertabelle ordnet diesen virtuellen PID ihrer `FourZone`-Kategorie zu. Diese Metadaten allein bestimmen die Zonenanzahl nicht. Die erfolgreichen Geräteabfragen vom 13. Oktober liefern **[1, 0, 0, 0]**, also **FourZoneWithNumpad**: vier Zonen mit Nummernblock.

## Klassischer Farb- und Helligkeitspfad

Diese Verträge wurden im verwalteten `OmenHsaClient` und im `FourZone`-Hintergrundmodul gefunden. Die drei GETs und die begrenzten Farb-/Modus-SETs sind auf 8E35/F.13 mit tatsächlicher Capability **[1,0,0,0]** erfolgreich gemessen. Andere Typen und allgemeine Helligkeitswerte erhalten keine Schreibfreigabe:

| Command / Type | Beobachtete Aufgabe | Eingabe | Ausgabe |
| --- | --- | --- | --- |
| 0x20008 / 0x2B | Tastaturvariante lesen | leer, Größe 0 | 4 Bytes |
| 0x20009 / 2 | Farbzustand lesen | ein Nullbyte | 128 Bytes |
| 0x20009 / 3 | Farbzustand schreiben | zuvor gelesener Farbblock, 128 Bytes | 4 Bytes und Firmwarestatus |
| 0x20009 / 4 | Helligkeitszustand lesen | ein Nullbyte | 128 Bytes |
| 0x20009 / 5 | Helligkeitswert schreiben | vier Bytes, Wert in Byte 0 | 4 Bytes und Firmwarestatus |

Farben liegen im gelesenen Block ab **Offset 25**, mit **drei RGB-Bytes pro Zone**. Der verwaltete Setter liest zuerst den vollständigen Block, ersetzt nur die zugehörigen RGB-Bytes und erhält den Rest. Einzonenvarianten verwenden eine Zone, die übrigen einschlägigen Varianten vier. Der eigene freigegebene Setter übernimmt nur das belegte Vierzonenschema: Er liest frisch, verlangt den vollständigen erwarteten Zustand, ersetzt ausschließlich Bytes 25…36 und vergleicht nach dem SET Capability und beide vollständigen 128-Byte-Blöcke. Das Originalbild bleibt dauerhaft im Transaktionsjournal.

Der Rohwert ist keine universelle Prozentzahl. Das untersuchte FourZone-Modul verwendet Bit 7 für eigene statische Farben. SetFnF4StatusAsync(true) führt zum qualifizierten Wert 0xE4, false zu 0x64. Der am 15. Oktober zusätzlich abgeglichene FourZoneOffControlViewModel.SendSetColorCMD-Pfad ruft false auf. Der eigene Adapter akzeptiert nur die gemessenen Zustände 0x64/0xE4, sendet [raw,0,0,0] und erhält alle übrigen Bytes. Auf8E35/F.13 bestätigen Readback und die Benutzerbeobachtung 0x64 = aus, 0xE4 = statisch ein. Der alte Profilwert firmware_default bleibt aus Kompatibilität erhalten; er ist keine dritte Wirkung und keine Helligkeitsskala.

## Native Transportbestätigung mit Ghidra

Der verwaltete `WMISDK`-Fallback ist selbst ein RPC-Client: **HPSysInfoRpcEndpoint / WMIService / RPC-Befehl 2**. Für die eigene Engine wird dieser Zwischenweg nicht übernommen.

Die native `WMIService.dll` bestätigt folgende Windows-Abbildung:

- Verbindung zu `ROOT\WMI`; BIOSklasse `hpqBIntM`.
- Ausgabemethode nach Größe, etwa `hpqBIOSInt4` oder `hpqBIOSInt128`.
- Eingebettetes `hpqBDataIn` mit `Sign = SECU`, `Command`, `CommandType`, `Size`, `hpqBData`, über den Parameter `InData`.
- Eingebettetes `OutData` mit `rwReturnCode` und `Data`; der native Parser prüft unter anderem die Antwortlänge.

Konkrete statische Fundstellen in dieser Binärversion: `ExecuteBiosWmiCommand` bei **0x18000DC60**, Windows-Transport bei **0x180011AB0**, Eingabeblock bei **0x180011F10**, Ausgabeblock bei **0x180012410**. Diese Adressen sind Analysebelege, keine Runtime-Adressen für einen Adapter. COM-/WMI-ABI kommt aus den Windows-Verträgen, nicht aus unvollständigen Decompiler-Prototypen.

Der bereits installierte `OMENLighting.sys` verbindet virtuelle HID-/VHF-Lampen mit derselben BIOS-WMI-Klasse. Ghidra zeigt `IoWMIOpenBlock`, `IoWMIExecuteMethod`, die BIOS-GUID **5FB7F034-2C63-45E9-BE91-3D44E2C707E4**, die Signatur SECU sowie die Beleuchtungskommandos 0x20009. Der VHF-Callback bei **0x1400063A0** erreicht klassische und animierte WMI-Pfade. Ein nativer klassischer Pfad verwendet beim GET eine andere vorbereitete Eingabegröße als der verwaltete Getter; diese Varianten dürfen nicht zu einem vermeintlich universellen Payload vermischt werden.

WMI-Metadaten auf dem Gerät nennen für `hpqBIntM` den Provider **WmiProv**, Hosting **LocalSystemHost**, und dieselbe GUID. Dies bestätigt die vorhandene Windows-Schnittstelle. Es beweist noch keinen erfolgreichen Betrieb nach einer späteren Entfernung der HP-Anwendung und ihrer Dienste.

`HIDService.dll` enumeriert HID-Geräte über SetupAPI, prüft VID/PID und einen optionalen Interfacefilter. Der beobachtete Datenzugriff verwendet Read/Write-Sharing und overlapped I/O. Der Dienst verwaltet eigene numerische Handle-Indizes; diese sind keine übernehmbaren Windows-HANDLEs. HID ist eine eigene, noch zu validierende Route für andere Tastaturvarianten.

## Animationen und persistente Zustände

Der beobachtete `FourZoneAniLightingControl` wählt HID ausdrücklich für Celsius/Rowdy; der Default ist `DojoLightingWmiHelperV2`. Das ist ein weiterer möglicher WMI-Pfad für Khalilah, sofern die Laufzeitfähigkeit ihn auswählt.

Der animierte SET **0x20009 / 0x0B** verwendet 128 Eingabebytes. Das Zielgerät steht in Byte 0, der Modus in Byte 1, Helligkeit in Byte 3, Farbanzahl in Byte 6, RGB ab Byte 7. Der GET **0x20009 / 0x0C** liefert im untersuchten Aufrufer lediglich einen Effektwert. Er ist kein vollständiger Zustand für Restore.

Ein dortiger Herstellerhelfer namens `SetBrightness` ignoriert seinen Parameter und erzeugt einen sonst leeren Block mit Zielbyte; er wird für „aus“ verwendet. Er wird nicht als allgemeiner Helligkeitssetter übernommen. Die HID-Variante ruft teilweise `StoreLightingToFlash` auf. Dauerhaftes Speichern und vorübergehende Beleuchtung müssen in einer späteren Engine getrennt behandelt werden. Keiner dieser Schritte wurde ausgeführt.

## Hintergrundaufgaben für den vollständigen Ersatz

Die eigene Engine muss gespeicherte Profile, Entsperren, Standby-/Resume-Neuvalidierung, Helligkeitstasten und gegebenenfalls Displayzustände behandeln. Audio Pulse benötigt einen laufenden Effektcontroller; eine einmalige Farbeinstellung ersetzt ihn nicht. Ein Stopp der HP-Threads beweist noch keine Wiederherstellung des vorherigen Firmwarezustands.

Der Originalcode berücksichtigt Light Studio und Windows Dynamic Lighting. Im Zielbetrieb ersetzt Omen Hub++ die HP-Writer vollständig; Windows-Lampensteuerung ist ein zusätzliches Betriebssystemfeature. Global aktiv bedeutet dabei nicht automatisch Kontrolle der Tastatur: Der Hersteller prüft auch Verbindung und den gerätespezifischen Enable-Zustand. Die hier beobachteten Einstellungen belegen keine aktuelle Windows-Übernahme. Originale Ownership-Helfer werden nicht für Diagnose aufgerufen, da einzelne davon Publisherdaten verändern können.

## Reader und ursprünglicher GET-Messstand

`keyboard-read` ist der native Rust-Reader für **8E35/F.13**. Er liest zuerst die Variante und nur bei klassischen Typen 1/2/4/5 anschließend Farben und Helligkeit. Andere bekannte Typen liefern ihre Capability mit `legacy = null`; ein unbekannter Typ ist ein Fehler und löst keine Folgeabfragen aus. Dieser CLI-Befehl führt ausschließlich GETs aus, erhöht seine Rechte nicht selbst und steuert keine Dienste. CIM- und VARIANT-Datentypen, Firmwarestatus und Antwortlänge werden geprüft. Feste GET-/SET-Verträge bleiben im privaten Windows-Adapter; Decoder und Zustandsmodelle liegen im plattformunabhängigen Core.

Aus einem Administrator-Terminal:

```powershell
& '.\dist\Omen Hub++.exe' keyboard-read | Out-String
```

`scripts/read-keyboard-state.ps1` ist der separate lesende Gegencheck. Er funktioniert mit der vorhandenen **Windows PowerShell 5.1**; `pwsh` beziehungsweise eine PowerShell-7-Installation ist nicht erforderlich. Falls die lokale Richtlinie Skripte sperrt, erlaubt die folgende Startoption nur diesen PowerShell-Prozess; sie ändert keine gespeicherte Richtlinie. In einem bereits als Administrator gestarteten Terminal:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\read-keyboard-state.ps1'
```

Ein normaler Prozess erhält beim Lesen der `hpqBIntM`-Instanz **AccessDenied / 0x80041003**. Der PowerShell-Fehlerdatensatz nennt `operation = bios_instance` und **bios_calls_attempted = 0**. Für die Geräteproben wurden einzelne Prozesse über Windows-UAC erhöht gestartet und anschließend beendet. Der aufrufende Codex-Prozess blieb uneleviert; die gespeicherte Windows-PowerShell-Richtlinie blieb `Restricted`. Der neue Editor fordert für ausdrückliches Prüfen, Anwenden und Wiederherstellen ebenfalls eine feste UAC-Kindaktion an; Öffnen und Speichern benötigen diese Freigabe nicht.

Die native Größe-0-Eingabe verwendet einen Null-SAFEARRAY-Zeiger mit Bytearray-Typ. Ein gewöhnliches leeres `Vec<u8>` wird von `wmi` 0.18.4 zu `VT_NULL` umgewandelt. Der eigene Adapter bildet deshalb ausdrücklich **VT_ARRAY | VT_UI1** mit Null-SAFEARRAY ab, anhand der Windows-COM-Definitionen und der nativen HP-Beobachtung. Ein Regressionstest prüft diese Darstellung. Die Geräteantwort bestätigt das Marshalling: Sowohl CIM mit leerem `byte[]` als auch die eigene native Darstellung liefern die erfolgreiche Capability.

Die PowerShell-Probe um **13:43 Uhr** und die native Rust-Probe um **14:04 Uhr** (Europe/Berlin, 13. Oktober 2026) lieferten Firmwarecode 0 und exakt 4/128/128 Antwortbytes. Die vollständigen Farb- und Helligkeitsblöcke stimmten byteweise überein. Das Helligkeits-Rohbyte war **228 / 0xE4**; es wird nicht als 228 % oder als universeller Prozentwert ausgegeben. Beide Reader haben ausschließlich GETs ausgeführt. Private Rohdaten und Startmetadaten liegen unter ignorierter `.local/research/keyboard-state/`; der getestete Rust-Build ist im [Validierungsprotokoll](VALIDATION.md) identifiziert.

Die erste native Probe fand einen zusätzlichen Adapterfehler: generische WMI-Map-Deserialisierung lässt Systemfelder wie `__PATH` weg. Der korrigierte Adapter liest die COM-Instanzfelder direkt und verlangt genau eine aktive BIOS-Instanz. Dieser Fehlversuch kam nicht bis zum Firmwareaufruf.

Der Adapter folgt den öffentlichen Verträgen für [IWbemClassObject::Put](https://learn.microsoft.com/en-us/windows/win32/api/wbemcli/nf-wbemcli-iwbemclassobject-put) und [IWbemClassObject::Get](https://learn.microsoft.com/en-us/windows/win32/api/wbemcli/nf-wbemcli-iwbemclassobject-get). Die Prozessgeltung der Startoption ist in [Microsofts Execution-Policy-Dokumentation](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies?view=powershell-5.1) beschrieben.

Die GET-Messungen allein begründeten noch keine Schreibfreigabe. Die folgenden Prüfungen ergänzen sie. Der Getter läuft auf ausdrücklichen CLI-/Editor-Aufruf, nicht als periodische Dashboard-Abfrage oder Heartbeat.

## Farb-/Modussteuerung und Wiederherstellung — 14. Oktober 2026

**02:35 CEST:** Der feste `keyboard-validate-colors`-Trial änderte ausschließlich eine RGB-Komponente in Zone 1 um eins. Firmwarestatus und vollständiger Sollzustand bestanden; der dauerhafte Originalzustand wurde sofort vollständig restauriert. **02:48 CEST:** `keyboard-validate-lighting` ergänzte den Moduswechsel **0x64 → 0xE4**, prüfte beide gekoppelten Settings und stellte Farben, Modus, Capability und sämtliche übrigen Bytes exakt zurück. NVIDIA-Indizes, Spannungsanker und programmierte Frequenz-Deltas blieben bei beiden Versuchen unverändert. Erfolgreiche Trials archivieren das bisherige abgeschlossene Journal und geben es nach geprüftem Restore wieder zurück; ein Fehler lässt die neue Recovery-Sperre bestehen.

**03:25 CEST:** Ein lokaler, ignorierter Test-Runner verwendete die aktuellen Core-/Platform-Crates mit einem isolierten `ProfileStore`. Ein Prozess wendete ein gespeichertes Profil mit vier einzelnen, um eins veränderten RGB-Komponenten und dem gegenüberliegenden 0x64/0xE4-Modus an. Ein zweiter Prozess öffnete ausschließlich das persistierte Journal und führte über eine neue Controller-/WMI-Sitzung `transaction::recover` aus. Vollständiger Ziel-Readback und Original-Restore bestanden, GPU-Steuerwerte waren gleich. Das vorhandene Benutzerjournal und die Startup-Auswahl wurden nicht verändert. Das ist ein Nachweis für Prozess- und Journal-Wiederherstellung, kein Reboot-/Anmeldetest.

Der Produktionsadapter prüft vor jedem Schreiben das genaue Board/BIOS und den gesamten erfassten Zustand. Bei gekoppelten Farben-/Modus-Settings akzeptiert er als Zwischenzustand ausschließlich das eigene vollständige Transaktionsziel. Persistierte Recovery validiert Gerätebindung, semantische Daten gegen Rohblöcke und unveränderte Steuerbytes; RGB und Modus müssen zum Original oder beantragten Ziel gehören. Payloads entstehen aus einem frischen gültigen Firmwarebild. Undurchsichtige Bytes aus einer Diagnosedatei werden nicht als freier Schreibblock verwendet. Teilfehler werden restauriert; ungeklärte Zustände bleiben gesperrt.

Die aktuelle UI ersetzt den früheren Profil-/Hex-/Check-Workflow vollständig. `keyboard-ui` beziehungsweise **Beleuchtung** öffnet ein eigenes erhöhtes Fenster mit vier anklickbaren Zonen und Ein/Aus. Bestätigte Farbwahl wird direkt transaktional angewendet; Abbrechen ändert nichts. Das getrennte Beleuchtungsjournal erhält das Performancejournal und dessen Startup-Auswahl. Bedienung und Fehlerzustände: [KEYBOARD_UI.md](KEYBOARD_UI.md).

Farben und Modus sind ausdrücklich **nicht `startup_safe`**. Eine Profilversion mit diesen Settings kann manuell angewendet werden, aber nicht für automatische Anwendung zur Anmeldung freigegeben werden. Fn-F4, Entsperren, Resume, konkurrierende HP-/Windows-Lampen-Writer, Animationen, andere Tastaturtypen und vollständiger Ersatzbetrieb brauchen separate Nachweise. Die aktuellen Proben installieren oder stoppen keine Treiber/Dienste, flashen keine Firmware und ändern keine Power-/Voltage-Werte. Binary-Identitäten und Messgrenzen stehen im [Validierungsprotokoll](VALIDATION.md).

Der Benutzer bestätigte am 14. Oktober **„Statische Farbe“** als aktuelle Auswahl im ursprünglichen OMEN Hub. Dies bestimmt die Ausgangslage für weitere Tests, beweist aber nicht, dass ein HP-Hintergrundprozess keine Farben erneut setzt. Es gab dazu keinen neuen zeitgleichen Firmware-GET und keine neue Hardwareänderung. Die Bestätigung wird deshalb getrennt von der gemessenen 0x64/0xE4-Moduswirkung und den Restore-Nachweisen geführt.

## Erneuter Abgleich und sichtbare Ein/Aus-Probe — 15. Oktober 2026

Ghidra 12.1.4 über den vorhandenen PyGhidra-MCP öffnete das bestehende ignorierte Analyseprojekt erneut. Gegenstand waren der VHF-Callback **0x1400063A0**, die klassischen Pfade **0x140006FAC / 0x140006BB8** in OMENLighting.sys und **0x18000DC60 / 0x180011F10 / 0x180012410** in WMIService.dll. Diese statischen Transportbelege wurden mit dem verwalteten FourZone-Off-Pfad abgeglichen; keine importierte Herstellerdatei wurde ausgeführt. Kopierte Dateien, Decompilation und Rohantworten bleiben unter `.local/`.

Die tatsächliche neue native UI bestand um **20:57 CEST** Ein/Aus, korrekt vorbelegten Farbdialog, Cancel ohne SET, Farbe **[255,80,80]** nur in der linken Zone, erhaltene übrige Zonen, gehaltenen Readback und vollständigen Original-Restore. GPU-Kurvensteuerung, HP-GPU-Status und Performancejournal/Startup-Dateien blieben gleich. Die Farbwirkung wurde vom Benutzer nicht beobachtet; der Nachweis dafür ist das vollständige Steuerbild.

Auf ausdrücklichen Benutzerwunsch folgte um **21:08 CEST** eine reine Schaltprobe: vier Sekunden an, vier aus, vier an, dann vollständiger Original-Restore. Der Benutzer bestätigte **„Ja, an/aus/an war sichtbar“**. Beide Proben verwendeten die Binary **2037248 Bytes**, SHA-256 **`9A7F6F52F24AF731B6C7545977C0E473F47554DA5B73E617B60D6A06A4B031D5`**. Spätere UI-Zeichen-/Auswahlkorrekturen und deren finaler Release sind separat in [VALIDATION.md](VALIDATION.md) zugeordnet.

Es gab keine Power-/Voltage-Änderungen, Dienststeuerung, Treiberinstallation oder Firmwareupdates. Der neue `Choice::Off` verwendet denselben qualifizierten 0x64-Payload wie das frühere `firmware_default`. Vollständiger Readback normalisiert nur diesen kompatiblen semantischen Namen; er schwächt die Byteprüfung nicht ab. Beleuchtungs-Startup, echte Power-Übergänge und vollständiger HP-Dienstersatz bleiben gesperrt beziehungsweise unbewiesen.

## Quellenidentität

| Datei | Dateiversion | SHA-256 |
| --- | --- | --- |
| HP.Omen.Core.Model.Device.dll | 1101.2609.3.0 | FEB984522D08B627FE3140BF31A0B32A64D5A14AD0F933BF83532BC78CD8E4E9 |
| HP.Omen.Background.FourZone.dll | 1101.2609.3.0 | 77E387D307E89192406AE7FF896FBD0280BA35D0AC75B409132BC955181EAA11 |
| HP.Omen.Background.DojoLightingBg.dll | 1101.2609.3.0 | 7BDC40879B8E206DFA07963F9BA2261BE4A0B94649D0A48A943B0E33FA428336 |
| WMISDK.dll | 1.3.10 | 9680AECA5F3A192C19F94A26A3580B0C422D3D592C7BCE458ED1C1888B665AD3 |
| WMIService.dll | 1.87.4769.0 | A46829F999DA49080F62C570B5913EAEDE009DC419B91FB796ACB24BFE1D56F8 |
| HIDService.dll | 0.9.0.1 | 0A46C5547B4282BBFFE01CA85BCD7BC5559AF55FA7C8AC6D2413CEDA5CD45586 |
| OMENLighting.sys | 10.0.10011.16384 | 4162BD54DEB78114835156190EF05B73CDE17E9825F71DCA65CB5E17004E93BA |

Die separate INF-/PnP-Version des Beleuchtungstreiberpakets ist **18.39.5.0**. Sie bezeichnet ein anderes Versionsfeld als die SYS-Dateiversion und belegt keine fehlerhafte Installation. Rohinventar, native Decompilation und Ghidra-Datenbank liegen ausschließlich unter `.local/`. Eine ILSpy-Dekompilierung des Device-Moduls meldete einen Fehler in `WindowsLightingHelper.Initialize`; verwendet wurden die erhaltenen Modell-/Capability-Methoden und Ressourcen. Eine vollständig fehlerfreie Assemblierungsanalyse wird nicht behauptet.
