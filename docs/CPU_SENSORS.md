# CPU-Hardwaresensoren

Der auf HP **8E35 / BIOS F.13 / Ryzen 9 8940HX** geprüfte Adapter liest Paketenergie und CPU-Steuertemperatur über den bereits installierten **PawnIO 2.2.0**-Treiber. Er installiert oder startet keinen Treiber und benötigt keine HP-Controllerprozesse oder AMD-SDK-Initialisierung. Noch laufende HP-Prozesse wurden während der Messung nicht entfernt; ein vollständiger Ersatzbetrieb wird dadurch nicht bestätigt.

Das native Dashboard fordert die Sensoren beim ersten Öffnen einmal über Windows-UAC an. Nach Abbruch oder Fehler fragt erst **Aktualisieren** erneut; Vorschauen fragen nie. Ein kurzlebiger eigener Startprozess kapselt den UAC-Aufruf und übergibt die freigegebene Prozess-ID über seine private Ausgabepipe. Ein separater CPU-Leseprozess liefert ausschließlich gemessene Werte an den normalen Benutzerprozess. Der lokale IPC-Kanal ist auf den Benutzer, die gestarteten Prozess-IDs, eine Sitzungs-Nonce und fortlaufende Antworten gebunden. Profile, Treiberblobs, beliebige Funktionsnamen oder Registeradressen werden über diesen Kanal nicht angenommen. Der Geräteabgleich benutzt einen begrenzten kurzlebigen Inventarprozess; der laufende Reader lädt dafür keine WMI-Verbindung. Treiberstatus und Binärpfad werden über Windows-Dienstabfragen ausschließlich gelesen.

Die Paketleistung ist der Mittelwert aus zwei Energiezählerständen über das angezeigte Intervall. Der erste Stand liefert noch keinen Wattwert. Die Temperatur ist **Tctl**, die CPU-Steuertemperatur, kein Mittelwert von Kerntemperaturen und keine unzugeordnete ACPI-Zone. Der Windows-MHz-Wert bleibt getrennt und ist kein gemessener Boost-/Effektivtakt. Ein konfiguriertes PPT-Limit wird nicht als gemessene Leistung verwendet.

**Takt und CCD-Temperaturen (seit 20. Oktober 2026, auf dem Gerät noch nicht abgeglichen):** Der Reader liest pro logischem Prozessor die schreibgeschützten Zähler APERF/MPERF (`0xC00000E8`/`0xC00000E7`) auf genau diesem Prozessor und einmal die P-State-0-Definition (`0xC0010064`, 200 MHz × Fid / Dfs). Daraus entstehen zwei Werte: der höchste Takt im Wachzustand (P0 × ΔAPERF/ΔMPERF, nach AMD APM „effective frequency“; das ist der tatsächlich erreichte Boost) und der mittlere Effektivtakt aller logischen Prozessoren (ΔAPERF pro Zeit, Leerlauf zählt als 0, wie HWiNFO). Wie die Leistung brauchen sie eine vorherige Baseline. Prozessoren unter 1 % Wachzeit liefern keinen Takt; ein MPERF schneller als der dekodierte P0 verwirft die Messung. Die CCD-Temperaturen kommen aus `0x59800 + 0x308 + 4 × CCD` (Family 19h Modell 60h…7Fh, Gültigkeitsbit 11, 0,125 °C ab −49 °C, wie k10temp); die CCD-Anzahl liefert die Windows-L3-Topologie. Ein Fehler bei Takt oder CCD lässt diese Felder leer und erscheint als Warnung; Paketleistung und Tctl bleiben davon unberührt. Werte pro Kern gibt es über diese Register nicht. Nur der Dashboard-Reader und `cpu-hardware-read` lesen Takt und CCDs; der Lüfterdienst bleibt bei Tctl, damit seine 2-Sekunden-Schleife nicht alle Kerne weckt.

Im Tray beziehungsweise bei Suspend wird der Leseprozess pausiert: Treiber-/Modulsitzung und Energie-Baseline werden freigegeben, beide Prozesse warten ohne periodische Sensorabfragen. Nach erneutem Öffnen, Resume, Refresh oder Versorgungswechsel ist eine neue Baseline erforderlich. Bei Fehlern werden alte Werte verworfen. Die Sensorfreigabe gilt für die laufende App-Sitzung, nicht als automatische Hardwarefreigabe nach Neustart.

Der Release mit dunkler Oberfläche wurde nach Aktivierung und zwei Tray-Rückkehrzyklen 30 Sekunden mit **49,39 MB kombiniertem Working Set** gemessen, ohne Working-Set-Trimming; beide Prozesse verbrauchten im Leerlauf-Messfenster keine zusätzliche CPU-Zeit. Der direkte Tray-Start ohne Sensoraktivierung lag bei **14,29 MiB**. Die offene Phase lag mit Reader bei **52,32 MB** und ist vom Benutzer ausdrücklich außerhalb des Tray-Budgets erlaubt. Das sind unterschiedliche Betriebszustände und kurze Messungen auf dem Referenzgerät; längere Dauerläufe stehen aus. Vollständige Zahlen und Binary-Identität stehen in [VALIDATION.md](VALIDATION.md).

## Quellen, feste Verträge und Identität

Der Reader folgt dem öffentlichen [PawnIO-Geräteprotokoll](https://github.com/namazso/PawnIO/blob/9d52965895588b0ffa3703b72eec75ba19f4ccc0/PawnIO/include/pawnio_um.h), lädt ausschließlich das gehashte signierte AMD-Modul und ruft darin ausschließlich `ioctl_read_msr` für **0xC0010299 / 0xC001029B / 0xC0010064 / 0xC00000E7 / 0xC00000E8** sowie `ioctl_read_smn` für **0x59800** und die CCD-Register **0x59B08…0x59B24** auf. Alle sind in der Whitelist des gepinnten Moduls 0.2.11 enthalten. Der SMN-Leseweg benötigt die Auswahl des Adressfensters; dies ist keine Änderung eines Leistungs-/Spannungsparameters. Er wird über `Global\Access_PCI` serialisiert.

Energieeinheiten und der niedrige 32-Bit-Zähler entsprechen den in [Linux RAPL](https://github.com/torvalds/linux/blob/master/arch/x86/events/rapl.c) dokumentierten AMD-MSRs. Temperaturregister und Bereichskorrektur sind in [k10temp](https://github.com/torvalds/linux/blob/master/drivers/hwmon/k10temp.c) beschrieben. Die Umrechnung im Core ist eigener Code. Paketenergie, Monotonzeit, Zählerüberlauf, geänderte Einheiten und veraltete Intervalle werden getrennt geprüft. Der Adapter bietet keinen MSR-, SMU-, Power- oder Voltage-Setter.

| Bestandteil | Qualifizierte Identität |
| --- | --- |
| PawnIO.sys | 2.2.0, SHA-256 `FCA6E7D58B0CF38DBB913A2B9E532F48629145D395F454B16A9F58E97B8D3940` |
| AMDFamily17.bin | Signiertes Release 0.2.11, 10652 Bytes, SHA-256 `DAE74615761B78BDF064DFB3E136252DDCC6FC727D88F14738D0E5800D427A91` |
| Modulsourcen | Tag 0.2.11, Commit `52a7e536dff3e53c96917a28caac5e0fa6510696` |
| CPU | CPUID-Signatur 10882834, Family19h/Model61h/Stepping2, Package1 |

Andere Board-/BIOS-/CPU-/Treiberkombinationen benötigen eine eigene Prüfung. Fehlender Treiber, fehlende Rechte, unpassende Version/Hashes und ungültige Antworten liefern einen sichtbaren Fehler statt Ersatzwerten.

## Bauen und Verteilen

`scripts/prepare-cpu-module.ps1` lädt das feste signierte Modularchiv samt LGPL-Quellen und Lizenz ausschließlich zur Buildvorbereitung nach ignorierter `.tools/pawnio/`. Es hat keinen Treiber- oder Hardwarezugriff. Danach wird das Modul beim Cargo-Build nach Größen-/SHA-Prüfung in die native Binary eingebettet. Die Runtime lädt keine Dateien aus `.tools/` und hat keine Netzwerkabfragen. Ein Build ohne vorbereitetes Modul bleibt für CPU-Hardwarewerte ausdrücklich nicht verfügbar; andere Funktionen bleiben nutzbar.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-cpu-module.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Der Release-Build legt die Modulquelle, das vollständige Quellarchiv und `COPYING` unter `dist/third-party/PawnIO.Modules/` ab. Diese LGPL-2.1-or-later-Komponente bleibt eigenständig lizenziert; eigener Rust-Code bleibt MIT. Für die Weitergabe gehören diese Hinweise und Quellen zur Binary. Die Modulvorbereitung muss nicht bei jedem Build wiederholt werden. Entwickler können die Integration und die Build-Prüfung aus dem veröffentlichten Rust-Quelltext ändern und neu bauen; der Treiber prüft seine Modulsignaturen unabhängig davon. Siehe [THIRD_PARTY.md](../THIRD_PARTY.md).

Eine separate erhöhte Diagnose ist `Omen Hub++.exe cpu-hardware-read`. Sie liest eine Baseline und nach mindestens 1100 ms einen zweiten Stand, mit Quelle, Messintervall und Tctl-Kennzeichnung. Sie ruft keine Setter auf. Messungen und aktuelle Grenzen stehen in [VALIDATION.md](VALIDATION.md).

Die Oberfläche unterscheidet fehlenden Build-Support, unbestätigtes Gerät/BIOS, fehlenden oder unpassenden Treiber, abgebrochene Windows-Freigabe und eine unterbrochene Messung mit deutschen Hinweisen. Dauerhafte Supportprobleme erhalten keine pauschale Wiederholungsempfehlung. Die genaue technische Ursache bleibt im Snapshot-Fehlerzustand und wird beim Fehler separat als `last-cpu-error.json` im Benutzerprofilspeicher gespeichert. `cpu-sensor-status` gibt diesen letzten Fehler samt Zeitpunkt und Benutzerhinweis aus; dies ist keine Aussage über eine inzwischen erfolgreich neu gestartete Sitzung. Die Diagnoseablage verändert weder das Hardwarejournal noch eine Startup-Freigabe.
