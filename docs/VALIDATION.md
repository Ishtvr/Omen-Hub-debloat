# Validierungsstand — 23. Oktober 2026

Referenz: HP OMEN 16-ap0xxx, Board 8E35, BIOS F.13, Ryzen 9 8940HX, RTX 5070 Laptop GPU. Windows x64. Build-Werkzeuge: Rust/Cargo 1.99.0, MSVC-Toolchain. Auf anderen Geräten ist dieser Stand nicht validiert.

## Lüfterfreigabe und Firmware-Rückfall — 23. Oktober 2026

Release **2.750.976 Bytes**, SHA-256 **`B485594013AB99A6926906B518D97FC2873BBA530B80AE943DC1B3D7ED2C803D`**. Formatprüfung, **171 Tests** (99 Core / 52 Platform / 20 App), Clippy für alle Workspace-Targets mit `-D warnings` und Release-Build bestanden. Die drei zusätzlichen Core-Tests prüfen den Erhalt der Verantwortung nach einem fehlgeschlagenen Freigabe-SET, erneutes Aktivieren und wiederholte Pause ohne weitere SETs sowie Vorrang eines positiv identifizierten Original-Hubs. Die frühere 168-Test-Abnahme unten gehört zu ihrem damaligen Build.

**Firmwareanalyse und reale Grenzen:** Die genaue F.13-DSDT wurde ohne Methodenaufruf gelesen und statisch disassembliert. OMCC, HPCM/FAMC und SRP1/SRP2 sind damit konkret zugeordnet; die installierte Hub-Version 1101.2609.11.0 wurde erneut dekompiliert. Die zunächst vorhandene Beobachtung von nur 36 Sekunden ohne Befehle reichte nicht aus: Die feste Probe **22:36…22:39 CEST** zeigte nach etwa 120 Sekunden eine autonome Reduktion von 4800/4800 auf schließlich 2300/2100 Firmware-RPM. Der separate direkte FAMC-Versuch um **22:48** zeigte bereits nach wenigen Sekunden Wirkung bei weiter aktivem Heartbeat, erreichte aber die konservative Temperatur-Abbruchbedingung und wurde zurückgestellt. Er gibt keinen produktiven BIOS-Auto-Adapter frei. Detaillierte Quellen, Zeitbezug, Payloadgrenzen und Rücksicherung: [FAN_HANDOVER.md](FAN_HANDOVER.md).

**Eigener Freigabepfad, 23:00:58…23:01:20:** Eine feste eigenständige Probe aus den aktuellen Core-/Platform-Crates, SHA-256 **`72B9FC0E9710DD10AAA3F1494A7C615DFFD756D85A8B86F69B6E76FAB56377A0`**, führte ausschließlich Maximalziele und anschließend Pause aus. Fünf aktive Statusmeldungen bestätigten 48/48 und Hochlauf bis 4800/4800 RPM. Die folgenden fünf Meldungen waren `handover_pending`, weiterhin 48/48 als letzte eigene Vorgabe. Keine bestätigte Firmware-Ownership wurde behauptet. Ein neuer Controller las die vollständige ursprüngliche GPU-Kurve und globale Offsets unverändert; CPU-SMU-Bilder wurden vor/nach der Probe erfasst. Es gab keinen niedrigeren Kühlungsbefehl, Leistungs-/Spannungs-SET, Lastgenerator, Treiberstart oder PC-Neustart.

Die anfänglichen passiven Vorläufe wurden verworfen: einmal lief die falsche feste Probe nur im korrekt erkannten Hub-Wartezustand; ein weiterer Vorlauf traf bereits im Ausgangszustand auf mehr als 85 °C Tctl. Deshalb wurde die abschließende Funktionsprobe auf maximale Kühlung begrenzt. Die Temperaturgrenze des direkten BIOS-Versuchs wurde nicht erhöht. HP-Windows-Dienste, MSI Afterburner und UXTU blieben vorhanden; diese Proben sind kein Nachweis der Unabhängigkeit von allen fremden Hardwarecontrollern.

**Wiederherstellung und Aktualisierung, abschließend 23:06:14:** Die erste Paketaktivierung nach der Abschlussprobe stellte nur die Original-Hub-Oberfläche her. Der von der Probe gestartete Oberflächenprozess wurde geschlossen und die registrierte Paketaktivierung wiederholt; danach liefen Oberfläche und Hintergrundcontroller wieder. Der spätere vollständige Gegencheck bestätigte Original-Hub-Konfiguration und sämtliche eigenen Profil-/Startup-/Lüfter-Metadaten bytegleich, alle 127 GPU-Punkte samt Offsets erhalten, alle 16 CO-Werte wieder **−20**, CPU-Slow **66 W**, cTGP/PPAB beide aktiv. Die automatische Profilauswahl bleibt deaktiviert; das Benutzerjournal hat `pending = false`. Das ausgelesene durchgesetzte GPU-Limit war zu diesem Zeitpunkt **107,67 W**; dieser dynamische Momentwert wird weder als verlorene Dynamic-Boost-Freigabe noch als gemessener 115-W-Lastnachweis umgedeutet. Es wurde kein Limit erzwungen.

Eigene geschützte App-/Lüfterkopie und `dist` besitzen den genannten Release-Hash. Die vorhandenen eigenen Aufgaben wurden mit den festen Installern aktualisiert; kein Profil wurde dafür ausgewählt oder angewendet. Der eigene Lüfterhelfer meldet `waiting_for_hub`. Kein HP-Windows-Dienst wurde durch diese Arbeit gestoppt oder neu installiert.

**Native Speicher-/Lifecycle-Probe, 23:07:55:** Derselbe Release bestand zwei Tray-Rückkehrzyklen mit realen CPU-Watt/Tctl/Effektivtakt-Baselines und sauberem Ende von Dashboard und freigegebenem Leseprozess. Nach Aktivierung und Pause des CPU-Lesers betrug der gesamte eigene Tray-Working-Set **42,13 MB**: Dashboard, pausierter CPU-Leser und residenter Lüfterhelfer (**14,07 MB**, wartet auf den Original-Hub). Dauer 15 Sekunden, davon 5 bis zur Leerlaufbasis und 10 Sekunden CPU-Differenzmessung; Dashboard/Leseprozess jeweils 0 s, Lüfterhelfer 0,0625 s CPU-Zeit. Keine Working-Set-Trimmung, gemeinsame Seiten mehrfach gezählt, keine NVIDIA-Bibliothek im Dashboard und keine verbleibenden eigenen Lesechildren. Die offene Stichprobe von 42,16 MB enthält keine kurzlebigen Inventar-/GPU-Lesechildren. Dies ist eine vollständige eigene Tray-Stichprobe bei wartendem Lüfterhelfer, keine aktive Kurvenmessung; die frühere aktive 47,60-MB-Probe unten bleibt an ihren älteren Build gebunden.

Rohbelege liegen ausschließlich unter `.local/research/fan-handover-20261023/`. Weiter offen sind vollständiger Modus-/Ownership-Readback, sichere direkte Firmware-Übernahme unter CPU-Last, Erhalt aller Leistungsprofile über den Watchdog, reale AC-/Standby-/Neustartübergänge und Betrieb ohne HP-Dienste.

## Vollständiger sicherer Steuerungsdurchgang — 22. Oktober, 07:14…07:53 CEST

Release **2.745.856 Bytes**, SHA-256 **EA849D346B97E9B0211063F229B97C0EF0CDEF351272003A7629975D38B80AA0**. Formatprüfung, **168 Tests** (96 Core / 52 Platform / 20 App), Clippy für alle Workspace-Targets mit `-D warnings` und Release-Build bestanden. Die Binärdatei in `dist` und die geschützten eigenen Helferkopien stimmen mit diesem Hash überein. Frühere Messungen unten bleiben an ihre damaligen Builds gebunden.

### Hardware und Rücksicherung

**07:42:42…07:44:10:** Der feste Durchgang bestand **25 positive Transaktionen** und **drei erwartete Ablehnungen vor jedem SET**. Letztere betreffen widersprüchlichen Windows-Energiemodus/Energiesparplan, entferntes einzelnes STAPM und den entfernten FPS-Limiter. Positive Proben: CO gemeinsam/einzeln **−20 → −19 → −20**, PBO unverändert **1× tatsächlich SET/GET/Restore**, kleine Senkungen von Fast/Slow, TDC/EDC und Tctl, gemeinsame Fast-/Slow-Vorgabe, Windows-Boost/Min/Max/EPP/Core-Parking, Energie/Plan einschließlich Zielplan-Richtlinien und Gesamtprofil aus Modus/Plan/Boost/Min/Max/EPP/Core-Parking, Bildwiederholrate, gespeicherte CCD-Auswahl, Dynamic Boost mit beiden Dateinamen sowie MUX-Startvorgabe. Alle sieben unabhängigen CPU-Steuerbilder wurden nach jeder CPU-Probe aus einer neuen Sitzung verglichen; die vollständigen GPU-Steuerwerte nach jeder Transaktion.

Zusätzlich bestanden Max-Fan-Override, RGB-Farbbyte, statisch Ein/Aus und die gemeinsame Thermikdiagnose. Die erneute GPU-Probe mit demselben Release erhielt alle **127 Spannungsanker/Deltas** und globale Offsets nach Kurven-/VRAM-/−15/+15-/−24/+24-MHz-Proben. Die vier HP-Modi bestätigten ihre GPU-Flags und Slow/PL1 **55 / 55 / 60 / 80 W**; der ausdrücklich gewünschte Endmodus ist Unleashed. Die Rohzelle 0 wird nicht mehr als zu verifizierendes unabhängiges STAPM-Ziel ausgegeben. Ein thermischer Modus-GET existiert weiterhin nicht.

`-CoolCpu` schaltete nur während des Durchgangs den vorhandenen Windows-Boost zum Abkühlen ab und restaurierte AC/DC danach exakt auf **4/4**. Keine Aufhebung von Spannungsschutz, kein Belastungstest, keine neue Treiberinstallation und kein Stopp eines HP-Windows-Dienstes. CPU-CO blieb abschließend auf allen 16 physischen Kernen bei **−20**. Das produktive Benutzerjournal wurde byteweise erhalten, SHA-256 **C3F8699541376B18036CF2D66299E635BDF8A749DF75EA71ACB29DD174755BC7**; keine Wiederherstellung blieb offen. Details: [SYSTEM_CONTROLS.md](SYSTEM_CONTROLS.md).

### Eigener Dienst und Anmeldeaufgabe

**07:14:** Stop-Event nach Sensoraktivierung, tatsächliches Ende des eigenen Helfers und erneuter Aufgabenstart bestanden. **07:23:** Der Max-Dienst bestätigte **48/48**, Zustand `max`, ohne CPU-/GPU-Temperaturleser zu initialisieren; die feste Maximumanforderung hängt nicht von einem optionalen GPU-Sensor ab. Die frühere konservative reale Kurven-/Max-Probe erreichte 4800/4800 Firmware-RPM. Ein eigener schlafender Testprozess bestätigte beide CCD-Masken und vollständiges Release; fremde Benutzerprogramme wurden dafür nicht umgeordnet.

**07:49:29…07:49:30**, nach einer ersten erfolgreichen Probe um 07:22: Ein temporäres Windows-only-Preset mit der bereits vorhandenen Boostvorgabe **4/4** bestand Apply, geschützte Freigabe der exakten Revision, tatsächliche Ausführung über `OmenHubPP-Profile-<SID>` und Recovery. Hier wurde eine gleiche Vorgabe angewendet, kein abweichender CPU-Boost programmiert. Das Aufgaben-Ergebnis bestätigte das genaue Preset und die geschützte EXE. Danach wurden temporäres Preset, Benutzerjournal, Startup-Auswahl, vorheriges Start-Ergebnis und geschützte Freigabedatei vollständig auf ihren Ausgangszustand gebracht. Der Anmeldestart bleibt wie zuvor **deaktiviert**. **Ein echter PC-Neustart wurde damit nicht simuliert oder behauptet.**

### Vollständiges Tray-Budget

Mit derselben Binary, aktiviertem und dann pausiertem CPU-Leser, zwei Tray-Rückkehrzyklen, ohne Working-Set-Trimming:

| Zustand | Gesamter Working Set | Eigener Lüfterhelfer | Beobachtung |
| --- | --- | --- | --- |
| Aktive feste 100-%-Lüfterkurve, 07:46 | **47,60 MB** | 19,22 MB | CPU-/GPU-/IR-Lesepfad aktiv, beide Level 48 |
| Pause nach dieser Sensoraktivierung, 07:47 | **44,88 MB** | 16,54 MB | Kein weiterer aktiver Kurvenbefehl; Original-Metadaten restauriert |

Beide Summen zählen **Dashboard + pausierten CPU-Leser + residenten Lüfterhelfer**. Gemeinsame Speicherseiten werden dabei mehrfach gezählt. Jeder Lauf dauerte 15 s, davon 5 s bis zur Pause-Stichprobe und 10 s CPU-Differenzmessung. Dashboard und CPU-Leser hatten in diesem Intervall jeweils **0 s** gemessene CPU-Zeit, der Lüfterhelfer je **0,0625 s**. Keine verbleibenden temporären Lesechildren; im Dashboard keine NVIDIA-Bibliotheken. Die offene Stichprobe lag bei 47,54 MB einschließlich des aktiven Lüfterhelfers, aber ohne kurzlebige Inventar-/GPU-Lesechildren; für das offene Fenster gilt der erhöhte Speicherrahmen.

Die vollständige aktive Prüfung hatte vorher **69,37 MB** mit allgemeiner NVIDIA-Telemetrie und **65,06 MB** mit einem NVML-only-Versuch ergeben. Diese Builds erfüllen das Budget nicht. Der aktuelle Lüfterpfad liest ausschließlich die öffentliche NVAPI-GPU-Temperatur, gibt Leser bei allen Pausepfaden frei und benötigt bei Max keine CPU-/GPU-Leser. Die früheren reinen Dashboard-/CPU-Reader-Summen wurden nicht als Gesamtbudget übernommen.

### Native Bedienung

Sechs Integrationsskripte bestanden auf diesem Release: Preset-Erstellen/Ändern/Löschen, Dirty-/Merge-/Ablehnungszustände, GPU-Chart aus neuen und geänderten Presets, vollständige Rückgabe/Abbruch, Dynamic-Boost-Aliase einschließlich neuem alleinigen Namen und Abwahl, getrennte CPU-/Windows-Bereiche, Dashboard/Editor-Navigation, Tastaturvorschau, GPU-/Lüfter-Maus- und Tastaturgesten, Shift-Bereich/Abflachen, Esc-Abbruch, Undo/Redo, Zoom/Fit und echtes Speichern/Verwerfen/Neuladen. CPU-Watt/Tctl/Effektivtakte, Tray-Rückkehr, Refresh und **simulierte** Suspend-/Resume-Nachrichten lieferten frische Baselines und beendeten ihre Leseprozesse sauber.

Ein alter Kurven-Smoke hatte die neue aktive Dateiübergabe beim Speichern nicht berücksichtigt und eine sichere Testvorlage vorübergehend an den eigenen Dienst gegeben. Der vorherige Zustand wurde anschließend wiederhergestellt. Der korrigierte passive Smoke verlangt die passende installierte Helferversion, pausiert die eigene Regelung, sichert und restauriert auch `fan-mode.json` und `active-fan-curve.json` und erlaubt keinen impliziten Helfer-Installationspfad. GUI-/Hardware-/Startup-Journale blieben bei den abschließenden passiven Prüfungen unverändert.

**Abschließender Betriebszustand, 07:53:** Der für die Proben geschlossene Original-Hub-Hintergrundprozess wurde über seine registrierte Windows-Paketaktivierung wieder gestartet. Der eigene Helfer meldet entsprechend `waiting_for_hub`. Vollständige GPU-Kurve und globale Offsets sind erhalten, alle 16 CO-Werte sind −20, cTGP und PPAB stehen auf Ein, NVIDIA meldet ein eingeschwungenes Limit von **115 W**. Benutzerjournal unverändert, `pending = false`, Startup-Auswahl deaktiviert. Keine Anwendung, die der Benutzer selbst bereits geschlossen hatte, wurde zusätzlich wieder gestartet.

**Offen:** echter Neustart, physisches Suspend/Resume, AC-Wechsel, MUX-Verdrahtung nach Neustart, langfristige Laststabilität und bestätigte Übergabe an die BIOS-Lüfterautomatik. Ein Stop der eigenen Befehle ist kein Ownership-GET. HP-Dienste liefen weiter; vollständiger unabhängiger Ersatz aller HP-Controller ist weiterhin nicht abgenommen.

## GPU-Treiber 617.42 — 22. Oktober, 03:20…03:21 CEST

Erneut qualifizierte Core-/VRAM-/V/F-Schreib- und Restore-Pfade mit exakter Implementierungsbindung. **156 Tests** (91 Core / 45 Platform / 20 App), Format, Clippy und Release-Build bestanden. Die ursprüngliche nicht uniforme Kurve, globalen Offsets sowie Journal/Startup-Metadaten blieben nach allen Proben erhalten; Afterburner wurde mit unverändertem Zustand wieder gestartet. Der echte native Preset→Chart-Smoke bestand. Aktuelle Binary für diesen Nachweis: **FB924A420FBF0211A2A24608C36579E5E685F08D69F4468087F2A9C074F2F80F**, **2.657.792 Bytes**. Umfang, Zeitpunkte und Grenzen: [NVIDIA_61742.md](NVIDIA_61742.md). Die folgenden CPU-/Speicherproben bleiben an ihren früheren Build gebunden.

## Effektive CPU-Takte — 22. Oktober 2026

Nach Zusammenführung mit `origin/main` (`04646fa`, einschließlich Anwenden-/Tray-Einstellungen und neuer Lüfter-/Grafikschalter) ist der aktuelle lokale Windows-Release **2.654.720 Bytes**, SHA-256 **B4C5063F92796830D24252247C9E79055DA350AAF9D1ABAF4197DCF5D8CFA5FF**. `cargo fmt --all -- --check`, **152 Tests** (89 Core / 43 Platform / 20 App), `cargo clippy --workspace --all-targets --locked -- -D warnings` und der gesperrte Release-Build bestanden. Die neue Abdeckung betrifft APERF-Wandzeitrechnung, MPERF-Fortschritt, Reset/CPU-Verwechslung/veraltete Intervalle, vollständige und eindeutige Abdeckung, Clock-Baseline-Verwurf und Wiederaufbau, getrennte Windows-MHz und die begrenzte IPC-Antwort einschließlich JSON-Escaping. Vorhandene Tests für Profile und Hardwaretransaktionen liefen mit; Unit-Tests schreiben keine Firmware.

- **02:43:16…02:43:18 CEST:** `scripts/check-cpu-clocks.ps1` lief im eigenen kurzlebigen erhöhten Prozess. Unverändert qualifiziert: HP 8E35/F.13, Ryzen 9 8940HX, bereits laufender PawnIO 2.2.0 mit `FCA6E7D5…` und eingebettetes signiertes AMDFamily17 0.2.11 mit `DAE74615…`. Baseline ohne Watt/Takt; zweiter Stand mit 32 geordneten logischen Prozessoren, jeweils **1101 ms**, höchstem Effektivtakt **831,10 MHz**, Paketmittel **35,07 W** und **67,875 °C Tctl**, ohne Warnungen. Dies ist eine Entwicklungsstichprobe einschließlich Leerlaufzeit, kein Last-/Boost-/Stabilitätsbenchmark.
- Der unabhängige bereits vorhandene 32-Slot-HWCR-GET mit unveränderten Probe-/Treiberhashes blieb vor/nach der CPU-Diagnose identisch. Öffentliche GPU-Core-/VRAM-Offsetbilder, Profiljournal/Startup-/Lüfter-Metadaten sowie Treiberstatus/-pfade blieben unverändert. Keine Treiberinstallation/-starts, Herstellerdienst-Stopps, Firmware-, Power-, Spannungs- oder Takt-SETs gehörten zu dieser Prüfung. Rohdaten: `.local/research/cpu-clocks/samples.json`, `validation.json` und `hwcr-after.jsonl`; genaue Register, Quellen und Grenzen in [CPU_SENSORS.md](CPU_SENSORS.md).
- **02:43:34 CEST:** Die eigene normale UI mit gesondertem freigegebenem CPU-Reader bestand `smoke-cpu-sensors.ps1`: reale Watt/Tctl und Effektivtakte mit 32-CPU-Abdeckung, getrennte Windows-MHz, frische Energie-/Takt-Baselines nach Tray, Refresh und simulierten Suspend/Resume-Nachrichten, gleicher freigegebener Helper und sauberes Ende beider Prozesse. Öffentliche GPU-Offsets blieben gleich. Echter Computer-Standby oder Neustart wurde nicht ausgelöst. Die früheren Sensor-Testbefehle 106/107 sind seit der Lüfterumsortierung Hardwareaktionen; beide Sensor-/Speicherskripte verwenden sie nicht mehr. Rohprotokoll: `.local/cpu-ui-smoke.json`.
- Zwei echte eigene `PrintWindow`-Clientcapturen zeigen den neuen CPU-Status im dunklen Dashboard bei **1124×741** und **944×711** Pixeln, 96 DPI: `.local/review/cpu-clocks/cpu-clocks-1140.png` und `cpu-clocks-960.png`. Effektivtakt, Auslastung, getrennte Windows-MHz und Messbasis passen in die bestehenden Felder; GPU/CPU-Hierarchie, native Felder und Aktionen bleiben erhalten. Sichtprüfung durch den implementierenden Agenten, kein neuer unabhängiger Impeccable-Finish-Verdict. Die fokussierten UI-UX-Datensuchen lieferten keinen passenden Treffer; verwendet wurden das vorhandene DESIGN.md und die allgemeine native Daten-/Zustandsführung. Andere DPI, Titlebar, High Contrast und vollständige Barrierefreiheit sind nicht durch diese Bilder bestätigt.

Die folgenden **30-Sekunden-Messungen** gehören zu exakt derselben Binary, ohne Working-Set-Trimming:

| Zustand / Zeitpunkt CEST | Working Set | Private Memory | CPU-Zuwachs im Ruhefenster |
| --- | --- | --- | --- |
| Direkt im Tray, ohne Sensoraktivierung, 02:46:44 | **13,84 MiB** (ca. 14,51 MB) | 2,45 MiB | 0 s nach den ersten 5 s |
| Im Tray nach Aktivierung und zwei Rückkehrzyklen, 02:45:26 | App **18,14 MiB** + Reader **10,91 MiB** = **29,05 MiB / 30,47 MB** | 2,96 + 1,71 MiB | Beide 0 s nach den ersten 5 s |
| Kurze offene App-/Reader-Stichprobe davor | **30,47 MB** | Nicht separat erfasst | Kein Leerlaufbenchmark |

App und Reader waren für die Tray-Probe verborgen; keine NVIDIA-Bibliothek blieb im dauerhaften Elternprozess, kein transienter Leseprozess blieb übrig. Gemeinsame Speicherseiten werden in der Summe zweimal gezählt. Die offene Stichprobe schließt transiente Leser aus; die Tray-Prüfung kontrolliert deren Ende. Das kombinierte Tray-Ergebnis erfüllt das **50-MB-Ziel** unter diesen Bedingungen, ersetzt aber keinen Dauerlauf auf anderen Systemen. Rohdaten: `.local/tray-measurement.json` und `.local/cpu-tray-measurement.json`.

**Geänderter NVIDIA-Treiber:** Das Gerät meldet jetzt **617.42** (WMI `32.0.16.1742`). Öffentliche Sensor-/Offset-GETs funktionieren. Der erste private Kurven-GET der alten Kontrollprobe wurde korrekt durch die bestehende 617.14-Gerätebindung abgewiesen; es wurde kein Schreibzugriff versucht und keine Bindung erweitert. Der CPU-Smoke vergleicht deshalb ausdrücklich nur öffentliche Core-/VRAM-Offsetzustände. Eine vollständige private V/F-Kontrollvergleichs- oder Schreibfreigabe für 617.42 besteht nicht. Die nachfolgenden älteren GPU-/V/F-Schreibnachweise behalten ihre ursprüngliche Treiber- und Buildbindung.

## Frühere Prüfungen — dokumentierter Stand vom 15. Oktober 2026

- Die damals **87 Rust-Tests** (66 Core, 18 Platform, drei App-Tests) behandeln typisierte Profile, exakte Anzeigeeinheiten und konfliktprüfenden Preset-Merge, Transaktions-Preflight/Readback/Rollback, gekoppelte GPU-Domänen, atomare Originaljournale, persistierte Recovery, archiviertes Löschen mit erhaltenem Originaljournal, Startup-Revisionsbindung und Migration. Kurvenregeln, unveränderte Spannungsanker, Eingabe/History/Zoom, Tastatur-Rohbilder, CPU-Energie/Tctl/IPC, Lüfter-/IR-Layouts und Altersgrenzen sind ebenfalls abgedeckt. Die Unit-Tests schreiben keine Hardware. Frühere Testzahlen und der inzwischen ersetzte Tastatur-Snapshot-Test gehören zu ihren datierten Builds; tatsächliche Hardwareproben sind gesondert aufgeführt.
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

## Dunkle native Oberfläche — 14. Oktober, 12:09…12:10 CEST

Der Benutzer erlaubt seit diesem UI-Schritt mehr als 50 MB bei geöffnetem Fenster; das **50-MB-Ziel bleibt für den gesamten Tray-Leerlauf einschließlich pausiertem CPU-Reader**. Die frühere 49,25-MB-Angabe war ebenfalls Tray-Verbrauch nach Sensoraktivierung, kein geöffnetes Dashboard.

Aktuelle Binary: **1920512 Bytes**, SHA-256 **`7F34B30BD47FDA57C79BC4C7951F8DB4F719511DD0D0013555F63383B222E006`**. Formatprüfung, **77 Tests (57 Core / 16 Platform / 4 App)**, Clippy ohne Warnungen und optimierter Release bestehen. Die nativen Aktionen bleiben getrennt von Renderer und Theme; Hardware-Setter oder Firmware-Payloads wurden für die Gestaltung nicht erweitert.

Die neue Win32-Darstellung verwendet dunkle Sensorflächen, GPU-Watt/Temperatur links und CPU direkt daneben, eine deutlich erkennbare Aktionszeile sowie gemeinsame Felder/Buttons in GPU-, Farb- und Kurvenfenstern. `theme.rs` besitzt die GDI-Ressourcen. Eigentümergezeichnete native Buttons verarbeiten ausschließlich `BN_CLICKED` als Aktion; Fokus-, Zeichen- und Doppelklick-Nachrichten lösen keine Freigabe, Speicherung oder Hardwareaktion aus. Der CPU-Funktionstest prüft diese Nicht-Klick-Nachrichten vor der expliziten Aktivierung.

Im Tray werden Dashboard-Kindfenster freigegeben; das Tray-/Ereignisfenster, die Profilvorschauauswahl und die Worker-Freigabe bleiben erhalten. Die NVIDIA-Lesesitzung läuft auf einem eigenen sichtbarkeitsgebundenen Thread, mit FFI-Handles ausschließlich auf diesem Thread und begrenztem Warten auf Ergebnisse/Beendigung. Der bestehende NVIDIA-Störfall mit hängender Vendor-FFI ist damit nicht provoziert oder zertifiziert. Die identische CPU-Treiberidentität wird nun in begrenzten 8-KiB-Blöcken gehasht, ohne temporäres wachsendes Datei-Array.

| Gemessener Zustand | Working Set | Private Memory | CPU-Zuwachs im Tray-Messfenster |
| --- | --- | --- | --- |
| Direkter `--tray`-Start, CPU-Sensoren aus; Fenster verborgen bestätigt | **14,29 MiB** | 2,16 MiB | 0 s |
| Sichtbares Dashboard mit CPU-Reader nach zwei Tray-Rückkehrzyklen | **52,32 MB kombiniert** | Nicht separat aufgenommen | Aktiver Sensorbetrieb |
| Dashboard anschließend wieder im Tray, Fenster verborgen bestätigt | 37,04 MiB | 22,46 MiB | 0 s |
| Zugehöriger pausierter CPU-Reader | 10,05 MiB | 1,65 MiB | 0 s |
| Summe der beiden Tray-Prozesse | **47,10 MiB / 49,39 MB** | etwa 24,11 MiB | 0 s |

Beide Tray-Messungen liefen 30 Sekunden, mit fünf Sekunden Beruhigungszeit. Das Script zeigt für die offene Phase das eigene Fenster ausdrücklich an und bestätigt für die Tray-Phase dessen tatsächliche Unsichtbarkeit. Die Summe zählt geteilte Seiten konservativ doppelt; keine Working-Set-Trimmung wurde verwendet. Frühe UI-Zwischenstände lagen bei etwa 50,5…51,6 MB kombiniert, und eine parallel laufende Messung war wegen ausbleibender/abgebrochener Messwerte nicht verwertbar. Die finalen obigen Proben liefen separat und halten das Tray-Ziel auf diesem Gerät ein; sie ersetzen keinen längeren Dauerlauf.

Der frische visuelle Reviewer prüfte echte eigene Clientbilder bei **1122×773**, **942×743**, **754×771**, **764×741** und **1364×901**. Die Produktion zeichnet zuerst das Elternfenster und dann echte native Controls; Vorschaufenster werden zum korrekten EDIT-Rendering ohne Tastaturaktivierung außerhalb des Desktops angezeigt. Der einzige benannte Finish-Fix **G1** — abgeschnittene zweizeilige GPU-Helfer-/Capabilitytexte — verwendet jetzt den gemeinsamen 14-Pixel-Stil bei unveränderten Fakten/Bounds. Verdict: **ship / G1 resolved**, begrenzt auf diese Korrektur im ursprünglichen abgebildeten Clientumfang. Aktive/Fehler-CPU-Darstellung, Titlebar, offene Dropdowns, andere DPI-/High-Contrast-/Accessibility-Zustände, reale Power-Übergänge und die spätere Telemetrie-Lebensdauer sind außerhalb des visuellen Verdicts.

Die sRGB-Kontraste der implementierten Palette betragen unter anderem **7,77:1** für sekundären Text auf Sensorflächen, **12,70:1** für Text auf Feldern, **10,46:1** für dunklen Text auf dem primären Mint-Button und **3,13:1** für Feld-/Buttonränder. `DESIGN.md` und die Design-Sidecar dokumentieren die implementierten Tokens; ihre HTML/CSS-Vorschauen sind Entwicklungsdokumentation und keine Runtime-Abhängigkeit.

Alle **fünf nativen Funktionstests** bestehen nach den letzten Lifetime-/Klick-Notification-Änderungen mit dem oben angegebenen SHA: Dashboard-Inventar, echte CPU/GPU-Werte, Vorschauwechsel, freigegebene und wieder aufgebaute Controls mit erhaltener Vorschauauswahl; GPU-Profilspeicherung/Validierung; vollständige Kurven-Gesten/History/Zoom und Lüfterentwürfe; Vier-Zonen-Farben-/Modusmetadaten; explizite CPU-Freigabe, Nicht-Klick-Sperre, Messintervalle/Tctl, Ausschalten, Tray-Rückkehr und simulierte Suspend-/Resume-Handler mit neuer Baseline. Kontrollierte GPU-Kurven bleiben dabei unverändert. Es wurden keine Hardware-Schreibversuche für die UI-Abnahme ausgeführt.

## Lüfter-GETs und manuelles Max-Fan — 15. Oktober 2026

Die genaue Gerätebindung bleibt **8E35/F.13**. Die statisch untersuchten Herstellerverträge und Quellenhashes stehen in [FAN_CONTROL.md](FAN_CONTROL.md). Der eigene Adapter verwendet feste WMI-Aufrufe und benötigt keine HP-RPC-/SDK-DLL. Vor dem SET werden die aktuelle Lüftertabelle, Topologie, Thermalpolicy und der vollständige Schalter geprüft; ein fremder Zustand wird nicht überschrieben.

| Zeit (CEST) | Probe | Ergebnis |
| --- | --- | --- |
| 08:57 / 08:58 | Fester unelevierter bzw. erhöhter Reader | Uneleviert AccessDenied vor dem ersten BIOS-Aufruf; erhöht sechs erfolgreiche GETs, Max-Fan Aus, 2900/3100 Firmware-RPM, Tabellenhash `A8E624C6573881F444E36D94A29CA0AF320248855377531F02A499437BEE57F2` |
| 09:17 | Festes Max-Fan Ein, zwei gehaltene Abfragen, Original-Restore | Vollständiges Flag `[0,0,0,0] → [1,0,0,0] → [0,0,0,0]`; RPM steigen von 3300/3600 auf 4100/4200; Restore vollständig bestanden |
| 09:30 | Gespeichertes Apply, vier Sekunden halten, Recovery in neuem Prozess | Ein-Flag und 4300/4400 RPM; gefälschter Tabellenhash wird ohne Flagänderung abgewiesen; danach Original-Aus-Flag restauriert, kein Recovery-Eintrag offen |

Der 09:17-Trial verwendete **1969152 Bytes**, SHA-256 **`063CF16D4E02A522E3B9F883E07167B95B7A825BEC456625E6F23A9E5E0D69D4`**. Die 09:30-Probe verwendete ein separates eigenes Proof-Programm aus aktuellen Core-/Platform-Crates mit isoliertem dauerhaftem Profil-/Journalspeicher: **1432576 Bytes**, SHA-256 **`D17C25B49500369CD8E9FD50C2DA1D17F0C105CD7173078683142FD99F8E245D`**. Bei beiden Proben blieben programmierte NVIDIA-Steuerwerte, HP-GPU-Leistungsstatus sowie die Byte-Hashes des produktiven Benutzerjournals und der Startup-Auswahl unverändert. Keine Power-/Voltage-Setter, Treiberinstallation oder Dienststeuerung; die vorhandenen HP-Controller liefen weiter.

Die Drehzahlen sind Firmware-Momentwerte in 100-RPM-Einheiten, keine kalibrierte externe Tachometermessung. Nach dem ersten Restore wurden wegen Auslauf noch 3700/3900 RPM gelesen; das Restore-Ziel ist das vollständige ursprüngliche Schalterbild. Die kurzen Proben belegen Schalterkontrolle und tatsächliches Hochlaufen, keine eingeschwungene maximale Nenndrehzahl oder längere Stabilität.

Der abschließende native GPU-Funktionstest um **09:36 CEST** verwendet die Release-Binary **1971712 Bytes**, SHA-256 **`BDBA3A6D2B71023684F6A0A5FADAEFF8652BB484F83BCF0B90F521BE68AFEC41`**. Er bestätigt **Ein/Aus/Nicht ändern**, korrektes Neuladen, Save-only-Metadaten, gesperrtes Apply bei ungespeicherten Änderungen, Max-Fan-Startup-Sperre und den Erhalt einer vollständigen individuellen 127-Punkt-GPU-Kurve. Die vorhandenen GPU-Steuerwerte bleiben unverändert; dieser UI-Test führt keinen Max-Fan- oder anderen Hardware-SET aus.

Der frische Impeccable-Reviewer bewertete den real gerenderten nativen Profil-Client **754×771** als **ship**, ohne materielle Korrekturen. Der neue beschriftete native Dreiwege-Schalter übernimmt das bestehende dunkle Design. Die Prüfung umfasst das abgebildete Fenster, Profil-/Save-/Apply-Trennung, Dirty-Gating und Startup-Sperre. Offenes Dropdown, andere DPI, Titlebar, vollständige Accessibility-Abnahme sowie Hardwarewirkung und reale Lifecycle-Ereignisse sind außerhalb dieses visuellen Verdicts. Die Dokumentationsübergabe behandelt dies als Erweiterung; `DESIGN.md` und die Token-Sidecar bleiben erhalten.

**Manuelles Max-Fan ist qualifiziert; eigene Temperaturkurven und BIOS-Automatikumschaltung sind es nicht.** „Aus“ gibt nur den Max-Fan-Override frei. Der gekoppelte Leistungs-/Lüftermodus-SET 0x1A, Level-SET 0x2E und Heartbeat 0x10 wurden nicht ausgeführt. Zusatzsensor 0 wird ausschließlich roh ausgegeben, ohne bestätigte CPU-/Oberflächentemperatur-Zuordnung. Ein Profil mit Max-Fan bleibt vom automatischen Anmeldestart ausgeschlossen. Reale AC-/Standby-/Reboot-Übergänge und vollständiger Ersatz ohne HP-Controller fehlen weiterhin.

## Finaler Release mit Max-Fan — 15. Oktober, 09:47…09:49 CEST

Die fertige Dashboard-Binary hat **1971712 Bytes** und SHA-256 **`BDBA3A6D2B71023684F6A0A5FADAEFF8652BB484F83BCF0B90F521BE68AFEC41`**. Formatprüfung, **82 Tests (62 Core / 16 Platform / 4 App)**, Clippy ohne Warnungen und optimierter Release bestehen. Alle fünf nativen Funktionstests bestehen mit dieser Binary: Dashboard, GPU-Profile einschließlich Max-Fan-Metadaten, Kurven-Gesten und getrennte Lüfterentwürfe, Vier-Zonen-Profilmetadaten sowie echte CPU-Paketenergie/Tctl mit expliziter UAC-Freigabe. Nicht-Klick-Sperre, Ausschalten, frische Baselines nach Tray-Rückkehr und simulierten Suspend-/Resume-Nachrichten, unveränderte GPU-Steuerwerte und sauberer Exit bleiben bestätigt. Kein tatsächlicher Standby und keine Hardware-Schreibaktionen zur UI-Abnahme.

| Zustand | Working Set | Private Memory | CPU-Zuwachs nach fünf Sekunden |
| --- | --- | --- | --- |
| Direkter `--tray`-Start, Sensoren nicht aktiviert | **13,00 MiB** | 2,17 MiB | 0 s |
| Sichtbares Dashboard mit CPU-Reader nach zwei Tray-Rückkehrzyklen | **52,46 MB kombiniert** | Nicht separat aufgenommen | Aktiver Sensorbetrieb |
| Anschließend verborgenes Dashboard | 37,20 MiB | 22,09 MiB | 0 s |
| Zugehöriger freigegebener, pausierter CPU-Reader | 10,04 MiB | 1,64 MiB | 0 s |
| Summe der beiden Tray-Prozesse | **47,24 MiB / 49,53 MB** | 23,73 MiB | 0 s |

Beide Tray-Proben dauerten 30 Sekunden und bestätigen die tatsächliche Unsichtbarkeit des eigenen Fensters. Es wurde kein Working Set getrimmt; geteilte Seiten werden konservativ doppelt gezählt. Diese Stichprobe hält das 50-MB-Ziel einschließlich Helfer ein, bei knappem Abstand zur Grenze. Andere DPI-/Treiberzustände und längere Dauerläufe sind nicht bestätigt. Max-Fan fügt keinen dauerhaften Polling-Thread hinzu; seine erhöhte Controller-Sitzung besteht nur während der ausdrücklichen Operation. Der vorhandene geschützte Startup-Build wird durch den lokalen Release-Build nicht automatisch ersetzt.

## Gemeinsame Thermikdiagnose und isolierte GPU-Reads — 15. Oktober, 19:21…19:39 CEST

Die weitere Analyse belegt für Khalilah25C1 den **IR-Selektor 0 in Celsius** und trennt die gespeicherte Auto-Auswahl von der tatsächlichen Software-/BIOS-Zuständigkeit. Ein vollständiger Firmware-Modus-/Zuständigkeits-GET bleibt unbelegt. `fan-read` erhält die Rohantwort und ergänzt ein typisiertes IR-Ergebnis. `cooling-read` erfasst HP-Fans/IR, CPU-Tctl und NVIDIA in einer einmaligen, ausdrücklich erhöhten Diagnose. Anfang/Ende jeder Quellenabfrage liegen auf demselben monotonen Zeitbezug. Daten, deren Abfragebeginn beim gemeinsamen Abschluss mehr als 2000 ms zurückliegt, werden verworfen; fehlende oder widersprüchliche Daten ebenfalls. Die Prüfung ist eine Altersgrenze für Messwerte und kein Abbruch des laufenden CLI-Aufrufs nach zwei Sekunden. Firmwaremodus und Zuständigkeit bleiben `null`; ein gültiger Messsatz erteilt keine Kurven-Schreibfreigabe.

Erster Diagnosebuild: **1991168 Bytes**, SHA-256 **`32D3D248B27734B5F79918D23165904F3880628F918B39D0209EDAA03585A97D`**. Um **19:23** bestanden drei Aufnahmen in **616…631 ms** mit **86,375…86,75 °C CPU-Tctl**, **60…61 °C GPU** und **42 °C Firmware-IR**. Eine frühere Aufnahme mit 2024 ms wurde korrekt als veraltet abgewiesen. Uneleviert blieben HP-/CPU-Zugriffsfehler sichtbar, GPU-Werte erhalten, Exitcode 1; kein Ersatzwert wurde erzeugt. Die feste Gegenprobe bestätigt unveränderte programmierte Graphics-Indizes/Spannungsanker/Deltas, HP-GPU-Status, Max-Fan sowie Journal-/Startup-Dateihashes. Diese Entwicklungsstichproben belegen keinen Lastbenchmark, physischen IR-Messort oder vollständigen thermischen Regelvertrag.

Die kombinierte Tray-Probe desselben Zwischenbuilds lag bei **51,76 MB** und damit über dem Ziel. Die NVIDIA-Telemetrie wurde daraufhin aus dem dauerhaft lebenden Dashboard ausgelagert: ein serialisierender Worker startet pro sichtbarer Abfrage einen eigenen festen Lese-Child. `read_process.rs` wird mit dem bisherigen Inventar geteilt, erlaubt nur zwei feste Lesekommandos, bindet den Child vor Ausführung an einen privaten Kill-on-close-Job und begrenzt Output/Warten auf **64 KiB / fünf Sekunden**. Der Worker wartet sechs Sekunden auf dessen Ergebnis. Der normale Prozessabschluss gibt auch prozessglobale NVIDIA-Allokationen frei. Keine Änderung an Hardware-Settern oder deren Transaktionslebensdauer. Der zusätzliche Prozessstart betrifft den offenen Sensorbetrieb; ein echter Treiberhang oder blockierte Kernel-Beendigung wurde nicht provoziert.

Finaler Release: **2013696 Bytes**, SHA-256 **`72EFFCCA62DE33A19CD8E5AB58B1567B657A7CE7DE77EC1CAFC7A55A4B2E7820`**. **85 Tests (64 Core / 17 Platform / 4 App)**, Formatprüfung, Clippy ohne Warnungen und Release-Build bestehen. Im zunächst eingeschränkten Sandbox-Lauf scheiterten vorhandene atomare Speicher-Tests an Zugriff verweigert; mit dem erforderlichen lokalen Dateizugriff bestanden sie vollständig. Alle fünf nativen Funktionstests bestehen mit dem finalen Hash, einschließlich echter Dashboard-GPU-Telemetrie aus dem Child, unveränderter Kurven-/Profilmetadaten, expliziter CPU-Freigabe, frischer Baselines und sauberem Exit.

Die abschließende erhöhte Thermikprobe um **19:38** bestätigt zwei gültige Messsätze in **616/619 ms**: **85,625 °C Tctl, 51 °C GPU, 37 °C Firmware-IR**. Der erste Lauf benötigte **5667 ms** und wurde wegen alter HP-/CPU-Zeitfenster korrekt mit Exitcode 1 verworfen. Der Test überprüft die Altersentscheidung unabhängig anhand der Zeitstempel und verlangt mindestens einen gültigen Messsatz. Alle verglichenen Steuerwerte und Dateihashes blieben gleich; es wurde keine Hardwaresteuerung angewendet.

| Finaler Zustand | Working Set | Private Memory | CPU-Zuwachs nach fünf Sekunden |
| --- | --- | --- | --- |
| Direkter `--tray`-Start, CPU-Sensoren aus | **12,96 MiB** | 2,15 MiB | 0 s |
| Dashboard nach Sensoraktivierung und zwei Tray-Rückkehrzyklen, verborgen | **17,72 MiB** | 2,59 MiB | 0 s |
| Zugehöriger pausierter CPU-Reader | **10,05 MiB** | 1,65 MiB | 0 s |
| Summe der beiden verbleibenden Tray-Prozesse | **27,77 MiB / 29,12 MB** | 4,24 MiB | 0 s |

Beide Tray-Proben dauerten 30 Sekunden, ohne Working-Set-Trimming. Das Fenster war tatsächlich verborgen; im Dashboard wurden keine NVAPI-/NVML-Bibliotheken mehr gefunden, und kein eigener Lese-Child blieb übrig. Geteilte Seiten werden weiterhin konservativ doppelt gezählt. Das 50-MB-Ziel ist in dieser finalen Stichprobe mit Abstand eingehalten. Die offene App-plus-CPU-Reader-Stichprobe betrug 29,09 MB, **schließt vorübergehende GPU-/Inventar-Children aus** und ist deshalb keine vollständige offene Spitzenmessung. Andere DPI-/Treiberzustände, Dauerläufe und echte Power-Übergänge bleiben separat zu prüfen.

## Direkte Beleuchtung und Custom Presets — 15. Oktober 2026, 21 Uhr

Der finale Release hat **2044928 Bytes**, SHA-256 **`505B721BE79887D550C2C71126F7FF3600A8907ED3EA71B27B87921B5A18239B`**. Formatprüfung, **87 Workspace-Tests (66 Core / 18 Platform / 3 App)**, Clippy über alle Targets ohne Warnungen und der optimierte Release-Build bestehen. Die Hardware-Schreibverträge wurden nicht erweitert, abgesehen vom kompatiblen semantischen Namen `Off` für das bereits qualifizierte 0x64.

**Reale Tastaturwirkung, eigene frühere Binary:** Um **20:57:10 CEST** bestand die direkte native Beleuchtungs-UI Ein/Aus, Vorbelegung des nativen Farbwählers mit der ausgewählten Zone, Cancel ohne Firmwareänderung, nur linke Zone `[255,80,80]`, erhaltene übrige Zonen, gehaltenen vollständigen Readback und Original-Restore. Nicht-Klick-Notifications lösten keine Aktion aus. GPU-Indizes/Spannungsanker/programmierte Deltas, HP-GPU-Status sowie Hauptjournal und Startup-Datei blieben gleich. Der Benutzer beobachtete diese Farbprobe nicht. Die ausdrücklich gewünschte reine Wiederholung um **21:08:02 CEST** schaltete je vier Sekunden **an → aus → an** und stellte den kompletten Ausgangszustand wieder her; der Benutzer bestätigte **„Ja, an/aus/an war sichtbar“**. Beide Hardwareproben verwendeten **2037248 Bytes**, SHA-256 **`9A7F6F52F24AF731B6C7545977C0E473F47554DA5B73E617B60D6A06A4B031D5`**. Danach änderten sich Zeichnung, CLI-Hilfe/Preview-Bounds und die Preset-Auswahlrücksetzung; die RGB-/0x64-/0xE4-Transportpayloads blieben gleich. Kein Power-/Voltage-SET, Dienststopp, Treiberinstallation oder Firmwareupdate wurde ausgeführt. Reboot/Resume, physische Farb-Sichtabnahme und HP-Dienst-unabhängiger Betrieb sind nicht bewiesen.

**Sechs native Funktionstests, finaler Hash, 21:18:24…47 CEST:**

| Test | Tatsächlich geprüft |
| --- | --- |
| `smoke-presets-ui.ps1` | Neu/Name/optional VRAM speichern; vollständige 127-Punkt-Kurve und parallel geänderte versteckte Beleuchtungswerte erhalten; 45,125 W exakt als 45125 mW speichern; unsupported Apply/Startup gesperrt; Löschen archiviert exakte Daten; GPU-Steuerbild und Hauptjournal/Startup gleich |
| Derselbe Preset-Test, Reviewer-Regression | Dirty Preset A → B wählen → Verwerfen mit Nein beantworten: Selector und Entwurf bleiben A; Save ändert A, die exakte Datei von B bleibt hashgleich |
| `smoke-controls-navigation.ps1` | Leistungsmodus listet die vier Defaults und das temporäre eigene Preset; getrennte GPU-/Fan-Einstiege zeigen nur erlaubte Arten; Scrollseiten/Anfang/Ende und dunkler Thumb-Drag bewegen die Settings; keine Profilanwendung |
| `smoke-ui.ps1` | Dashboard, NVIDIA-Telemetrie aus Child, CPU-unavailable/Windows-MHz/Auslastung, Default-Auswahlstatus, freigegebene Controls im Tray und Rückkehr mit erhaltener Auswahl, sauberer Exit |
| `smoke-curves-ui.ps1` | Vorhandene vollständige GPU-/Lüfterbearbeitung, feste Anker, Gesten/Verlauf/Zoom, getrennte Metadaten und unveränderte Hardware |
| `smoke-cpu-sensors.ps1` | Explizite UAC-Freigabe des Lese-Helpers, Paketenergie-Mittelwert/Tctl, Ausschalten, Pause/Rückkehr und simulierte Suspend-/Resume-Handler mit frischer Baseline, sauberer Exit; kein echter Standby |
| `smoke-keyboard-ui.ps1` | Echte Produktions-Controls in Ein/Aus und Standard-/Mindestgröße aus synthetischen Preview-Metadaten; keine Tastatur-GETs/UAC/SETs, Recovery-/Startup-Dateien gleich |

Frühere Fehlstarts in Prüfscripts betrafen PowerShell-Ausführungsrichtlinie/Quell-Encoding oder Sichtbarkeitsprüfung eines absichtlich verborgen gestarteten eigenen Fensters. Es gab dabei keine ungeprüften Firmwareänderungen. Die Skriptausführungsoption war ausschließlich pro Prozess gesetzt, keine dauerhaft gespeicherte Richtlinie.

**Darstellung und unabhängiger Abschluss:** Die gültigen eigenen WM_PRINT-Clientbilder umfassen Dashboard **1122×773 / 942×743**, Beleuchtung ein/aus **984×551 / 744×441**, Preset-Übersicht/GPU-/CPU-Gruppe **1084×861** und Preset-Minimum **904×681**. Der Builder prüfte sie in begrenzten Batches. Eine frische Impeccable-Abschlussprüfung identifizierte genau den abgelehnten Presetwechsel; nach Korrektur und dem echten Modal-/Save-Test bewertete der Reviewer **F1 resolved**, Disposition **ship**. Der Verdict-Pass gilt für diesen gelisteten Fix und die gleichen zehn gültigen Recaptures, nicht für eine umfassende neue Hardware-/DPI-/Barrierefreiheitsfreigabe. Keine neue Palette, Schrift, Browserlaufzeit oder Raster-Asset; der native Build überspringt den HTML/CSS-Detektor. DESIGN/Sidecar bleiben für diese gewöhnliche Erweiterung erhalten.

**Anmeldeeinrichtung, 21:26:25 CEST:** Der neue feste `startup-setup`-Einstieg installierte die aktuelle Binary und verifizierte den geschützten Benutzer-Logon-Task über die öffentliche Prüfung. Ein zweiter Aufruf war idempotent: das Installationsreceipt blieb hashgleich. Installierte Binary und Release sind hashgleich; feste Aktion und Identität stimmen. Hauptjournal, Beleuchtungsjournal und Startup-Datei blieben gleich; die Auswahl blieb **aus**. Kein Profil wurde angewendet, kein Task zur Hardwareanwendung gestartet und kein echter Neustart ausgeführt. Dies prüft die Einrichtung; die tatsächliche Task-Anwendung einer qualifizierten Revision ist nur durch ihre früheren datierten Versuche belegt.

**Tray-Messungen desselben finalen Releases:**

| Zustand | Working Set | Private Memory | CPU-Zeitdelta nach Startphase |
| --- | --- | --- | --- |
| Direkt mit `--tray`, verborgen, 30 s, 21:22:54 | **13,44 MiB** (rund 14,1 MB) | 2,14 MiB | 0 s |
| Dashboard nach CPU-Freigabe und zwei Rückkehrzyklen, verborgen, 30 s, 21:23:36 | **15,75 MiB** | 2,39 MiB | 0 s |
| Zugehöriger pausierter CPU-Reader | **10,29 MiB** | 1,65 MiB | 0 s |
| Summe der beiden verbleibenden Tray-Prozesse | **26,04 MiB / 27,30 MB** | 4,04 MiB | 0 s |

Das 50-MB-Traybudget ist in dieser Stichprobe eingehalten. Kein Working-Set-Trim, keine NVIDIA-Bibliothek im persistenten Dashboard und kein verbleibender eigener Lese-Child. Geteilte Seiten werden konservativ doppelt gezählt. Die offene App-plus-CPU-Reader-Stichprobe **27,42 MB** schließt vorübergehende GPU-/Inventar-Children aus und ist keine vollständige offene Peak-Messung. Keine Langzeit- oder Laststabilitätszusage.

Die technischen Workflows stehen in [CUSTOM_PRESETS.md](CUSTOM_PRESETS.md) und [KEYBOARD_UI.md](KEYBOARD_UI.md). Private Raw-/Probe-Dateien, Ghidra-Projekt und Captures bleiben ignoriert; versioniert werden Originalcode und diese kompakten Befunde.

## Bereinigtes GPU-Menü und V/F-Chart aus Preset-Entwürfen — 15. Oktober, 21:59…22:04 CEST

Finaler Release: **2055168 Bytes**, SHA-256 **`E55DB62F434ABD80259E1DFB6C20CB49404CFBB0511EC3EF94266748F2BB6567`**. **87 Workspace-Tests (66 Core / 18 Platform / 3 App)**, Formatcheck, Clippy über alle Targets ohne Warnungen und Release-Build bestehen. Die sechs neuen CPU-/Windows-Features des vorigen Builds bleiben im 91-Schlüssel-Schema; dieser Fix erweitert keine Hardwarefreigabe.

Die frische lesende GPU-Prüfung auf 8E35/F.13 / RTX 5070 Laptop / 617.14 bestätigt V/F, VRAM-Offset und den bereits qualifizierten Max-Fan-Pfad. Der globale Core-Offset ist bei der derzeit aktiven nicht uniformen V/F-Kurve gesperrt, obwohl NVML den globalen Wert 0 meldet; er bleibt als grundsätzlich qualifizierte Funktion mit **V/F-Kurve aktiv** erhalten. Direkte GPU-/VRAM-Spannung und Spannungslimit, Power-/Temperatur-/Locked-Clock-/cTGP-/Dynamic-Boost-/HP-Modusregler sowie der FPS-Platzhalter wurden aus der GPU-Gruppe entfernt. Dies beschreibt die fehlende nutzbare Implementierung auf dem geprüften Gerät und behauptet keine generelle Hardware-Unmöglichkeit auf allen Notebooks.

Die Chartursache war die Bedingung **gespeichertes und unverändertes Preset**: Das Anhaken von V/F machte den Entwurf dirty und deaktivierte Bearbeiten; ohne gespeicherte ID startete auch der bisherige Child-Pfad nicht. Der neue eigene Editor verwendet dasselbe vollständige native Kurvenmodell im Speicher, ohne Datei-/Hardwareworker. **Übernehmen** gibt die Kurve an den bestehenden Preset-Entwurf zurück, **Abbrechen** verwirft nur diese Chartbearbeitung. Das Besitzerfenster ist währenddessen gesperrt; nach Rückkehr bleiben Name und andere Entwurfswerte erhalten. Erst der ausdrückliche Parent-Save persistiert. Wiederholtes Öffnen benötigt keinen neuen Prozess und verliert den ursprünglichen Preset-Message-Loop nicht.

| Finaler nativer Test | Tatsächlicher Nachweis |
| --- | --- |
| `smoke-preset-curve-ui.ps1`, 21:59:57 | Chart über das echte Control bei neuem unbenanntem Preset öffnen; erste Frequenz als Metadatum ändern und abbrechen; danach ursprünglicher Entwurf erhalten |
| Derselbe Test | V/F anhaken und anderes dirty Feld ändern; Chart bleibt verfügbar. Kurvenänderung übernehmen, erneut öffnen und alle 127 Punkte behalten. Name/VRAM bleiben gleich, vor Parent-Save entsteht keine Profil-Datei |
| Derselbe Test | Ein bereits gespeichertes Preset mit dirty Namen öffnet/abbricht ebenfalls ohne Dateischreiben. Parent-Save speichert Kurve/VRAM exakt |
| Derselbe Test | Alle zwölf entfernten Alt-Schlüssel laden, beim Öffnen Datei hashgleich behalten, erst bei Save bereinigen; vollständige Kurve und versteckte Beleuchtungswerte erhalten |
| `smoke-presets-ui.ps1`, 21:59:59 | Exakte W/mW, konfliktprüfender Merge, A→B→Nein-Identität, archiviertes Delete, gesperrtes unsupported Apply/Startup und unveränderte Hardwaremetadaten |
| `smoke-curves-ui.ps1`, 22:00:04 | Bestehende Maus-/Tastatur-/Shift-/Bereichsgesten, feste Spannungsanker, History/Zoom und getrennte Lüfterentwürfe im eigenständigen Editor |
| `smoke-controls-navigation.ps1`, 22:00:05 | Default-/Custom-Dropdown, getrennte GPU-/Fan-Einstiege, Scrollseiten/Thumb-Drag und sauberer Exit |

Alle vier Tests bestehen mit dem finalen Hash. Programmierte GPU-Indizes, Spannungsanker und Deltas sowie Performance-/Beleuchtungsjournal und Startup-Dateien blieben beim neuen Draft-Test unverändert. Es wurde kein GPU-/Power-/Voltage-Setter, Max-Fan-Apply, Dienststopp oder Treiber-/Firmwareeingriff ausgeführt. Teständerungen betrafen nur eigene temporäre Profile; die alten Benutzervorgaben werden erst bei deren ausdrücklichem Save bereinigt.

Die fünf finalen gültigen Produktions-Clientrenders unter `.impeccable/review/gpu-controls-fix/` wurden in begrenzten Batches geöffnet: GPU-Gruppe **1084×861 / 904×681**, Chart **1364×901 / 944×671**, numerischer Draft-Editor **1364×901**. Ein unabhängiger frischer Impeccable-Finish-Reviewer gab **ship**, ohne materielle Fixes, für diese Ansichten und gesampelte Quell-/Testbelege. Dies ist keine unabhängige Hardwareprüfung oder vollständige Titlebar-/DPI-/Barrierefreiheitszertifizierung. Das bestehende Theme wurde erhalten, kein HTML/CSS-Detektor für den nativen Build ausgeführt.

Eine neue **30-Sekunden-Kalt-Trayprobe um 22:04:08 CEST** derselben Binary ergab **13,13 MiB Working Set / 2,19 MiB Private Memory**, verborgen, antwortend und **0 s CPU-Zuwachs nach den ersten fünf Sekunden**. Kein Working-Set-Trim. Das ist eine Kaltprobe ohne freigegebenen CPU-Helper; die frühere kombinierte **27,30-MB**-Messung gehört ausdrücklich zur älteren 505B-Binary und wird nicht auf diesen Build übertragen. Eine bereits laufende alte Instanz wurde nur nach Nachweis des verborgenen Hauptfensters ohne Editor/Dialog und ohne offenes Hardwarejournal über ihre eigene Exit-Aktion beendet. Nach Paketierung und Prüfung wurde die aktualisierte App wieder im Tray gestartet. Keine sichtbare Bearbeitung wurde geschlossen.

## Dynamic Boost erhalten, CPU ohne APU-Felder und eigenständiges Windows — 15. Oktober, 22:29 CEST

Finaler Release **2054656 Bytes**, SHA-256 **`BF6088D99C1FC4BE446E43FDC818D51BFC6C29384246AF9B02E19CA770441180`**. Formatcheck, **87 Workspace-Tests (66 Core / 18 Platform / 3 App)**, Clippy ohne Warnungen und Release-Build bestehen. Keine Hardware-Schreibfreigabe wurde ergänzt.

Auf ausdrücklichen Nutzerwunsch ist **Dynamic Boost** mit Include-Checkbox und Ein/Aus wieder im GPU-Bereich. Die vorherige `gpu_unleashed`-Serialisierung bleibt erhalten; auch bestehende `gpu_dynamic_boost`-Werte werden nicht mehr bereinigt. Die zehn anderen entfernten GPU-Schlüssel bleiben aus dem Menü. Der HP-Schreib-/Heartbeat-/Restore-Pfad ist weiterhin ungeprüft; ein enthaltenes Boost-Setting sperrt das gesamte Apply und Startup. Die gemeldete 115-W-Treiberobergrenze beweist keine bewirkte Limitanhebung oder 115 W unter Last.

Der Nutzer ergänzte die Aufgabe um CPU-/APU-Bereinigung und eine eigene Windows-Kategorie. Der lokale UXTU-Custom-Preset-Quellstand trennt CPU-Temperatur/CPU-PPT/TDC/EDC von APU-Skin-/STAPM-/Fast-Slow-PPT-/Zeitfenstern sowie APU-VRM/Soft-Clocks. Der native Editor entfernt 26 APU-/iGPU-Felder einschließlich SoC/Gfx-Strom, iGPU-Tuning, APU-Soft-Clocks (auch CPU-Soft-Limits) und Intel-iGPU-Feldern. CPU-Temperatur, PPT, CPU-TDC/EDC, PBO/Boost, CPU Curve Optimizer/per-core und CPU-/Intel-CPU-Takt/Spannung bleiben. Drei deklarative Grenzen in `preset_fields::SECTIONS` erzeugen **GPU / CPU / Windows**; Windows-Prozessorpolitik und Energie/Anzeige stehen separat. Legacy-Keys bleiben lesbar; die 36 entfernten GPU-/APU-Anweisungen werden erst beim ausdrücklichen Speichern bereinigt; volle Kurven, beide Boost-Schlüssel und andere Settings bleiben erhalten.

Drei native Tests bestehen mit demselben finalen Hash:

- **22:29:27, `smoke-presets-ui.ps1`:** keine APU-/iGPU-Felder im CPU-Menü, eigenständige CPU-/Windows-Headings, CPU-PPT **45,125 W → 45125 mW**, Windows-EPP **74** und Energiemodus **Best Performance** exakt als Metadaten speichern/neu laden. Dynamic Boost Ein/Aus und Weglassen werden korrekt gespeichert, Plan/Apply/Startup bleiben dafür gesperrt. Name, vollständige 127-Punkt-Kurve und konkurrierende fremde Settings bleiben erhalten.
- **22:29:30, `smoke-preset-curve-ui.ps1`:** echter Chart-Klickweg bei neuen/dirty Presets, Cancel/Übernehmen/Reopen und Parent-Save bleiben funktionsfähig. Alle 26 APU- plus zehn entfernten GPU-Anweisungen werden erst bei Save bereinigt; beide Boost-Keys, komplette Kurve und Beleuchtungswerte bleiben erhalten. Programmierte GPU-Indizes/Spannungsanker/Deltas sowie Hardwarejournal-/Startup-Dateien sind unverändert.
- **`smoke-controls-navigation.ps1`:** Windows folgt als eigenständiger Abschnitt auf CPU, sein Heading ist nach Scroll-to-bottom vollständig innerhalb der Settings-Ansicht sichtbar. Vorhandene Default-/Custom-Listen, GPU-/Fan-Einstiege, Seiten-/Thumb-Scroll und sauberer Exit bestehen.

Die vier neuen Produktions-Clientbilder unter `.impeccable/review/cpu-windows-sections/` zeigen GPU/CPU/Windows mit jeweils 1084×861 und Windows mindestens 904×681 Pixeln. Die beiden Abschnittsbilder sind gezielt gescrollte Vorschauen desselben nativen Fensters. Ein frischer vollständiger Impeccable-Finish-Review gab **ship**, ohne materielle Fixes, für diese 96-DPI-Clients und gesampelte Quell-/Testbelege. Er bestätigt keine unabhängige Hardwarewirkung oder vollständige DPI-/Accessibility-/Titlebar-Abnahme. Die bekannte frühere Dokumentationskorrektur (fünf GPU-Felder, zehn entfernte GPU-Schlüssel und erhaltenes Dynamic Boost) ist im aktuellen Produkttext berücksichtigt.

Die alten Debug-/Hardware- und Speichermessungen weiter oben behalten ihre eigenen Build-Hashes; eine neue CPU-Helper-/Warm-Tray-Messung oder eine 115-W-Leistungsprobe wurde nicht ausgeführt. Die Regressionen schreiben nur eigene temporäre Profildateien, keine CPU/APU-/GPU-Power-/Voltage-Werte. Keine Dienste, Treiber oder Firmware wurden geändert.

## STAPM/Slow, Info-Erklärungen und Sprachdateien — 16. Oktober 2026, 01:07…01:40 CEST

Vom Nutzer freigegeben („Dann mach die STAPM/Fast/Slow settings“). Einmalige Messung mit Windows-Freigabe: RSMU `0xCB` (50 W) wurde angenommen, keine PM-Zelle änderte sich; die Nachricht wurde wieder entfernt. HP `0x29` `[50, 50, FF, FF]` senkte Zelle 0 (80 → 50 W) und Zelle 4 (55 → 50 W). Das erste Zurücksetzen mit `[80, 55]` vertauschte die Werte (Zelle 0 = 55, Zelle 4 = 80) und bewies die Byte-Zuordnung Slow/STAPM; `[55, 80]` stellte 80/55 W exakt wieder her (Rücklesen 01:09). Der Preset-Adapter für STAPM/Slow schreibt beide Bytes, behält das jeweils andere Limit auf dem gelesenen Wert, liest die Zelle zurück und stellt beim Restore den erfassten Wert her. Eine Anwendung über ein Preset unter Last ist noch nicht getestet.

Jede Preset-Einstellung hat ein rundes **i**; ein Klick zeigt Name und Erklärung. Alle sichtbaren Texte stehen in `crates/app/lang/de.json` und `en.json`; Meldungen aus core/platform reisen als Marker und werden in der App übersetzt (docs/LOCALIZATION.md). Tests prüfen gleiche Schlüssel in beiden Sprachen, jeden im Code verwendeten Schlüssel und Name/Erklärung jeder Einstellung. Vorschauen in Deutsch und Englisch (Dashboard, Limits-Gruppe) wurden gerendert und geprüft.

## Quiet/Performance, PL1, CCD-Affinität und OC-Probe — 16. Oktober 2026 (Cloud-Build, ungemessen)

Gebaut ohne Hardwarelauf: Quiet (L0) und Performance (L7) im Dashboard-Moduswahlfeld mit rundem **i**; jeder Modus schreibt jetzt auch PL1 über HP `0x29` und liest STAPM/Slow aus der SMU-PM-Tabelle zurück (Werte und Quelle in [models/8e35.md](models/8e35.md)). CCD-Affinität bindet über den Hintergrunddienst das Vordergrundprogramm an die gewählten CCDs und stellt die ursprüngliche Affinität wieder her. `cpu-smu-probe` meldet zusätzlich `oc_allowed`, `oc_capabilities_raw`, `oc_mode_active` und `boost_limit_mhz` (nur lesend). AMD-Boostprofil und All-Core-Spannungsoffset wurden entfernt, weil Dragon Range dafür keine Steuerung hat. Fester CPU-Takt und feste Spannung bleiben gesperrt. Geprüft: `cargo fmt`, Clippy für `x86_64-pc-windows-msvc`, core-/platform-Tests unter Linux; Windows-Tests und Release-Build laufen in der CI.

## App-Symbol und Installationspaket — 16. Oktober 2026

Auf Nutzerwunsch hat die Anwendung ein eigenes Symbol für Desktopverknüpfung und Programm: dunkle Kachel, Mint-Instrument und darunter der gezeichnete Schriftzug **Hub** (statt eines ersten „++“-Entwurfs, ebenfalls auf Nutzerwunsch). `crates/app/build.rs` schreibt Symbolgruppe und Versionsinformationen ohne Ressourcencompiler als `.res`; Dashboard, Leistungsprofile, Beleuchtung, Kurveneditor und Tray laden das Symbol aus der EXE und fallen ohne Ressource auf das Windows-Standardsymbol zurück.

Geprüft wurde in einer Linux-Cloud-Umgebung, **nicht auf dem Referenzgerät**:

- Die erzeugte `.res` ist byteidentisch mit der Ausgabe von `llvm-rc` für dieselbe Symbol-/VERSIONINFO-Definition.
- Ein mit `lld-link` und den Windows-SDK-Bibliotheken gelinkter x64-Release (2077184 Bytes, nicht verteilt) enthält zehn ICON-Einträge (16…256 px), GROUP_ICON 1 und VERSIONINFO 1.
- `cargo fmt --check`, Clippy mit `-D warnings` für `x86_64-pc-windows-msvc` und die Linux-Tests bestehen: **79 Tests** (66 Core, 9 plattformneutrale Platform-Tests und die vier neuen Ressourcen-Tests der App). Windows-spezifische Platform-/App-Tests liefen hier nicht; die Windows-CI führt die vollständige Suite mit MSVC `link.exe` aus.
- `install-app.ps1` und `build-release.ps1` sind mit PowerShell 7.4 syntaxgeprüft; der Lizenzsammler fand für alle 51 eingebundenen Crates eigene Lizenzdateien.

Nach dem Zusammenführen mit PR #2 (HP-Modi, SMU-Adapter, Lüfterdienst) beschreiben Hardwarematrix und Wiederherstellungsanleitung auch diese Wege; das Paket enthält zusätzlich `install-startup.ps1` und `install-fan-service.ps1` zum Entfernen und bei eingebettetem RyzenSMU-Modul dessen LGPL-Quelle. Fenster aus PR #2 verwenden dieselbe gemeinsame Fensterklassen-Registrierung und damit das Symbol.

Nicht geprüft: Installation, Verknüpfungen, Entfernen und `Install.cmd` auf Windows; Symboldarstellung bei verschiedenen DPI, in Taskleiste und Tray; Speicherbedarf und Hash des neuen Windows-Releases. Hardwareverträge, Journale und Startup-Prüfung sind unverändert.

## Lüfter- und Grafikschalter im Dashboard — 17. Oktober 2026 (Cloud-Build, ungemessen)

Auf Nutzerwunsch stehen rechts neben **Steuerung** zwei Positionsschalter mit den Modi darunter: **Lüfter** Auto/Kurve/Max und **Grafik** Hybrid/GPU only. Auto heißt jetzt BIOS-Standardkurve (der Dienst sendet nichts), Kurve folgt der gespeicherten Kurve; der alte Dateiwert `auto` wird als Kurve gelesen. Der Grafikschalter schreibt über einen festen UAC-Befehl `mux-ui-apply` nur bei Abweichung ins BIOS, liest zurück und merkt sich Auswahl, Schreibzeit und Startmodus; **Neustart nötig** erscheint nur, wenn die Auswahl vom laufenden Modus abweicht. Ohne Eintrag zeigt der Schalter den Standard Hybrid.

Geprüft in der Linux-Cloud: `cargo fmt`, Clippy für `x86_64-pc-windows-msvc` und die vollständige Testsuite als Windows-Binaries unter Wine 9.0: **142 Tests** (86 Core / 40 Platform / 16 App), darunter neue Tests für Modi, Neustartlogik, Speicherung und Schaltergeometrie sowie die vom Nutzer am 17. Oktober ergänzten Tests für Lüfterdienst, SMU-Tuning, Windows-Einstellungen, CCD und Notfallkühlung. Unter Wine laufen die Tests ohne HP-Firmware; sie prüfen Logik, keine Hardware. Ein mit `lld-link` gebauter Release lief unter Wine 9.0 mit Xvfb; Screenshots zeigten beide Schalter, Beschriftungen in Deutsch und Englisch, die Auswahl aus gespeicherten Dateien und den Neustarthinweis. Nicht geprüft: tatsächliches Schreiben des Grafikmodus und Neustart auf dem Referenzgerät, Lüfterdienst-Aktualisierung, andere DPI.

## Anwenden, Autostart und Tray-Einstellung — 17. Oktober 2026 (Cloud-Build, ungemessen)

Auf Nutzerwunsch übernimmt das Dashboard einen Leistungsmodus erst mit **Anwenden**; die Auswahl allein zeigt nur einen Hinweis. **Mit Windows starten** schreibt oder löscht den Benutzer-Autostarteintrag `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Omen Hub++` mit `"<exe>" --tray`; **Beim Schließen in den Tray minimieren** (Standard an) steht in `app-settings.json`. Fester CPU-Takt, Core-Ratios und feste Spannungen sind aus dem Preset-Editor entfernt; gespeicherte Werte bleiben lesbar und fallen beim Speichern weg.

Unter Wine 9.0 + Xvfb geprüft: Darstellung von Anwenden-Schaltfläche und Einstellungen; Moduswahl ohne Anwenden schreibt nichts; der Autostart-Klick erzeugte den erwarteten Registry-Eintrag; ein WM_DELETE_WINDOW versteckte das Fenster bei aktiver Tray-Einstellung (Prozess lief weiter) und beendete die App bei ausgeschalteter Einstellung. Nicht geprüft: echter Windows-Start mit Autostart, Anwenden eines Modus auf dem Referenzgerät.

## Noch ausstehend

Tastatur-GETs sowie manuelle Vier-Zonen-Farben und statisch ein/aus (0xE4/0x64) sind auf 8E35/F.13 mit vollständigem Restore bestätigt; der Benutzer hat die sichtbare Ein/Aus-Wirkung am 15. Oktober bestätigt. Quellen, Payload-Unterschiede und die verbleibenden Freigabekriterien stehen in [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md). Herstellerdienste und Treiberzustände wurden nicht verändert; der native Analyseprozess führt die importierten Dateien nicht aus.

- Bedeutung von D-State 0; der Vergleich mit der Benutzerangabe Unleashed ist erfolgt, eine Feldbedeutung folgt daraus nicht.
- Tastaturanimationen, RGB pro Taste, allgemeine Helligkeitsskala und Fn-F4-/Entsperr-/Resume-Lifecycle. Eigenständiger Betrieb ohne ursprüngliche HP-Omen-Controller ist noch nicht gemessen; Beleuchtungssettings bleiben für Startup gesperrt.
- OEM-Leistungsrahmen, Dynamic Boost, Netzteilanforderungen, CPU/GPU-Verteilung und Akkudrain unter vergleichbarer Last.
- Echte Standby-/Resume-/AC-Wechsel, Display-Treiber-Neustart und MUX-Neustart. Lifecycle-Policy und Event-Handler sind implementiert, aber deren tatsächliche Hardwareübergänge nicht getestet.
- Gezielt hängender WMI-Provider und verlorene Treibersitzung; Abgrenzung/Timeout/Backoff sind implementiert, die Störfälle noch nicht auf dem echten System provoziert.
- CPU-Sensoren auf weiteren Geräten/Treiberversionen, gemessener Boost-/Effektivtakt und CCD-/Kerntemperaturen. Paketenergie-Mittelwert und Tctl sind auf der dokumentierten 8E35/F.13-/PawnIO-Kombination inzwischen bestätigt; keine ACPI-Zone und kein konfiguriertes Power-Limit werden als Messwert verwendet.
- Quiet/Performance/Balanced/Unleashed mit PL1-Rücklesen, CCD-Affinität mit dem Hintergrunddienst und die OC-Felder von `cpu-smu-probe` auf 8E35/F.13.
- Eigene Lüfterkurven, BIOS-Lüftermodus und zusätzlicher thermischer Sicherheitsweg; Max-Fan bei realen Power-Übergängen. Weitere Beleuchtungsvarianten, MUX, CPU-Tuning, GPU-/VRAM-Spannung und vollständiger Unleashed-Adapter. Tatsächlicher Reboot mit ausgewähltem Profil und ein gegebenenfalls nötiger privilegierter Dienst.
- Visuelle Prüfung und Skalierung bei mehreren DPI-/Display-Konfigurationen; längere Tray- und Sensor-Dauerläufe.

Die Runtime enthält keine Netzwerkaufrufe. Build und lokale Werkzeuginstallation benötigen dagegen Downloads. Proprietäre Analysedaten und lokale Diagnoseberichte liegen in ignorierten `.local/`- bzw. `.tools/`-Ordnern. Versioniert werden Originalcode und kompakte Interface- und Validierungsbefunde.
