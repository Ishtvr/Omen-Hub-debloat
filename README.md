# Omen Hub++

Eine native, leichtgewichtige Open-Source-Alternative zum OMEN Gaming Hub in Rust. Ziel sind HP-Systemkontrolle und die relevanten Tuning-Möglichkeiten von UXTU in einer einzigen Anwendung: ohne Werbung, Overlay, Telemetrie, Konto oder Hintergrund-Updater.

**Status: native Diagnose, GPU-/VRAM-Taktsteuerung, vollständige GPU-V/F-Kurven und eigene gespeicherte Profile; noch kein vollständiger Gaming-Hub-Ersatz.** Die NVIDIA-Steuerpfade sind auf dem Referenzgerät mit Readback und Restore validiert. Der GPU-Profil-Editor speichert globale Offsets oder vollständige Frequenzkurven über alle 127 vorhandenen Spannungsanker. Der gemeinsame Kurveneditor bietet dafür Punkt-/Bereichsbearbeitung, Shift/Alt/Strg-Gesten, Plateau und Verlauf. V/F-Kurven ermöglichen Tuning für Undervolting; ein hartes Spannungslimit und der physische Effekt unter Last sind damit nicht bewiesen. Unleashed, direkte mV-Offsets und VRAM-Spannungssteuerung bleiben gesperrt. Die vier HP-Systemprofile im Dashboard sind weiterhin ausdrücklich Vorschauen.

## Ausführen

Windows x64, Rust mit MSVC-Toolchain und Visual Studio C++ Build Tools:

```powershell
cargo run -p omen-hub-plus-plus
cargo run -p omen-hub-plus-plus -- --tray
cargo run -p omen-hub-plus-plus -- diagnose
cargo run -p omen-hub-plus-plus -- gpu-ui
cargo run -p omen-hub-plus-plus -- curve-ui
cargo run -p omen-hub-plus-plus -- gpu-capabilities
cargo run -p omen-hub-plus-plus -- profile-list
cargo run -p omen-hub-plus-plus -- hp-read
cargo run -p omen-hub-plus-plus -- keyboard-read
cargo run -p omen-hub-plus-plus -- plan unleashed
```

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Der Release-Build liegt danach in `dist/Omen Hub++.exe`. Er verwendet Windows und die vorhandenen NVIDIA-Treiberbibliotheken; keine separate Rust-Laufzeit und kein OMEN-Dienst werden für die implementierten Diagnosefunktionen benötigt. Fehlende Treiberabfragen ergeben „Nicht verfügbar“, keine erfundenen Messwerte. Das erste Inventar wird über Windows-WMI abgefragt.

Das Fenster zeigt GPU- und CPU-Leistung vor den Systemdetails und aktualisiert die Sensoren alle drei Sekunden. Schließen oder Minimieren versteckt es im Tray. Im Tray werden keine Sensoren periodisch abgefragt und NVIDIA-Sitzungen freigegeben; beim Öffnen beginnt die CPU-Auslastungsmessung mit einer frischen Zeitbasis. Rechtsklick auf das Tray-Symbol bietet Öffnen und Beenden. `--tray` startet direkt verborgen. **GPU-Profile …** öffnet den nativen Editor. Der optionale, feste Windows-Anmeldungseintrag wird getrennt eingerichtet und führt nur ausdrücklich bestätigte Profilversionen aus: [GPU und Profile](docs/GPU_PROFILES.md).

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
| Eigene Performanceprofile | Versioniertes Format mit 85 Feature-Schlüsseln; schreibbar sind Core-/Memory-Offset und Graphics-V/F-Kurve; weitere Adapter stehen aus |
| Neustart-Wiederherstellung | Fester, geschützter Einstieg nach Windows-Benutzeranmeldung; überprüfter Task-Aufruf, kein echter Neustarttest |
| CPU-Auslastung | Windows-CPU-Zeitdifferenzen, implementiert; derzeit 1–64 logische Prozessoren in einer Prozessorgruppe |
| CPU-Takt (Windows) | Durchschnitt der von Windows gemeldeten MHz, implementiert; kein gemessener Boost-/Effektivtakt |
| CPU-Paketleistung und Prozessortemperatur | Noch nicht verfügbar; AMD-SDK-Sensorpfad identifiziert, aber auf dem Gerät noch nicht validiert |
| Native Oberfläche, Tray, AC-/Standby-Ereignisse | Implementiert; Suspend/Resume auf Hardware noch zu prüfen |
| Quiet/Balanced/Performance/Unleashed | Vorschau; nicht anwendbar |
| Lüfterkurven | Getestete Interpolation/Failsafe-Policy, noch kein EC-/WMI-Adapter |
| Interaktiver Kurveneditor | Dunkles natives V/F-Diagramm mit unveränderten 127 GPU-Ankern, Bereichsauswahl, Shift/Alt/Strg-Gesten, Abflachen, Undo/Redo und Zoom; getrennte CPU-/GPU-Lüfterentwürfe, Zahlenfelder und Profilspeicherung; Lüfter-Hardwareanwendung noch gesperrt |
| Keep-Alive | Getestete Lifecycle-Policy, kein laufender Firmware-Heartbeat |
| CPU-Wattlimits, Undervolting, MUX, RGB-Steuerung | Geplant; noch nicht implementiert |
| Tastaturtyp / RGB- und Helligkeitszustand | Nativer fester GET implementiert und erhöht auf 8E35/F.13 validiert: vier Zonen mit Nummernblock; keine Beleuchtungsänderung |
| Privilegierter Hardware-Dienst | Nicht implementiert; feste kurzlebige UAC-Prozesse für Steueraktionen |

Sensoren beziehen sich derzeit auf die erste von NVIDIA enumerierte GPU. Bei unterschiedlichen NVAPI-/NVML-Namen werden die Messungen nicht zusammengeführt. Mehrere gleichnamige NVIDIA-GPUs benötigen später eine Zuordnung über PCI-Identität.

CPU-Auslastung misst aktive Prozessorzeit und ist kein Wattwert. Der Windows-Takt kann vom tatsächlichen Boost-Takt abweichen. CPU-Paketleistung und Prozessortemperatur bleiben ausdrücklich „Nicht verfügbar“, bis ein geprüfter Hardwareadapter existiert. Ein CPU-Leistungslimit oder eine ACPI-Thermalzone wird nicht als Ersatzmesswert angezeigt.

`hp-read` und `keyboard-read` sind separate, feste lesende BIOS-Abfragen, die nicht im Dashboard und nicht als Heartbeat laufen. Sie sind auf das untersuchte Board 8E35 mit BIOS F.13 begrenzt. Der Provider verweigert den Zugriff im nicht erhöhten Prozess (`0x80041003`). In einem als Administrator gestarteten Terminal:

```powershell
& '.\dist\Omen Hub++.exe' keyboard-read | Out-String
& '.\dist\Omen Hub++.exe' hp-read | Out-String
```

Die festen HP-CLI-Reader erhöhen ihre Rechte nicht selbst und bieten keine BIOS-Setter. Datentypen, Returncode, Länge und bekannte Feldwerte werden geprüft. Der erhöhte GPU-Status-GET liefert **[1, 1, 0, 87]**. Der Benutzer bestätigt gleichzeitig Unleashed im Original-Hub. Der Decoder erhält deshalb den beobachteten D-State 0 mit `d_state_known: false`; daraus folgt keine erfundene Bedeutung oder Schreibfreigabe. `hp-performance-read` liest zusätzlich die 128 Byte SystemDesignData aus der Firmware, ohne Registry-Cache als Livezustand auszugeben.

Der GPU-Editor fordert für **Anwenden**, **Original wiederherstellen** und die geschützte Freigabe zur Anmeldung gezielt Windows-UAC an. Es gibt keinen beliebigen erhöhten Kommando-Dienst. Das Profil wird vollständig geprüft, vor dem ersten Schreibzugriff protokolliert und bei Teilfehlern zurückgesetzt. Nicht unterstützte Settings sperren das ganze Profil. Eigene Profile liegen unter dem aus dem Windows-Token ermittelten Benutzerprofil in `.omen-hub-pp`; dieser Ort ist unabhängig vom Codex-MSIX-AppData-Cache.

Auf NVIDIA 617.14 setzt ein NVML-VRAM-Offset-Aufruf eine individuelle Graphics-Kurve zurück. Omen Hub++ erfasst deshalb auch bei VRAM-Änderungen das vollständige Graphics-Original, erhält es nach dem Aufruf und setzt den endgültigen GPU-Zielzustand zuletzt. Ein globaler Core-Offset und eine vollständige V/F-Kurve sind im selben Profil ausgeschlossen. Aktuelle Frequenzbeobachtungen werden getrennt von den exakt geprüften programmierten Steuerwerten ausgegeben.

`keyboard-read` liest die Capability und bei klassischen Varianten Farben/Helligkeit. Auf 8E35/F.13 wurde **FourZoneWithNumpad** bestätigt. Die vollständigen 128-Byte-Zustände stimmen mit dem unabhängigen PowerShell-Gegencheck überein; Helligkeit bleibt ein Rohbyte. Die Größe-0-Eingabe des Rust-Adapters ist am Gerät validiert. Farbänderung, Readback nach einer Änderung und Restore sind noch nicht implementiert oder freigegeben. Die Herstellerdienste liefen während der Proben weiter; ein vollständiger Ersatzbetrieb ist damit noch nicht nachgewiesen.

Der Gegencheck `scripts/read-keyboard-state.ps1` läuft mit vorhandener Windows PowerShell **5.1**, ohne `pwsh`-Installation. Er speichert Diagnosezustände und Rohblöcke ausschließlich unter ignorierter `.local/research/keyboard-state/`:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\read-keyboard-state.ps1'
```

Die Startoption gilt nur für diesen Prozess; keine gespeicherte Richtlinie wird geändert. Auch dieser Reader braucht hier ein Administrator-Terminal und erhöht sich nicht selbst. Befunde: [Tastaturbeleuchtung](docs/KEYBOARD_LIGHTING.md).

Omen Hub++ soll den originalen Hub und seine Hintergrundcontroller vollständig ersetzen. Der eigene Hardwaretransport soll deshalb keine HP-RPC-/SDK-Wrapper als notwendige Runtime-Abhängigkeit erhalten. Welche heutigen Hintergrundaufgaben die eigene Engine übernehmen muss, steht im [Ersatzplan](docs/REPLACEMENT_PLAN.md).

## Leistungsziel

**Lüfter & V/F …** öffnet den gemeinsamen nativen Kurveneditor in einer dunklen Diagrammansicht nach der MSI-Afterburner-Referenz des Benutzers. GPU-Frequenzpunkte und Spannungsbereiche lassen sich mit Shift/Alt/Strg-Gesten, Tastatur, Abflachen und 64-Schritt-Verlauf bearbeiten. Separate CPU-/GPU-Lüfterentwürfe bleiben beim Ansichtswechsel erhalten. **Profil / Werte …** oder Enter im Diagramm öffnet die numerische Bearbeitung; F1 zeigt die Tastenkürzel. Zoom und **Ganze Kurve** verändern nur den Ausschnitt. Bearbeiten und Speichern ändern keine Hardware; Anwenden erfolgt getrennt im GPU-Profilfenster. Lüfteranwendung, fester Spannungs-/Takt-Lock und OC Scanner bleiben nicht unterstützt. Bedienung und Einschränkungen: [Kurveneditor](docs/CURVE_EDITOR.md).

Der Unleashed-Modus soll den **vom konkreten Gerät vorgesehenen** Leistungsrahmen einschließlich Dynamic Boost korrekt aktivieren und erhalten. Er verspricht kein Umgehen von VBIOS-, Firmware-, Netzteil- oder thermischen Grenzen. Ein gemeldetes maximales Power-Limit ist kein Nachweis dauerhaft erreichbarer GPU-Leistung.

Ziel sind weniger als 50 MB RAM im Tray-Leerlauf (vom Benutzer am 13. Oktober 2026 angehoben). Für die erste Windows-Version nutzen wir direkt die native Win32-Oberfläche, ohne Electron, WebView oder dauerhafte GPU-Render-Schleife. Das Budget wird am Release-Build gemessen und ist kein pauschales Versprechen für alle Systeme. Ergebnisse stehen in [VALIDATION.md](docs/VALIDATION.md).

## Projektaufbau und Fahrplan

* `crates/core`: Hardware-unabhängige Profile, Diagnosedaten, Lüfter- und Heartbeat-Policies.
* `crates/platform`: Windows-WMI, Power-API, NVAPI/NVML, validierter GPU-Controller, Profil-/Transaktionsspeicher und geschützter Anmeldestart.
* `crates/app`: Standalone-Anwendung mit nativer Oberfläche, Tray und Diagnose-CLI.
* `docs`: Architektur, Fahrplan, Modellbefunde und Reverse-Engineering-Protokoll.
* `scripts`: Reproduzierbare Release- und lokale Analysewerkzeuge.

[Projektziel und Fahrplan](docs/ROADMAP.md) · [Architektur](docs/ARCHITECTURE.md) · [GPU und Profile](docs/GPU_PROFILES.md) · [HP-Analyse](docs/REVERSE_ENGINEERING.md) · [Referenzgerät 8E35](docs/models/8e35.md)

Die lokale Analyse von HP-Dateien liegt ausschließlich in `.local/`; Werkzeuge in `.tools/`. Beide Ordner sind von Git ausgeschlossen. Proprietäre DLLs, Treiber und dekompilierter HP-Quelltext werden nicht verteilt. Technische Projektnamen verwenden `omen-hub-plus-plus`/`omen-hub-pp`; der Produktname ist **Omen Hub++**. Die bestehende GitHub-URL bleibt `Ishtvr/Omen-Hub-debloat`.

## Referenzen

Die [OmenMon-Dokumentation](https://omenmon.github.io/cli) und [OmenHwCtl](https://github.com/GeographicCone/OmenHwCtl) dokumentieren vorhandene HP-Schnittstellen. Daraus folgt keine bestätigte Unterstützung dieses Modells. Für NVIDIA verwenden wir die [öffentlichen NVAPI-Header](https://github.com/NVIDIA/nvapi) und [NVML-Geräteabfragen](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html). Die spätere Ryzen-Anbindung orientiert sich am [libryzenadj-Header](https://github.com/FlyGoat/RyzenAdj/blob/master/lib/ryzenadj.h).

Eigener Code: MIT. Abhängigkeiten und ihre Lizenzen: [THIRD_PARTY.md](THIRD_PARTY.md). Nicht mit HP oder NVIDIA verbunden oder von ihnen unterstützt.
