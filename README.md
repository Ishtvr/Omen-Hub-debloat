# Omen Hub++

Eine native, leichtgewichtige Open-Source-Alternative zum OMEN Gaming Hub in Rust. Ziel sind HP-Systemkontrolle und die relevanten Tuning-Möglichkeiten von UXTU in einer einzigen Anwendung: ohne Werbung, Overlay, Telemetrie, Konto oder Hintergrund-Updater.

**Status: erster lauffähiger Diagnose-Prototyp, noch kein vollständiger Gaming-Hub-Ersatz.** Die Anwendung liest Hardwaredaten und Sensoren. Hardware-Schreibzugriffe sind nicht implementiert; die Profilauswahl zeigt ausdrücklich eine Vorschau.

## Ausführen

Windows x64, Rust mit MSVC-Toolchain und Visual Studio C++ Build Tools:

```powershell
cargo run -p omen-hub-plus-plus
cargo run -p omen-hub-plus-plus -- --tray
cargo run -p omen-hub-plus-plus -- diagnose
cargo run -p omen-hub-plus-plus -- plan unleashed
```

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
pwsh -File scripts/build-release.ps1
```

Der Release-Build liegt danach in `dist/Omen Hub++.exe`. Er verwendet Windows und die vorhandenen NVIDIA-Treiberbibliotheken; keine separate Rust-Laufzeit und kein OMEN-Dienst werden für die implementierten Diagnosefunktionen benötigt. Fehlende Treiberabfragen ergeben „Nicht verfügbar“, keine erfundenen Messwerte. Das erste Inventar wird über Windows-WMI abgefragt.

Das Fenster aktualisiert NVIDIA-Sensoren alle drei Sekunden. Schließen oder Minimieren versteckt es im Tray. Im Tray werden keine Sensoren periodisch abgefragt und NVIDIA-Sitzungen freigegeben. Rechtsklick auf das Tray-Symbol bietet Öffnen und Beenden. `--tray` startet direkt verborgen. Es wird kein Autostart eingerichtet.

## Funktionsstand

| Funktion | Stand |
| --- | --- |
| Modell, Board, BIOS, CPU, GPUs | Windows-WMI, implementiert |
| Vorhandensein der HP-BIOS-Schnittstelle | WMI-Metadaten, implementiert; keine Capability-Zusage |
| Netzbetrieb und Akkustand | Windows-Power-API, implementiert |
| NVIDIA-Temperatur und aktueller Takt | Öffentliche NVAPI, implementiert |
| GPU-Watt, Standard-Limit und Treiber-Obergrenze | Öffentliche NVML, implementiert, soweit der Treiber Werte liefert |
| Native Oberfläche, Tray, AC-/Standby-Ereignisse | Implementiert; Suspend/Resume auf Hardware noch zu prüfen |
| Quiet/Balanced/Performance/Unleashed | Vorschau; nicht anwendbar |
| Lüfterkurven | Getestete Interpolation/Failsafe-Policy, noch kein EC-/WMI-Adapter |
| Keep-Alive | Getestete Lifecycle-Policy, kein laufender Firmware-Heartbeat |
| CPU-Wattlimits, Undervolting, MUX, RGB | Geplant; noch nicht implementiert |
| Autostart / privilegierter Hardware-Dienst | Geplant; noch nicht implementiert |

Sensoren beziehen sich derzeit auf die erste von NVIDIA enumerierte GPU. Bei unterschiedlichen NVAPI-/NVML-Namen werden die Messungen nicht zusammengeführt. Mehrere gleichnamige NVIDIA-GPUs benötigen später eine Zuordnung über PCI-Identität.

## Leistungsziel

Der Unleashed-Modus soll den **vom konkreten Gerät vorgesehenen** Leistungsrahmen einschließlich Dynamic Boost korrekt aktivieren und erhalten. Er verspricht kein Umgehen von VBIOS-, Firmware-, Netzteil- oder thermischen Grenzen. Ein gemeldetes maximales Power-Limit ist kein Nachweis dauerhaft erreichbarer GPU-Leistung.

Ziel sind weniger als 20 MB RAM im Tray-Leerlauf. Für die erste Windows-Version nutzen wir direkt die native Win32-Oberfläche, ohne Electron, WebView oder dauerhafte GPU-Render-Schleife. Das Budget wird am Release-Build gemessen und ist kein pauschales Versprechen für alle Systeme. Ergebnisse stehen in [VALIDATION.md](docs/VALIDATION.md).

## Projektaufbau und Fahrplan

* `crates/core`: Hardware-unabhängige Profile, Diagnosedaten, Lüfter- und Heartbeat-Policies.
* `crates/platform`: Windows-WMI, Power-API, NVAPI/NVML als lesende Adapter.
* `crates/app`: Standalone-Anwendung mit nativer Oberfläche, Tray und Diagnose-CLI.
* `docs`: Architektur, Fahrplan, Modellbefunde und Reverse-Engineering-Protokoll.
* `scripts`: Reproduzierbare Release- und lokale Analysewerkzeuge.

[Projektziel und Fahrplan](docs/ROADMAP.md) · [Architektur](docs/ARCHITECTURE.md) · [HP-Analyse](docs/REVERSE_ENGINEERING.md) · [Referenzgerät 8E35](docs/models/8e35.md)

Die lokale Analyse von HP-Dateien liegt ausschließlich in `.local/`; Werkzeuge in `.tools/`. Beide Ordner sind von Git ausgeschlossen. Proprietäre DLLs, Treiber und dekompilierter HP-Quelltext werden nicht verteilt. Technische Projektnamen verwenden `omen-hub-plus-plus`/`omen-hub-pp`; der Produktname ist **Omen Hub++**. Die bestehende GitHub-URL bleibt `Ishtvr/Omen-Hub-debloat`.

## Referenzen

Die [OmenMon-Dokumentation](https://omenmon.github.io/cli) und [OmenHwCtl](https://github.com/GeographicCone/OmenHwCtl) dokumentieren vorhandene HP-Schnittstellen. Daraus folgt keine bestätigte Unterstützung dieses Modells. Für NVIDIA verwenden wir die [öffentlichen NVAPI-Header](https://github.com/NVIDIA/nvapi) und [NVML-Geräteabfragen](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html). Die spätere Ryzen-Anbindung orientiert sich am [libryzenadj-Header](https://github.com/FlyGoat/RyzenAdj/blob/master/lib/ryzenadj.h).

Eigener Code: MIT. Abhängigkeiten und ihre Lizenzen: [THIRD_PARTY.md](THIRD_PARTY.md). Nicht mit HP oder NVIDIA verbunden oder von ihnen unterstützt.

