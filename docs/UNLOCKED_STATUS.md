# Erweiterter Funktionsumfang — Entwicklungsstand

Stand: **26. Oktober 2026, `0.9.0-dev.2`**. Der Projektinhaber hat die Veröffentlichung der ersten Beta zurückziehen lassen und ausdrücklich die echte Umsetzung der fehlenden Hardwarefunktionen verlangt. Die README dokumentiert Entwicklung, Eigenverantwortung und den Haftungs-/Gewährleistungshinweis der MIT-Lizenz. Ein vollständig funktionsfähiger neuer „Unlocked“-Release ist noch nicht fertig und wurde nicht veröffentlicht.

## Aktuelle Umsetzung und tatsächliche Grenzen

| Funktion | Stand |
| --- | --- |
| Öffentliches NVIDIA-Wattlimit | Neuer Rust-Adapter mit Treibergrenzen, Original-Capture, einmaligem SET, genauem Readback, dauerhaftem Journal und Rücksetzung implementiert. Presetfeld und Metadatenweg geprüft. Auf der vorhandenen RTX 5070 Laptop ist der konfigurierte Originalwert über diesen öffentlichen Pfad nicht lesbar; daher keine reale Schreibprobe. |
| Direkte TGP-/Spannungs-/Clock-Lock-Erweiterungen | Keine vollständigen neuen Steuer-/Originalverträge vorhanden. Ein frei gewählter Wert oder ein sichtbares Eingabefeld ersetzt diese nicht. Dynamic Boost und die bisher qualifizierten Takt-/V/F-Pfade bleiben davon getrennt. |
| Intel PL1/PL2/PL4 und weitere Intel-Optionen | Noch kein integrierter Intel-Hardwareadapter. Die öffentlichen MSR- und vorhandenen signierten Modulverträge sind Referenzen; sie erlauben nicht pauschal jeden Registerzugriff. Der verfügbare Rechner hat einen AMD Ryzen 9 8940HX. Der Benutzer hat bestätigt, dass kein Intel-Testgerät zur Verfügung steht. |
| Dauerhafter eigenständiger Betrieb ohne HPOmenCap | Bisheriger nativer Dienst-/Kühlungs-Prüfablauf ist begrenzt. Dauerhafter App-/Anmeldestart mit vollständiger Zuständigkeitsübernahme und echte gemeinsame Geräteabnahme fehlen weiterhin. |
| CPU-PPT/Slow bei laufendem HPOmenCap | Konfliktsperre weiter aktiv; ihr Ersatz braucht eine tatsächlich funktionierende eigene Zuständigkeitsübernahme. Neue CPU-Power-Startup-Freigaben bleiben bei weiterhin vorhandenem OEM-Dienst eingeschränkt. |
| Lüfterkurven als Teil einer Gesamttransaktion | Eigener vorhandener Hintergrundhelfer und separate Kurvensteuerung sind nutzbar. Vollständiger Profiladapter sowie BIOS-Ownership-Rückgabe sind noch nicht fertig qualifiziert/integriert. |
| iGPU-only/MUX, Reboot und Standby | Startvorgaben und Power-Ereignislogik sind vorhanden. Tatsächliche Hardwarewechsel und vollständige Wiederanwendung nach realen Übergängen bleiben offen. |
| Beleuchtungs-/Hotkey-Lifecycle und zusätzliche Effekte | Vorhandene Vier-Zonen-Farben und statisch Ein/Aus sind getrennt qualifiziert. Die übrigen Fähigkeiten benötigen eigene Modell-/Transport- und Lifecycle-Verträge. |

## GPU-Befund des vorhandenen Geräts

Auf der RTX 5070 Laptop/617.42 liefern die öffentlichen NVML-Abfragen 5…115 W als erlaubten Bereich und 80 W als Default. Das effektiv erzwungene Limit ist separat als 115 W lesbar; das ist keine gemessene Leistungsaufnahme. Sowohl der ursprüngliche konfigurierte Limit-GET als auch das Feld für einen angeforderten Grenzwert melden **nicht unterstützt**. Der Default und der effektive Wert wurden deshalb nicht als vermeintliches Original in das neue Rücksetzungsjournal übernommen. Es gab keinen Leistungs-/Spannungs-SET. Details und öffentliche Schnittstellen: [GPU_PROFILES.md](GPU_PROFILES.md).

## Bereits geprüfter Entwicklungsstand

233 Rust-Tests, 14 Prozess-/Dateiszenarien, Formatprüfung, Clippy und Release-Kompilierung bestanden. Native Metadatenprüfungen decken die neue Wattangabe, exakte Milliwatt-Speicherung, Abwählen und den bestehenden Preset→Chart-Pfad ab. Benötigte reale Intel-, Dienst-/Lüfter-, Power- und Langzeitprüfungen werden damit nicht behauptet. Der vollständige Prüfstand steht in [VALIDATION.md](VALIDATION.md).
