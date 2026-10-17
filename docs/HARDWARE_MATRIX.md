# Hardwarematrix

Stand 16. Oktober 2026, nach Übernahme der HP-Modi, des SMU-Adapters und des Lüfterdienstes. Die Matrix fasst Code und vorhandene Nachweise zusammen; sie erweitert keine Freigabe. Details, Hashes und Messzeiten stehen in [models/8e35.md](models/8e35.md), [models/8e35-hub-modes.md](models/8e35-hub-modes.md), [FAN_CONTROL.md](FAN_CONTROL.md) und [VALIDATION.md](VALIDATION.md).

## Qualifizierte Geräte

| Gerät | Board / BIOS | CPU | GPU / Treiber / VBIOS | Status |
| --- | --- | --- | --- | --- |
| HP OMEN 16-ap0xxx | 8E35 / F.13 | Ryzen 9 8940HX | RTX 5070 Laptop / 617.14 / 98.06.2a.40.da | Referenzgerät, einzige qualifizierte Kombination |
| Alle anderen | – | – | – | Nur lesende Windows-/NVIDIA-Werte; HP-, SMU- und GPU-Schreibaktionen werden blockiert |

Ein anderes BIOS, ein anderer NVIDIA-Treiber oder ein anderes VBIOS verlangt eine neue Qualifikation, auch auf Board 8E35.

## Funktionen auf 8E35 / F.13

Legende: **Lesen** = Messwert oder Zustand wird angezeigt. **Schreiben** = Setter mit Readback und Restore implementiert; **gemessen** heißt, er lief mindestens einmal am Referenzgerät mit Rücklesen, **ungemessen**, dass er nur gebaut und getestet ist. **Startup** = darf beim Windows-Anmeldestart automatisch angewendet werden; seit 16. Oktober gilt das auf Nutzerwunsch für alle unterstützten Preset-Einstellungen, auch im Akkubetrieb.

| Funktion | Lesen | Schreiben | Startup | Weg / Bedingung |
| --- | --- | --- | --- | --- |
| Inventar, Netzteil, Akku | Ja | – | – | Windows-WMI, Power-API |
| GPU-Watt, Temperatur, Takt | Ja | – | – | Öffentliche NVML/NVAPI im kurzlebigen Leseprozess |
| CPU-Auslastung, Windows-MHz | Ja | – | – | Windows; MHz ist kein gemessener Boost |
| CPU-Paketwatt, Tctl | Ja, nach UAC | – | – | Bereits laufendes PawnIO 2.2.0 und eingebettetes AMDFamily17 0.2.11 |
| NVIDIA-Core-/VRAM-Offset | Ja | Ja, gemessen | Ja | Exakter Treiber/VBIOS; Core-Offset gesperrt bei aktiver V/F-Kurve |
| NVIDIA V/F-Kurve (127 Anker) | Ja | Ja, gemessen | Ja | Exakter Treiber-SHA-256; Spannungsanker unveränderlich |
| HP-Modus Balanced / Unleashed | – | Ja, Unleashed einmal gemessen | – | Dashboard-Auswahl; Unleashed nur im Netzbetrieb; Thermalmodus ohne GET, Rückweg ist ein expliziter Balanced-Write |
| HP-Modus Quiet / Performance, PL1 je Modus | STAPM/Slow mit SMU-Modul | Ja, ungemessen | – | Payloads aus der statischen Hub-Analyse |
| Dynamic Boost (PPAB) | Ja, erhöht | Ja | Ja | HP-GPU-Status-Bit mit Readback; cTGP bleibt unverändert |
| CPU PPT/Fast, STAPM/Slow, TDC, EDC, Tctl-Limit | Ja, mit SMU-Modul | Ja, einmal kalibriert | Ja | Eingebettetes RyzenSMU 0.2.11; Preset-Apply unter Last ungetestet |
| Curve Optimizer, PBO-Scalar | Ja, mit SMU-Modul | Ja, ungemessen | Ja | Eigene SMU-GETs; Werte werden vollständig erfasst und restauriert |
| Fester CPU-Takt / feste Spannung, Core-Ratios | – | – | – | Bewusst nicht vorgesehen (meist von der Laptop-Firmware gesperrt); Editorfelder entfernt |
| Windows-Energie-/Prozessorrichtlinien, Bildwiederholrate | Ja | Ja | Ja | Dokumentierte Windows-APIs mit exaktem Rücklesen |
| CCD-Affinität | Ja | Ja, ungemessen | Ja | Lüfterdienst pinnt das Vordergrundprogramm; alte Maske wird zurückgegeben |
| Grafikmodus (MUX) | Ja, erhöht | Ja, ungemessen | Ja | HP-BIOS; Preset oder Dashboard-Schalter Hybrid/GPU only mit Rücklesen; wirkt erst nach Neustart |
| Max-Fan-Override | Ja, erhöht | Ja, gemessen | Ja | Exakte Lüftertabelle/Topologie |
| Eigene Lüfterkurve | Ja, erhöht | Über den Lüfterdienst | Dienst bei Anmeldung | Gemeinsame Kurve, Notfall-Vollkühlung; wartet, solange der Original-Hub läuft |
| Lüfterdrehzahlen, IR-Sensor | Ja, erhöht | – | – | Feste BIOS-WMI-GETs, `cooling-read` |
| Tastatur, vier Zonen + Nummernblock | Ja, erhöht | Ja, gemessen (RGB, statisch ein/aus) | Ja | Capability `[1,0,0,0]`; Ein/Aus vom Benutzer sichtbar bestätigt |
| HP-GPU-Status, SystemDesignData | Ja, erhöht | – | – | D-State 0 bleibt ausdrücklich unbekannt |
| Direkte GPU-/VRAM-Spannung, GPU-Power-Limit, Clock-Lock | – | Nein | Nein | Kein belegter Steuervertrag |
| AMD-Boostprofil, All-Core-Spannungsoffset | – | – | – | Gibt es für Dragon Range nicht; Felder entfernt |
| Helligkeit, Animationen, Hotkeys | – | Nein | Nein | Nicht qualifiziert |

Jede nicht unterstützte Einstellung blockiert das gesamte Apply eines Presets; es wird nie ein Teil eines Profils still übersprungen. Ohne eingebettetes RyzenSMU-Modul sind alle SMU-Zeilen nicht verfügbar.

## Offene Lebenszyklusnachweise

Für keine Schreibfunktion sind echter Neustart, Standby/Resume, Netzteilwechsel unter Last oder Betrieb ohne die weiterlaufenden HP-Controller vollständig gemessen. Läuft der Original-Hub weiter, kann er Modus und Lüfter bei eigenen Ereignissen erneut setzen. Die Wiederherstellung beschreibt [RECOVERY.md](RECOVERY.md).
