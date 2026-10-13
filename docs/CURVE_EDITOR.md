# Native GPU- und Lüfterkurven

Der Kurveneditor ist im Dashboard über **Lüfter & V/F …** und im GPU-Profilfenster über **GPU-V/F- und Lüfterkurven bearbeiten …** erreichbar. Alternativ: `curve-ui [profil-id]`. Er verwendet native Windows-Controls und einen gemeinsamen GDI-Canvas, ohne WebView oder zusätzliche UI-Laufzeit.

## Bedienung

- **GPU V/F:** Alle 127 tatsächlich gelesenen Graphics-Spannungsanker sind sichtbar. Punkte auswählen und ihre Frequenz vertikal ziehen; Spannung und Punktzahl bleiben fest. **Ab hier Plateau** setzt den gewählten Frequenzwert für den ausgewählten Punkt und alle höheren Anker. Nachbarordnung und sämtliche tatsächlichen Treibergrenzen werden geprüft.
- **CPU-Lüfter / GPU-Lüfter:** Getrennte Kurven mit Temperatur in °C und Lüfterwert in Prozent. Punkte lassen sich ziehen, über Zahlenfelder bearbeiten, hinzufügen oder entfernen. Temperaturen bleiben streng geordnet, Lüfterwerte nicht fallend, höchstens 32 Punkte und mindestens zwei. Vorlagen sind ausdrücklich Entwürfe, keine ausgelesenen Hardwarezustände.
- **Tastatur:** Links/rechts wählt Punkte, oben/unten ändert den Y-Wert, Umschalt vergrößert den Schritt. Bei Lüftern ändert Strg + links/rechts die Temperatur; Einfg/Entf fügt hinzu oder entfernt. Die native Punktauswahl und Zahlenfelder bieten eine weitere Alternative zur Maus. Strg + S speichert.
- **Kurve im Profil:** Legt fest, welche Kurven gespeichert werden. Eine Bearbeitung wählt die betreffende Kurve automatisch aus. **Kurve zurücksetzen** verwirft die Kurvenbearbeitung bis zum geladenen Ausgangsentwurf; es ist kein Firmware-Reset.

Ein vorhandenes Profil behält seine anderen Einstellungen, etwa den VRAM-Offset. Beim Speichern werden die übrigen Parameter frisch aus dem Speicher gelesen und erhalten. Zwischenzeitlich in einem anderen Fenster geänderte Kurven werden nicht still überschrieben. GPU-V/F und globaler Core-Offset sind weiterhin im selben Profil ausgeschlossen.

## Hardwarestatus

**Bearbeiten und Speichern verändern keine Hardware.** Der Editor enthält keine SET-/UAC-Aktion. Das Profilfenster mit **Anwenden & Startoptionen …** bietet die getrennte, geprüfte Anwendung freigegebener Parameter.

GPU-V/F-Frequenzsteuerung ist auf 8E35/F.13 / RTX 5070 Laptop / NVIDIA 617.14 / VBIOS 98.06.2a.40.da freigegeben. Die Spannungsanker werden nicht geschrieben. Es gibt kein hartes physisches Spannungslimit und noch keinen Last-Stabilitätsnachweis.

**Lüfterkurven lassen sich bearbeiten und speichern, aber noch nicht auf die HP-Hardware anwenden.** Prozentwerte sind noch nicht in modellbezogene PWM-/RPM-Payloads übersetzt; aktueller Zustand, Readback und Restore sind nicht validiert. Ein Profil mit solchen Settings wird vollständig vor dem ersten Hardware-Schreibzugriff gesperrt. CPU-Temperatur und Package Power bleiben ohne geprüften Sensoradapter unbekannt.

## Implementierung und Prüfung

`core/curve_editor.rs` enthält plattformfreie Editing-/Grenzregeln. `app/curve_widget.rs` besitzt den wiederverwendbaren Canvas, Maus-/Tastaturbehandlung, GDI-Zeichnung und Objekt-Lifetimes. `app/curve_ui.rs` verwaltet native Controls, Hintergrund-Laden, Profilzusammenführung und Speicherung. Der Renderer läuft ereignisgesteuert und zeichnet zwischen Benutzeraktionen nicht periodisch.

`scripts/smoke-curves-ui.ps1` prüft ausschließlich einen eigenen Testprozess: GPU-Maus-/Tastatureingaben, unveränderte Spannungsanker, CPU-Lüfter-Drag, Hinzufügen/Entfernen, separate GPU-Lüfterkurve, Speichern/Neuladen und Erhalt der übrigen Parameter. GPU-/VRAM-Offsets und alle programmierten Graphics-Deltas bleiben unverändert. Der Test prüft außerdem, dass gespeicherte Lüfter-Entwürfe keine Hardware-Schreibfreigabe erhalten.

Widget-Renderings werden direkt mit derselben GDI-Zeichnung in einen eigenen Speicher-Bitmap gerendert und visuell geprüft. Dabei wird kein Desktop und keine andere Anwendung aufgenommen. Das ersetzt keinen vollständigen Fenster-/DPI-Test. Details und Build-Messungen: [VALIDATION.md](VALIDATION.md).
