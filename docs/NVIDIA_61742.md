# NVIDIA 617.42 — Referenzgerät 8E35/F.13

Am 22. Oktober 2026 wurden auf der RTX 5070 Laptop, VBIOS `98.06.2a.40.da`, die vorhandenen GPU-Adapter erneut qualifiziert. Die Runtime akzeptiert getrennte feste Evidenzeinträge für 617.14 und 617.42; weitere Treiberversionen bleiben gesperrt. Ein lesbarer Vertrag allein erteilt keine Schreibfreigabe.

## Feste Implementierung und statischer Abgleich

Die tatsächlich installierte und von den Funktionsadressen ermittelte `nvapi64_impl.dll` ist 5.688.768 Bytes groß, SHA-256 **C6A2014763C3662512B5D420DFC7632B777338881A13AEAB3F21D8033EAE6C71**. Der System32-Loader hat SHA-256 `912AC885975DA8D8DF80AA0D1C04704E7DD2CA42FA2A02D341AE70237890E9BB`. Die Runtime prüft die geladene Implementierung für alle vier Getter und vor jedem privaten SET gegen den zum gemeldeten Treiber gehörenden Hash.

Ghidra/PyGhidra analysierte die neue Implementierung ohne PDB-Download. Die fünf nachstehenden Funktionsbereiche und ihre alternativen RID-Einträge sind bytegleich zu 617.14; die gesamten `.text`-/`.rdata`-Abschnitte sind es nicht. Die Interface-Tabelle wurde zusätzlich über die PE-Daten und den dekompilierten QueryInterface-Pfad abgeglichen.

| Funktion | Interface-ID | Adresse im analysierten Image | v1-Vertrag |
| --- | --- | --- | --- |
| Punktmaske GET | `0x507B4B59` | `0x180206CC0` | 0x182C Bytes, 255 × 24 Bytes |
| Frequenz/Spannungsanker GET | `0x21537AD4` | `0x1802075A0` | 0x1C28 Bytes, 255 × 28 Bytes |
| Steuerdeltas GET | `0x23F1B133` | `0x1802062F0` | 0x2420 Bytes, 255 × 36 Bytes |
| Domänengrenzen GET | `0x64B43A6A` | `0x180205C90` | 0x928 Bytes, 32 × 72 Bytes |
| Frequenzdeltas SET | `0x0733E009` | `0x180208220` | Steuerblock, nur ausgewählte Graphics-Frequenzwörter |

Punktblöcke beginnen nach 68 Bytes. Der Frequenzdelta liegt bei `0x58 + Index × 36`. Die native Probe las 127 Graphics-Punkte mit unveränderten Ankern 450…1240 mV. Ein privater Buffer oder ein Opcode kann nicht vom Benutzer vorgegeben werden. Proprietäre Bilder, Decompilierungen und Rohdaten bleiben in ignorierter `.local/research/nvidia-61742/` bzw. dem dort referenzierten Ghidra-Projekt.

## Gemessene Schreib-/Restore-Proben

Der Benutzer erlaubte das vorübergehende Beenden von MSI Afterburner. Seine vorhandene individuelle Kurve mit 126 von null abweichenden Deltas wurde vollständig bewahrt. Die Proben verwenden das dauerhafte Transaktionsjournal und vergleichen vor jeder neuen Operation die ursprünglichen Steuerbilder. Der globale Offsettest darf innerhalb dieses festen Prüfpfads vorübergehend die Kurve ersetzen; normale Presets behalten ihre Sperre gegen globale Core-Offsets bei aktiver individueller Kurve.

| Probe | Ziel | Ergebnis |
| --- | --- | --- |
| Vollständige V/F-Kurve | Alle Frequenzen 15 MHz unter dem gelesenen Zustand | Alle 127 Steuerdeltas geprüft und Original vollständig restauriert |
| VRAM nach Kurvenänderung | Erster Graphics-Punkt −15 MHz, danach VRAM −24 MHz | Kurvenziel blieb erhalten; anschließender vollständiger Restore |
| Globale Offsets, negativ | GPU −15 MHz, VRAM −24 MHz | Exakter Readback, komplette ursprüngliche Kurve und 0/0 restauriert |
| Globale Offsets, positiv | GPU +15 MHz, VRAM +24 MHz | Exakter Readback, komplette ursprüngliche Kurve und 0/0 restauriert |

Die freigegebene Binary **FB924A420FBF0211A2A24608C36579E5E685F08D69F4468087F2A9C074F2F80F**, **2.657.792 Bytes**, wiederholte alle drei CLI-Prüfpfade um **03:20:45…03:20:48 CEST** erfolgreich. Unabhängige Leseprozesse bestätigten nach jedem Pfad sämtliche Anker/Deltas und globalen Offsets. Vorhandenes Produktionsjournal, Startup-, Lüfter- und UI-Metadaten blieben hashgleich; nur das vollständig restaurierte eigene Prüfjournal darf zum zuvor archivierten Checkpoint zurückkehren. Ein unvollständiger Restore bleibt offen und blockiert weitere Schreibaktionen.

Afterburner wurde anschließend mit seinem ursprünglichen `/s`-Aufruf und Rechten wieder gestartet. Um **03:21:20 CEST** waren die ursprüngliche Kurve und 0/0-Offsets auch danach unverändert. Der echte Preset→Chart-Smoke bestand um **03:21:24 CEST**: neuer/ungespeicherter Entwurf, 127 Punkte, Abbrechen/Übernehmen/Wiederöffnen, Speichern und Erhalt anderer Felder ohne Hardwareänderung. Der Smoke berücksichtigt nun die 24 tatsächlich entfernten APU-Felder und erhält die zwischenzeitlich wieder unterstützten STAPM-/Slow-Felder.

Format, **156 Tests** (91 Core / 45 Platform / 20 App), Clippy mit verweigerten Warnungen und Release-Build bestanden. Neue Tests behandeln exakte Geräte-/Versionsbindung, getrennte Implementierungshashes und feste kleine Kurvenproben auf bereits individuellen Kurven einschließlich falscher Anker, geänderter Beobachtungen und fehlender Delta-Reserve.

Reproduzierbarer, ausdrücklich erhöhter Entwicklercheck:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/check-gpu-driver.ps1 -ExpectedBinarySha256 '<SHA-256 der gebauten target/release/omen-hub-pp.exe>'
```

Der Check beendet keine fremden Programme und startet keine Treiber. Er verweigert die Taktproben bei laufendem Afterburner oder vorhandenen globalen Nichtnull-Offsets. Die alten individuellen Kurven werden gesichert und restauriert. Die öffentlichen Offset-Verträge bleiben [NVML GetClockOffsets](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html) und [SetClockOffsets](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceCommands.html); der private Vertrag ist ausschließlich durch diese konkrete Analyse/Messung begrenzt.

Das qualifiziert die Steuer-/Restore-Pfade. Direkte GPU-/VRAM-Spannung, ein hartes Spannungslimit, Overclock-Stabilität unter Last, echter Neustart/Standby und dauerhaft erreichbare 115 W sind dadurch nicht nachgewiesen. Frühere Profile bleiben an ihre gespeicherte Treiberversion gebunden; es erfolgt keine stille Migration oder neue Startup-Freigabe.
