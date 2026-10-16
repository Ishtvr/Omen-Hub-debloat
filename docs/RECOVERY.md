# Wiederherstellung und Deinstallation

Diese Anleitung beschreibt, was Omen Hub++ auf einem Gerät verändern kann, wie der vor einer Änderung gespeicherte Originalzustand zurückkommt und wie die Anwendung vollständig entfernt wird. Sie gilt für den aktuellen Stand und das qualifizierte Referenzgerät **HP 8E35 / BIOS F.13**; auf anderen Geräten werden Schreibaktionen bereits vor dem ersten SET blockiert.

## Was Omen Hub++ verändert

| Bereich | Wann geschrieben wird | Originalzustand |
| --- | --- | --- |
| NVIDIA-Core-/VRAM-Offset, vollständige V/F-Kurve, Max-Fan | Nur bei ausdrücklichem **Anwenden** eines gespeicherten Custom Presets (UAC) oder beim freigegebenen Anmeldestart eines vollständig unterstützten Presets | Vor jedem SET in `control-journal.json` gespeichert; Readback nach dem Schreiben, Rollback bei Teilfehlern |
| Tastaturfarben und Ein/Aus | Nur durch Klick im Fenster **Beleuchtung** (UAC) | Eigenes Journal unter `keyboard-control\` |
| Profile, Entwürfe, Lüfterkurven-Entwürfe | **Speichern** im Editor | Reine Metadaten, keine Hardwarewirkung |
| Windows-Anmeldestart | Nur nach ausdrücklicher Freigabe eines bereits angewendeten, unterstützten Presets | Geplante Aufgabe `OmenHubPP-Profile-<SID>` |

Omen Hub++ stoppt oder entfernt keine HP-Dienste, installiert keinen Treiber, flasht keine Firmware und ändert keine Spannungen oder Leistungsgrenzen. Ein Vorschau-Leistungsmodus (Quiet/Balanced/Performance) schreibt nichts.

## Speicherorte

- **Benutzerdaten:** `%USERPROFILE%\.omen-hub-pp\` – `profiles\`, `deleted-profiles\` (archivierte Originalbytes), `control-journal.json` (Originalbild des letzten Apply), `control-history\` (Sicherungen früherer abgeschlossener Journale aus Qualifikationsläufen), `startup.json`, `last-*.json`, `keyboard-control\` (Beleuchtungsjournal).
- **Installation:** `%ProgramFiles%\OmenHubPlusPlus\` – `Omen Hub++.exe`, `installation.json` (Startup-Beleg) und `startup-authorized-<SID>.json`.
- **Verknüpfungen:** `Omen Hub++.lnk` im Startmenü (Programme) und auf dem Desktop des installierenden Benutzers.

Die Journale sind die einzige Quelle für den gespeicherten Originalzustand. **Löschen Sie `.omen-hub-pp` nicht, bevor Sie wiederhergestellt haben.**

## Originalzustand zurückholen

### Leistungsprofil, GPU-Takt/-Kurve und Max-Fan

1. Ist eine Transaktion unvollständig (z. B. Absturz während **Anwenden**), zeigt **Leistungsprofile** statt **Anwenden** die Aktion **Wiederherstellen**. Sie stellt das gespeicherte Original nach UAC wieder her.
2. Unabhängig davon stellt die Befehlszeile den vor dem letzten Apply gespeicherten Zustand wieder her. Die Funktion deaktiviert zuerst eine automatische Anmeldeauswahl:

   ```powershell
   & "$env:ProgramFiles\OmenHubPlusPlus\Omen Hub++.exe" profile-restore
   ```

   In einer **als Administrator** geöffneten PowerShell ausführen. Ein anderes Board, BIOS oder ein anderer NVIDIA-Treiber blockiert die Wiederherstellung ausdrücklich, statt fremde Werte zu schreiben.
3. Max-Fan lässt sich zusätzlich über ein Preset mit **Max-Fan: Aus** freigeben. **Aus** gibt nur den Override frei und wählt keinen bestimmten BIOS-Lüftermodus.

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
# 2. Geplante Anmeldeaufgabe, Startup-Beleg und Freigabe entfernen:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-startup.ps1 -Mode Remove
# 3. Programmdatei und eigene Verknüpfungen entfernen:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-app.ps1 -Mode Remove
```

`install-startup.ps1` ist nicht Teil des Release-Pakets; dort reicht Schritt 3, solange kein Anmeldestart eingerichtet wurde. Ohne vorher entfernte Anmeldeaufgabe entfernt `install-app.ps1` nur die Verknüpfungen und lässt die von der Aufgabe benutzte Programmdatei stehen. Beide Skripte löschen nie Profile, Journale oder den Ordner `.omen-hub-pp`. Diesen Ordner erst nach erfolgreicher Wiederherstellung von Hand löschen.

## Grenzen

- Ob NVIDIA-Offsets, Tastaturfarben oder Max-Fan einen echten Neustart, Standby/Resume oder einen Netzteilwechsel überdauern, ist nicht gemessen. Verlassen Sie sich nicht auf einen Neustart als Rücksetzung.
- Das Originalbild beschreibt den Zustand **vor dem letzten Apply**. Sicherungen in `control-history\` werden nie automatisch angewendet.
- Andere RGB- oder Tuningprogramme, auch der weiterhin installierte OMEN Gaming Hub, können Zustände unabhängig ändern. Der eigene Prozess-Mutex schließt nur Omen Hub++ selbst aus.
