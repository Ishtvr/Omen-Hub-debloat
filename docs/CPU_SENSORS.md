# CPU-Hardwaresensoren

Der auf HP **8E35 / BIOS F.13 / Ryzen 9 8940HX** geprüfte Adapter liest Paketenergie, CPU-Steuertemperatur und seit 22. Oktober 2026 die effektiven Takte aller **32 logischen Prozessoren** über den bereits installierten **PawnIO 2.2.0**-Treiber. Er installiert oder startet keinen Treiber und benötigt keine HP-Controllerprozesse oder AMD-SDK-Initialisierung. Noch laufende HP-Prozesse wurden während der Messung nicht entfernt; ein vollständiger Ersatzbetrieb wird dadurch nicht bestätigt.

Das native Dashboard fordert die Sensoren beim ersten Öffnen einmal über Windows-UAC an. Nach Abbruch oder Fehler fragt erst **Aktualisieren** erneut; Vorschauen fragen nie. Ein kurzlebiger eigener Startprozess kapselt den UAC-Aufruf und übergibt die freigegebene Prozess-ID über seine private Ausgabepipe. Ein separater CPU-Leseprozess liefert ausschließlich gemessene Werte an den normalen Benutzerprozess. Der lokale IPC-Kanal ist auf den Benutzer, die gestarteten Prozess-IDs, eine Sitzungs-Nonce und fortlaufende Antworten gebunden. Profile, Treiberblobs, beliebige Funktionsnamen oder Registeradressen werden über diesen Kanal nicht angenommen. Der Geräteabgleich benutzt einen begrenzten kurzlebigen Inventarprozess; der laufende Reader lädt dafür keine WMI-Verbindung. Treiberstatus und Binärpfad werden über Windows-Dienstabfragen ausschließlich gelesen.

Die Paketleistung ist der Mittelwert aus zwei Energiezählerständen. Der erste Stand liefert noch keinen Wattwert. Die Temperatur ist **Tctl**, die CPU-Steuertemperatur, kein Mittelwert von Kerntemperaturen und keine unzugeordnete ACPI-Zone. Der Windows-MHz-Wert bleibt getrennt und ist kein gemessener Boost-/Effektivtakt. Ein konfiguriertes PPT-Limit wird nicht als gemessene Leistung verwendet.

**Effektiv max.** zeigt den höchsten gemessenen Intervallmittelwert der logischen Prozessoren. Der Reader zählt tatsächliche Zyklen aus **APERF Read-Only (0xC00000E8)** über monotone Wandzeit; Leerlauf-/Haltzeit bleibt im Zeitnenner enthalten. **MPERF Read-Only (0xC00000E7)** muss fortschreiten und dient der Plausibilitätsprüfung. Angezeigt werden Abdeckung und das längste Einzelintervall der Messung. Das ist weder ein augenblicklicher Boost-Spitzentakt noch ein Mittel nur während aktiver CPU-Zeit. Die ausführliche CLI-Ausgabe enthält jeden logischen Prozessor mit eigenem Intervall und expliziter Quelle `amd_aperf_read_only`. CCD-/Kerntemperaturen und aktive Boost-Takte bleiben offen.

Vor jedem Taktbatch prüft der Adapter AMD `EffFreqRO`, eine einzelne Prozessorgruppe und die vollständige 32-CPU-Affinitätsmaske. Nur der eigene Lesethread wird kurz auf den jeweiligen Prozessor gesetzt und anschließend auf seine gespeicherte Maske zurückgesetzt; weder Prozessrichtlinien noch fremde Programme werden verändert. Für den APERF-Lesezugriff gelten höchstens 10 ms, für den vollständigen Durchlauf 100 ms, für ein Messintervall 200…10000 ms. Sinkende oder eingefrorene Zähler, CPU-/Abdeckungskonflikte, fehlende Antworten und unplausible Taktraten verwerfen die komplette Taktmessbasis. Die 8000-MHz-Grenze ist lediglich eine Plausibilitätsprüfung, keine Schreibfreigabe. Eine Taktstörung lässt gültige Paket-Watt/Tctl-Werte bestehen und wird separat als nicht verfügbar angezeigt. HWCR, Zähler-Enable/Reset, Spannungen und Frequenzvorgaben werden hierfür nicht geschrieben.

Im Tray beziehungsweise bei Suspend wird der Leseprozess pausiert: Treiber-/Modulsitzung sowie Energie- und Takt-Baselines werden freigegeben, beide Prozesse warten ohne periodische Sensorabfragen. Nach erneutem Öffnen, Resume, Refresh oder Versorgungswechsel ist eine neue Baseline erforderlich. Bei Fehlern werden alte Werte verworfen. Die Sensorfreigabe gilt für die laufende App-Sitzung, nicht als automatische Hardwarefreigabe nach Neustart.

Buildbezogene Working-Set-/CPU-Messungen, aktuelle Binary-Identität und Grenzen stehen in [VALIDATION.md](VALIDATION.md). Das Tray-Budget von 50 MB zählt den pausierten Leseprozess mit; ältere Messungen werden nicht auf spätere Releases übertragen.

## Quellen, feste Verträge und Identität

Der Reader folgt dem öffentlichen [PawnIO-Geräteprotokoll](https://github.com/namazso/PawnIO/blob/9d52965895588b0ffa3703b72eec75ba19f4ccc0/PawnIO/include/pawnio_um.h), lädt ausschließlich das gehashte signierte AMD-Modul und ruft darin ausschließlich `ioctl_read_msr` für **0xC0010299 / 0xC001029B / 0xC00000E7 / 0xC00000E8** sowie `ioctl_read_smn` für **0x59800** auf. Der SMN-Leseweg benötigt die Auswahl des Adressfensters; dies ist keine Änderung eines Leistungs-/Spannungsparameters. Er wird über `Global\Access_PCI` serialisiert und vor dem Taktbatch wieder freigegeben.

Energieeinheiten und der niedrige 32-Bit-Zähler entsprechen den in [Linux RAPL](https://github.com/torvalds/linux/blob/master/arch/x86/events/rapl.c) dokumentierten AMD-MSRs. Temperaturregister und Bereichskorrektur sind in [k10temp](https://github.com/torvalds/linux/blob/master/drivers/hwmon/k10temp.c) beschrieben. Die Umrechnung im Core ist eigener Code. Paketenergie, Monotonzeit, Zählerüberlauf, geänderte Einheiten und veraltete Intervalle werden getrennt geprüft. Der Adapter bietet keinen MSR-, SMU-, Power- oder Voltage-Setter.

| Bestandteil | Qualifizierte Identität |
| --- | --- |
| PawnIO.sys | 2.2.0, SHA-256 `FCA6E7D58B0CF38DBB913A2B9E532F48629145D395F454B16A9F58E97B8D3940` |
| AMDFamily17.bin | Signiertes Release 0.2.11, 10652 Bytes, SHA-256 `DAE74615761B78BDF064DFB3E136252DDCC6FC727D88F14738D0E5800D427A91` |
| Modulsourcen | Tag 0.2.11, Commit `52a7e536dff3e53c96917a28caac5e0fa6510696` |
| CPU | CPUID-Signatur 10882834, Family19h/Model61h/Stepping2, Package1 |

Andere Board-/BIOS-/CPU-/Treiberkombinationen benötigen eine eigene Prüfung. Fehlender Treiber, fehlende Rechte, unpassende Version/Hashes und ungültige Antworten liefern einen sichtbaren Fehler statt Ersatzwerten.

AMD beschreibt `CPUID Fn8000_0007_EDX[10]` und die Read-Only-MSRs in der [CPUID Specification, 25481](https://www.amd.com/content/dam/amd/en/documents/archived-tech-docs/design-guides/25481.pdf). Der APERF-Zyklen/Wandzeit-Ansatz ist auch in [LibreHardwareMonitor, Amd17Cpu](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/LibreHardwareMonitorLib/Hardware/Cpu/Amd17Cpu.cs) nachvollziehbar. Die feste Read-Allowlist ist Teil von [PawnIO.Modules 0.2.11, AMDFamily17.p](https://github.com/namazso/PawnIO.Modules/blob/52a7e536dff3e53c96917a28caac5e0fa6510696/AMDFamily17.p). Die Takt-Erweiterung verwendet eigene Rust-Berechnungen und benötigt keine Moduländerung oder neuen Treiber.

## Bauen und Verteilen

`scripts/prepare-cpu-module.ps1` lädt das feste signierte Modularchiv samt LGPL-Quellen und Lizenz ausschließlich zur Buildvorbereitung nach ignorierter `.tools/pawnio/`. Es hat keinen Treiber- oder Hardwarezugriff. Danach wird das Modul beim Cargo-Build nach Größen-/SHA-Prüfung in die native Binary eingebettet. Die Runtime lädt keine Dateien aus `.tools/` und hat keine Netzwerkabfragen. Ein Build ohne vorbereitetes Modul bleibt für CPU-Hardwarewerte ausdrücklich nicht verfügbar; andere Funktionen bleiben nutzbar.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-cpu-module.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Der Release-Build legt die Modulquelle, das vollständige Quellarchiv und `COPYING` unter `dist/third-party/PawnIO.Modules/` ab. Diese LGPL-2.1-or-later-Komponente bleibt eigenständig lizenziert; eigener Rust-Code bleibt MIT. Für die Weitergabe gehören diese Hinweise und Quellen zur Binary. Die Modulvorbereitung muss nicht bei jedem Build wiederholt werden. Entwickler können die Integration und die Build-Prüfung aus dem veröffentlichten Rust-Quelltext ändern und neu bauen; der Treiber prüft seine Modulsignaturen unabhängig davon. Siehe [THIRD_PARTY.md](../THIRD_PARTY.md).

Eine separate erhöhte Diagnose ist `Omen Hub++.exe cpu-hardware-read`. Sie liest eine Baseline und nach mindestens 1100 ms einen zweiten Stand, mit Quelle, Messintervall und Tctl-Kennzeichnung. Sie ruft keine Setter auf. Messungen und aktuelle Grenzen stehen in [VALIDATION.md](VALIDATION.md).

`scripts/check-cpu-clocks.ps1` validiert diese feste Diagnose in einem erhöhten Prozess: erste Baseline ohne Watt/Takte, alle 32 Taktwerte, gültige Intervalle, weiter verfügbare Watt/Tctl, unveränderte öffentliche GPU-Offsets und Profil-/Startup-/Lüfter-Metadaten. Ein optionaler zuvor qualifizierter unabhängiger HWCR-GET wird nur bei unveränderten Probe-/Treiberhashes und bereits laufendem Treiber verwendet. Fehlende Voraussetzungen werden weder installiert noch repariert. Rohdaten bleiben in ignorierter `.local/research/cpu-clocks/`. `smoke-cpu-sensors.ps1` prüft die reale eigene UI/IPC mit frischen Baselines nach Tray, Refresh und simuliertem Suspend/Resume. 106/107 sind inzwischen Lüfter-/Grafikbefehle und werden von diesen Sensor-/Speicherprüfungen nicht gesendet. Das ersetzt keinen echten Standby-/Neustart- oder Lasttest.

Die Oberfläche unterscheidet fehlenden Build-Support, unbestätigtes Gerät/BIOS, fehlenden oder unpassenden Treiber, abgebrochene Windows-Freigabe und eine unterbrochene Messung mit deutschen Hinweisen. Dauerhafte Supportprobleme erhalten keine pauschale Wiederholungsempfehlung. Die genaue technische Ursache bleibt im Snapshot-Fehlerzustand und wird beim Fehler separat als `last-cpu-error.json` im Benutzerprofilspeicher gespeichert. `cpu-sensor-status` gibt diesen letzten Fehler samt Zeitpunkt und Benutzerhinweis aus; dies ist keine Aussage über eine inzwischen erfolgreich neu gestartete Sitzung. Die Diagnoseablage verändert weder das Hardwarejournal noch eine Startup-Freigabe.
