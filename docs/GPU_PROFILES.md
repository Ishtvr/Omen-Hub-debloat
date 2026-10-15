# GPU-Steuerung und eigene Profile

Stand: 15. Oktober 2026. **Leistungsprofile → Custom Presets** ersetzt das frühere GPU-Profilfenster; `presets-ui [id]` ist der direkte Einstieg, `gpu-ui` bleibt Alias. Die vollständige Bedienung steht in [CUSTOM_PRESETS.md](CUSTOM_PRESETS.md). Speichern verändert keine Hardware. Anwenden/Wiederherstellen/Anmeldefreigabe verwenden feste UAC-Children; Dashboard und Tray bleiben uneleviert.

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

Die **GPU-Leistungskurve** liegt im GPU-Bereich des Presets. Bearbeiten öffnet den eigenen nativen Chart unmittelbar auch bei neuen oder geänderten Presets. Übernehmen gibt die Kurve an den ungespeicherten Preset-Entwurf zurück; Abbrechen verwirft nur den Chart-Entwurf. Erst Save im Preset schreibt Metadaten. Die eigenständigen CLI-Einstiege `gpu-curve-ui`, `fan-ui` und `curve-ui` bleiben erhalten. Alle verwenden denselben [Kurveneditor](CURVE_EDITOR.md); enthaltene Lüfterkurven sperren die Hardwareanwendung weiterhin. Max-Fan ist kein Kurvenadapter.

Das versionierte Format enthält **91 typisierte Feature-Schlüssel**, einschließlich sechs ergänzter CPU-Boost-/Windows-Prozessorfelder. Das ist ein Datenkatalog, keine Zusage von 91 funktionierenden Adaptern. Core-/VRAM-Offsets, vollständige Graphics-V/F-Kurven, modellgebundene Vier-Zonen-Farben/statisch-aus und manuelles Max-Fan sind unter ihren bisherigen Grenzen schreibbar. Beleuchtung und Max-Fan bleiben für Startup gesperrt. Die UI lässt Super Resolution und Radeon Graphics Options weg, erhält vorhandene solche Settings beim Save jedoch vollständig.

Integer, Schalter, Auswahllisten, V/F-Punkte, Werte pro CPU-Kern, Core-Ratios, Lüfterkurven und RGB-Farben sind verschiedene Datentypen. Einheiten sind explizit: MHz, mV, mW, °C oder Curve-Optimizer-Schritte. Curve Optimizer wird nicht als Millivolt-Offset ausgegeben. Profile enthalten keine Shell-Kommandos, frei wählbaren Opcodes, EC-Adressen oder Treiberstrukturen.

Nicht unterstützte Parameter des weiterhin lesbaren Schemas werden vor dem ersten Schreibzugriff abgewiesen; ein einziges solches Setting sperrt das gesamte Profil. Die GUI entfernt jetzt die zehn übrigen unqualifizierten GPU-/FPS-/HP-Modusoptionen aus der Auswahl und bereinigt ihre alten Profilanweisungen erst beim ausdrücklichen Save. Andere Settings und vollständige Kurven bleiben erhalten. Der Unterschied zwischen unterstütztem Feature und momentaner Sperre bleibt sichtbar: Ein globaler Core-Offset ist mit einer aktiven individuellen V/F-Kurve unvereinbar. Die drei Dashboard-Defaults bleiben ausdrücklich nicht verfügbar; Dynamic Boost ist auf ausdrücklichen Nutzerwunsch wieder als Ein/Aus-Vorgabe im GPU-Bereich vorhanden. Der kompatible `gpu_unleashed`-Wert und vorhandene `gpu_dynamic_boost`-Einträge bleiben erhalten. Der HP-Adapter ist noch nicht freigegeben; gespeicherte Auswahl bedeutet keine Anhebung auf 115 W.

CPU-/Windows-/iGPU-Profile brauchen keine NVIDIA-Sitzung, sofern sie keine NVIDIA-GPU-Settings enthalten. Ihre Geräteprüfung verlangt deshalb keine aufgeweckte diskrete GPU. Dies bereitet den gewünschten späteren Battery-Modus vor; MUX/iGPU-only, effizientes CPU-Boost und CPU-Voltage/Curve-Optimizer-Adapter sind noch nicht implementiert.

## Max-Fan als Profilvorgabe

**Max-Fan** in der NVIDIA-Gruppe wird optional per Checkbox aufgenommen, mit getrenntem Ein/Aus-Wert. Ein fehlendes Setting entfernt nur die Profilvorgabe und setzt keinen aktiven Override zurück. Aus gibt beim ausdrücklichen Apply den Override frei und wählt keine BIOS-Automatik. Die Auswahl ist Profilabsicht, kein gelesener Firmwarezustand. Änderungen sperren Apply bis Save; Name und fremde Settings sowie die vollständige GPU-Kurve bleiben erhalten.

Das boolesche `max_fan` ist auf **8E35/F.13**, Netzbetrieb und die frisch bestätigte Tabellen-/Topologie-/Schalteridentität begrenzt. Der vollständige Originalschalter wird dauerhaft protokolliert und kann auch in einem neuen Prozess restauriert werden. Ein Max-Fan-only-Profil benötigt keine NVIDIA-Sitzung. Reine Max-Fan-Profile lassen sich im Custom-Presets-Editor ohne GPU-Taktvorgaben anlegen. Max-Fan bleibt bis zu realen Lifecycle-Proben für automatischen Anmeldestart gesperrt. Details und die Grenze zu eigenen Temperaturkurven: [FAN_CONTROL.md](FAN_CONTROL.md).

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
