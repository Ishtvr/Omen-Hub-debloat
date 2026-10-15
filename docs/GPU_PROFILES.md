# GPU-Steuerung und eigene Profile

Stand: 15. Oktober 2026. Der native Editor ist im Dashboard über **GPU-Profile …** erreichbar, alternativ über `gpu-ui`. Speichern ändert keine Hardware. Anwenden, Original wiederherstellen und die Freigabe für die nächste Anmeldung laufen über einen einzelnen, festen UAC-Prozess. Der normale Tray-Prozess bleibt uneleviert.

## Tatsächlich freigegeben

GPU-Core- und VRAM-Takt-Offsets verwenden die öffentlichen NVML-Funktionen `nvmlDeviceGetClockOffsets` und `nvmlDeviceSetClockOffsets`. Die C-Struktur ist 24 Bytes groß, mit NVML-Versionbits 24–31, Graphics/Memory-Domänen und P-State 0. Die Bibliothek wird aus System32 geladen; Handles und Funktionspointer leben nur innerhalb ihrer Treibersitzung.

Die Freigabe gilt für **HP 8E35 / BIOS F.13, RTX 5070 Laptop, NVIDIA 617.14, VBIOS 98.06.2a.40.da**. Profile binden zusätzlich die tatsächliche GPU-UUID. Ein anderes BIOS, ein anderer Treiber oder ein anderes VBIOS verlangt neue Evidenz. Bei mehreren NVIDIA-Geräten ist ein explizites Ziel erforderlich.

Gemessen wurden Core **−15/+15 MHz** und Memory **−24/+24 MHz**, jeweils mit exaktem Readback und vollständiger Rückkehr zu 0/0. Die Treiberspannen −1000…+1000 und −2000…+6000 MHz sind **keine Stabilitätsfreigabe**. VRAM-Werte sind NVML-MHz, keine versprochene effektive DDR-Datenrate. Ein Versuch mit Memory −25 MHz ergab tatsächlich −24 MHz; die Transaktion wies die Abweichung ab und stellte beide Originalwerte wieder her. Nicht darstellbare Werte werden nicht stillschweigend gerundet.

Schreiben verlangt bestätigten Netzbetrieb und eine frische GPU-Temperatur. Es bleibt mindestens drei Grad Abstand zur niedrigeren Grenze aus dem beobachteten HP-Feld 87 °C und der Treiber-Slowdown-Schwelle. Dieser Softwarecheck ändert keine thermische Firmwaregrenze. Restore erfasst den gesamten Offset-Zustand einschließlich Treibergrenzen und GPU-/Treiberidentität.

**V/F-Frequenzsteuerung:** Der separate private Adapter ist zusätzlich auf den SHA-256 der tatsächlich geladenen NVIDIA-Implementierung begrenzt. Ghidra-Beobachtungen und native GETs belegen die exakte v1-ABI für diesen Treiber. Alle **127 Graphics-Punkte** zwischen **450 und 1240 mV** werden vollständig erfasst; verändert wird nur ihr signierter Frequenz-Delta-Wert, innerhalb −1000000…+1000000 kHz. Spannungsanker, Memory-Punkte und sämtliche Power-/Voltage-Felder bleiben unverändert. Um 18:02 wurden alle Graphics-Deltas −15000 kHz exakt zurückgelesen und vollständig restauriert.

Der Editor bietet ein Plateau ab einem vorhandenen Spannungsanker. Die niedrigeren Punkte bleiben erhalten; ein nicht monotoner oder unvollständiger Kurvenwunsch wird abgewiesen. Ein gespeichertes komplettes Kurvenprofil bleibt beim Ändern von Name oder VRAM-Offset vollständig erhalten. Ein neuer Anker/Frequenzwert erzeugt ausdrücklich ein neues Plateau. Ein globaler Core-Offset und eine vollständige V/F-Kurve dürfen nicht gemeinsam im selben Profil stehen.

Das ist **Frequenzsteuerung an Spannungsankern**, wie sie für Undervolting-Ziele verwendet wird. Es ist kein direkter mV-Offset, kein hartes physisches Spannungslimit und kein nachgewiesenes stabiles Lastprofil. Die gemeldeten Kurvenfrequenzen können sich bei identischen programmierten Deltas ändern. `verified` beschreibt die geprüften Steuerziele; `observed` enthält die aktuellen Frequenz-/Offset-Beobachtungen. Restore verlangt identische vollständige Deltas, Spannungsanker, Domänengrenzen und zugehörige globale Offsets; eine alte Boost-Beobachtung wird nicht als unveränderlicher Steuerwert ausgegeben.

**Gemessene Kopplung:** Auf 617.14 löschte ein NVML-Memory-Setter eine unmittelbar zuvor programmierte individuelle Graphics-Kurve. Die Gesamtprüfung erkannte das und restaurierte alle Originale. Der Memory-Adapter erfasst deshalb auch sämtliche Graphics-Deltas und den globalen Core-Offset. Er erhält den vor seinem Aufruf gültigen, geprüften Graphics-Zustand nach der VRAM-Änderung. Die normale Profilreihenfolge setzt Memory vor Core/V/F; Reverse-Recovery folgt derselben Abhängigkeit. Ein gespeichertes nicht uniformes Kurven-/Memory-Profil bestand anschließend Apply, unabhängigen Readback und Restore aus einem neuen Prozess.

## Einheitliches Profilformat

Der neue gemeinsame [Kurveneditor](CURVE_EDITOR.md) ist über **Lüfter & V/F …** erreichbar. Er bearbeitet einzelne GPU-Frequenzpunkte und getrennte CPU-/GPU-Lüfterkurven interaktiv. Er speichert ausschließlich Metadaten; eine enthaltene Lüfterkurve sperrt die Hardware-Anwendung weiterhin bis zur validierten HP-Anbindung. Der getrennte Max-Fan-Override ist inzwischen manuell qualifiziert, aber kein Kurvenadapter.

Das versionierte Format enthält **85 typisierte Feature-Schlüssel** aus den relevanten GPU-, CPU/APU-, Intel-, Windows-, Radeon-, Upscaling- und OMEN-Funktionsgruppen. Das ist ein Datenkatalog, keine Zusage von 85 funktionierenden Adaptern. Im NVIDIA-Bereich sind Core-/Memory-Offsets und vollständige Graphics-V/F-Kurven schreibbar. Seit 14. Oktober sind zusätzlich modellgebundene Vier-Zonen-Farben und der statische/Firmware-Beleuchtungsmodus manuell anwendbar, seit 15. Oktober auch Max-Fan; Beleuchtung und Max-Fan bleiben für Startup gesperrt. Bedienung und Grenzen: [KEYBOARD_UI.md](KEYBOARD_UI.md), [FAN_CONTROL.md](FAN_CONTROL.md).

Integer, Schalter, Auswahllisten, V/F-Punkte, Werte pro CPU-Kern, Core-Ratios, Lüfterkurven und RGB-Farben sind verschiedene Datentypen. Einheiten sind explizit: MHz, mV, mW, °C oder Curve-Optimizer-Schritte. Curve Optimizer wird nicht als Millivolt-Offset ausgegeben. Profile enthalten keine Shell-Kommandos, frei wählbaren Opcodes, EC-Adressen oder Treiberstrukturen.

Nicht unterstützte Parameter können im Datenformat gespeichert werden. **Ein einziges nicht anwendbares Setting sperrt das gesamte Profil vor dem ersten Schreibzugriff.** Es gibt keinen stillen Teilerfolg. Die vier HP-Systemprofile im ursprünglichen Dashboard bleiben ausdrücklich Vorschauen.

CPU-/Windows-/iGPU-Profile brauchen keine NVIDIA-Sitzung, sofern sie keine NVIDIA-GPU-Settings enthalten. Ihre Geräteprüfung verlangt deshalb keine aufgeweckte diskrete GPU. Dies bereitet den gewünschten späteren Battery-Modus vor; MUX/iGPU-only, effizientes CPU-Boost und CPU-Voltage/Curve-Optimizer-Adapter sind noch nicht implementiert.

## Max-Fan als Profilvorgabe

**Max-Fan im Profil** im Kopf des GPU-Editors unterscheidet **Nicht ändern**, **Ein** und **Aus**. Diese Auswahl zeigt gespeicherte Absicht, keinen aktuellen Firmwarezustand. „Nicht ändern“ entfernt nur die Profilvorgabe; ein bereits aktiver Override bleibt davon unberührt. „Aus“ gibt beim ausdrücklichen Apply den Override frei und wählt keine BIOS-Lüfterautomatik. Änderungen sperren Apply bis zum Speichern. Save erhält die vollständige individuelle GPU-Kurve und fremde Profileinstellungen.

Das boolesche `max_fan` ist auf **8E35/F.13**, Netzbetrieb und die frisch bestätigte Tabellen-/Topologie-/Schalteridentität begrenzt. Der vollständige Originalschalter wird dauerhaft protokolliert und kann auch in einem neuen Prozess restauriert werden. Ein Max-Fan-only-Profil benötigt keine NVIDIA-Sitzung. Die native Ansicht enthält weiterhin GPU-Vorgaben; reine Fan-Profile können per CLI angelegt werden. Max-Fan bleibt bis zu realen Lifecycle-Proben für automatischen Anmeldestart gesperrt. Details und die Grenze zu eigenen Temperaturkurven: [FAN_CONTROL.md](FAN_CONTROL.md).

## Transaktion und Wiederherstellung

1. Profil, Gerätebindung, sämtliche Capabilities, Versorgung, komplexe Kurvenziele und Schreibabhängigkeiten prüfen.
2. Alle betroffenen Originalzustände erfassen und das Protokoll atomar mit Flush speichern.
3. Vor jedem Schreibschritt Identität, Versorgung, Limits und unveränderten Ausgangswert erneut prüfen.
4. Jeden Schritt und anschließend das gesamte Zielprofil exakt zurücklesen. Treiberrückgabe 0 allein reicht nicht aus.
5. Bei Fehlern in umgekehrter Reihenfolge zurücksetzen, einschließlich des Setters, der möglicherweise trotz Fehler bereits geändert hat.

Ein unvollständiger Restore lässt das Protokoll offen und sperrt neue Schreibtransaktionen sowie automatische Wiederholungen. `profile-restore` ist eine explizite Wiederherstellung des letzten vollständigen Originalbilds. Ein neuer Prozess akzeptiert es erst nach Schema-/Geräte-/Treiberprüfung und nur dann, wenn der aktuelle Zustand dem aufgezeichneten Original oder Ziel entspricht. Fremde, zwischenzeitlich geänderte Offsets werden nicht überschrieben.

Ein globaler Windows-Mutex verhindert parallele Omen-Hub++-Schreibprozesse. Er ist kein exklusiver Lock gegen fremde Controller; Vor-/Nachprüfungen erkennen Änderungen während der Transaktion. Dauerhafte Ownership, Suspend/Resume und Ersatzbetrieb ohne ursprüngliche HP-Prozesse sind noch nicht am Gerät bewiesen.

## Speicherung und Windows-Anmeldung

Die Daten liegen unter **`<Windows-Benutzerprofil>\.omen-hub-pp`**. Der Pfad wird aus dem Windows-Benutzertoken ermittelt. `profile-storage` zeigt den tatsächlichen Ort. AppData wurde als Speicherort verworfen, nachdem ein realer Test unterschiedliche Dateien im Codex-MSIX-Paket-Cache und im normalen Windows-Prozess nachgewiesen hatte.

`profile-migrate-legacy` übernimmt vorhandene Profile und das Originalprotokoll aus dem zuvor sichtbaren AppData-Speicher. Bestehende Zieldaten werden nicht überschrieben, alte Dateien bleiben erhalten, offene Transaktionen bleiben gesperrt und die Anmeldungsauswahl bleibt ausgeschaltet.

Die feste Anmeldeaufgabe ist über `scripts/install-startup.ps1` eingerichtet. Sie startet **nur** die geschützte installierte Binary mit `profile-startup-apply`, 15 Sekunden nach Anmeldung des konkreten Benutzers, mit dessen interaktivem Token und höchsten verfügbaren Rechten. Sie enthält keine frei wählbaren Befehle, Passwörter, Hintergrund-Updater oder Netzwerkaktion. Binary und Aufgabenänderungen sind gegen normale Schreibzugriffe geschützt.

Zur Auswahl für die Anmeldung müssen die genaue Profilversion bereits erfolgreich angewendet sein, ihr gesamter aktueller Zielzustand erneut passen, alle Settings für Startup freigegeben sein und das Originalprotokoll vollständig abgeschlossen sein. Die bestätigte Profil-/Build-Version wird zusätzlich im geschützten Installationsordner hinterlegt. Ein editierbarer Benutzereintrag allein autorisiert keinen neuen automatischen Tuning-Wert. Bei Profil-/BIOS-/Treiber-/Build-Änderung, fehlendem Netzteil oder offenem Restore bleibt die Anwendung gesperrt; ein Startup-Fehler schaltet die Auswahl aus.

Der installierte Einstieg wurde über Windows Task Scheduler tatsächlich gestartet und mit einem 0/0-MHz-Profil zurückgelesen. **Kein tatsächlicher Neustart wurde ausgeführt.** Wiederherstellung gilt nach Benutzeranmeldung, nicht als vorgezogene Steuerung vor dem Login. Aktuell ist die automatische Profilauswahl ausgeschaltet.

Auch ein vollständiges unverändertes Graphics-Kurvenprofil plus Memory-Offset 0 wurde gespeichert, angewendet, geschützt bestätigt und durch den installierten Task tatsächlich geprüft. Der Task endete mit Ergebnis 0. Der Sicherheitsnachweis verlangt die genaue erfolgreich angewendete Profilrevision und ihre vollständigen programmierten Steuerbilder, einschließlich der erfassten V/F-Basis für eine spätere Prüfung in einem neuen Prozess.

## Bedienung per CLI

```powershell
& '.\dist\Omen Hub++.exe' gpu-ui
& '.\dist\Omen Hub++.exe' gpu-capabilities
& '.\dist\Omen Hub++.exe' profile-create mein-profil 'Mein GPU-Profil'
& '.\dist\Omen Hub++.exe' profile-set mein-profil gpu_core_offset_mhz 0
& '.\dist\Omen Hub++.exe' profile-set mein-profil gpu_memory_offset_mhz 0
& '.\dist\Omen Hub++.exe' profile-plan mein-profil
```

Kurvenprofile lassen sich auch ohne Hardwareänderung vorbereiten:

```powershell
& '.\dist\Omen Hub++.exe' gpu-curve-read
$kurve = & '.\dist\Omen Hub++.exe' gpu-curve-read | Out-String | ConvertFrom-Json
$anker = $kurve.nodes[-1]
& '.\dist\Omen Hub++.exe' profile-set-vf mein-profil ($anker.voltage_uv / 1000) ($anker.frequency_khz / 1000) | Out-String
```

Das Beispiel übernimmt den aktuell gelesenen letzten Anker ohne Tuning-Änderung. Der Befehl verlangt einen vorhandenen Anker und prüft sämtliche Punkte. Er ersetzt einen eventuell gespeicherten globalen Core-Offset durch die vollständige Kurve. `profile-import` übernimmt vollständige eigene JSON-Kurven; dies vermeidet unter Windows PowerShell 5.1 außerdem das Entfernen eingebetteter JSON-Anführungszeichen bei nativen Kommandozeilenargumenten.

Die folgenden Aktionen brauchen hier ein Administrator-Terminal; der native Editor übernimmt dafür die feste UAC-Anforderung:

```powershell
& '.\dist\Omen Hub++.exe' profile-apply mein-profil
& '.\dist\Omen Hub++.exe' profile-restore
& '.\dist\Omen Hub++.exe' profile-startup-enable mein-profil
```

`profile-startup-enable` ist erst nach erneutem erfolgreichem Apply möglich; Restore schaltet die Auswahl aus. `profile-startup-disable` wählt keine automatischen Hardwareänderungen mehr aus. Der Installer verwendet vorhandene Windows PowerShell 5.1:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\install-startup.ps1'
```

`-Mode Inspect` liest den Eintrag, `-Mode Remove` entfernt die feste eigene Aufgabe und bekannte Installationsdateien; Benutzerprofile bleiben bestehen. Die Prozessoption verändert keine gespeicherte PowerShell-Richtlinie.

## Noch gesperrte GPU-Funktionen

**Unleashed, cTGP und Dynamic Boost:** Die genaue HP-Modellressource und mehrere Payloads sind statisch gefunden. Auf diesem BIOS fehlen aber ein vollständiger aktueller Leistungs-/Thermalzustand, belegtes Restore des gekoppelten CPU/GPU-Budgets und die eigenständig validierte Schutz-/Heartbeat-Schleife. Der Benutzer bestätigt Unleashed im Original-Hub; die Firmware liefert zugleich D-State 0. Es gibt deshalb noch keinen eigenen Unleashed-Schreibadapter und keinen nachgewiesenen dauerhaften 115-W-TGP.

**Direkter GPU-mV-Offset / hartes Spannungslimit:** Dafür gibt es weiterhin keinen eigenen validierten Setter. Die oben beschriebene vollständige Frequenzkurve ist freigegeben; ihr physischer Voltage-Effekt und die Laststabilität sind nicht gemessen. MHz-Offsets werden nicht als mV-Undervolting bezeichnet.

**VRAM-Spannung:** Noch kein verifizierter Steuervertrag auf dieser Hardware. Die fünf gelesenen Memory-Domänenpunkte beweisen keine Kontrolle oder Messung der DRAM-Versorgungsschiene. VRAM-Taktsteuerung ist bereits implementiert; das stellt keine VRAM-Spannungssteuerung bereit.

Power-Limit-, Temperature-Limit-, Clock-Lock- und Frame-Cap-Adapter bleiben ebenfalls gesperrt, soweit Readback/Restore/Modellvalidierung fehlen. Es werden keine Schutzgrenzen, Dienste, Firmware oder Kernel-Treiber im Rahmen der normalen Verifikation verändert.

Öffentliche Verträge: [NVIDIA NVML Clock Offset](https://docs.nvidia.com/deploy/nvml-api/latest/api/structnvmlClockOffset__v1__t.html), [NVML Queries](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html), [NVML Commands](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceCommands.html). Der gefundene AppData-Effekt entspricht [Microsofts MSIX-Dokumentation](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes). Messwerte und Grenzen: [VALIDATION.md](VALIDATION.md), [8e35.md](models/8e35.md).
