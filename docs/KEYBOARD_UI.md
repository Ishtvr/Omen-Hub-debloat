# Direkte Tastaturbeleuchtung

**Beleuchtung** im Dashboard oder `omen-hub-pp.exe keyboard-ui` öffnet die native Tastaturansicht. Sie enthält vier anklickbare Zonen einschließlich Nummernblock, einen **Ein/Aus**-Schalter und eine kurze Zustandszeile. Das Fenster fordert einmal Windows-UAC an; Dashboard und Tray bleiben uneleviert.

1. **Ein** aktiviert die gespeicherten statischen Zonenfarben. **Aus** schaltet die Beleuchtung aus; die RGB-Werte bleiben erhalten.
2. Bei eingeschalteter Beleuchtung eine Zone anklicken oder per Tab und Leertaste öffnen. Der Windows-Farbdialog startet mit ihrer gelesenen Farbe.
3. Bestätigen überträgt die Farbe unmittelbar und prüft das vollständige Ergebnis. Abbrechen ändert nichts. Ein gesondertes Speichern/Anwenden ist nicht erforderlich.

Die native Tastaturzeichnung stellt die vier Farbbereiche dar; sie bietet keine Einzeltastensteuerung. Bei ausgeschalteter Beleuchtung sind die Zonen deaktiviert. Ohne gültigen Hardwarezustand bietet die Schalteraktion eine erneute Verbindung an, statt erfundene Farben als gelesen auszugeben. Ein zwischenzeitlich durch einen anderen Controller geänderter Zustand wird neu gelesen; die abgewiesene Benutzeraktion wird nicht still wiederholt.

## Hardware und Wiederherstellung

Freigegeben ist ausschließlich **HP 8E35 / BIOS F.13 / Capability [1,0,0,0] / FourZoneWithNumpad**. Der Provider verlangt auf diesem Gerät auch für GET Administratorzugriff. Der eigene Rust-Transport verwendet die vorhandene HP-BIOS-WMI-Schnittstelle; HP-RPC-/SDK-DLLs werden dafür nicht geladen.

Die Farben ändern nur die zwölf qualifizierten RGB-Bytes im vollständigen 128-Byte-Bild. Der Modus verwendet genau die belegten Zustände **0xE4 = statisch ein** und **0x64 = aus**. Die frühere Beschreibung von 0x64 als „Firmware-Voreinstellung“ war für dieses Modell zu ungenau. Hersteller-Off-Pfad, gemessener Readback und die sichtbare Bestätigung des Benutzers am 15. Oktober belegen Ein/Aus. Der alte serialisierte Wert `firmware_default` bleibt als Alias für bestehende Profile und Recovery-Journale lesbar.

`windows/lighting.rs::DirectLighting` nutzt dieselbe typisierte Transaktionsengine wie Performanceprofile, aber einen eigenen dauerhaften Speicher unter **`<Windows-Benutzerprofil>\.omen-hub-pp\keyboard-control`**. Vor jedem Schreiben müssen Gerätebindung, Capability und das komplette erwartete Steuerbild passen. Originale werden vor dem SET atomar gespeichert; Fehler lösen den geprüften Restore aus. Eine offene eigene Beleuchtungstransaktion kann beim erneuten Verbinden nur nach streng begrenzter Journalprüfung wiederhergestellt werden. Ein offenes Performancejournal sperrt neue Schreibaktionen; ein gemeinsamer Prozess-Mutex serialisiert beide Controllerwege.

Beleuchtungsaktionen ersetzen weder das Performancejournal noch dessen Startup-Auswahl. Fehlerdetails liegen getrennt im Beleuchtungsspeicher. Das Fenster wartet beim Schließen auf eine laufende Änderung; es beendet keinen aktiven Hardware-Worker gewaltsam. Der Mutex ist keine Exklusivgarantie gegenüber HP oder anderer RGB-Software.

## Gemessene Prüfung

Am **15. Oktober 2026, 20:57 CEST**, prüfte `scripts/check-lighting-ui.ps1` die tatsächlichen nativen Controls: Ein/Aus, korrekt vorbelegten Farbdialog, Abbrechen ohne Änderung, rote linke Zone bei erhaltenen anderen Zonen und vollständigen Original-Restore. Nicht-Klick-Notifications starteten keine Aktion. GPU-Steuerbilder, HP-GPU-Status und das Performancejournal samt Startup-Datei blieben unverändert. Diese Farbprobe wurde vom Benutzer nicht beobachtet; ihre Evidenz ist der vollständige Firmware-Readback.

Die ausdrücklich angeforderte Wiederholung um **21:08 CEST** hielt jeweils vier Sekunden **an → aus → an** und restaurierte anschließend den Ausgangszustand. Der Benutzer bestätigte **„Ja, an/aus/an war sichtbar“**. Das bestätigt die sichtbare Schaltwirkung auf diesem Gerät. Es ist keine Sichtabnahme der Farbe und kein Nachweis für Reboot/Resume oder dauerhafte Ownership.

`scripts/smoke-keyboard-ui.ps1` rendert Ein/Aus in Standard- und Mindestgröße aus temporären, ausdrücklich synthetischen Profilwerten. Dieser Test fordert keine UAC an, liest keine Tastaturfirmware und schreibt keine Hardware. Tatsächliche Schaltproben sind davon getrennt und erfordern ein erhöhtes Terminal sowie eine ausdrücklich gewollte Hardwareprüfung:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/check-lighting-ui.ps1 -SwitchOnly -HoldSeconds 4
```

Beleuchtung bleibt **`startup_safe = false`**. Allgemeine Helligkeitsprozente, Animationen, weitere Tastaturtypen, Fn-F4-/Entsperr-/Power-Lifecycle und Betrieb ohne die noch laufenden HP-Controller sind nicht qualifiziert. Quellen und vollständige Build-Zuordnung: [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md), [8e35.md](models/8e35.md), [VALIDATION.md](VALIDATION.md).

## Native Darstellung

Die Ansicht folgt dem vorhandenen dunklen Segoe-UI-System. Die Zonen sind echte benannte native Buttons mit Tastaturfokus; GDI zeichnet ihre Tastengeometrie und Datenfarben. Es werden keine Hersteller-Bitmaps, dekompilierten Quellen oder proprietären Bibliotheken ausgeliefert. Client-Renderings prüfen die vollständige Tastatur bei **984×551** und **744×441** Pixeln (96 DPI). Andere DPI, Titlebars, geöffneter Systemfarbdialog, umfassende Barrierefreiheit und dynamische High-Contrast-Wechsel sind dadurch nicht zertifiziert.
