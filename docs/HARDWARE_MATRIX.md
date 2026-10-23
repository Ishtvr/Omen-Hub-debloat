# Hardwarematrix

Stand: **23. Oktober 2026**. Diese Matrix unterscheidet Implementierung, kurze reale Funktionsprobe und offene Lebenszyklusnachweise. Die vollständigen aktuellen Interface-Beobachtungen stehen in [SYSTEM_CONTROLS.md](SYSTEM_CONTROLS.md), der neue Lüfterbefund in [FAN_HANDOVER.md](FAN_HANDOVER.md), buildbezogene Ergebnisse in [VALIDATION.md](VALIDATION.md).

## Qualifizierte Kombination

**HP OMEN 16-ap0xxx, Board 8E35, BIOS F.13, Ryzen 9 8940HX, SMU 0x546F00, PM-Tabelle 0x540208, RTX 5070 Laptop, VBIOS 98.06.2a.40.da.** NVIDIA-Schreibpfade sind getrennt für **617.14** und **617.42** qualifiziert und zusätzlich an die genaue geladene Implementierung gebunden. Profile binden die GPU-UUID. Andere Boards, BIOS-, SMU-, Tabellen-, Treiber- und VBIOS-Versionen erhalten diese Freigabe nicht automatisch.

## Funktionsumfang

| Funktion | Reale Probe / Zustand | Anmeldung und Grenzen |
| --- | --- | --- |
| GPU-Watt, Temperatur, Takt | Öffentliche NVML/NVAPI, isolierter Leseprozess | Treiberabhängige Verfügbarkeit |
| CPU-Auslastung / Windows-MHz | Windows-APIs | Windows-MHz ist kein gemessener Boost |
| CPU-Paketwatt / Tctl / Effektivtakte | Gehashter vorhandener PawnIO-Reader; 32 logische Prozessoren | Expliziter UAC-Leseprozess, frische Basis nach Pause; keine Installation eines Treibers |
| GPU-Core / VRAM | −15/+15 bzw. −24/+24 MHz, voller Readback/Restore | Autorisierte Profilrevision; Core blockiert bei aktiver individueller V/F-Kurve |
| GPU-V/F | Alle 127 Anker/Deltas und globale Offsets geprüft | Nur Frequenzwerte; kein direktes mV-Limit oder Laststabilitätsnachweis |
| Quiet / Balanced / Performance / Unleashed | Feste Hersteller-SETs, GPU-Flags und Slow/PL1-Readback | Thermalmodus ohne GET; Modustests enden im ausdrücklich vorgegebenen Unleashed |
| Dynamic Boost | PPAB Ein/Aus/Restore, cTGP erhalten | Autorisierte Profile; für 115 W muss auch der passende HP-Leistungszustand bestehen |
| CPU Fast / Slow / TDC / EDC / Tctl | Feste RSMU-Befehle, kleine Senkung, vollständige Originale zurückgelesen | Exakte SMU-/Tabellenversion; FFI-Transport und alle unabhängigen CPU-Steuerbilder geprüft |
| CO gemeinsam / einzeln | −20 → −19 → −20; vollständige 16-Kern-Bilder | Schritte, keine mV; beide Arten dürfen nicht gemeinsam im Profil stehen |
| PBO-Scalar | 1× GET/SET/Readback/Restore | Höhere Werte sind kein Bestandteil der sicheren Hardwareprobe |
| Einzelnes STAPM / feste CPU-Spannung / fester CPU-Takt | Entfernt bzw. gesperrt | Kein ausgedachter Steuervertrag; alte HP-Limitbilder wählen keinen neuen RSMU-Transport |
| Windows-Boost, Min/Max, EPP, Core Parking | AC und DC sowie Originale des Zielplans geprüft | Aktiver Plan wird vor Richtlinien gesetzt; Teilfehler bleiben wiederherstellbar |
| Windows-Energiemodus | Konfigurierter und effektiver Modus getrennt | Plan Ausbalanciert im selben Preset wählen; unpassende Kombination blockiert vor SET |
| Energiesparplan / Bildwiederholrate | Native SET/GET/Restore-Proben | Unterstützte Windows-/Display-Werte; kein physischer AC-/Monitorwechsel-Nachweis |
| CCD-Auswahl | Gespeicherte Auswahl und echter eigener Testprozess, beide CCDs und Release | Hintergrunddienst; andere Benutzerprogramme wurden in der Probe nicht umgeordnet |
| MUX | Hybrid/Discrete-Startvorgabe gesetzt, zurückgelesen und restauriert | Wirkt nach Neustart; physischer Wechsel und iGPU-only nicht damit qualifiziert |
| Max-Fan-Override | Vollständiger Schalter, Tabelle/Zuordnung, Readback/Restore | Profilfreigabe ist kein Lifecycle-Nachweis |
| Eigene Lüfterkurve / Max im Dienst | Konservative reale Probe bis 4800/4800 RPM | Warten auf Original-Hub, CPU/GPU/IR-Schutz; Pause/Stop fordert nach eigenen Befehlen einmal volle Kühlung an. `handover_pending` bleibt explizit; direkter BIOS-Auto-Schalter nicht freigegeben |
| Vier-Zonen-RGB / statisch Ein/Aus | Vollständige Capability-, Farb- und Modusbilder; sichtbare Wirkung früher vom Benutzer bestätigt | Erhöhter Capture vor jedem Write; keine freigegebenen Animationen, Prozenthelligkeit oder Hotkey-Lifecycle |
| Direkte GPU-/VRAM-Spannungen, FPS-Limiter, Clock-Lock | Entfernt / nicht unterstützt | Kein zulässiges partielles Apply alter Profile |
| Profil-Anmeldung | Geschützte Kopie, feste Aufgabe, exakt autorisierte Revision und dauerhaftes Journal | Echte Neustart-/Resume-/AC-Probe bleibt erforderlich; Speichern alleine autorisiert keinen Autostart |

## Was weiterhin offen ist

Ein kurzer erfolgreicher Setter-/Restore-Test bestätigt weder Langzeitstabilität noch einen echten Neustart, Standby/Resume oder einen Netzteilwechsel unter Last. Originale HP-Windows-Dienste liefen während dieser Proben weiter. Ihr vollständiger Ersatz, sichere Lüfter-Ownership und die automatische Wiederanwendung nach realen Power-Übergängen sind weiterhin getrennt zu qualifizieren. Die aktuelle Anwendung darf deshalb noch nicht als vollständig geprüfter Ersatz aller HP-Hardwarecontroller bezeichnet werden.

Das Tray-Budget beträgt **50 MB einschließlich aller eigenen residenten Helfer**. Ein geöffnetes Fenster darf mehr benötigen. Aktuelle Messung und genaue Prozessabgrenzung: [VALIDATION.md](VALIDATION.md). Wiederherstellung und Installationshinweise: [RECOVERY.md](RECOVERY.md).
