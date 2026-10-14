# Validierungsstand — 14. Oktober 2026

Referenz: HP OMEN 16-ap0xxx, Board 8E35, BIOS F.13, Ryzen 9 8940HX, RTX 5070 Laptop GPU. Windows x64. Build-Werkzeuge: Rust/Cargo 1.99.0, MSVC-Toolchain. Auf anderen Geräten ist dieser Stand nicht validiert.

## Bereits geprüft

- Die inzwischen **77 Rust-Tests** (57 Core, 16 Platform, vier App-Tests) behandeln typisierte Profile/Einheiten, Gesamt-Preflight, fehlerhafte Setter mit bereits erfolgter Mutation, Treiberrundung, gekoppelte Werte, Netzteilverlust nach dem letzten Schritt, Trial-Restore, persistierte Recovery, atomaren Speicher, Startup-Revisionsbindung, beschädigte Dateien, strukturelle Task-SID-Prüfung, Migrationssperren und die plattformfreien Kurvenregeln. Die Kurventests prüfen Gruppen-/Bereichsgrenzen, unterschiedliche Shift-/Alt-Bezugsformen, Strg-Neigung, atomare Ablehnung, begrenzte Historie, Abbruch eines Drags sowie Zoom/Pan ohne Datenänderung. Die RGB-Tests prüfen erhaltene Steuerbytes und gefälschte Bilder; die native Snapshot-Prüfung verwirft alte GET-Anzeigen. Zusätzliche CPU-Tests prüfen Tctl-Bereichsflags, Energieeinheiten/Überlauf/Intervalle, beschädigte Messprovenienz, genaue öffentliche PawnIO-Frames, CPU-Berichtseinheiten, deutsche Fehlerklassen und Erhalt des Hardwarejournals/der Startup-Freigabe bei separater Diagnoseablage. Die Unit-Tests schreiben keine Hardwarewerte. Reale Takt-/RGB-/CPU-Leseproben sind separat dokumentiert.
- `cargo fmt --all -- --check` und `cargo clippy --workspace --all-targets --locked -- -D warnings` liefen erfolgreich. Der optimierte Windows-Release wurde gebaut.
- `diagnose` liest Modell/Board/BIOS, CPU/GPU-Namen, AC/Akku und NVIDIA-Telemetrie ohne Warnung. Eine Stichprobe am 13. Oktober lieferte 46 °C, 2280 MHz und 31,168 W. Das sind Momentwerte während der Entwicklung, keine Gaming-Lastmessung.
- Nach der CPU-Erweiterung lieferte `diagnose` zusätzlich 14,02 % CPU-Auslastung, 2401 MHz als Windows-Meldung und 32 logische Prozessoren, ebenfalls ohne Warnung. Die GPU meldete dabei 45 °C, 2362 MHz und 33,961 W. CPU-Paketleistung und Prozessortemperatur blieben korrekt `null`. Die CPU-Auslastung ist aktive Zeit, die MHz sind kein gemessener Boost; diese Entwicklungsstichprobe ist kein Leistungsbenchmark.
- NVIDIA meldet 80 W Standard-Power-Limit und 115 W maximale Power-Limit-Constraint. Ein aktuell gesetztes Limit war bei der früheren `nvidia-smi`-Abfrage nicht verfügbar. Es wurde keine dauerhafte 115-W-Leistung nachgewiesen.
- `scripts/smoke-ui.ps1` startet ausschließlich einen eigenen Testprozess verborgen, prüft native Fenster-/Control-Erzeugung, Inventartext, GPU-/CPU-Abschnitte, qualifizierten Windows-Takt, echte CPU-Auslastung nach der Baseline, korrekt fehlende CPU-Watt-/Temperaturwerte, Umschaltung der ausdrücklich als Vorschau bezeichneten Profilauswahl, Close-to-Tray und vollständiges Beenden innerhalb von drei Sekunden. Dieser Funktionstest besteht. Er ist kein visueller Screenshot-/DPI-Test und setzt das Referenzgerät 8E35 voraus.
- `hp-read` und `keyboard-read` beenden sich bei fehlendem Zugriff mit Exitcode 1 und `0x80041003`. Erhöht ist der Transport erreichbar. GPU-Status **[1,1,0,87]** wird inzwischen mit bekannten Bool-Schaltern und explizit unbekanntem D-State 0 gelesen; der Benutzer bestätigt gleichzeitig Unleashed im Original-Hub. Die ältere D0-Abweisung bleibt nur als historischer Probe-Befund erhalten. Frischer SystemDesignData-GET bestätigte 128 Bytes, Shipping-Adapter 230 W, Rohbyte 0x0B und Default-Shared-Budget 45. HP-Leistungssetter oder Heartbeats wurden nicht ausgeführt; die späteren begrenzten Beleuchtungs-SETs stehen unten.
- Der erhöhte Tastatur-Gegencheck unter Windows PowerShell **5.1.26100.9444** vom 13. Oktober um **13:43 Uhr** lieferte drei erfolgreiche GETs: Capability **[1, 0, 0, 0] / FourZoneWithNumpad**, 128 Farbbytes und 128 Helligkeitsbytes, jeweils Firmwarecode 0. Eine erhöhte native Rust-Probe um **14:04 Uhr** bestätigte dieselbe Capability und byteweise gleiche vollständige Blöcke mit CLI-Exitcode 0. Helligkeit **228 / 0xE4** bleibt ein Rohbyte. Die native Größe-0-Eingabe **VT_ARRAY | VT_UI1 mit Null-SAFEARRAY** funktioniert am Provider. Rohdaten stehen ausschließlich unter ignorierter `.local/research/keyboard-state/`.
- Die erste native Probe legte den Verlust von `__PATH` bei generischer WMI-Map-Deserialisierung offen. Der Reader liest nun die COM-Systemfelder direkt und verlangt genau eine aktive Instanz. Diese erste Probe stoppte vor einem Firmwareaufruf. Die erfolgreichen GETs laden keine HP-RPC-/SDK-Wrapper, jedoch liefen HPOmenCap, HPSysInfoCap und HPAppHelperCap weiterhin; ein Betrieb ohne diese Controller ist dadurch noch nicht bewiesen.
- Für die Lese-, Takt- und RGB-Proben wurden einzelne kurzlebige UAC-Prozesse verwendet; Codex und Tray blieben uneleviert. `-ExecutionPolicy Bypass` betrifft nur den jeweiligen Prozess. Es wurden keine gespeicherten Richtlinien oder Herstellerdienste geändert, keine Treiber installiert und keine Firmware geflasht. CPU-/GPU-Power-Limits und Spannungen blieben unverändert; geprüfte Takt-/RGB-Steuerwerte wurden vollständig restauriert. Eine eigene geschützte Task-Scheduler-Aufgabe für den festen Profil-Einstieg wurde eingerichtet; die automatische Profilauswahl ist nach den Tests ausgeschaltet.

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

## Vier-Zonen-Farben-/Modusnachweis — 14. Oktober 2026

Der typisierte direkte WMI-Adapter ist auf **HP 8E35 / F.13 / Capability [1,0,0,0]** gebunden. Farb-SET 0x20009/type3 erhält den vollständigen frischen 128-Byte-Block und verändert ausschließlich RGB-Bytes 25…36. Modus-SET type5 sendet genau vier Bytes mit dem belegten Wert 0x64 oder 0xE4. Firmwarestatus, Datentypen, Länge, Capability sowie vollständige Farb- und Helligkeitsblöcke werden geprüft. Das Helligkeitsbyte bleibt roh; die Modusfreigabe bedeutet keine allgemeine Prozenthelligkeit.

| Zeit (CEST) | Versuch | Build-Identität | Ergebnis |
| --- | --- | --- | --- |
| 02:35:09…11 | Zone-1-RGB-Komponente um eins, sofortiger Trial-Restore | Native Binary SHA-256 `98F3B08925619025671905FA8E32CBD109E7F1F59B8F35217AA7D87EC6A522F9` | Vollständiger Ziel-Readback und 4/128/128-Byte-Original wiederhergestellt |
| 02:48:39…40 | Dieselbe RGB-Änderung plus eigene Farben 0x64 → 0xE4, anschließend vollständiger Restore | Native Binary SHA-256 `5E5926A88545169D27CBBF3EA5F69161BEFFDC93A85496049439711688DC7B78` | Beide Settings und alle unveränderten Steuerbytes geprüft; Originalmodus 0x64 restauriert |
| 03:25:05…06 | Gespeichertes Profil mit je einer um eins veränderten Komponente aller vier Zonen plus Gegenmodus; Apply und Recovery in getrennten Prozessen | Ignorierter Proof-Runner aus aktuellen Core-/Platform-Crates SHA-256 `E0AEBF4932C808C9D20E1EE313D32E41AED926CBEECFD1C8A434561650BFD7D3` | Vollständiger Sollzustand und Original restauriert; neues WMI-/Controller-Objekt allein aus isoliertem dauerhaftem Journal |

NVIDIA-Indizes, Spannungsanker und programmierte Graphics-Deltas blieben in allen Versuchen gleich. Der letzte Proof benutzte ein isoliertes `ProfileStore` und hielt die globale Steuerlease über beide Kinder; das produktive Benutzerjournal und die Startup-Auswahl blieben anhand ihrer SHA-256 byteweise unverändert. Apply-PID 47036 und Recovery-PID 40956 waren verschieden. Der Proof prüft die Produktions-Core-/Platform-Transaktionen in neuen Prozessen, nicht die Benutzerinteraktion mit UAC oder einen tatsächlichen Reboot. Proprietäre DLLs, Rohblöcke, Probe-Runner und vollständige Ergebnisse verbleiben ausschließlich unter ignorierter `.local/`/`.tools/`.

Die Aufrufe luden keine HP-RPC-/SDK-Wrapper, die vorhandenen HP-Dienste liefen jedoch weiter. Keine Dienst-/Treiberinstallation oder Power-/Voltage-/Flash-Aktion. Sichtbare physische Effekte, Fn-F4, Entsperren, Suspend/Resume und Verhalten ohne HP-Controller sind nicht abgenommen. Farben/Modus sind daher nur für **manuelle Profilanwendung** freigegeben; beide Capabilities bleiben `startup_safe = false`.

Der native Beleuchtungseditor wurde mit einem eigenen Prozess und dessen Klasse/Controls geprüft: ungültige Hexwerte werden abgewiesen; vier exakte Zonenfarben und ein getrenntes Modussetting werden gespeichert und neu geladen; vorhandene VRAM-Einstellungen bleiben erhalten. Speichern allein aktiviert keinen Apply-Button, GPU-Steuerwerte bleiben unverändert, das Fenster beendet sich sauber. Der Smoke-Test führt keine UAC-/SET-/Restore-Aktion aus. Ein zusätzlicher nativer Rust-Test prüft, dass Refresh sowie Lese-/Apply-/Restore-Fehlerpfade die vorherige GET-Kopierquelle und vier Firmwarelabels verwerfen.

Die native Clientzeichnung wurde in zwei eigenen Korrekturrunden und einer Reviewer-Korrekturrunde im Speicher gerendert. Deklarierte Größe: **764 × 741 Pixel**. Label-/Button-/Swatch-Layout, Herkunftstext und Sperrzustand sind in dieser Aufnahme sichtbar; EDIT-/COMBOBOX-Werte werden von der verwendeten versteckten `WM_PRINT`-Route nicht mitgezeichnet und sind ausdrücklich von der visuellen Abnahme ausgeschlossen. Der Impeccable-Finish-Reviewer fand eine veraltete GET-Anzeige nach Apply/Restore; nach Invalidation samt nativem Regressionstest wertete er genau diese Korrektur als **resolved**, Disposition **ship**. Es wird kein vollständiger Fenster-/DPI-/Barrierefreiheitspass behauptet. Der HTML-/CSS-Detector wurde für diese native Erweiterung nicht ausgeführt.

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

## Release mit nativer Beleuchtungssteuerung — 14. Oktober 2026

Am **14. Oktober um 03:37 CEST** wurde die Binary mit typisierten HP-Farb-/Modusadaptern, nativem Vierzoneneditor und gemeinsamem festen UAC-Einstieg 30 Sekunden im Tray gemessen:

| Kennzahl | Ergebnis |
| --- | --- |
| Binärgröße | 1776128 Bytes (rund 1,78 MB) |
| Working Set | **12,89 MiB (rund 13,52 MB)** |
| Private Memory | 2,12 MiB |
| CPU-Zeit | 0,015625 Sekunden |
| CPU-Zuwachs nach den ersten 5 Sekunden | 0,0 Sekunden |
| Reagiert auf Nachrichten | Ja |
| SHA-256 | `95A05B0EE42224281C38A82D1232CC616C42C7E1C6EA439D0F63D6FF77B04DD9` |

Formatprüfung, **68 Workspace-Tests**, Clippy ohne Warnungen und optimierter Release-Build bestehen. Die nativen Dashboard-, GPU-Profil-, Kurven- und Beleuchtungseditor-Smokes bestanden mit dieser Binary unter Windows PowerShell **5.1**, ohne UAC oder Hardware-Schreibaktionen. Die Skripte verwenden explizite `UIntPtr`-Konstruktoren und eine UTF-8-BOM für ihre deutschen Quelltexte; die testweise importierte JSON-Kurve wird weiterhin ohne BOM geschrieben. Die Beleuchtungsprobe wartet auf die abgeschlossene Profilansicht, bevor sie Sperrzustände prüft. Eine frühere Probe während Fensterkonstruktion und PowerShell-5.1-Encoding-/Cast-Fehler wurden als Testfehler korrigiert; sie belegen keinen Firmwarezugriff.

Ein zusätzlicher normaler CLI-Gegencheck verlangte Restore mit einer abweichenden Profil-ID. Er erhielt Exitcode 1 und den passenden Nonce-/Operations-Fehlerdatensatz; vorhandenes Journal und Startup-Datei blieben byteweise unverändert. Die Zuordnungsprüfung findet vor dem Erzeugen des Controllers und vor Hardwarezugriff statt. Die letzte RGB-Trial-Abfrage gibt ebenfalls Fehler statt Erfolg zurück, wenn der erneute Gesamt-GET fehlschlägt oder vom Original abweicht.

Der erhöhte isolierte RGB-Recovery-Proof ist separat oben identifiziert; er ist keine Messung dieser normalen UI-Prozesse. Die Tray-Messung startet direkt mit `--tray` bei geschlossenen Editoren und ohne periodische Sensorsitzungen. Sie bestätigt das 50-MB-Ziel auf diesem Gerät für diesen Build, keinen Langzeittest oder Editorverbrauch. Der geschützte bereits installierte Task-Build wird durch diesen lokalen Build nicht automatisch ersetzt; Beleuchtungsprofile sind ohnehin nicht für Startup freigegeben.

## CPU-Lesenachweis — 14. Oktober 2026

Der aktuell geprüfte CPU-Reader verwendet **PawnIO 2.2.0**, Treiber-SHA-256 `FCA6E7D58B0CF38DBB913A2B9E532F48629145D395F454B16A9F58E97B8D3940`, und das signierte **AMDFamily17 0.2.11**-Modul, SHA-256 `DAE74615761B78BDF064DFB3E136252DDCC6FC727D88F14738D0E5800D427A91`. Die öffentlichen ABI-Verträge wurden anhand der Upstream-Sourcen geprüft und in einer eigenen nativen Probe sowie Rust bestätigt. Der Adapter verlangt HP8E35/F.13 und die exakte 8940HX-CPU-Signatur; fehlende oder abweichende Komponenten liefern Fehler. Compiler, Skripte und Proben ändern keine Windows-Sicherheitskonfiguration oder Treiberdienste.

| Zeit (CEST) | Probe | Nachweis |
| --- | --- | --- |
| 08:35 | 32 feste HWCR-GETs, laufender AMD-Treiber, keine SDK-Initialisierung | Zwölf Antwortbytes je logischem Prozessor; überall `0x0000000189006111`; nur als unabhängiger Kontrollvergleich benutzt |
| 08:49…52 | Eigene PawnIO-C++-Probe mit acht Energie-/Tctl-Ständen | Sieben Intervalle etwa 1,10…1,13 s: **50,73…56,29 W**, Tctl etwa **79,63…83,38 °C**; HWCR auf allen 32 Prozessoren und GPU-Steuerwerte unverändert; laufende Treiber unverändert |
| 09:15 | Native Rust-Abfrage | Erste Energie-Baseline korrekt ohne Wattwert, anschließend **69,00 W / 1100 ms / 84,25 °C Tctl**, keine Warnungen, alle 32 HWCR-Bilder gleich; SHA `A254B03D51BEE907937385E396DECEF394BAECBE533CED33FC51E9234FC5A02C` |
| 09:28 | Echter eigener Dashboard-/UAC-Reader-Funktionstest | Normale UI zeigt tatsächlich **76,4 W / 3,02 s / 74,8 °C Tctl**; gezielte Freigabe, Ausschalten, Tray-Rückkehr und simulierte Suspend/Resume-Handler mit neuer Baseline, GPU-Steuerwerte gleich und sauberer Exit; SHA `536B938EE4DAADBB8018ED25D738CFC11DFB4CCE0AF30162F5C303AB4F04AABF` |

Dies sind Entwicklungsstichproben unter unterschiedlicher laufender Arbeit, keine Gaming-/Idle-/Stabilitätsbenchmarks. Die Paketleistung entsteht aus Energiezählerdifferenz und Monotonzeit, nicht aus Auslastung oder einem konfigurierten Limit. Tctl ist CPU-Steuertemperatur, nicht gemessener Boost oder Kerntemperatur. Der normale Prozess erhält beim direkten Treiber-Open AccessDenied; er lädt deshalb keine AMD-SDK-DLL. Der gezielt erhöhte CPU-Reader ist auf einem lokalen per-user/SYSTEM-Kanal mit beiden Prozess-IDs, Nonce, Sequenz, Schema und 4096-Byte-Limit verbunden. Er nimmt ausschließlich Sample/Pause/Stop an. Nach Pause sind Modulsitzung und Baseline freigegeben; keine Sensorabfragen laufen im Tray.

Die Ryzen-SDK-Analyse identifizierte mögliche Steuerregister-Schreibzugriffe bei Init/UnInit und einen Treiberinstallations-Fallback. Diese Route wurde nicht ausgeführt oder übernommen. Der Produktionsreader lädt ausschließlich das gehashte signierte öffentliche Modul und ruft feste GETs auf. Kein MSR-/Power-/Voltage-Setter, keine Treiberinstallation, kein Start/Stop eines Treiberdienstes, keine Änderung von VBS oder gespeicherter Execution Policy. HP-Dienste liefen weiter. Reale Standby-/Resume-/Reboot-Übergänge und vollständiger Betrieb ohne Originalcontroller stehen aus.

Der Impeccable-Reviewer prüfte den eigenen nativen Client **790×633** sowie seinen Footer-Crop **790×153**. GPU und CPU vor System-/Profilkontrollen, explizite Aktivierung und Default-Unavailable-State sind tatsächlich im eigenen WM_PRINT-Rendering sichtbar; kein Desktop-Screenshot. Die aktive Messdarstellung ist durch den eigenen Control-Text/Funktionstest belegt, keine visuelle Active-/DPI-Abnahme. Ein gefundener Rohfehlertext wurde durch deutsche Support-/Freigabe-/Verbindungs-/Messfehlerhinweise mit passender Recovery ersetzt; exakte Details bleiben separat in `last-cpu-error.json` und `cpu-sensor-status`, ohne Änderung des Hardwarejournals. Der anschließende Verdict bewertet diesen einzelnen Fix als **resolved / ship**. Aktive Fehlerdarstellung, Titlebar, weitere DPI-Einstellungen, Accessibility, echter Standby und Speicher sind außerhalb dieses visuellen Verdicts.

## Finaler CPU-Release und Speicher — 14. Oktober, 10:35 CEST

Binary **1871872 Bytes**, SHA-256 **`D12F6D188CAF9CD397CCA28A191E082094E8F33FF2CC36389A5DEE950396F63F`**. Alle **77 Tests (57 Core / 16 Platform / 4 App)**, Formatprüfung, Clippy ohne Warnungen und optimierter Release bestehen. Die vier bisherigen nativen UI-Funktionstests und `smoke-cpu-sensors.ps1` bestehen mit dieser Binary. Die CPU-Live-Anzeige lieferte dabei **58,9 W über 3,01 s / 84,5 °C Tctl**. Aktivierung über UAC, Ausschalten mit verworfenen Werten, Tray-Rückkehr und simulierte Suspend-/Resume-Handler mit neuer Baseline, unveränderte programmierte GPU-Kurven und sauberer Exit sind bestätigt. Der Computer wurde dafür nicht tatsächlich in Standby versetzt.

| 30-Sekunden-Zustand | Working Set | Private Memory | CPU-Zeit-Zuwachs nach fünf Sekunden |
| --- | --- | --- | --- |
| Direkter Start mit `--tray`, Sensoren nicht aktiviert | 12,89 MiB | 2,16 MiB | 0 s |
| Dashboard nach CPU-Aktivierung und zwei Tray-Rückkehrzyklen, wieder verborgen | 36,98 MiB | 21,89 MiB | 0 s |
| Zugehöriger freigegebener, pausierter CPU-Reader | 9,99 MiB | 2,14 MiB | 0 s |
| Summe der beiden verbleibenden Prozesse | **46,97 MiB / 49,25 MB** | 24,03 MiB | 0 s |

Die Summe zählt geteilte Seiten konservativ doppelt; sie umfasst den freigegebenen CPU-Helfer. UAC-Start und Inventarabfrage laufen in bereits beendeten kurzlebigen Prozessen. Es wurde kein Working Set getrimmt. Der ursprüngliche Zwischenstand lag kombiniert bei 59,79 MB; das Kapseln der Windows-Shell- und WMI-Lebensdauer senkte den tatsächlichen Verbrauch. Das 50-MB-Tray-Ziel wird in der finalen Stichprobe auf diesem Gerät eingehalten. Andere DPI-/Treiberzustände und längere Dauerläufe sind damit nicht bestätigt. Die geschützte installierte Startup-Binary wird durch diesen lokalen Build weiterhin nicht automatisch ersetzt; CPU-Sensorfreigabe bleibt sitzungsbezogen.

## Noch ausstehend

Tastatur-GETs sowie manuelle Vier-Zonen-Farben und der begrenzte eigene Farbmodus/Firmware-Voreinstellung sind auf 8E35/F.13 mit vollständigem Restore bestätigt. Quellen, Payload-Unterschiede und die verbleibenden Freigabekriterien stehen in [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md). Herstellerdienste und Treiberzustände wurden nicht verändert; der native Analyseprozess führt die importierten Dateien nicht aus.

- Bedeutung von D-State 0; der Vergleich mit der Benutzerangabe Unleashed ist erfolgt, eine Feldbedeutung folgt daraus nicht.
- Tastaturanimationen, RGB pro Taste, allgemeine Helligkeitsskala und Fn-F4-/Entsperr-/Resume-Lifecycle. Eigenständiger Betrieb ohne ursprüngliche HP-Omen-Controller ist noch nicht gemessen; Beleuchtungssettings bleiben für Startup gesperrt.
- OEM-Leistungsrahmen, Dynamic Boost, Netzteilanforderungen, CPU/GPU-Verteilung und Akkudrain unter vergleichbarer Last.
- Echte Standby-/Resume-/AC-Wechsel, Display-Treiber-Neustart und MUX-Neustart. Lifecycle-Policy und Event-Handler sind implementiert, aber deren tatsächliche Hardwareübergänge nicht getestet.
- Gezielt hängender WMI-Provider und verlorene Treibersitzung; Abgrenzung/Timeout/Backoff sind implementiert, die Störfälle noch nicht auf dem echten System provoziert.
- CPU-Sensoren auf weiteren Geräten/Treiberversionen, gemessener Boost-/Effektivtakt und CCD-/Kerntemperaturen. Paketenergie-Mittelwert und Tctl sind auf der dokumentierten 8E35/F.13-/PawnIO-Kombination inzwischen bestätigt; keine ACPI-Zone und kein konfiguriertes Power-Limit werden als Messwert verwendet.
- Lüfter, Max-Fan, weitere Beleuchtungsvarianten, MUX, CPU-Tuning, GPU-/VRAM-Spannung und vollständiger Unleashed-Adapter. Tatsächlicher Reboot mit ausgewähltem Profil und ein gegebenenfalls nötiger privilegierter Dienst.
- Visuelle Prüfung und Skalierung bei mehreren DPI-/Display-Konfigurationen; längere Tray- und Sensor-Dauerläufe.

Die Runtime enthält keine Netzwerkaufrufe. Build und lokale Werkzeuginstallation benötigen dagegen Downloads. Proprietäre Analysedaten und lokale Diagnoseberichte liegen in ignorierten `.local/`- bzw. `.tools/`-Ordnern. Versioniert werden Originalcode und kompakte Interface- und Validierungsbefunde.
