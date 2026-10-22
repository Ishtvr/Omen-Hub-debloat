# GPU-Steuerung und gespeicherte Profile

Stand: **22. Oktober 2026**. Die Oberfläche bietet getrennte GPU-, CPU- und Windows-Bereiche unter **Leistungsprofile → Custom Presets**. Die GPU-Leistungskurve öffnet direkt einen nativen Editor, auch aus einem neuen oder geänderten Preset. GPU-Watt und Temperatur stehen im Dashboard neben den gemessenen CPU-Werten. Der aktuelle geräteübergreifende Steuerungsnachweis steht in [SYSTEM_CONTROLS.md](SYSTEM_CONTROLS.md), konkrete Build-/Hardwareergebnisse in [VALIDATION.md](VALIDATION.md).

## GPU-Verträge

Schreibfreigabe: **HP 8E35 / BIOS F.13, RTX 5070 Laptop, VBIOS 98.06.2a.40.da**, getrennt qualifizierte Treiber **617.14 / 617.42**. Die GPU-UUID bleibt Teil der Profilbindung. Andere Hardware-/Treiberkombinationen erben keine Freigabe.

- **Core-/VRAM-Offsets:** öffentliche NVML-Clock-Offset-ABI, exakter Readback und vollständiges Original einschließlich gekoppelter Graphics-Kurve. Kleine negative/positive Proben bestanden; nicht darstellbare Werte werden abgewiesen. Treiberspannen sind keine Laststabilitätsgarantie.
- **V/F:** eigener typisierter privater Adapter, an den SHA-256 der tatsächlich geladenen NVIDIA-Implementierung gebunden. Alle 127 Spannungsanker und Deltas werden erfasst. Nur Frequenz-Deltas ändern sich; Spannungsanker bleiben unverändert. Private Power-/Voltage-Felder werden nicht geschrieben. Details: [NVIDIA_61742.md](NVIDIA_61742.md).
- **Kopplung:** Ein VRAM-SET kann die Graphics-Kurve zurücksetzen. Der Adapter erfasst und erhält deshalb den vollständigen Graphics-Zustand; Memory wird vor dem endgültigen Core-/V/F-Ziel geschrieben und die Gesamtheit anschließend erneut geprüft.
- **Core versus V/F:** Eine nicht uniforme aktive Kurve blockiert den globalen Core-Offset. Im Profil dürfen beide Vorgaben nicht dieselbe Frequenzdomäne widersprüchlich adressieren. Der Chart bleibt unabhängig davon bearbeitbar.
- **Dynamic Boost:** HP-PPAB-Flag mit Readback und Restore; cTGP wird erhalten. Beide gespeicherten Namen `gpu_unleashed` und `gpu_dynamic_boost` werden als eine logische Vorgabe angezeigt/bearbeitet. Unterschiedliche Ziele blockieren Apply. Für das beobachtete Limit von 115 W ist zusätzlich der passende HP-Leistungszustand erforderlich.
- **HP-Modi:** Quiet, Balanced, Performance und Unleashed verwenden die statisch belegten Hersteller-Payloads. GPU-Flags und Slow/PL1 werden zurückgelesen; ein aktueller Thermalmodus-GET fehlt. Die sichere Modusprobe endete im vom Benutzer ausdrücklich festgelegten Unleashed. Ein gemeldetes 115-W-Limit ist kein Lastbenchmark.

Direkte GPU-/VRAM-Spannung, ein hartes Spannungslimit, FPS-Limiter, Power-/Temperaturlimit und Clock-Lock sind nicht freigegeben und aus der normalen GPU-Bedienung entfernt. V/F-Frequenzsteuerung kann für ein Undervolting-Ziel genutzt werden, ist aber kein direkter mV-Offset und keine Stabilitätszusage. Historische Diagnose- und proprietäre Quelldaten bleiben in ignorierten Ablagen.

## Entwurf, Apply und Wiederherstellung

**Speichern** eines normalen Presets verändert Metadaten. **Anwenden** startet den festen erhöhten Transaktionsprozess. Vor dem ersten SET werden alle vollständigen Originale gesichert; Identität, Rechte, Grenzen und eigene Zielzustände werden vor dem Dispatch erneut geprüft. Fehler schließen kein unvollständiges Recovery. Das Journal liegt im eigenen Benutzerordner `.omen-hub-pp`, getrennt von MSIX-AppData.

Die strukturelle Dateiprüfung lässt ältere widersprüchliche Presets zum Korrigieren lesbar. Der zusätzliche Hardware-Preflight blockiert unzulässige Kombinationen und nicht unterstützte Einstellungen vor jedem SET. Öffnen bereinigt keine Dateien; ausdrücklich Speichern entfernt die ausgemusterten GPU-/APU-/CPU-Schlüssel. Konfliktprüfender Merge erhält vollständige Kurven und fremde gleichzeitig bearbeitete Felder.

**Übernehmen** im GPU-Chart gibt die vollständige Kurve an den ungespeicherten Preset-Entwurf zurück. **Abbrechen** verwirft nur diesen Chart-Entwurf. Name und andere Änderungen bleiben erhalten. Erst Speichern im Elternfenster schreibt die Datei. Die eigene Lüfterkurve ist davon getrennt: ihr Speichern hinterlegt die aktive gemeinsame Kurvendatei für den Lüfterdienst; genutzt wird sie im Modus Kurve.

Eine ausdrücklich gewählte Startoption kann beim Speichern zunächst Apply und danach Freigabe anfordern. Allein Speichern ohne diese Auswahl autorisiert keinen Windows-Start. Die geschützte Anmeldung verlangt den passenden Build und die exakt bestätigte Profilrevision. Der reale Aufgabenpfad ist geprüft; ein echter Neustart-/Power-Lifecycle bleibt offen.

## Native CLI

`gpu-ui` bleibt Alias für `presets-ui`. `gpu-curve-ui`, `fan-ui` und der ältere gemeinsame `curve-ui`-Einstieg verwenden den nativen Kurveneditor. Normale Kontrollbeispiele:

```powershell
& '.\dist\Omen Hub++.exe' profile-plan mein-profil
& '.\dist\Omen Hub++.exe' profile-apply mein-profil
& '.\dist\Omen Hub++.exe' profile-restore
& '.\dist\Omen Hub++.exe' profile-startup-enable mein-profil
```

Die eigenen geschützten Aufgaben werden durch die festen Installer eingerichtet. Sie installieren keinen Kernel-Treiber. Wiederherstellung, Deinstallation und Grenzen zu BIOS-Ownership: [RECOVERY.md](RECOVERY.md), [HARDWARE_MATRIX.md](HARDWARE_MATRIX.md). Öffentliche APIs: [NVML Clock Offset](https://docs.nvidia.com/deploy/nvml-api/latest/api/structnvmlClockOffset__v1__t.html), [NVML-Geräteabfragen](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html).
