# Hardwarematrix

Stand 16. Oktober 2026. Die Matrix fasst die bestehenden Nachweise zusammen; sie erweitert keine Freigabe. Details, Hashes und Messzeiten stehen in [models/8e35.md](models/8e35.md), [VALIDATION.md](VALIDATION.md) und den verlinkten Einzeldokumenten.

## Qualifizierte Geräte

| Gerät | Board / BIOS | CPU | GPU / Treiber / VBIOS | Status |
| --- | --- | --- | --- | --- |
| HP OMEN 16-ap0xxx | 8E35 / F.13 | Ryzen 9 8940HX | RTX 5070 Laptop / 617.14 / 98.06.2a.40.da | Referenzgerät, einzige qualifizierte Kombination |
| Alle anderen | – | – | – | Nur lesende Windows-/NVIDIA-Werte; Schreibaktionen und HP-GETs werden blockiert |

Ein anderes BIOS, ein anderer NVIDIA-Treiber oder ein anderes VBIOS verlangt eine neue Qualifikation, auch auf Board 8E35.

## Funktionen auf 8E35 / F.13

Legende: **Lesen** = Messwert oder Zustand wird angezeigt; **Schreiben** = Setter mit vollständigem Readback und Original-Restore am Gerät geprüft; **Startup** = darf beim Windows-Anmeldestart automatisch angewendet werden.

| Funktion | Lesen | Schreiben | Startup | Weg / Bedingung |
| --- | --- | --- | --- | --- |
| Inventar, Netzteil, Akku | Ja | – | – | Windows-WMI, Power-API |
| GPU-Watt, Temperatur, Takt | Ja | – | – | Öffentliche NVML/NVAPI im kurzlebigen Leseprozess |
| CPU-Auslastung, Windows-MHz | Ja | – | – | Windows; MHz ist kein gemessener Boost |
| CPU-Paketwatt, Tctl | Ja, nach UAC | – | – | Bereits laufendes PawnIO 2.2.0 und eingebettetes AMDFamily17 0.2.11 |
| NVIDIA-Core-/VRAM-Offset | Ja | Ja | Ja | Exakter Treiber/VBIOS, Netzbetrieb; Core-Offset gesperrt bei aktiver V/F-Kurve |
| NVIDIA V/F-Kurve (127 Anker) | Ja | Ja | Ja | Exakter Treiber-SHA-256, Netzbetrieb; Spannungsanker unveränderlich |
| Max-Fan-Override | Ja, erhöht | Ja | Nein | Netzbetrieb, exakte Lüftertabelle/Topologie |
| Lüfterdrehzahlen, IR-Sensor | Ja, erhöht | – | – | Feste BIOS-WMI-GETs, `cooling-read` |
| Tastatur, vier Zonen + Nummernblock | Ja, erhöht | Ja (RGB, statisch ein/aus) | Nein | Capability `[1,0,0,0]`; Ein/Aus vom Benutzer sichtbar bestätigt |
| HP-GPU-Status, SystemDesignData | Ja, erhöht | – | – | D-State 0 bleibt ausdrücklich unbekannt |
| Dynamic Boost / Unleashed | – | Nein | Nein | Wird gespeichert, blockiert aber Apply und Startup |
| HP-Leistungsmodi Quiet/Balanced/Performance | Vorschau | Nein | Nein | Kein qualifizierter HP-Adapter |
| Eigene Lüfterkurven, BIOS-Automatik | Entwurf | Nein | Nein | Modus-Readback/Restore und Sicherheitsweg fehlen |
| CPU-Leistungsgrenzen, Curve Optimizer, Undervolting | – | Nein | Nein | Kein CPU-SMU-Adapter |
| Windows-Energie-/Prozessorrichtlinien | – | Nein | Nein | Felder speicherbar, Adapter fehlt |
| Direkte GPU-/VRAM-Spannung, Power-Limit | – | Nein | Nein | Kein belegter Steuervertrag |
| MUX, Helligkeit, Animationen, Hotkeys | – | Nein | Nein | Nicht analysiert bzw. nicht qualifiziert |

Jede nicht unterstützte Einstellung blockiert das gesamte Apply eines Presets; es wird nie ein Teil eines Profils still übersprungen.

## Offene Lebenszyklusnachweise

Für keine Schreibfunktion sind echter Neustart, Standby/Resume, Netzteilwechsel unter Last oder Betrieb ohne die weiterhin laufenden HP-Controller gemessen. Die Wiederherstellung beschreibt [RECOVERY.md](RECOVERY.md).
