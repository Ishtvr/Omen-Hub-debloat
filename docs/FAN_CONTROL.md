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
| Zusatzsensor, Selektor 0 | 0x23 | 4 Bytes | Ausschließlich Rohbytes; noch kein qualifizierter CPU-/Oberflächentemperatur-Messwert |

Die gemessene Zuordnung lautet CPU/GPU, mit Capabilitybytes 0/0 und Thermalpolicy-Version 1. Der vollständige Tabellenblock hat SHA-256 **`A8E624C6573881F444E36D94A29CA0AF320248855377531F02A499437BEE57F2`**. Sein Header ist `[2,12]`, aber er enthält **elf** nichtleere Dreiergruppen. Byte 1 wird im untersuchten Herstellerparser nicht interpretiert und wird deshalb auch hier nicht als Zeilenzahl ausgegeben. Die CPU-/GPU-Paarung ändert ihren Abstand entlang der Tabelle; ein gemeinsamer Prozentwert oder konstanter Offset wird daraus nicht abgeleitet. Der Akustikrohwert ist keine kalibrierte dBA-Messung.

## Profil- und Restore-Verhalten

`max_fan` ist eine boolesche Profilvorgabe. Im nativen GPU-Profilfenster gibt es **Max-Fan im Profil: Nicht ändern / Ein / Aus**. „Nicht ändern“ entfernt nur diese Vorgabe aus dem Profil; es setzt keinen bereits aktiven Firmware-Override zurück. „Aus“ gibt den Max-Fan-Override frei und wählt keinen anderen BIOS-Lüftermodus. Speichern ändert ausschließlich Metadaten. Erst das ausdrückliche Anwenden startet den erhöhten Transaktionsprozess.

Vor dem ersten SET erfasst die Transaktion sämtliche Originalwerte. Der Max-Fan-Adapter prüft dabei Board/BIOS, Tabellenhash, Topologie, Thermalpolicy und die vollständigen reservierten Schalterbytes. Vor dem Schreiben wird das eigene erwartete Zustandsbild erneut mit der Firmware verglichen. Geänderte oder unbekannte Zustände blockieren die Operation. Diese Freigabe verlangt Netzbetrieb; die Capability ist ausdrücklich **nicht startup-safe**.

Das dauerhafte Journal enthält Board/BIOS, Tabellenidentität und den vollständigen ursprünglichen Max-Fan-Schalter. Eine neue Controller-Sitzung kann den Originalzustand restaurieren, wenn die aktuellen Werte noch dem Original oder dem eigenen angewendeten Ziel entsprechen. Ein fremder Zustand wird nicht überschrieben. Manipulierte oder widersprüchliche Restore-Bilder werden verworfen. RPM-Werte dienen nur zur Beobachtung und sind niemals Restore-Ziele. Der Diagnose-Checkpoint erhält ein bereits vorhandenes Benutzerjournal byteweise einschließlich Formatierung.

Lesen: `omen-hub-pp fan-read` oder `scripts/read-fan-state.ps1` in einer erhöhten Windows-PowerShell. `fan-validate-max` ist eine feste Qualifikationsprobe mit Ausgangsbedingung Max-Fan **Aus**, kurzem Ein-/Readback-Test und Original-Restore; sie ist kein gewöhnlicher Statusbefehl. Aktuelle Proben: [VALIDATION.md](VALIDATION.md), Gerätebindung: [models/8e35.md](models/8e35.md).

## Reale Qualifikation

- **08:58 CEST:** Sechs feste GETs erfolgreich. Ausgangsflag `[0,0,0,0]`; Firmware-Drehzahlen 2900/3100 RPM. Ein vorheriger unelevierter Versuch erhielt AccessDenied vor dem ersten BIOS-Aufruf.
- **09:17 CEST:** Fester ON-/Readback-/Restore-Trial mit Release-Binary SHA-256 `063CF16D4E02A522E3B9F883E07167B95B7A825BEC456625E6F23A9E5E0D69D4`. Zwei gehaltene Abfragen lesen `[1,0,0,0]` und 3300/3600, anschließend 4100/4200 RPM. Der ursprüngliche vollständige Schalter `[0,0,0,0]` wurde wiederhergestellt. Danach 3700/3900 RPM durch Auslauf; dies ist kein fehlgeschlagener Schalter-Restore.
- **09:30 CEST:** Eigenes isoliertes Proof-Programm aus aktuellen Core-/Platform-Crates, SHA-256 `D17C25B49500369CD8E9FD50C2DA1D17F0C105CD7173078683142FD99F8E245D`. Gespeichertes Apply blieb vier Sekunden aktiv, Firmware-Drehzahlen 4300/4400 RPM. Ein neuer Prozess verwarf zunächst ein Restore-Bild mit gefälschtem Tabellenhash ohne Schalteränderung und restaurierte dann über das dauerhafte Journal den ursprünglichen Aus-Zustand. Kein Recovery-Eintrag blieb offen.

Programmierbare NVIDIA-Steuerwerte, HP-GPU-Leistungsstatus sowie das produktive Benutzerjournal und die Startup-Auswahl blieben bei beiden Schreibproben unverändert. Die kurzen Proben belegen Schalterkontrolle und tatsächliches Hochlaufen, weder eine eingeschwungene maximale Nenndrehzahl noch Dauerstabilität. HP-Dienste liefen weiter. Keine Leistungs-/Spannungssetter, Treiberinstallation oder Dienststeuerung waren Teil der Proben.

Der Benutzer bestätigte am **15. Oktober**, nach Abschluss dieser Proben, die aktuell im ursprünglichen OMEN Gaming Hub gewählte Lüftereinstellung **„Automatisch“**. Diese Angabe ist kein zeitgleicher Firmware-Readback zum GET um 08:58 oder zu den Schreibproben. Sie ergänzt den Benutzerkontext, belegt aber weder ein eigenes Automatik-Statusfeld noch die Gleichsetzung „Max-Fan Aus = Automatisch“. Für eine solche Zuordnung fehlen weiterhin vollständiger Modus-Readback und Restore; aufgrund dieser Antwort wurde keine Hardwareeinstellung verändert.

## Warum eigene Lüfterkurven noch nicht angewendet werden

Der untersuchte Performance-/BIOS-Lüftermodus-SET **0x1A** koppelt Leistungsmodus und Lüfterzuständigkeit in einem gemeinsamen Payload. Das vollständige aktuelle Modusbild und seine unabhängige Rücklese-/Restore-Semantik sind noch nicht qualifiziert. Der Software-Level-SET **0x2E** wurde weder implementiert noch ausgeführt. Der als Lüfteranzahl benannte Aufruf **0x10** hat im Herstellerablauf Heartbeat-/Zustandswirkung und wurde ebenfalls nicht ausgeführt.

Der Hersteller regelt Softwarekurven anhand von CPU-, GPU- und zusätzlicher IR-/Oberflächensensoranforderung, mit gemeinsamer Auswahl, Glättung/Hysterese und modellabhängigen Unter-/Obergrenzen. Ein reiner CPU-/GPU-Editor ersetzt diesen Sicherheitsweg noch nicht. Die vorhandenen Kurven-Widgets speichern deshalb weiterhin Entwürfe. Für reales Apply fehlen abgesicherte Modusübernahme und Original-Restore, die qualifizierte Zusatzsensorbedeutung, koordinierte Grenzwerte sowie Fehler-, AC-, Standby-/Resume- und Reboot-Proben. Max-Fan wird dafür nicht als Ersatzbeweis verwendet.

## Statische Quellen und Grenzen

Auswertung der lokal installierten OMEN-Gaming-Hub-Version **1101.2609.3.0**, ohne Ausführen ihrer DLLs. Die eigenen Schnittstellenbeobachtungen stammen aus den Managed-Methoden für `GetMaxFan`/`SetMaxFan`, Drehzahlen, FanTable und SoftwareFanControl. Relevante Quellenhashes:

| Lokale Quelldatei | SHA-256 |
| --- | --- |
| PerformanceControl.dll, 1.0.37 | `0308E96A2623F0DCBCE55D7DF5F35F619A8612431F8E8AB1FDC3187F5240228E` |
| HP.Omen.Core.Model.Device.dll | `FEB984522D08B627FE3140BF31A0B32A64D5A14AD0F933BF83532BC78CD8E4E9` |
| HP.Omen.Core.Common.dll | `50DE558636ED95DE4453B07150C9F29112E9C13D7584D6D80ABC0CDCC265CAA4` |

Proprietäre Dateien, dekompilierte Quellen und vollständige lokale Dumps liegen ausschließlich in ignorierten Verzeichnissen. Die direkte Transportimplementierung benötigt diese DLLs zur Laufzeit nicht. Unabhängiger Alltagsbetrieb ohne HP-Controller und reale Neustart-/Power-Übergänge sind damit weiterhin nicht nachgewiesen.
