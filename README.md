# Omen Hub++

**Entwicklungsstand:** Die Veröffentlichung von Beta 0.9.0 wurde zurückgezogen. Ein neuer experimenteller Release wird für den erweiterten Funktionsumfang vorbereitet. Der Quellstand und die bisherigen Prüfnachweise bleiben nachvollziehbar erhalten.

## Entwicklung, Nutzung und Haftung

**Dieses Programm befindet sich in aktiver Entwicklung.** Funktionen können unvollständig, experimentell oder auf bestimmten Geräten nicht verfügbar sein. Die Anwendung greift auf Leistungs-, Spannungs-, Temperatur- und Lüftereinstellungen zu; ungeeignete Einstellungen oder Programmfehler können Instabilität, Datenverlust und Schäden an Hardware verursachen. Ein sichtbarer Regler ist keine Zusicherung, dass das jeweilige Gerät die Funktion unterstützt.

**Die Nutzung erfolgt auf eigene Gefahr. Soweit gesetzlich zulässig, übernehmen der Autor und die Mitwirkenden keine Haftung für Schäden, Datenverlust oder sonstige Folgen aus der Nutzung der Software.** Die Software wird ohne Gewährleistung bereitgestellt. Maßgeblich sind die Gewährleistungs- und Haftungsbestimmungen der [MIT-Lizenz](LICENSE).

Native Windows-Anwendung in Rust zur Steuerung unterstützter HP-OMEN-Laptops: Leistungsprofile, CPU-/GPU-Einstellungen, Lüfterkurven und Tastaturbeleuchtung. Das Ziel ist ein vollständiger Ersatz des ursprünglichen OMEN Gaming Hubs ohne dessen Hintergrunddienste, Werbung oder Telemetrie.

Das Projekt befindet sich in Entwicklung. Hardwarefreigaben gelten nur für die jeweils dokumentierte Geräte-/BIOS-/Treiberversion; vollständiger eigenständiger Betrieb und reale Neustart-/Standby-Übergänge sind noch nicht abschließend geprüft. Aktueller Stand: [Validierung](docs/VALIDATION.md), [Hardwarematrix](docs/HARDWARE_MATRIX.md) und [Roadmap](docs/ROADMAP.md).

## Projektstruktur

| Pfad | Inhalt |
| --- | --- |
| `crates/core/` | Plattformunabhängige Profile, Regeln und Wiederherstellungslogik |
| `crates/platform/` | Windows- und Hardwareadapter |
| `crates/app/` | Native Oberfläche, Tray, Übersetzungen und Kommandozeile |
| `scripts/` | Build, Installation und abgegrenzte Prüfabläufe |
| `docs/` | Architektur, Schnittstellenbeobachtungen und Validierungsnachweise |
| `.github/` | Automatische Rust-Prüfungen |
| `.impeccable/` | Versionierte Designkonfiguration und Oberflächenanforderungen; lokale Reviews sind ignoriert |

`PRODUCT.md` beschreibt das Produkt, `DESIGN.md` das Oberflächendesign. Lizenzen und Fremdkomponenten stehen in [THIRD_PARTY.md](THIRD_PARTY.md).

## Bauen

Voraussetzungen: Rust mit MSVC-Ziel für Windows x64 sowie die Visual-Studio-C++-Buildtools und das Windows SDK.

```powershell
# Optional: geprüfte CPU-Module und zugehörige Lizenz/Quellen lokal vorbereiten.
# Das Skript installiert keinen Treiber.
.\scripts\prepare-cpu-module.ps1

cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
.\scripts\build-release.ps1
```

Das Buildskript erzeugt `dist/Omen Hub++.exe` und `target/OmenHubPlusPlus-0.9.0-beta.1-x64.zip` samt SHA-256-Prüfsumme. Das ZIP wird aus einem frischen Staging-Ordner erstellt und enthält auch Installer, Release Notes, Buildnachweis und erforderliche Lizenzdateien. Ohne vorbereitete CPU-Module stehen die entsprechenden Hardwarefunktionen im Build nicht zur Verfügung. Laufzeitvoraussetzungen: [CPU-Sensoren](docs/CPU_SENSORS.md).

## Lokale Dateien und Bereinigung

Build-Ausgaben (`target/`, `dist/`), Werkzeuge und heruntergeladene Komponenten (`.tools/`), Forschungsdaten und rohe Messprotokolle (`.local/`) sowie lokal installierte Skills (`.agents/`) gehören nicht in Git. Proprietäre DLLs, Treiber, dekompilierter Code und persönliche Diagnoseabbilder werden nicht veröffentlicht. Versioniert werden eigener Quellcode, Tests, Buildskripte, Originalgrafiken und nachvollziehbare Schnittstellen-/Prüfberichte.

Compiler-Caches können bei beendetem Build gelöscht und neu erzeugt werden. Die vorbereiteten Module unter `.tools/pawnio/`, deren Lizenz-/Quellarchive, benötigte Prüfprogramme und Hardware-Messnachweise müssen für weitere Entwicklung erhalten bleiben. Aktuelle Release-Dateien sind von alten Caches zu unterscheiden. Ein pauschales Löschen aller ignorierten Dateien würde auch diese benötigten lokalen Daten entfernen.

<img src="docs/assets/app-icon.png" alt="Omen Hub++ App-Symbol" width="96" align="right">

Eine native, leichtgewichtige Open-Source-Alternative zum OMEN Gaming Hub in Rust. Ziel sind HP-Systemkontrolle und die relevanten Tuning-Möglichkeiten von UXTU in einer einzigen Anwendung: ohne Werbung, Overlay, Telemetrie, Konto oder Hintergrund-Updater.

**Status: native Anwendung mit Hardwaresensoren, gespeicherten Custom Presets, modellgebundenen GPU-/CPU-/Windows-Adaptern, V/F-Kurveneditor, direkter Vier-Zonen-Beleuchtung mit Ein/Aus und Lüftersteuerung; noch kein vollständiger Gaming-Hub-Ersatz.** Leistungsprofile trennt GPU-, CPU- und Windows-Optionen. Dynamic Boost bleibt als GPU-Unterpunkt vorhanden. Die vier HP-Defaults und eigene Presets sind auswählbar; Gerätefreigaben, bestätigte Schreibverträge und offene Lifecycle-Prüfungen sind in [Funktionsstand](#funktionsstand) und [Modellnachweis](docs/models/8e35.md) getrennt dokumentiert. **Auf dem Referenzgerät läuft seit der Prüfung am 22. Oktober NVIDIA 617.42.** GPU-Takt-/V/F-Steuerung ist nach Ghidra-Abgleich und vollständigen Schreib-/Restore-Proben jetzt auch für 617.42 qualifiziert. Andere Treiberversionen bleiben gesperrt; Details stehen im [617.42-Nachweis](docs/NVIDIA_61742.md).

Auf **HP 8E35 / BIOS F.13 / Ryzen 9 8940HX** liest der eigene CPU-Adapter Paketenergie, die ausdrücklich als **Tctl** bezeichnete Steuertemperatur und die effektiven Takte aller 32 logischen Prozessoren. Beim ersten Öffnen des Dashboards fordert die App einmal Windows-UAC für einen getrennten Leseprozess an; die Freigabe gilt nur für die laufende App-Sitzung. Nach Abbruch fragt erst **Aktualisieren** erneut. Voraussetzung sind der bereits installierte, laufende und exakt geprüfte **PawnIO 2.2.0**-Treiber sowie das beim Build eingebettete signierte **AMDFamily17 0.2.11**-Modul. Die Anwendung installiert oder startet keinen Treiber und lädt keine proprietäre AMD-SDK-DLL. **Effektiv max.** zeigt den höchsten gemessenen Intervallmittelwert der logischen Prozessoren einschließlich Leerlaufzeit; der Windows-Takt bleibt separat. Die Wattzahl ist ein Energie-Mittelwert über ein separat erfasstes Messintervall; die erste Baseline enthält noch keinen Wattwert. Quelle, geprüfte Hashes, Buildvorbereitung und Grenzen: [CPU-Sensoren](docs/CPU_SENSORS.md).

Die neue **Beleuchtung** zeigt nur die vier anklickbaren Tastaturzonen einschließlich Nummernblock, **Ein/Aus** und eine kurze Zustandszeile. Zone wählen, im nativen Farbwähler bestätigen: Die Änderung wird unmittelbar geschrieben und vollständig zurückgelesen. Abbrechen schreibt nichts. Ein/Aus ist auf 8E35/F.13 per Readback und am 15. Oktober vom Benutzer sichtbar bestätigt; die Farbprobe bestätigte den vollständigen Firmwarezustand und Original-Restore. Das eigene Beleuchtungsjournal erhält Performancejournal und Startup-Auswahl. Beim Öffnen braucht nur dieses Fenster einmal Windows-UAC. Helligkeitsprozente, Animationen, automatischer Lifecycle und Betrieb ohne HP-Controller sind noch unbewiesen. [Bedienung und Grenzen](docs/KEYBOARD_UI.md).

Auf **8E35/F.13** liest der eigene BIOS-WMI-Adapter Max-Fan-Zustand, CPU-/GPU-Drehzahlen, Lüfterzuordnung und die gekoppelte Firmwaretabelle. Der manuelle Max-Fan-Schalter besitzt vollständigen Readback, Original-Restore und persistierte Recovery. Im Custom Preset wird Max-Fan über seine optionale Checkbox aufgenommen und als Ein/Aus-Wert gespeichert. Ohne Checkbox fehlt nur die Profilanweisung; ein aktiver Override wird dadurch nicht zurückgesetzt. Aus gibt den Override frei und wählt keine BIOS-Automatik. Anwenden bleibt eine ausdrückliche UAC-Aktion. Eigene Lüfterkurven laufen über den Lüfterdienst, der bei laufendem Original-Hub wartet. Beim Pausieren nach eigener Regelung fordert er zunächst volle Kühlung an und meldet die ausstehende Übergabe; ein Firmware-Rückfall nach ungefähr zwei Minuten ist beobachtet. Der direkte BIOS-Auto-Schalter bleibt nach einer thermisch abgebrochenen Probe gesperrt. Die Preset-Startup-Einrichtung ersetzt keinen echten Neustart-/Power-Lifecycle-Test. [Lüfter und Max-Fan](docs/FAN_CONTROL.md), [Übergabe und Grenzen](docs/FAN_HANDOVER.md).

## Ausführen

Beim Minimieren oder Schließen ins Tray prüft Omen Hub++ das eigene App-Symbol erneut und meldet es bei Bedarf wieder bei Windows an. Die Wiederherstellung nach der Explorer-Meldung `TaskbarCreated` ist wiederholbar; fehlgeschlagene Anmeldungen werden begrenzt erneut versucht. Ohne erreichbares Tray-Symbol bleibt die Oberfläche zugänglich. Das Tray-Menü beendet die App vollständig; die Einstellung **Beim Schließen in den Tray minimieren** bleibt berücksichtigt. Windows verwaltet weiterhin, ob das Symbol direkt neben der Uhr oder im Überlauf angezeigt wird. Native Regressionsprüfung: `scripts/smoke-tray-ui.ps1`; genaue Ergebnisse in [VALIDATION.md](docs/VALIDATION.md).

Windows x64, Rust mit MSVC-Toolchain und Visual Studio C++ Build Tools:

```powershell
cargo run -p omen-hub-plus-plus
cargo run -p omen-hub-plus-plus -- --tray
cargo run -p omen-hub-plus-plus -- diagnose
cargo run -p omen-hub-plus-plus -- presets-ui
cargo run -p omen-hub-plus-plus -- fan-ui
cargo run -p omen-hub-plus-plus -- gpu-curve-ui
cargo run -p omen-hub-plus-plus -- keyboard-ui
cargo run -p omen-hub-plus-plus -- gpu-capabilities
cargo run -p omen-hub-plus-plus -- profile-list
cargo run -p omen-hub-plus-plus -- hp-read
cargo run -p omen-hub-plus-plus -- keyboard-read
cargo run -p omen-hub-plus-plus -- fan-read
cargo run -p omen-hub-plus-plus -- cooling-read
cargo run -p omen-hub-plus-plus -- cpu-sensor-status
cargo run -p omen-hub-plus-plus -- plan unleashed
```

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Der Release-Build liegt danach in `dist/Omen Hub++.exe`. Die EXE enthält das eigene App-Symbol und Versionsinformationen als Windows-Ressource; Explorer, Desktopverknüpfung, Taskleiste, Fenster und Tray verwenden dasselbe Symbol. Er verwendet Windows und die vorhandenen NVIDIA-Treiberbibliotheken; keine separate Rust-Laufzeit und kein OMEN-Dienst werden für die implementierten Diagnosefunktionen benötigt. Fehlende Treiberabfragen ergeben „Nicht verfügbar“, keine erfundenen Messwerte. Das erste Inventar wird über Windows-WMI abgefragt.

### Installation

`scripts/build-release.ps1` erzeugt zusätzlich ein vollständiges Paket in `dist/` und als `target/OmenHubPlusPlus-<Version>-x64.zip`: EXE, `Install.cmd`/`install-app.ps1`, Lizenz, [Hardwarematrix](docs/HARDWARE_MATRIX.md), [Wiederherstellungsanleitung](docs/RECOVERY.md) sowie unter `third-party/` die Lizenztexte aller eingebundenen Rust-Abhängigkeiten. Ein Doppelklick auf `Install.cmd` fragt einmal nach Windows-UAC, kopiert die EXE in den geschützten Ordner `%ProgramFiles%\OmenHubPlusPlus` (derselbe Ort wie beim Anmeldestart) und legt **Omen Hub++** im Startmenü und auf dem Desktop des aktuellen Benutzers an:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/install-app.ps1                    # installieren
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/install-app.ps1 -NoDesktopShortcut # nur Startmenü
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/install-app.ps1 -Mode Inspect
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/install-app.ps1 -Mode Remove
```

Die Installation wählt kein Profil, richtet keinen Anmeldestart ein und schreibt keine Hardware. Eine ersetzte EXE macht eine frühere Startup-Freigabe ausdrücklich ungültig. Entfernen löscht nur Programmdatei und eigene Verknüpfungen, nie Profile oder Journale; Details in [RECOVERY.md](docs/RECOVERY.md). Das Symbol entsteht reproduzierbar mit `python scripts/generate-icon.py` (Pillow, nur zur Entwicklungszeit).

Die optionale CPU-Modulvorbereitung erfolgt **vor** dem Cargo-/Release-Build. Dieses Skript lädt ausschließlich das feste öffentliche Modulrelease samt entsprechenden Quellen und Lizenz nach ignorierter `.tools/pawnio/`; es greift nicht auf Hardware oder Treiberdienste zu:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-cpu-module.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Ohne vorbereitetes Modul bleibt CPU-Paketleistung/Tctl nicht verfügbar. Mit Modul gehören `dist/third-party/PawnIO.Modules/AMDFamily17.p`, das vollständige Quellarchiv und `COPYING` zum ausgelieferten Paket. Der LGPL-2.1-or-later-Modulanteil ist getrennt vom eigenen MIT-Rust-Code lizenziert; ein Treiber wird nicht mitgeliefert. Die Runtime benötigt keine Downloads.

Das moderne, dunkle Dashboard zeigt GPU-Watt und Temperatur links und CPU-Leistung, Tctl und Auslastung direkt daneben. Die drei hervorgehobenen Werkzeugaktionen folgen darunter; Systemdetails und die ausdrücklich markierte HP-Profilvorschau stehen im unteren Bereich. GPU-, Beleuchtungs- und Kurveneditor verwenden dieselben dunklen Felder und Aktionsstile. Das V/F-Diagramm behält seine eigene Auswahl- und Datenpalette. Die native Gestaltung ist in [DESIGN.md](DESIGN.md) festgehalten.

Sichtbare Sensoren werden alle drei Sekunden aktualisiert. Schließen oder Minimieren versteckt das Dashboard im Tray und gibt dessen Kindcontrols frei; mit der Einstellung **Beim Schließen in den Tray minimieren** (Standard: an) lässt sich das abschalten, dann beendet Schließen die App. **Mit Windows starten** trägt die App für den aktuellen Benutzer in den Windows-Autostart ein (`HKCU\…\Run`, startet mit `--tray` verborgen, ohne Adminrechte und ohne ein Profil anzuwenden). Einen **Leistungsmodus** wählt man im Auswahlfeld und übernimmt ihn erst mit **Anwenden**. Der gewählte Leistungsmodus wird beim Öffnen erhalten; der Wiederaufbau wendet kein Profil an. Im Tray/Suspend werden keine neuen periodischen Samples angefordert; CPU-Treibersitzung und Energie-Baseline werden freigegeben, Worker und pausierter CPU-Reader blockieren. Öffnen, Resume, Refresh und AC-Wechsel beginnen mit frischer Messbasis. Rechtsklick auf das Tray-Symbol bietet Öffnen/Beenden, --tray startet verborgen. Der CPU-Lese-Helper wird beim ersten Öffnen einmal über UAC angefordert; nach Abbruch oder Fehler startet ihn nur **Aktualisieren** neu. Unter **Steuerung** stehen **Leistungsprofile**, **Lüfterkurve** und **Beleuchtung**, rechts daneben zwei Positionsschalter mit den Modi darunter: **Lüfter** mit **Auto** (Standardkurve des BIOS), **Kurve** (gespeicherte Kurve über den Lüfterdienst; ohne Kurve regelt das BIOS) und **Max** (beide Lüfter voll), sowie **Grafik** mit **Hybrid** (Standard) und **GPU only**. Der Grafikmodus wird nach Windows-Freigabe ins BIOS geschrieben, zurückgelesen und erst nach dem nächsten Neustart aktiv; bis dahin steht **Neustart nötig** unter dem Schalter. Die Lüfterkurve wird nicht mehr als Leistungsprofil gespeichert, sondern nur als aktive Kurve des Lüfterdienstes. Der feste Windows-Anmeldungseinstieg wird bei der ausdrücklich freigegebenen Startup-Auswahl eines bereits angewendeten, vollständig unterstützten Presets eingerichtet. Beleuchtung und Max-Fan bleiben für Startup gesperrt. [Custom Presets](docs/CUSTOM_PRESETS.md), [GPU und Profile](docs/GPU_PROFILES.md). Jede Einstellung hat ein rundes **i** mit Erklärung; alle Texte stehen in Sprachdateien (Deutsch/Englisch, [Lokalisierung](docs/LOCALIZATION.md)).

Die sichtbare NVIDIA-Telemetrie läuft pro Abfrage in einem kurzlebigen eigenen Leseprozess. Sein Ende gibt auch Prozessspeicher der Treiberbibliotheken frei. Ein Worker serialisiert die Abfragen; nur begrenzte JSON-Messwerte, Warnungen und ein Verluststatus kommen ins Dashboard. Der feste Child besitzt dieselbe Fünf-Sekunden-Deadline und private Job-Begrenzung wie das Inventar; der Worker wartet mit einem Sechs-Sekunden-Puffer auf dessen Ergebnis. Fehler verwerfen alte Werte und nutzen die 30-Sekunden-Wiederverbindungspause. Im Tray werden keine neuen GPU-Leseprozesse gestartet. Der zusätzliche Prozessstart kostet Zeit im sichtbaren Sensorbetrieb; es entsteht keine dauerhaft aktive GPU-Sitzung im Dashboard. Ein echter hängender Treiber wurde nicht provoziert. Die Hardware-Schreibverträge bleiben unverändert.

## Funktionsstand

| Funktion | Stand |
| --- | --- |
| Modell, Board, BIOS, CPU, GPUs | Windows-WMI, implementiert |
| Vorhandensein der HP-BIOS-Schnittstelle | WMI-Metadaten, implementiert; keine Capability-Zusage |
| HP cTGP-/PPAB-Status | Fester GET für 8E35/F.13; belegte Schalter werden gelesen, D-State 0 bleibt als unbekannter Rohzustand erhalten |
| Netzbetrieb und Akkustand | Windows-Power-API, implementiert |
| NVIDIA-Temperatur und aktueller Takt | Öffentliche NVAPI, implementiert |
| GPU-Watt, Standard-Limit und Treiber-Obergrenze | Öffentliche NVML, implementiert, soweit der Treiber Werte liefert |
| GPU- und VRAM-Takt-Offsets | Öffentliche NVML, schreibbar auf 8E35/F.13 / RTX 5070 / 617.14 oder 617.42 / VBIOS 98.06.2a.40.da; exakter Readback, Originalbild und Restore |
| GPU-V/F-Kurve | 127 vollständige Graphics-Punkte, freigegebene private Frequenz-ABI des exakt gehashten Treibers; Spannungsanker unverändert, kein hartes Spannungslimit |
| Eigene Performanceprofile | Custom Presets mit getrennten GPU-/CPU-/Windows-Gruppen, Auswahl/Name und Anwenden/Speichern/Löschen; versioniertes Format mit typisierten Einstellungen. Nur qualifizierte Adapter schreiben Hardware; ein nicht unterstütztes Setting sperrt das ganze Apply |
| Neustart-Wiederherstellung | Fester, geschützter Einstieg nach Windows-Benutzeranmeldung; überprüfter Task-Aufruf, kein echter Neustarttest |
| CPU-Auslastung | Windows-CPU-Zeitdifferenzen, implementiert; derzeit 1–64 logische Prozessoren in einer Prozessorgruppe |
| CPU-Takt (Windows) | Durchschnitt der von Windows gemeldeten MHz, implementiert; kein gemessener Boost-/Effektivtakt |
| CPU-Effektivtakt | Read-Only-APERF/MPERF aller 32 logischen Prozessoren auf 8E35/F.13/8940HX; höchster Intervallmittelwert einschließlich Leerlaufzeit, vollständige Abdeckung und frische Basis nach Pause; kein augenblicklicher Boost-Spitzentakt |
| CPU-Paketleistung und Tctl | Feste PawnIO-/AMD-RAPL-Leseoperationen auf 8E35/F.13/8940HX mit exakt geprüftem Treiber/Modul; nach expliziter UAC-Freigabe, Energie-Mittelwert samt Intervall, erste Baseline ohne Wattwert |
| Native Oberfläche, Tray, AC-/Standby-Ereignisse | Implementiert; Suspend/Resume auf Hardware noch zu prüfen |
| Leistungsmodus | Quiet, Balanced, Performance und Unleashed über feste HP-Payloads; GPU-Flags und qualifiziertes Slow/PL1 werden zurückgelesen. Thermalmodus ohne GET. Eigene Presets stehen im selben Dropdown. Aktuelle Proben: [SYSTEM_CONTROLS.md](docs/SYSTEM_CONTROLS.md) |
| Lüfterzustand und Firmwaretabelle | Feste BIOS-WMI-GETs auf 8E35/F.13; CPU-/GPU-Drehzahlen in 100-RPM-Einheiten, Zuordnung, vollständiger Max-Fan-Schalter und gekoppelte Tabelle |
| Gemeinsame Thermikdiagnose | `cooling-read` erfasst einmalig CPU-Tctl, GPU, Firmware-IR in °C und Lüfterwerte; monotone Zeitfenster, Fehler bei fehlenden/veralteten Daten, keine Kurven-Schreibfreigabe; erhöhte PowerShell erforderlich |
| Max-Fan | Manueller BIOS-WMI-Override auf 8E35/F.13 mit Identitäts-/Tabellen-/Zustandsprüfung, vollständigem Readback und persistiertem Original-Restore; kurzer Hardware-Readback-/Restore-Nachweis; echter Neustart-/Power-Lifecycle offen |
| Lüfterkurven | Eigener Hintergrunddienst mit konservativer Hardwareprobe, CPU-/GPU-/IR-Schutz, Glättung und Max. Auto pausiert eigene Befehle; BIOS-Ownership und Power-Lifecycle sind damit nicht bestätigt |
| Interaktiver Kurveneditor | Native V/F- und Lüftercharts, vollständiges GPU-Raster, Shift/Alt/Strg, Abflachen, Undo/Redo und Zoom. GPU-Kurven bleiben getrennt von der gemeinsamen aktiven Lüfterkurve |
| Keep-Alive | Lüfterdienst sendet den belegten Heartbeat alle 20 s bei aktiver Kurve/Max; Original-Hub wird als Besitzer respektiert. Kein daraus abgeleiteter BIOS-Übergabenachweis |
| CPU-Wattlimits, Curve Optimizer/PBO und MUX | Fast/Slow, TDC, EDC, Tctl, CO und PBO 1× mit echten Rücklese-/Restore-Proben; MUX-Startvorgabe geprüft, physischer Neustartwechsel offen. Unzuverlässiger einzelner STAPM-Regler sowie feste Takt-/Spannungsregler entfernt |
| Tastaturtyp / RGB- und Helligkeitszustand | Native feste GETs auf 8E35/F.13 validiert: vier Zonen mit Nummernblock, vollständige 128-Byte-Bilder; Helligkeit bleibt ein Rohbyte |
| Direkte Vier-Zonen-Beleuchtung | Native Zonenzeichnung/Farbwahl und Ein/Aus; komplette Readbacks und Restore auf 8E35/F.13, sichtbares Ein/Aus vom Benutzer bestätigt; genau 0x64/0xE4 |
| Helligkeitsprozente, Animationen und Einzeltasten-RGB | Noch keine freigegebenen Adapter; automatische Beleuchtungs-Wiederanwendung bleibt gesperrt |
| Privilegierter Hardware-Dienst | Nicht implementiert; feste kurzlebige UAC-Prozesse für Steueraktionen und ein gesonderter, sitzungsgebundener CPU-Leseprozess |

Sensoren beziehen sich derzeit auf die erste von NVIDIA enumerierte GPU. Bei unterschiedlichen NVAPI-/NVML-Namen werden die Messungen nicht zusammengeführt. Mehrere gleichnamige NVIDIA-GPUs benötigen später eine Zuordnung über PCI-Identität.

CPU-Auslastung misst aktive Prozessorzeit und ist kein Wattwert. Der Windows-Takt bleibt ein OS-Wert und kein gemessener Boost-/Effektivtakt. CPU-Paketleistung/Tctl bleiben ohne qualifizierte Komponenten und gezielte Freigabe „Nicht verfügbar“. Ein CPU-Leistungslimit oder eine ACPI-Thermalzone wird nicht als Ersatzmesswert angezeigt. Fehler verwerfen alte Messwerte; deutsche Hinweise unterscheiden Support, Windows-Freigabe, Verbindung und Messung samt passender Wiederherstellung. Die exakte Ursache wird separat als `last-cpu-error.json` gespeichert und mit `cpu-sensor-status` ausgegeben; dieser letzte Fehler ist kein Live-Sitzungsstatus.

Für eine separate feste CPU-Leseprobe im bereits erhöhten Terminal:

```powershell
& '.\dist\Omen Hub++.exe' cpu-hardware-read | Out-String
```

Der normale UI-Prozess startet für die gezielte UAC-Anfrage zunächst einen vorübergehenden Launch-Prozess; dieser meldet die genehmigte Leseprozess-ID zurück und endet. Der lokale CPU-Kanal erlaubt ausschließlich `Sample`, `Pause` und `Stop`, mit Benutzer-/SYSTEM-Zugriff, beiden Prozess-IDs, Sitzungs-Token, Schema, Sequenz und begrenzten Antworten. Er bietet keine Tuner, frei wählbaren Register oder Treiberinstallation.

`hp-read`, `keyboard-read` und `fan-read` sind separate, feste lesende BIOS-Abfragen, die nicht im Dashboard und nicht als Heartbeat laufen. Sie sind auf das untersuchte Board 8E35 mit BIOS F.13 begrenzt. Der Provider verweigert den Zugriff im nicht erhöhten Prozess (`0x80041003`). In einem als Administrator gestarteten Terminal:

```powershell
& '.\dist\Omen Hub++.exe' keyboard-read | Out-String
& '.\dist\Omen Hub++.exe' hp-read | Out-String
& '.\dist\Omen Hub++.exe' fan-read | Out-String
```

Die festen HP-CLI-Reader erhöhen ihre Rechte nicht selbst und bieten keine BIOS-Setter. Datentypen, Returncode, Länge und bekannte Feldwerte werden geprüft. Der erhöhte GPU-Status-GET liefert **[1, 1, 0, 87]**. Der Benutzer bestätigt gleichzeitig Unleashed im Original-Hub. Der Decoder erhält deshalb den beobachteten D-State 0 mit `d_state_known: false`; daraus folgt keine erfundene Bedeutung oder Schreibfreigabe. `hp-performance-read` liest zusätzlich die 128 Byte SystemDesignData aus der Firmware, ohne Registry-Cache als Livezustand auszugeben.

Der Preset-Editor fordert für Anwenden, Wiederherstellen und die geschützte Anmeldefreigabe gezielt einen festen UAC-Child an. Die direkte Beleuchtung hat stattdessen ein eigenes erhöhtes Fenster und ein getrenntes dauerhaftes Journal. Es gibt keinen beliebigen erhöhten Kommandodienst. Alle Schreibwege prüfen das vollständige Ziel, speichern Originale vor dem SET und restaurieren bei Teilfehlern. Profile liegen unter dem aus dem Windows-Token ermittelten Benutzerprofil in .omen-hub-pp, unabhängig vom Codex-MSIX-AppData-Cache.

Auf NVIDIA 617.14 setzt ein NVML-VRAM-Offset-Aufruf eine individuelle Graphics-Kurve zurück. Omen Hub++ erfasst deshalb auch bei VRAM-Änderungen das vollständige Graphics-Original, erhält es nach dem Aufruf und setzt den endgültigen GPU-Zielzustand zuletzt. Ein globaler Core-Offset und eine vollständige V/F-Kurve sind im selben Profil ausgeschlossen. Aktuelle Frequenzbeobachtungen werden getrennt von den exakt geprüften programmierten Steuerwerten ausgegeben.

`keyboard-read` liest die Capability und bei klassischen Varianten Farben/Helligkeit. Auf 8E35/F.13 wurde **FourZoneWithNumpad**, Capability **[1, 0, 0, 0]**, bestätigt. Die vollständigen 128-Byte-Zustände stimmen mit dem unabhängigen PowerShell-Gegencheck überein; Helligkeit bleibt ein Rohbyte. Die Größe-0-Eingabe des Rust-Adapters ist am Gerät validiert. Der getrennte Schreibadapter ändert nur die zwölf RGB-Bytes oder den exakt qualifizierten Modusschalter 0x64/0xE4 und prüft anschließend die vollständigen Steuerbilder. Kurze Proben und ein dauerhafter Apply-/Recovery-Versuch in getrennten Prozessen bestätigten Zielzustand, Original-Restore und unveränderte GPU-Steuerungen. Die Herstellerdienste liefen währenddessen weiter; Reboot, Resume, Hotkeys und vollständiger Ersatzbetrieb sind damit noch nicht nachgewiesen.

Der Gegencheck `scripts/read-keyboard-state.ps1` läuft mit vorhandener Windows PowerShell **5.1**, ohne `pwsh`-Installation. Er speichert Diagnosezustände und Rohblöcke ausschließlich unter ignorierter `.local/research/keyboard-state/`:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\read-keyboard-state.ps1'
```

Die Startoption gilt nur für diesen Prozess; keine gespeicherte Richtlinie wird geändert. Auch dieser Reader braucht hier ein Administrator-Terminal und erhöht sich nicht selbst. Befunde: [Tastaturbeleuchtung](docs/KEYBOARD_LIGHTING.md).

Omen Hub++ soll den originalen Hub und seine Hintergrundcontroller vollständig ersetzen. Der eigene Hardwaretransport soll deshalb keine HP-RPC-/SDK-Wrapper als notwendige Runtime-Abhängigkeit erhalten. Welche heutigen Hintergrundaufgaben die eigene Engine übernehmen muss, steht im [Ersatzplan](docs/REPLACEMENT_PLAN.md).

## Leistungsziel

**Lüfterkurve** öffnet CPU-/GPU-Lüfterentwürfe. **Leistungsprofile → GPU → GPU-Leistungskurve → Bearbeiten …** öffnet das native V/F-Diagramm auch für neue oder ungespeicherte Presets. Shift/Alt/Strg-Gesten, Bereichsauswahl, Abflachen, Tastatur und Verlauf bleiben erhalten. Übernehmen gibt die Kurve an den Preset-Entwurf zurück; Abbrechen verwirft diese Chart-Bearbeitung. Erst Speichern im Preset persistiert sie, ohne Hardwareänderung. Lüfterkurven-Apply, fester Spannungs-/Takt-Lock und OC Scanner bleiben unsupported. [Kurveneditor](docs/CURVE_EDITOR.md).

Der Unleashed-Modus soll den **vom konkreten Gerät vorgesehenen** Leistungsrahmen einschließlich Dynamic Boost korrekt aktivieren und erhalten. Er verspricht kein Umgehen von VBIOS-, Firmware-, Netzteil- oder thermischen Grenzen. Ein gemeldetes maximales Power-Limit ist kein Nachweis dauerhaft erreichbarer GPU-Leistung.

Das Budget beträgt **50 MB Working Set im Tray-Leerlauf einschließlich aller eigenen Helfer**. Ein geöffnetes Fenster darf für eine bessere Oberfläche mehr benötigen. Die vollständige aktive Kurven-Stichprobe einschließlich Dashboard, pausiertem CPU-Leser und Lüfterhelfer lag bei **47,60 MB**, die anschließende warme Pause bei **44,88 MB**, ohne Working-Set-Trimming. Diese Werte sind an den in [VALIDATION.md](docs/VALIDATION.md) genannten Build und die dort beschriebene Stichprobe gebunden; frühere Parent-/Reader-Summen sind kein Gesamtbudgetnachweis.

## Projektaufbau und Fahrplan

* `crates/core`: Hardware-unabhängige Profile, Diagnosedaten, CPU-Energie-/Tctl-/Effektivtakt-Umrechnung, Lüfter- und Heartbeat-Policies.
* `crates/platform`: Windows-WMI, Power-API, NVAPI/NVML, qualifizierter fester CPU-Reader, validierter GPU-Controller, Profil-/Transaktionsspeicher und geschützter Anmeldestart.
* `crates/app`: Standalone-Anwendung mit nativer Oberfläche, Tray, sitzungsgebundener CPU-Lesebrücke und Diagnose-CLI.
* `docs`: Architektur, Fahrplan, Modellbefunde und Reverse-Engineering-Protokoll.
* `scripts`: Reproduzierbare Release- und lokale Analysewerkzeuge.

[Projektziel und Fahrplan](docs/ROADMAP.md) · [Hardwarematrix](docs/HARDWARE_MATRIX.md) · [Wiederherstellung](docs/RECOVERY.md) · [Architektur](docs/ARCHITECTURE.md) · [GPU und Profile](docs/GPU_PROFILES.md) · [Lüfter und Max-Fan](docs/FAN_CONTROL.md) · [HP-Analyse](docs/REVERSE_ENGINEERING.md) · [Referenzgerät 8E35](docs/models/8e35.md)

Die lokale Analyse von HP-Dateien liegt ausschließlich in `.local/`; Werkzeuge in `.tools/`. Beide Ordner sind von Git ausgeschlossen. Proprietäre DLLs, Treiber und dekompilierter HP-Quelltext werden nicht verteilt. Technische Projektnamen verwenden `omen-hub-plus-plus`/`omen-hub-pp`; der Produktname ist **Omen Hub++**. Die bestehende GitHub-URL bleibt `Ishtvr/Omen-Hub-debloat`.

## Referenzen

Die [OmenMon-Dokumentation](https://omenmon.github.io/cli) und [OmenHwCtl](https://github.com/GeographicCone/OmenHwCtl) dokumentieren vorhandene HP-Schnittstellen. Daraus folgt keine bestätigte Unterstützung dieses Modells. Für NVIDIA verwenden wir die [öffentlichen NVAPI-Header](https://github.com/NVIDIA/nvapi) und [NVML-Geräteabfragen](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html). Der CPU-Reader folgt dem öffentlichen [PawnIO-Protokoll](https://github.com/namazso/PawnIO/blob/9d52965895588b0ffa3703b72eec75ba19f4ccc0/PawnIO/include/pawnio_um.h) und dem signierten [PawnIO.Modules-Release 0.2.11](https://github.com/namazso/PawnIO.Modules/releases/tag/0.2.11). Die spätere CPU-Tuning-Anbindung orientiert sich am [libryzenadj-Header](https://github.com/FlyGoat/RyzenAdj/blob/master/lib/ryzenadj.h).

Eigener Code: MIT. Abhängigkeiten und ihre Lizenzen: [THIRD_PARTY.md](THIRD_PARTY.md). Nicht mit HP oder NVIDIA verbunden oder von ihnen unterstützt.
