# HP-Lüfter: unabhängiger Max-Fan-Override

Stand: **15. Oktober 2026**, HP OMEN 16-ap0xxx, Board **8E35**, BIOS **F.13**. Der eigene Windows-Adapter liest die Firmware direkt über den vorhandenen HP-BIOS-WMI-Provider. Er lädt keine OMEN-Hub-DLL. Auf dieser Kombination sind der manuelle Max-Fan-Schalter, vollständiger Schalter-Readback und persistierter Original-Restore qualifiziert. Eigene Temperaturkurven, BIOS-Automatikumschaltung und automatischer Profilstart bleiben gesperrt.

## Feste Schnittstellen

Alle folgenden Aufrufe verwenden `ROOT\WMI/hpqBIntM`, den vorhandenen `SECU`-Umschlag und Command **0x20008**. GETs haben vier Nullbytes Eingabe. Die eigene Implementierung verlangt genau eine aktive Instanz, den passenden COM-Typ, Firmwarecode 0 und die exakte Ausgabelänge. Es gibt keine frei wählbare Opcode-/Payload-CLI.

| Funktion | CommandType | Ausgabe | Belegte Bedeutung |
| --- | --- | --- | --- |
| Max-Fan GET | 0x26 | 4 Bytes | Byte 0: Aus=0 / Ein=1; übrige Bytes bleiben Teil des vollständigen Zustandsbildes |
| Max-Fan SET | 0x27 | 4 Bytes | Genau **ein** Eingabebyte, 0 oder 1; anschließend vollständiger GET-Abgleich |
| Lüfterdrehzahlen | 0x2D | 128 Bytes | Erste zwei Slots CPU/GPU in Firmware-Einheiten zu 100 RPM; Werte ab 128 bleiben unbekannt |
| Lüfterzuordnung | 0x2C | 128 Bytes | Sieben Nibbles für Zuordnung; rohe Capabilitybytes 8/9 bleiben erhalten |
| Gekoppelte Lüftertabelle | 0x2F | 128 Bytes | Byte 0 Lüfteranzahl; Dreiergruppen ab Offset 2 für CPU/GPU/Akustikrohwert; Nullgruppe als Ende |
| SystemDesignData | 0x28 | 128 Bytes | Modellmerkmale und Thermalpolicy-Version; keine aktuellen Leistungswerte |
| Zusatzsensor, Selektor 0 | 0x23 | 4 Bytes | Auf 8E35/25C1 vom Hersteller als IR-Temperatur in °C verwendet; Rohbytes bleiben erhalten, physischer Messort nicht kalibriert |

Die gemessene Zuordnung lautet CPU/GPU, mit Capabilitybytes 0/0 und Thermalpolicy-Version 1. Der vollständige Tabellenblock hat SHA-256 **`A8E624C6573881F444E36D94A29CA0AF320248855377531F02A499437BEE57F2`**. Sein Header ist `[2,12]`, aber er enthält **elf** nichtleere Dreiergruppen. Byte 1 wird im untersuchten Herstellerparser nicht interpretiert und wird deshalb auch hier nicht als Zeilenzahl ausgegeben. Die CPU-/GPU-Paarung ändert ihren Abstand entlang der Tabelle; ein gemeinsamer Prozentwert oder konstanter Offset wird daraus nicht abgeleitet. Der Akustikrohwert ist keine kalibrierte dBA-Messung.

## Profil- und Restore-Verhalten

`max_fan` ist eine boolesche Profilvorgabe. **Leistungsprofile → NVIDIA GPU → Max-Fan** nimmt sie optional per Checkbox auf und speichert Ein/Aus. Ohne Checkbox fehlt nur die Vorgabe; ein aktiver Firmware-Override wird nicht zurückgesetzt. Aus gibt den Max-Fan-Override frei und wählt keinen anderen BIOS-Lüftermodus. Speichern ändert nur Metadaten; erst das ausdrückliche Anwenden startet den erhöhten Transaktionsprozess.

Vor dem ersten SET erfasst die Transaktion sämtliche Originalwerte. Der Max-Fan-Adapter prüft dabei Board/BIOS, Tabellenhash, Topologie, Thermalpolicy und die vollständigen reservierten Schalterbytes. Vor dem Schreiben wird das eigene erwartete Zustandsbild erneut mit der Firmware verglichen. Geänderte oder unbekannte Zustände blockieren die Operation. Diese Freigabe verlangt Netzbetrieb; die Capability ist ausdrücklich **nicht startup-safe**.

Das dauerhafte Journal enthält Board/BIOS, Tabellenidentität und den vollständigen ursprünglichen Max-Fan-Schalter. Eine neue Controller-Sitzung kann den Originalzustand restaurieren, wenn die aktuellen Werte noch dem Original oder dem eigenen angewendeten Ziel entsprechen. Ein fremder Zustand wird nicht überschrieben. Manipulierte oder widersprüchliche Restore-Bilder werden verworfen. RPM-Werte dienen nur zur Beobachtung und sind niemals Restore-Ziele. Der Diagnose-Checkpoint erhält ein bereits vorhandenes Benutzerjournal byteweise einschließlich Formatierung.

Lesen: `omen-hub-pp fan-read` oder `scripts/read-fan-state.ps1` in einer erhöhten Windows-PowerShell. `cooling-read` ergänzt die einmalige gemeinsame CPU-/GPU-/IR-/Lüfterdiagnose einschließlich Messzeitfenstern und Fehlerstatus. `fan-validate-max` ist eine feste Qualifikationsprobe mit Ausgangsbedingung Max-Fan **Aus**, kurzem Ein-/Readback-Test und Original-Restore; sie ist kein gewöhnlicher Statusbefehl. Aktuelle Proben: [VALIDATION.md](VALIDATION.md), Gerätebindung: [models/8e35.md](models/8e35.md).

## Reale Qualifikation

- **08:58 CEST:** Sechs feste GETs erfolgreich. Ausgangsflag `[0,0,0,0]`; Firmware-Drehzahlen 2900/3100 RPM. Ein vorheriger unelevierter Versuch erhielt AccessDenied vor dem ersten BIOS-Aufruf.
- **09:17 CEST:** Fester ON-/Readback-/Restore-Trial mit Release-Binary SHA-256 `063CF16D4E02A522E3B9F883E07167B95B7A825BEC456625E6F23A9E5E0D69D4`. Zwei gehaltene Abfragen lesen `[1,0,0,0]` und 3300/3600, anschließend 4100/4200 RPM. Der ursprüngliche vollständige Schalter `[0,0,0,0]` wurde wiederhergestellt. Danach 3700/3900 RPM durch Auslauf; dies ist kein fehlgeschlagener Schalter-Restore.
- **09:30 CEST:** Eigenes isoliertes Proof-Programm aus aktuellen Core-/Platform-Crates, SHA-256 `D17C25B49500369CD8E9FD50C2DA1D17F0C105CD7173078683142FD99F8E245D`. Gespeichertes Apply blieb vier Sekunden aktiv, Firmware-Drehzahlen 4300/4400 RPM. Ein neuer Prozess verwarf zunächst ein Restore-Bild mit gefälschtem Tabellenhash ohne Schalteränderung und restaurierte dann über das dauerhafte Journal den ursprünglichen Aus-Zustand. Kein Recovery-Eintrag blieb offen.

Programmierbare NVIDIA-Steuerwerte, HP-GPU-Leistungsstatus sowie das produktive Benutzerjournal und die Startup-Auswahl blieben bei beiden Schreibproben unverändert. Die kurzen Proben belegen Schalterkontrolle und tatsächliches Hochlaufen, weder eine eingeschwungene maximale Nenndrehzahl noch Dauerstabilität. HP-Dienste liefen weiter. Keine Leistungs-/Spannungssetter, Treiberinstallation oder Dienststeuerung waren Teil der Proben.

Der Benutzer bestätigte am **15. Oktober**, nach Abschluss dieser Proben, die aktuell im ursprünglichen OMEN Gaming Hub gewählte Lüftereinstellung **„Automatisch“**. Diese Angabe ist kein zeitgleicher Firmware-Readback zum GET um 08:58 oder zu den Schreibproben. Sie ergänzt den Benutzerkontext, belegt aber weder ein eigenes Automatik-Statusfeld noch die Gleichsetzung „Max-Fan Aus = Automatisch“. Für eine solche Zuordnung fehlen weiterhin vollständiger Modus-Readback und Restore; aufgrund dieser Antwort wurde keine Hardwareeinstellung verändert.

## Lüfterdienst, Kurve und Schnellschalter (Stand 16. Oktober 2026)

Seit 16. Oktober übernimmt ein eigener Lüfterdienst (Anmeldeaufgabe `OmenHubPP-Fan-<SID>`, geschützte Kopie unter `Program Files\OmenHubPlusPlus`, einmalige Windows-Freigabe) die Softwareregelung: Heartbeat `0x10` alle 20 s, Level `0x2E` alle 2 s, CPU-Tctl, GPU- und IR-Temperatur, Notfall-Vollkühlung und Glättung. Solange der Original-Hub läuft, wartet der Dienst; ohne Kurve sendet er nichts und das BIOS regelt.

- Das Lüfterfenster speichert die gemeinsame CPU-/GPU-Kurve nur noch als `active-fan-curve.json`, nicht mehr als Leistungsprofil.
- Der Schnellschalter auf der Startseite (rechts neben **Steuerung**) schreibt `fan-mode.json`: **Auto** folgt der gespeicherten Kurve, **Max** setzt beide Lüfter auf Höchststufe. Beim ersten Umschalten ohne installierten Dienst fragt Windows einmal nach der Freigabe.

Der folgende Abschnitt beschreibt den Stand vor dem Lüfterdienst.

## Warum eigene Lüfterkurven noch nicht angewendet werden

Der untersuchte Performance-/BIOS-Lüftermodus-SET **0x1A** koppelt Leistungsmodus und Lüfterzuständigkeit in einem gemeinsamen Payload. Das vollständige aktuelle Modusbild und seine unabhängige Rücklese-/Restore-Semantik sind noch nicht qualifiziert. Der Software-Level-SET **0x2E** wurde weder implementiert noch ausgeführt. Der als Lüfteranzahl benannte Aufruf **0x10** hat im Herstellerablauf Heartbeat-/Zustandswirkung und wurde ebenfalls nicht ausgeführt.

Der Hersteller regelt Softwarekurven anhand von CPU-, GPU- und zusätzlicher IR-/Oberflächensensoranforderung, mit gemeinsamer Auswahl, Glättung/Hysterese und modellabhängigen Unter-/Obergrenzen. Ein reiner CPU-/GPU-Editor ersetzt diesen Sicherheitsweg noch nicht. Die vorhandenen Kurven-Widgets speichern deshalb weiterhin Entwürfe. Für reales Apply fehlen abgesicherte Modusübernahme und Original-Restore, physischer Zusatzsensor-Messort/Ausfallverhalten, koordinierte Grenzwerte sowie Fehler-, AC-, Standby-/Resume- und Reboot-Proben. Max-Fan wird dafür nicht als Ersatzbeweis verwendet.

## Automatik und IR-Sensor: vertiefter Befund am 15. Oktober

`GetThermalControl` liest die gespeicherte Profileinstellung; `Auto` hat dort den Wert 1. `GetPowerModeNB` liest ebenfalls den gespeicherten Modus. Beide Herstellerhelfer können bei fehlendem Eintrag Standardwerte speichern und wurden deshalb für die Analyse nicht aufgerufen. `ProfileFactory` führt für diese installierte Version über `LocalProfileProvider` zur Publisher-Cache-Datei `OMEN/profiles.json`. Die Datei ist Konfiguration, kein Firmwarezustand. Die lesend untersuchte Default-Konfiguration enthielt ThermalControl 1, MaxFan 0, Mode 48 und eine deaktivierte eigene Kurve; ACTIVEPROFILE war der String `null`. Daraus wurde keine aktive Hardwaremodus-Zuordnung abgeleitet. Die früher betrachtete Registry-Route reicht für diesen Profilpfad nicht aus.

Im untersuchten Notebook-Controller liefert `IsNBBiosFanControl()` im Netzbetrieb immer false. Nur bei DC, Auto, Plattformfreigabe und passender BIOS-Version kann der BIOS-Pfad gewählt werden. Das erfolgreiche Setzen von 0x1A setzt anschließend die interne Controller-Variable; diese ist kein Firmware-GET. Der Softwarepfad ruft bei Auto die CPU-/GPU-/IR-Regelung auf. Die passende gebündelte Khalilah-Ressource setzt `BiosAutoFanControlInDc` nicht; der SDK-Standard ist false. Ein möglicher späterer Konfigurations-Override oder tatsächlicher DC-Übergang wurde nicht gemessen. **„Automatisch“ belegt somit keine Übernahme durch das BIOS.** In den untersuchten Pfaden wurde weiterhin kein vollständiger unabhängiger Modus-/Zuständigkeits-GET gefunden; keine benachbarten Opcodes wurden ausprobiert.

`GetIRSensorValue` verwendet für **Khalilah25C1 / 8E35** Selektor 0 des bekannten 0x23-GETs. Der Wechsel zu Board-/Ambient-Sensor 1 ist an Plattformen ab 26C1 und zusätzliche SystemDesign-Bits gebunden. Der Monitorpfad übernimmt Byte 0 und rechnet es für Fahrenheit ausdrücklich aus Celsius um. Damit ist die Firmware-Semantik **IR, °C** für diesen Pfad belegt. Physischer Sensorort, Genauigkeit, Ausfallverhalten und der komplette thermische Regelvertrag bleiben getrennte offene Punkte.

Der eigene Decoder erhält alle vier Rohbytes. Er akzeptiert für diese Diagnose nur 1…125 °C mit drei Nullbytes im Rest; Null, Sentinelwerte, unbekannte Bits, Fehlercodes und falsche Längen liefern einen expliziten Fehler. Diese Plausibilitätsgrenze ist kein freigegebenes thermisches Betriebslimit. Der Zusatzbefund ist ein eigenes Feld `ir_temperature`; er wird niemals als CPU-Tctl oder ACPI-Ersatzwert ausgegeben. Der vorhandene Max-Fan-Setter und seine Schreibfreigabe werden dadurch nicht erweitert.

## Gemeinsame Thermikdiagnose

`cooling-read` erfasst einmalig die bekannten HP-GETs, den qualifizierten CPU-Tctl-Reader und die öffentliche NVIDIA-Telemetrie. Jede Quelle erhält Anfang und Ende im selben monotonen Zeitbezug. Zum Abschluss darf selbst der Anfang ihrer Abfrage höchstens **2000 ms** zurückliegen. Die 2-s-Grenze ist eine konservative Diagnosevorgabe, kein belegter Watchdog- oder Regelzyklus. Sie erkennt auch eine langsame Abfrage, deren früh gelesener Wert inzwischen alt ist. Der Bericht prüft außerdem, ob die vor/nach der Erfassung gelesene Stromquelle bekannt und gleich ist; kurze zwischenzeitliche Wechsel werden damit nicht vollständig erfasst.

Fehlende Rechte, fehlerhafte Leser, unbekannte IR-Antworten und veraltete Messsätze erscheinen in `sensor_faults`; CLI-Exitcode 1 und `temperature_batch_valid = false` verhindern einen stillen Erfolg. Die erste CPU-Energieprobe bleibt eine Baseline ohne Paketwatt. Ein gültiger Temperatur-Messsatz gibt keine Steuerung frei: Firmwaremodus und Lüfterzuständigkeit bleiben ausdrücklich `null`, Softwarekurven gesperrt und `hardware_applied = false`. Die Diagnose lädt keine OMEN-DLL, liest keine Herstellerprofile als Steuerersatz und schreibt kein Benutzerjournal. Sie besitzt keinen dauerhaften Thread oder Timer.

Die feste lesende Gegenprüfung `scripts/check-cooling-read.ps1` vergleicht drei Aufnahmen mit ihren Zeitfenstern und kontrolliert GPU-Steuerwerte, HP-GPU-Status, Max-Fan sowie Journal/Startup vor und nach der Probe. Am **19:23 CEST** waren drei Messsätze in **616…631 ms** gültig: **86,375…86,75 °C CPU-Tctl**, **60…61 °C GPU**, **42 °C Firmware-IR**, **2900/3100 Firmware-RPM**. Eine vorherige Aufnahme um 19:21 dauerte 2024 ms und wurde mit Exitcode 1 korrekt verworfen; die Grenze wurde dafür nicht gelockert. Kontrollwerte blieben gleich. Dies sind Entwicklungsstichproben, kein Lastbenchmark oder vollständiger Thermik-Sicherheitsnachweis.

## Statische Quellen und Grenzen

Auswertung der lokal installierten OMEN-Gaming-Hub-Version **1101.2609.3.0**, ohne Ausführen ihrer DLLs. Die eigenen Schnittstellenbeobachtungen stammen aus den Managed-Methoden für `GetMaxFan`/`SetMaxFan`, Drehzahlen, FanTable und SoftwareFanControl. Relevante Quellenhashes:

| Lokale Quelldatei | SHA-256 |
| --- | --- |
| PerformanceControl.dll, 1.0.37 | `0308E96A2623F0DCBCE55D7DF5F35F619A8612431F8E8AB1FDC3187F5240228E` |
| HP.Omen.Core.Model.Device.dll | `FEB984522D08B627FE3140BF31A0B32A64D5A14AD0F933BF83532BC78CD8E4E9` |
| HP.Omen.Core.Common.dll | `50DE558636ED95DE4453B07150C9F29112E9C13D7584D6D80ABC0CDCC265CAA4` |
| HP.Omen.Background.PerformanceControl.dll | `CB10CB067F219CABC3A20D3C1192F3B7511621F392A85950C00321CE839B07B2` |
| HP.Omen.Features.Hardware.PerformanceMonitorBg.dll | `9037340906BD3B07036706E96A53F8937ACF608D92B6AEF766B1A8018A19182B` |
| HP.Omen.Features.Services.Profiles.Common.dll | `A732C920DDAAC0063F833640B89D038FBE9D658F5606A464E4F5031778B38BD8` |

Proprietäre Dateien, dekompilierte Quellen und vollständige lokale Dumps liegen ausschließlich in ignorierten Verzeichnissen. Die direkte Transportimplementierung benötigt diese DLLs zur Laufzeit nicht. Unabhängiger Alltagsbetrieb ohne HP-Controller und reale Neustart-/Power-Übergänge sind damit weiterhin nicht nachgewiesen.
