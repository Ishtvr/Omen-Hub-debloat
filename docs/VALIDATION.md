# Validierungsstand — 13. Oktober 2026

Referenz: HP OMEN 16-ap0xxx, Board 8E35, BIOS F.13, Ryzen 9 8940HX, RTX 5070 Laptop GPU. Windows x64. Build-Werkzeuge: Rust/Cargo 1.99.0, MSVC-Toolchain. Auf anderen Geräten ist dieser Stand nicht validiert.

## Bereits geprüft

- Die inzwischen **63 Rust-Tests** behandeln typisierte Profile/Einheiten, Gesamt-Preflight, fehlerhafte Setter mit bereits erfolgter Mutation, Treiberrundung, gekoppelte Werte, Netzteilverlust nach dem letzten Schritt, Trial-Restore, persistierte Recovery, atomaren Speicher, Startup-Revisionsbindung, beschädigte Dateien, strukturelle Task-SID-Prüfung, Migrationssperren und die plattformfreien Kurvenregeln. Die zusätzlichen Kurventests prüfen Gruppen-/Bereichsgrenzen, unterschiedliche Shift-/Alt-Bezugsformen, Strg-Neigung, atomare Ablehnung, begrenzte Historie, Abbruch eines Drags sowie Zoom/Pan ohne Datenänderung. Die Unit-Tests schreiben keine Hardwarewerte. Reale Taktversuche sind separat dokumentiert.
- `cargo fmt --all -- --check` und `cargo clippy --workspace --all-targets --locked -- -D warnings` liefen erfolgreich. Der optimierte Windows-Release wurde gebaut.
- `diagnose` liest Modell/Board/BIOS, CPU/GPU-Namen, AC/Akku und NVIDIA-Telemetrie ohne Warnung. Eine Stichprobe am 13. Oktober lieferte 46 °C, 2280 MHz und 31,168 W. Das sind Momentwerte während der Entwicklung, keine Gaming-Lastmessung.
- Nach der CPU-Erweiterung lieferte `diagnose` zusätzlich 14,02 % CPU-Auslastung, 2401 MHz als Windows-Meldung und 32 logische Prozessoren, ebenfalls ohne Warnung. Die GPU meldete dabei 45 °C, 2362 MHz und 33,961 W. CPU-Paketleistung und Prozessortemperatur blieben korrekt `null`. Die CPU-Auslastung ist aktive Zeit, die MHz sind kein gemessener Boost; diese Entwicklungsstichprobe ist kein Leistungsbenchmark.
- NVIDIA meldet 80 W Standard-Power-Limit und 115 W maximale Power-Limit-Constraint. Ein aktuell gesetztes Limit war bei der früheren `nvidia-smi`-Abfrage nicht verfügbar. Es wurde keine dauerhafte 115-W-Leistung nachgewiesen.
- `scripts/smoke-ui.ps1` startet ausschließlich einen eigenen Testprozess verborgen, prüft native Fenster-/Control-Erzeugung, Inventartext, GPU-/CPU-Abschnitte, qualifizierten Windows-Takt, echte CPU-Auslastung nach der Baseline, korrekt fehlende CPU-Watt-/Temperaturwerte, Umschaltung der ausdrücklich als Vorschau bezeichneten Profilauswahl, Close-to-Tray und vollständiges Beenden innerhalb von drei Sekunden. Dieser Funktionstest besteht. Er ist kein visueller Screenshot-/DPI-Test und setzt das Referenzgerät 8E35 voraus.
- `hp-read` und `keyboard-read` beenden sich bei fehlendem Zugriff mit Exitcode 1 und `0x80041003`. Erhöht ist der Transport erreichbar. GPU-Status **[1,1,0,87]** wird inzwischen mit bekannten Bool-Schaltern und explizit unbekanntem D-State 0 gelesen; der Benutzer bestätigt gleichzeitig Unleashed im Original-Hub. Die ältere D0-Abweisung bleibt nur als historischer Probe-Befund erhalten. Frischer SystemDesignData-GET bestätigte 128 Bytes, Shipping-Adapter 230 W, Rohbyte 0x0B und Default-Shared-Budget 45. Keine HP-Setter oder Heartbeats wurden ausgeführt.
- Der erhöhte Tastatur-Gegencheck unter Windows PowerShell **5.1.26100.9444** vom 13. Oktober um **13:43 Uhr** lieferte drei erfolgreiche GETs: Capability **[1, 0, 0, 0] / FourZoneWithNumpad**, 128 Farbbytes und 128 Helligkeitsbytes, jeweils Firmwarecode 0. Eine erhöhte native Rust-Probe um **14:04 Uhr** bestätigte dieselbe Capability und byteweise gleiche vollständige Blöcke mit CLI-Exitcode 0. Helligkeit **228 / 0xE4** bleibt ein Rohbyte. Die native Größe-0-Eingabe **VT_ARRAY | VT_UI1 mit Null-SAFEARRAY** funktioniert am Provider. Rohdaten stehen ausschließlich unter ignorierter `.local/research/keyboard-state/`.
- Die erste native Probe legte den Verlust von `__PATH` bei generischer WMI-Map-Deserialisierung offen. Der Reader liest nun die COM-Systemfelder direkt und verlangt genau eine aktive Instanz. Diese erste Probe stoppte vor einem Firmwareaufruf. Die erfolgreichen GETs laden keine HP-RPC-/SDK-Wrapper, jedoch liefen HPOmenCap, HPSysInfoCap und HPAppHelperCap weiterhin; ein Betrieb ohne diese Controller ist dadurch noch nicht bewiesen.
- Für die Lese- und Taktproben wurden einzelne kurzlebige UAC-Prozesse verwendet; Codex und Tray blieben uneleviert. `-ExecutionPolicy Bypass` betrifft nur den jeweiligen Prozess. Es wurden keine gespeicherten Richtlinien, Herstellerdienste, Kernel-Treiber, Firmware, CPU-/GPU-Power-Limits, Spannungen oder Beleuchtung geändert. Eine eigene geschützte Task-Scheduler-Aufgabe für den festen Profil-Einstieg wurde eingerichtet; die automatische Profilauswahl ist nach den Tests ausgeschaltet.

## GPU-Schreib- und Profilnachweis

Das tatsächliche Gerät meldet NVIDIA 617.14, VBIOS 98.06.2a.40.da und eine einzelne RTX 5070 Laptop. Öffentliche NVML-GETs liefern Core/Memory-Offsets und signierte Grenzen. Diese beiden Offset-Adapter und die separat belegte vollständige Graphics-V/F-Frequenzsteuerung sind freigegeben. Direkte Voltage- und HP-Power-Setter sind nicht freigegeben.

- **15:17 CEST:** Core −15 und VRAM −24 MHz exakt zurückgelesen, komplette Originalbilder wiederhergestellt, Endwerte 0/0. Testbinary 942080 Bytes, SHA-256 `2938F21C79778943DED915A20D771408CB38F9197A23311D4C6249EAD2F0B50F`.
- Ein vorheriger Versuch mit VRAM −25 MHz las −24 zurück. Trotz Treiberrückgabe 0 meldete die Anwendung Fehler und verifizierte Restore beider Domains; kein stilles Runden.
- **16:07 CEST:** Zwei feste Trials −15/−24 und +15/+24 MHz, jeweils exakter Soll-Readback und Restore zu 0/0. Binary 1178624 Bytes, SHA-256 `A1B027781DDE34BD3E1FF586BDC16E0E80AC3946AF1886DA37B116A0205792B7`.
- **16:48 CEST:** Damaliger Release 1250304 Bytes, SHA-256 `F6E8D089014FF3532F42B174DF8CA92D0FC8352D907FF18773B25FD09CC33C3D`: gespeichertes −15/−24-Profil wirklich angewendet, unabhängiger Readback, persistierter Original-Restore in einem neuen Prozess. Startup-Auswahl einer nur zurückgestellten Revision wird abgewiesen. Danach ein 0/0-Profil angewendet/geschützt bestätigt und der installierte feste Task-Einstieg tatsächlich gestartet; er las die zwei Null-Offsets erfolgreich zurück. Abschließend Restore und Auswahl ausgeschaltet, Testprofil entfernt, Endwerte 0/0.

Die ersten Task-Tests sperrten korrekt bei normalisierten Kontonamen und zeigten anschließend getrennte AppData-Dateien: Start aus dem Codex-MSIX-Kontext schrieb im Paket-Cache, Task Scheduler sah den normalen AppData-Pfad. Ein Kernel-Dateipfad-Gegencheck belegte die tatsächlichen unterschiedlichen Ziele. Die korrigierte Engine ermittelt das Benutzerprofil aus dem Windows-Token und speichert in `.omen-hub-pp`; Task und normaler/UAC-Prozess sehen jetzt denselben Ort. Die kontrollierte Migration behielt alte Daten und Originalbilder, ohne Startup auszuwählen.

Der Task-Nachweis ist **kein echter Neustarttest**. Er verwendete bereits aktuelle Null-Offsets; dabei wurden keine Taktwerte verändert. Positives OC wurde nur kurz und klein zur Vertrag-/Restore-Prüfung angewendet, nicht unter Last auf Stabilität getestet. Längerfristige OC-Stabilität, Power-Lifecycle und vollständiger OEM-Ersatz bleiben unbewiesen.

`scripts/smoke-gpu-ui.ps1` prüft ausschließlich einen eigenen Testprozess: echte GPU-GETs, erklärtes Unsupported für Voltage/Unleashed, numerische Eingabeprüfung, deaktiviertes Apply für ungespeicherte Entwürfe, Speichern eines typisierten Profils ohne GPU-Schreibzugriff, unveränderte Offsets und sauberes Schließen. Der Test besteht. Er löst keine UAC-/Apply-/Restore-Aktion aus. Impeccables mechanischer Detector meldet keine Findings in den geänderten Rust-UI-Dateien; das ersetzt keinen nativen Screenshot-/DPI-Nachweis.

## Ergänzter V/F- und Kurveneditor-Nachweis

- **17:22 CEST:** Native private GETs lieferten Status 0, 127 Graphics-Punkte mit festen Ankern 450…1240 mV sowie fünf Memory-Domänenpunkte. Die tatsächlich geladene Implementierung 617.14 ist auf SHA-256 `FF1527CEA533C75F89033F7C0D9CDBAA6EC84BCAC26C1DB082227D68EFD6A16E` begrenzt. Ihre v1-Strukturen wurden am nativen Treiber nachvollzogen.
- **18:02 CEST:** Alle 127 Graphics-Deltas −15000 kHz geschrieben, vollständig exakt zurückgelesen und restauriert. Binary 1321984 Bytes, SHA-256 `3BDF3D1F850D3E867159080A07F4807389B8F4AC020FC5986FB4A274EA718551`. Spannungsanker und Power-/Voltage-Felder wurden nicht geschrieben.
- Ein kombinierter Versuch zeigte, dass NVML-Memory-SET eine zuvor programmierte individuelle Graphics-Kurve löscht. Die Gesamtprüfung erkannte den Verlust und restaurierte sämtliche Originale. Der Adapter erfasst und erhält nun auch die Graphics-Kurve und berücksichtigt die Abhängigkeit der Schreib-/Restore-Reihenfolge.
- **18:57 CEST:** Gespeichertes Profil mit einem Graphics-Punkt −15 MHz und Memory −24 MHz erfolgreich angewendet, unabhängig gelesen und aus einem neuen Prozess restauriert. Anschließend tatsächlicher geschützter Task-Aufruf für eine unveränderte vollständige Kurve plus Memory 0; Ergebnis 0. SHA-256 `3DD8A8B459DAD3643E7513D103E535702E22505ED8F06B6FA02B581CFE71C697`.
- **19:15 CEST:** Fester Trial setzt zunächst eine individuelle Graphics-Kurve und danach Memory −24 MHz. Die Kurve bleibt trotz gemessener Setter-Kopplung vollständig erhalten; anschließend werden alle Originale restauriert. SHA-256 `ECDC96BE1ECFF9DC33096250B3838B966E2D8923CA6AAF4B6325B6FEA840CCF3`.
- **19:26/19:35 CEST:** Gespeichertes Core-/Memory-Profil und unabhängiger Restore bestanden. Beim Core-SET −15 MHz wurden auch alle 127 privaten Graphics-Deltas −15000 kHz beobachtet; eine einzelne Offset-Zahl ersetzt keinen vollständigen Kurvenzustand. Der Task lieferte einen erfolgreichen 0/0-Readback; die erste Testhilfe bewertete seinen Abschlussstatus vorzeitig. Der korrigierte Gegencheck wartet auch auf `LastTaskResult == 0` und bestand. Keine App-Fehlermeldung wurde als Erfolg umgedeutet; der ursprüngliche Testbefund bleibt lokal erhalten.
- **19:55 CEST:** Gespeicherte Kurve mit 118 unveränderten und neun oberen Punkten −15 MHz plus Memory −24 MHz bestand Apply, kompletten Readback und Restore aus einem neuen Prozess. Der installierte Task prüfte anschließend ein vollständiges Kurvenprofil mit Ergebnis 0. Binary 1381376 Bytes, SHA-256 `D8D57548EB6FF9DD188FFC46EB77484980AD90305D9D7D31D48D5CCCA3CB9E54`. Endzustand: alle Graphics-Deltas 0, globale Offsets 0/0, automatische Auswahl aus.

Der neue Kurveneditor hat separate GPU-V/F-, CPU-Lüfter- und GPU-Lüfter-Ansichten. `scripts/smoke-curves-ui.ps1` bestand Maus-/Tastaturbedienung, unveränderte Spannungsanker, Lüfterpunkte hinzufügen/entfernen, getrennte Speicherung, Neuladen und Erhalt des VRAM-Parameters. Weder globale Offsets noch programmierte Graphics-Deltas änderten sich. Profile mit Lüfter-Entwürfen bleiben tatsächlich gesperrt; dies ist keine Lüfter-Schreibvalidierung.

Die GDI-Widgets wurden in eigene Speicher-Bitmaps gerendert und gemeinsam visuell geprüft; Achsenabstände und Einheiten wurden in einer Korrekturrunde berichtigt und erneut bestätigt. Diese Artefakte enthalten ausschließlich unsere Zeichnung, keine Desktopaufnahme. Vollständige native Fenster-/DPI-Prüfung bleibt offen. Der mechanische Impeccable-Detector meldete keine Findings; das ersetzt keine visuelle Geräteabnahme.

## Tray-Messung

Der erste Release vor der Isolation des WMI-Inventars hatte 348160 Bytes. Nach rund 29 Sekunden im Tray wurden **16,20 MiB Working Set** und **2,58 MiB Private Memory** gemessen. Die CPU-Zeit betrug 0,03125 Sekunden; zwischen der 5-Sekunden- und Schlussprobe kam keine messbare CPU-Zeit hinzu. Es wurde kein Working-Set-Trimming erzwungen.

Der anschließend gebaute Release mit atomarer Inventory-Job-Zuweisung wurde am 13. Oktober um 12:08 Uhr (Europe/Berlin) erneut 30 Sekunden gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 460288 Bytes (449,5 KiB) |
| Working Set | 11,46 MiB (rund 12,02 MB) |
| Private Memory | 1,90 MiB |
| Gemessene CPU-Zeit | 0,0 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `AFC9C467A2644FAC4CFA4EE41DE2F86222021F05934EFB8917ECD6974D27E683` |

Das vom Benutzer angehobene Ziel beträgt **50 MB Working Set im Tray-Leerlauf**. Die Messung liegt darunter. Sie gilt für den Diagnose-Prototyp vor dem UI-Neuentwurf, nicht als vorweggenommene Messung der kommenden Oberfläche.

Der Release mit CPU-Telemetrie und präzisierter Windows-Taktbeschreibung wurde am 13. Oktober um 12:37 Uhr (Europe/Berlin) erneut 30 Sekunden im Tray gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 468480 Bytes (457,5 KiB) |
| Working Set | 11,49 MiB (rund 12,05 MB) |
| Private Memory | 1,90 MiB |
| Gemessene CPU-Zeit | 0,015625 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `05CF544B5FA4884D6A344C46850AB29B3294CF704DA02B763DE80BEF81AE224E` |

Der Release mit nativem Tastatur-GET und präzisierten HP-Fehlerdiagnosen wurde am 13. Oktober um **14:05 Uhr** (Europe/Berlin) erneut 30 Sekunden im Tray gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 481280 Bytes (470 KiB) |
| Working Set | 11,55 MiB (rund 12,11 MB) |
| Private Memory | 1,92 MiB |
| Gemessene CPU-Zeit | 0,015625 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `1AEDF63C921222C83B54E67B2C253EB158BAB8EF75F44B35737C1F6FB59D93DE` |

Die native Tastaturprobe um 14:04 Uhr verwendete diesen identischen SHA-256. CPU, NVIDIA und Tastatur-BIOS werden im Tray nicht periodisch abgefragt; die HP-GETs laufen ausschließlich auf separaten CLI-Aufruf. Diese Messung betrifft den Start mit `--tray`; eine Messung nach Öffnen/Schließen und die kommende Neugestaltung bleiben separat zu prüfen.

Der Release mit GPU-Profilen, persistierter Recovery, geschütztem Task-Einstieg und gemeinsamem Benutzerprofilspeicher wurde am **13. Oktober um 16:50 Uhr** erneut 30 Sekunden im Tray gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 1250304 Bytes (rund 1,25 MB) |
| Working Set | **12,37 MiB (rund 12,97 MB)** |
| Private Memory | 2,00 MiB |
| Gemessene CPU-Zeit | 0,0625 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `F6E8D089014FF3532F42B174DF8CA92D0FC8352D907FF18773B25FD09CC33C3D` |

Das 50-MB-Ziel wird in dieser Tray-Start-Stichprobe eingehalten. Der separate GPU-Editor war geschlossen; seine Anzeige ist keine Tray-Leerlaufmessung. Diese kurze Stichprobe ersetzt keinen Dauerlauf.

Die separate Impeccable-Finish-Prüfung der CPU-Erweiterung hat die Disposition **`recapture`**, weil kein gültiger nativer Fenster-Screenshot samt Skalierungsangabe vorliegt. Sie ist keine visuelle Freigabe. Der einzige separate Code-/Textpunkt zur Windows-Taktbeschreibung wurde korrigiert und als `resolved` bewertet; der danach wiederholte Funktionstest enthält den korrigierten Text. Schrift, Kontraste, Abschneiden und DPI sind weiterhin unbewertet.

Working Set und Private Memory sind verschiedene Windows-Kennzahlen und werden nicht addiert. „Keine messbare CPU-Zeit“ bezieht sich auf das Messintervall und die Auflösung des Prozesszählers. Die kurze Messung bestätigt das Speicherziel für diesen Build auf diesem Gerät; sie ersetzt keinen längeren Dauerlauf. `scripts/measure-tray.ps1` schreibt Größe, SHA-256 und Messwerte lokal in `.local/tray-measurement.json`.

## Finaler Release mit nativen Kurven-Widgets

Am 13. Oktober um **21:04 CEST** wurde die finale Binary nach 30 Sekunden im Tray gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 1502720 Bytes (rund 1,50 MB) |
| Working Set | **12,40 MiB (rund 13,00 MB)** |
| Private Memory | 1,95 MiB |
| CPU-Zeit | 0,03125 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `50A73644DC567496DE0772BA61B6A22F658A52DE96014A9EEE368A1A96A2B829` |

Dieser Build liegt unter dem 50-MB-Tray-Ziel. Hauptfenster-, GPU-Profil- und Kurveneditor-Funktionstests bestehen mit derselben Binary. Die 57 Rust-Tests, Formatprüfung, Clippy ohne Warnungen und der optimierte Release-Build sind erfolgreich. Die Widgets wurden mit der tatsächlich verwendeten Segoe-UI-Schrift gerendert und in zwei begrenzten visuellen Runden bestätigt. Es wurden keine Hardware-Schreibversuche zur UI-Verifikation ausgeführt. Testprofile wurden entfernt; globale GPU-/VRAM-Offsets und alle Graphics-Deltas stehen wieder bei 0, die Anmeldungsauswahl ist ausgeschaltet.

## Erweiterter V/F-Editor im Stil der Benutzerreferenz

Am **13. Oktober um 22:51 CEST** wurde der finale Release mit dunklem GDI-Diagramm, quadratischen Punkten, geladener Referenz, Bereichsauswahl, Gruppenbearbeitung, Historie und Zoom/Pan erneut 30 Sekunden im Tray gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 1539584 Bytes (rund 1,54 MB) |
| Working Set | **12,55 MiB (rund 13,16 MB)** |
| Private Memory | 2,00 MiB |
| CPU-Zeit | 0,03125 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `5812C234C1E8FE61067367E01A35812966D5EC8A541283F9662B438184DBB270` |

Formatprüfung, **63 Workspace-Tests**, Clippy ohne Warnungen und Release-Build bestehen. Alle drei nativen Funktionstests bestehen mit dieser Binary. Der Kurventest prüft echtes Shift-Ziehen aller 127 Punkte mit gemeinsamem Offset, feste Spannungsanker, Bereichsauswahl mit Shift+Enter, einen Rückgängig-Schritt pro Drag, Wiederholen, Zoom/Fit ohne Kurvenänderung sowie getrennte Lüfterkurven, Speicherung und Erhalt des VRAM-Settings. Der GPU-Profiltest berücksichtigt jetzt auch eine bereits aktive individuelle Kurve; er prüft die aktivierte Eingabe statt eines absichtlich gesperrten globalen Core-Feldes. Die vollständige importierte Kurve bleibt bei Namensänderungen erhalten.

Die lesende Abschlussstichprobe um **22:35 CEST** liefert weiterhin 8E35/F.13, RTX 5070 Laptop und NVIDIA 617.14. Globale Core-/Memory-Offsets stehen bei **0/0 MHz**, die 127 Graphics-Punkte enthalten aber **31 verschiedene programmierte Deltas**. Der vorhandene individuelle Kurvenzustand wird erhalten; die Capability sperrt deshalb den globalen Core-Offset. Die früheren Restore-Endstände mit lauter 0-Deltas sind historische Messungen, keine Aussage über diese spätere Lesestichprobe. In dieser UI-Arbeit wurden keine GPU-, Power-, Voltage- oder HP-Setter ausgeführt; der Funktionstest vergleicht die programmierten Controls vor und nach der Bedienung. Weder eine Herkunft der vorhandenen Kurve noch deren Laststabilität wird daraus abgeleitet.

Die zwei begrenzten visuellen Runden verwenden den tatsächlichen GDI-Canvas und die eigenen nativen Fenster-Controls als **Client-Renderings im Speicher**, keine Aufnahme des Desktops. Die separate Impeccable-Prüfung hat **`ship`** für die gezeigte Diagrammansicht und Controls bei 1364×901 sowie 944×671 logischen Pixeln vergeben. Der markierte Beispielentwurf ist sichtbar als nicht gespeichert und nicht angewendet gekennzeichnet. Die Renderings stammen vom Build `F156650C…`; danach wurden der Supporttext im separaten GPU-Profilfenster und das Abbrechen einer laufenden Bereichsauswahl ergänzt. Die geprüften Zeichenfunktionen, Farben und Layoutregeln blieben unverändert.

Die vollständige Zahlen-/Profilansicht ist **nicht visuell freigegeben**: versteckte native EDIT-/COMBOBOX-Texte werden durch WM_PRINT unvollständig dargestellt, weshalb dieses Rendering ausgeschlossen wurde. Deren Werte und Bedienung sind durch die eigenen Control-Funktionstests geprüft. Titelrahmen, mehrere DPI-Konfigurationen und eine vollständige Accessibility-Prüfung bleiben offen. Der HTML-/CSS-Detektor ist für Win32/GDI nicht anwendbar. Fester Spannungs-/Takt-Lock, OC Scanner und Hardware-Anwendung von Lüfterkurven sind weiterhin nicht implementiert.

## Noch ausstehend

Die Tastatur-GETs sind auf 8E35/F.13 bestätigt, keine Beleuchtungs-Schreibfunktion ist freigegeben. Quellen, Payload-Unterschiede und die verbleibenden Freigabekriterien stehen in [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md). Herstellerdienste und Treiberzustände wurden nicht verändert; der native Analyseprozess führt die importierten Dateien nicht aus.

- Bedeutung von D-State 0; der Vergleich mit der Benutzerangabe Unleashed ist erfolgt, eine Feldbedeutung folgt daraus nicht.
- Typgerechter Tastatur-Setter, verifizierter Readback nach einer Änderung und vollständiges Restore. Eigenständiger Betrieb ohne ursprüngliche HP-Omen-Controller ist noch nicht gemessen.
- OEM-Leistungsrahmen, Dynamic Boost, Netzteilanforderungen, CPU/GPU-Verteilung und Akkudrain unter vergleichbarer Last.
- Echte Standby-/Resume-/AC-Wechsel, Display-Treiber-Neustart und MUX-Neustart. Lifecycle-Policy und Event-Handler sind implementiert, aber deren tatsächliche Hardwareübergänge nicht getestet.
- Gezielt hängender WMI-Provider und verlorene Treibersitzung; Abgrenzung/Timeout/Backoff sind implementiert, die Störfälle noch nicht auf dem echten System provoziert.
- CPU-Paketleistung, Prozessortemperatur und gemessener Boost-/Effektivtakt: AMD-SDK-/HP-Sensorpfade sind statisch untersucht, echte Hardware-Messwerte und SDK-Initialisierung noch nicht validiert. Keine ACPI-Zone und kein konfiguriertes CPU-Power-Limit werden als Messwert verwendet.
- Lüfter, Max-Fan, RGB, MUX, CPU-Tuning, GPU-/VRAM-Spannung und vollständiger Unleashed-Adapter. Tatsächlicher Reboot mit ausgewähltem Profil und ein gegebenenfalls nötiger privilegierter Dienst.
- Visuelle Prüfung und Skalierung bei mehreren DPI-/Display-Konfigurationen; längere Tray- und Sensor-Dauerläufe.

Die Runtime enthält keine Netzwerkaufrufe. Build und lokale Werkzeuginstallation benötigen dagegen Downloads. Proprietäre Analysedaten und lokale Diagnoseberichte liegen in ignorierten `.local/`- bzw. `.tools/`-Ordnern. Versioniert werden Originalcode und kompakte Interface- und Validierungsbefunde.
