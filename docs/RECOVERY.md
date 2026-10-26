# Wiederherstellung und Deinstallation

Diese Anleitung beschreibt, was Omen Hub++ auf einem Gerät verändern kann, wie der vor einer Änderung gespeicherte Originalzustand zurückkommt und wie die Anwendung vollständig entfernt wird. Sie gilt für den aktuellen Stand und das qualifizierte Referenzgerät **HP 8E35 / BIOS F.13**; auf anderen Geräten werden Schreibaktionen bereits vor dem ersten SET blockiert.

## Neue und ältere Zustandsbilder

Seit dem 26. Oktober hält die begrenzte native HP-Migrationsprobe zusätzlich `controller-cooling.json` in der geschützten Installation vor. Dienst-Restore und Rückgabe der vorherigen Lüfterauswahl müssen beide bestätigt sein; ein wieder laufender HP-Dienst allein bedeutet keinen vollständigen Abschluss. `hp-control-status` liest beide Journale und den letzten Wächterbericht. `hp-control-recover` bleibt eine ausdrückliche Reparatur für einen beendeten Besitzer und kann nun auch eine noch offene Kühlungsrückgabe nach bereits erfolgtem Dienst-Restore abschließen. Bei fremder Identität oder offenem Hardware-Recovery bleibt die Verantwortung gespeichert. Die Installer blockieren dann Update/Entfernung beider Helfer. Ein laufender Lüfterhelfer wird beim Update nicht mehr gewaltsam beendet. Diese Rückgabe betrifft ausschließlich die abgesicherte Entwicklungsprobe; der geplante eigenständige Betrieb benötigt keinen Original-Hub-Dienst.

Seit 22. Oktober tragen CPU-Limitbilder den eigenen RSMU-Vertrag und die Tabellenversion. Ein altes HP-Limitbild wird nicht als Beleg für den neuen Transport übernommen. Auch ein alter ausschließlich effektiver Windows-Overlay-Wert ist kein vollständiger Originalzustand der konfigurierten Benutzerauswahl. Solche Bilder blockieren die automatische Rücksicherung, statt unbekannte Werte zu schreiben. Das vorhandene Journal bleibt erhalten; die technische Ursache und der aktuelle [Steuerungsnachweis](SYSTEM_CONTROLS.md) helfen beim gezielten Wiederherstellen. Nicht durch Löschen eines offenen Journals eine erfolgreiche Rücksicherung vortäuschen.

Alte widersprüchliche Presets bleiben im Editor lesbar. Das ist keine Schreibfreigabe: erst der eigene Anwendungs-Preflight prüft CO-/Dynamic-Boost-Konflikte und Windows-Modus/Plan. Ein Profil kann daher zum Korrigieren geöffnet werden, während Anwenden weiter blockiert ist.

## Was Omen Hub++ verändert

| Bereich | Wann geschrieben wird | Originalzustand |
| --- | --- | --- |
| Preset-Einstellungen: NVIDIA-Offsets/V/F-Kurve, Max-Fan, Dynamic Boost, CPU-Limits/Curve Optimizer/PBO (SMU), Windows-Richtlinien, Bildwiederholrate, CCD-Affinität, Grafikmodus (MUX) | Nur bei ausdrücklichem **Anwenden** eines gespeicherten Custom Presets (UAC) oder beim freigegebenen Anmeldestart | Vor jedem SET in `control-journal.json` gespeichert; Readback nach dem Schreiben, Rollback bei Teilfehlern |
| HP-Leistungsmodus Quiet/Balanced/Performance/Unleashed samt PL1 | Auswahl im Dashboard (UAC) | Kein Journal: Der Thermalmodus hat keinen GET; zurück geht es durch Auswahl von **Balanced** |
| Lüfterkurve, Lüfterschalter Auto/Kurve/Max, CCD-Affinität im Betrieb | Lüfterdienst (Anmeldeaufgabe `OmenHubPP-Fan-<SID>`) | Ohne gespeicherte Kurve sendet er nichts; Entfernen gibt die Lüfter an das BIOS zurück |
| Tastaturfarben und Ein/Aus | Nur durch Klick im Fenster **Beleuchtung** (UAC) | Eigenes Journal unter `keyboard-control\` |
| Profile und Entwürfe | **Speichern** im Editor | Reine Metadaten, keine Hardwarewirkung |
| Windows-Anmeldestart | Nur nach ausdrücklicher Freigabe eines bereits angewendeten, unterstützten Presets | Geplante Aufgabe `OmenHubPP-Profile-<SID>` |

Omen Hub++ stoppt oder entfernt keine HP-Dienste, installiert keinen Treiber und flasht keine Firmware. Leistungsgrenzen (PL1, STAPM, PPT, TDC/EDC) und der Curve Optimizer werden nur über die oben genannten, ausdrücklich ausgelösten Wege geändert; feste CPU-Spannungen oder direkte GPU-Spannungen werden nie gesetzt. Ein geänderter Grafikmodus wirkt erst nach einem Neustart.

## Speicherorte

- **Benutzerdaten:** `%USERPROFILE%\.omen-hub-pp\` – `profiles\`, `deleted-profiles\` (archivierte Originalbytes), `control-journal.json` (Originalbild des letzten Apply), `control-history\` (Sicherungen früherer abgeschlossener Journale aus Qualifikationsläufen), `startup.json`, `last-*.json`, `keyboard-control\` (Beleuchtungsjournal).
- **Lüfterdienst und CCD-Affinität:** `active-fan-curve.json`, `fan-mode.json` und `ccd-affinity.json` im selben Ordner; der Grafikschalter merkt sich seine letzte Auswahl in `graphics-mode.json`.
- **Installation:** `%ProgramFiles%\OmenHubPlusPlus\` – `Omen Hub++.exe`, `installation.json` (Startup-Beleg), `startup-authorized-<SID>.json` sowie für den Lüfterdienst `Omen Hub++ Fan.exe` und `fan-service-<SID>.json`.
- **Verknüpfungen:** `Omen Hub++.lnk` im Startmenü (Programme) und auf dem Desktop des installierenden Benutzers.

Die Journale sind die einzige Quelle für den gespeicherten Originalzustand. **Löschen Sie `.omen-hub-pp` nicht, bevor Sie wiederhergestellt haben.**

## Originalzustand zurückholen

### Preset-Einstellungen (GPU, CPU, Windows, Max-Fan, MUX)

1. Ist eine Transaktion unvollständig (z. B. Absturz während **Anwenden**), zeigt **Leistungsprofile** statt **Anwenden** die Aktion **Wiederherstellen**. Sie stellt das gespeicherte Original nach UAC wieder her.
2. Unabhängig davon stellt die Befehlszeile den vor dem letzten Apply gespeicherten Zustand wieder her. Die Funktion deaktiviert zuerst eine automatische Anmeldeauswahl:

   ```powershell
   & "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-restore
   ```

   In einer **als Administrator** geöffneten PowerShell ausführen. Ein anderes Board, BIOS oder ein anderer NVIDIA-Treiber blockiert die Wiederherstellung ausdrücklich, statt fremde Werte zu schreiben.
3. Max-Fan lässt sich zusätzlich über ein Preset mit **Max-Fan: Aus** freigeben. **Aus** gibt nur den Override frei und wählt keinen bestimmten BIOS-Lüftermodus.

### HP-Leistungsmodus

Im Dashboard **Balanced (Default)** wählen. Das schreibt den ausgelieferten Balanced-Zustand samt PL1 und liest GPU-Bits sowie, mit SMU-Modul, STAPM/Slow zurück. Ein fehlgeschlagener Moduswechsel fällt selbst auf Balanced zurück.

### Lüfter

Der Lüfterschalter auf der Startseite auf **Auto** stellen: Der Dienst sendet dann nichts mehr und das BIOS regelt mit seiner Standardkurve. Um den Dienst ganz zu entfernen, siehe Deinstallation, Schritt 2.

### Grafikmodus (MUX)

Der Grafikschalter auf der Startseite schreibt **Hybrid** oder **GPU only** nach Windows-Freigabe ins BIOS und liest den Wert zurück; aktiv wird er erst nach einem Neustart. Zurück geht es mit **Hybrid** und einem weiteren Neustart. Die letzte Auswahl steht in `graphics-mode.json`.

### Tastaturbeleuchtung

Das Fenster **Beleuchtung** prüft beim Öffnen sein eigenes Journal und stellt eine offene, eigene Beleuchtungstransaktion wieder her, bevor es neue Werte liest. Gewünschte Farben lassen sich danach direkt wieder einstellen. Ein offenes Performancejournal muss zuerst über die Schritte oben abgeschlossen werden.

### Anmeldestart abschalten

```powershell
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-startup-disable
& "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-startup-status
```

Alternativ das Häkchen **Bei Windows-Start aktivieren** in **Leistungsprofile** entfernen.

## Deinstallation

In einer **als Administrator** geöffneten Windows PowerShell 5.1 im entpackten Paket oder im Repository:

```powershell
# 1. Originalzustand wiederherstellen (siehe oben).
# 2. Lüfterdienst entfernen; die Lüfter regelt danach wieder das BIOS:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-fan-service.ps1 -Mode Remove
# 3. Geplante Anmeldeaufgabe, Startup-Beleg und Freigabe entfernen:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-startup.ps1 -Mode Remove
# 4. Programmdatei und eigene Verknüpfungen entfernen:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-app.ps1 -Mode Remove
```

Im Release-Paket liegen alle drei Skripte direkt neben der EXE; dort den Ordner `scripts\` weglassen. Ohne vorher entfernte Anmeldeaufgabe entfernt `install-app.ps1` nur die Verknüpfungen und lässt die von der Aufgabe benutzte Programmdatei stehen. Beide Skripte löschen nie Profile, Journale oder den Ordner `.omen-hub-pp`. Diesen Ordner erst nach erfolgreicher Wiederherstellung von Hand löschen.

## Grenzen

- Ob NVIDIA-Offsets, CPU-Limits, Tastaturfarben oder Max-Fan einen echten Neustart, Standby/Resume oder einen Netzteilwechsel überdauern, ist nicht gemessen. Verlassen Sie sich nicht auf einen Neustart als Rücksetzung.
- Das Originalbild beschreibt den Zustand **vor dem letzten Apply**. Sicherungen in `control-history\` werden nie automatisch angewendet.
- Andere RGB- oder Tuningprogramme, auch der weiterhin installierte OMEN Gaming Hub, können Zustände unabhängig ändern. Der eigene Prozess-Mutex schließt nur Omen Hub++ selbst aus.
