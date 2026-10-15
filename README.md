# Omen Hub++

Eine native, leichtgewichtige Open-Source-Alternative zum OMEN Gaming Hub in Rust. Ziel sind HP-Systemkontrolle und die relevanten Tuning-Möglichkeiten von UXTU in einer einzigen Anwendung: ohne Werbung, Overlay, Telemetrie, Konto oder Hintergrund-Updater.

**Status: native Diagnose, qualifizierte GPU-/VRAM-Taktsteuerung und vollständige V/F-Kurven, direkte Vier-Zonen-Beleuchtung mit Ein/Aus, manuelles Max-Fan und gespeicherte Custom Presets; noch kein vollständiger Gaming-Hub-Ersatz.** Leistungsprofile trennt GPU-, CPU- und Windows-Optionen. APU-/iGPU-Tuning ist aus dem CPU-Bereich entfernt. Die GPU-Gruppe bietet Takt-Offsets, GPU-Leistungskurve, Max-Fan und Dynamic Boost. Unqualifizierte Spannungs-/Power-/Temperatur-/Locked-Clock-/cTGP-Regler und der FPS-Limiter sind entfernt. Dynamic Boost ist auf ausdrücklichen Nutzerwunsch wieder als speicherbare GPU-Option enthalten; die HP-Hardwareanwendung bleibt bis zum qualifizierten Power-/Heartbeat-/Restore-Pfad gesperrt. Alte Dynamic-Boost-Vorgaben werden beim Save erhalten. CPU-/Windows-Tuning, MUX und eigene Lüfterkurven bleiben noch nicht hardwareseitig freigegeben. Die drei HP-Defaults sind nicht verfügbar; eigene vollständig unterstützte Presets lassen sich anwenden.

Auf **HP 8E35 / BIOS F.13 / Ryzen 9 8940HX** liest der eigene CPU-Adapter Paketenergie und die ausdrücklich als **Tctl** bezeichnete Steuertemperatur. Beim ersten Öffnen des Dashboards fordert die App einmal Windows-UAC für einen getrennten Leseprozess an; die Freigabe gilt nur für die laufende App-Sitzung. Nach Abbruch fragt erst **Aktualisieren** erneut. Voraussetzung sind der bereits installierte, laufende und exakt geprüfte **PawnIO 2.2.0**-Treiber sowie das beim Build eingebettete signierte **AMDFamily17 0.2.11**-Modul. Die Anwendung installiert oder startet keinen Treiber und lädt keine proprietäre AMD-SDK-DLL. Die Wattzahl ist ein Energie-Mittelwert über das angezeigte Messintervall; die erste Baseline enthält noch keinen Wattwert. Quelle, geprüfte Hashes, Buildvorbereitung und Grenzen: [CPU-Sensoren](docs/CPU_SENSORS.md).

Die neue **Beleuchtung** zeigt nur die vier anklickbaren Tastaturzonen einschließlich Nummernblock, **Ein/Aus** und eine kurze Zustandszeile. Zone wählen, im nativen Farbwähler bestätigen: Die Änderung wird unmittelbar geschrieben und vollständig zurückgelesen. Abbrechen schreibt nichts. Ein/Aus ist auf 8E35/F.13 per Readback und am 15. Oktober vom Benutzer sichtbar bestätigt; die Farbprobe bestätigte den vollständigen Firmwarezustand und Original-Restore. Das eigene Beleuchtungsjournal erhält Performancejournal und Startup-Auswahl. Beim Öffnen braucht nur dieses Fenster einmal Windows-UAC. Helligkeitsprozente, Animationen, automatischer Lifecycle und Betrieb ohne HP-Controller sind noch unbewiesen. [Bedienung und Grenzen](docs/KEYBOARD_UI.md).

Auf **8E35/F.13** liest der eigene BIOS-WMI-Adapter Max-Fan-Zustand, CPU-/GPU-Drehzahlen, Lüfterzuordnung und die gekoppelte Firmwaretabelle. Der manuelle Max-Fan-Schalter besitzt vollständigen Readback, Original-Restore und persistierte Recovery. Im Custom Preset wird Max-Fan über seine optionale Checkbox aufgenommen und als Ein/Aus-Wert gespeichert. Ohne Checkbox fehlt nur die Profilanweisung; ein aktiver Override wird dadurch nicht zurückgesetzt. Aus gibt den Override frei und wählt keine BIOS-Automatik. Anwenden bleibt eine ausdrückliche UAC-Aktion im Netzbetrieb. Eigene Lüfterkurven und automatischer Profilstart mit Max-Fan bleiben gesperrt. [Lüfter und Max-Fan](docs/FAN_CONTROL.md).

## Ausführen

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

Der Release-Build liegt danach in `dist/Omen Hub++.exe`. Er verwendet Windows und die vorhandenen NVIDIA-Treiberbibliotheken; keine separate Rust-Laufzeit und kein OMEN-Dienst werden für die implementierten Diagnosefunktionen benötigt. Fehlende Treiberabfragen ergeben „Nicht verfügbar“, keine erfundenen Messwerte. Das erste Inventar wird über Windows-WMI abgefragt.

Die optionale CPU-Modulvorbereitung erfolgt **vor** dem Cargo-/Release-Build. Dieses Skript lädt ausschließlich das feste öffentliche Modulrelease samt entsprechenden Quellen und Lizenz nach ignorierter `.tools/pawnio/`; es greift nicht auf Hardware oder Treiberdienste zu:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-cpu-module.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Ohne vorbereitetes Modul bleibt CPU-Paketleistung/Tctl nicht verfügbar. Mit Modul gehören `dist/third-party/PawnIO.Modules/AMDFamily17.p`, das vollständige Quellarchiv und `COPYING` zum ausgelieferten Paket. Der LGPL-2.1-or-later-Modulanteil ist getrennt vom eigenen MIT-Rust-Code lizenziert; ein Treiber wird nicht mitgeliefert. Die Runtime benötigt keine Downloads.

Das moderne, dunkle Dashboard zeigt GPU-Watt und Temperatur links und CPU-Leistung, Tctl und Auslastung direkt daneben. Die drei hervorgehobenen Werkzeugaktionen folgen darunter; Systemdetails und die ausdrücklich markierte HP-Profilvorschau stehen im unteren Bereich. GPU-, Beleuchtungs- und Kurveneditor verwenden dieselben dunklen Felder und Aktionsstile. Das V/F-Diagramm behält seine eigene Auswahl- und Datenpalette. Die native Gestaltung ist in [DESIGN.md](DESIGN.md) festgehalten.

Sichtbare Sensoren werden alle drei Sekunden aktualisiert. Schließen oder Minimieren versteckt das Dashboard im Tray und gibt dessen Kindcontrols frei. Der gewählte Leistungsmodus wird beim Öffnen erhalten; der Wiederaufbau wendet kein Profil an. Im Tray/Suspend werden keine neuen periodischen Samples angefordert; CPU-Treibersitzung und Energie-Baseline werden freigegeben, Worker und pausierter CPU-Reader blockieren. Öffnen, Resume, Refresh und AC-Wechsel beginnen mit frischer Messbasis. Rechtsklick auf das Tray-Symbol bietet Öffnen/Beenden, --tray startet verborgen. Der CPU-Lese-Helper wird beim ersten Öffnen einmal über UAC angefordert; nach Abbruch oder Fehler startet ihn nur **Aktualisieren** neu. Unter **Steuerung** stehen **Leistungsprofile**, **Lüfterkurve** und **Beleuchtung**, rechts daneben der Lüfter-Schnellschalter **Auto** (gespeicherte Kurve; ohne Kurve regelt das BIOS) / **Max** (beide Lüfter voll über den Lüfterdienst). Die Lüfterkurve wird nicht mehr als Leistungsprofil gespeichert, sondern nur als aktive Kurve des Lüfterdienstes. Der feste Windows-Anmeldungseinstieg wird bei der ausdrücklich freigegebenen Startup-Auswahl eines bereits angewendeten, vollständig unterstützten Presets eingerichtet. Beleuchtung und Max-Fan bleiben für Startup gesperrt. [Custom Presets](docs/CUSTOM_PRESETS.md), [GPU und Profile](docs/GPU_PROFILES.md). Jede Einstellung hat ein rundes **i** mit Erklärung; alle Texte stehen in Sprachdateien (Deutsch/Englisch, [Lokalisierung](docs/LOCALIZATION.md)).

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
| GPU- und VRAM-Takt-Offsets | Öffentliche NVML, schreibbar auf 8E35/F.13 / RTX 5070 / 617.14 / VBIOS 98.06.2a.40.da; exakter Readback, Originalbild und Restore |
| GPU-V/F-Kurve | 127 vollständige Graphics-Punkte, freigegebene private Frequenz-ABI des exakt gehashten Treibers; Spannungsanker unverändert, kein hartes Spannungslimit |
| Eigene Performanceprofile | Custom Presets mit getrennten GPU-/CPU-Gruppen, Auswahl/Name und Anwenden/Speichern/Löschen; Format mit 91 typisierten Schlüsseln. Nur qualifizierte Adapter schreiben Hardware; ein nicht unterstütztes Setting sperrt das ganze Apply |
| Neustart-Wiederherstellung | Fester, geschützter Einstieg nach Windows-Benutzeranmeldung; überprüfter Task-Aufruf, kein echter Neustarttest |
| CPU-Auslastung | Windows-CPU-Zeitdifferenzen, implementiert; derzeit 1–64 logische Prozessoren in einer Prozessorgruppe |
| CPU-Takt (Windows) | Durchschnitt der von Windows gemeldeten MHz, implementiert; kein gemessener Boost-/Effektivtakt |
| CPU-Paketleistung und Tctl | Feste PawnIO-/AMD-RAPL-Leseoperationen auf 8E35/F.13/8940HX mit exakt geprüftem Treiber/Modul; nach expliziter UAC-Freigabe, Energie-Mittelwert samt Intervall, erste Baseline ohne Wattwert |
| Native Oberfläche, Tray, AC-/Standby-Ereignisse | Implementiert; Suspend/Resume auf Hardware noch zu prüfen |
| Leistungsmodus | Quiet (Default), Balanced (Default), Performance (Default) noch nicht anwendbar; gespeicherte Custom Presets stehen im selben Dropdown. Dynamic Boost steht im GPU-Presetbereich, die Hardwareanwendung ist noch nicht freigegeben |
| Lüfterzustand und Firmwaretabelle | Feste BIOS-WMI-GETs auf 8E35/F.13; CPU-/GPU-Drehzahlen in 100-RPM-Einheiten, Zuordnung, vollständiger Max-Fan-Schalter und gekoppelte Tabelle |
| Gemeinsame Thermikdiagnose | `cooling-read` erfasst einmalig CPU-Tctl, GPU, Firmware-IR in °C und Lüfterwerte; monotone Zeitfenster, Fehler bei fehlenden/veralteten Daten, keine Kurven-Schreibfreigabe; erhöhte PowerShell erforderlich |
| Max-Fan | Manueller BIOS-WMI-Override auf 8E35/F.13 mit Identitäts-/Tabellen-/Zustandsprüfung, vollständigem Readback und persistiertem Original-Restore; Netzbetrieb erforderlich, kein automatischer Profilstart |
| Lüfterkurven | Getestete Interpolation/Failsafe-Policy und gespeicherte Entwürfe; Modusübernahme, zusätzliche thermische Sicherheitsanforderung und koordinierter Hardwareadapter noch nicht qualifiziert |
| Interaktiver Kurveneditor | Dunkles natives V/F-Diagramm mit unveränderten 127 GPU-Ankern, Bereichsauswahl, Shift/Alt/Strg-Gesten, Abflachen, Undo/Redo und Zoom; getrennte CPU-/GPU-Lüfterentwürfe, Zahlenfelder und Profilspeicherung; Lüfterkurven-Anwendung noch gesperrt |
| Keep-Alive | Getestete Lifecycle-Policy, kein laufender Firmware-Heartbeat |
| CPU-Wattlimits, Undervolting und MUX | Geplant; noch nicht implementiert |
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

Das Budget beträgt **50 MB Working Set im Tray-Leerlauf**, einschließlich eines bereits aktivierten und nun pausierten CPU-Leseprozesses (am 13. Oktober 2026 angehoben). Am 14. Oktober erlaubte der Benutzer ausdrücklich mehr als 50 MB bei geöffnetem Fenster für eine bessere Oberfläche. Die native Win32-Oberfläche benötigt weder Electron, WebView noch eine dauerhafte GPU-Render-Schleife. Das aktuelle Dashboard startet mit 1140×820 Außenmaßen und hat ein Minimum von 960×790; die geprüften Clientansichten und ihre Grenzen stehen im [Dashboard-Brief](.impeccable/surface-briefs/dashboard.md). Das Speicherbudget wird am jeweiligen Release-Build gemessen und ist kein pauschales Versprechen für alle Systeme. Buildbezogene Ergebnisse einschließlich offenem Fenster und kombiniertem Tray-Verbrauch stehen in [VALIDATION.md](docs/VALIDATION.md).

## Projektaufbau und Fahrplan

* `crates/core`: Hardware-unabhängige Profile, Diagnosedaten, CPU-Energie-/Tctl-Umrechnung, Lüfter- und Heartbeat-Policies.
* `crates/platform`: Windows-WMI, Power-API, NVAPI/NVML, qualifizierter fester CPU-Reader, validierter GPU-Controller, Profil-/Transaktionsspeicher und geschützter Anmeldestart.
* `crates/app`: Standalone-Anwendung mit nativer Oberfläche, Tray, sitzungsgebundener CPU-Lesebrücke und Diagnose-CLI.
* `docs`: Architektur, Fahrplan, Modellbefunde und Reverse-Engineering-Protokoll.
* `scripts`: Reproduzierbare Release- und lokale Analysewerkzeuge.

[Projektziel und Fahrplan](docs/ROADMAP.md) · [Architektur](docs/ARCHITECTURE.md) · [GPU und Profile](docs/GPU_PROFILES.md) · [Lüfter und Max-Fan](docs/FAN_CONTROL.md) · [HP-Analyse](docs/REVERSE_ENGINEERING.md) · [Referenzgerät 8E35](docs/models/8e35.md)

Die lokale Analyse von HP-Dateien liegt ausschließlich in `.local/`; Werkzeuge in `.tools/`. Beide Ordner sind von Git ausgeschlossen. Proprietäre DLLs, Treiber und dekompilierter HP-Quelltext werden nicht verteilt. Technische Projektnamen verwenden `omen-hub-plus-plus`/`omen-hub-pp`; der Produktname ist **Omen Hub++**. Die bestehende GitHub-URL bleibt `Ishtvr/Omen-Hub-debloat`.

## Referenzen

Die [OmenMon-Dokumentation](https://omenmon.github.io/cli) und [OmenHwCtl](https://github.com/GeographicCone/OmenHwCtl) dokumentieren vorhandene HP-Schnittstellen. Daraus folgt keine bestätigte Unterstützung dieses Modells. Für NVIDIA verwenden wir die [öffentlichen NVAPI-Header](https://github.com/NVIDIA/nvapi) und [NVML-Geräteabfragen](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html). Der CPU-Reader folgt dem öffentlichen [PawnIO-Protokoll](https://github.com/namazso/PawnIO/blob/9d52965895588b0ffa3703b72eec75ba19f4ccc0/PawnIO/include/pawnio_um.h) und dem signierten [PawnIO.Modules-Release 0.2.11](https://github.com/namazso/PawnIO.Modules/releases/tag/0.2.11). Die spätere CPU-Tuning-Anbindung orientiert sich am [libryzenadj-Header](https://github.com/FlyGoat/RyzenAdj/blob/master/lib/ryzenadj.h).

Eigener Code: MIT. Abhängigkeiten und ihre Lizenzen: [THIRD_PARTY.md](THIRD_PARTY.md). Nicht mit HP oder NVIDIA verbunden oder von ihnen unterstützt.
