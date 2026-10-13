# Native GPU- und Lüfterkurven

Der Kurveneditor ist im Dashboard über **Lüfter & V/F …** und im GPU-Profilfenster über **GPU-V/F- und Lüfterkurven bearbeiten …** erreichbar. Alternativ: `curve-ui [profil-id]`. Er verwendet native Windows-Controls und einen gemeinsamen GDI-Canvas, ohne WebView oder zusätzliche UI-Laufzeit.

Standardmäßig öffnet sich die dunkle **Diagrammansicht** mit großer Kurve, Kurvenauswahl, Speichern, Rückgängig/Wiederholen, Hilfe und Zoom. **Profil / Werte …** öffnet Profilauswahl, Namen, Punktauswahl und Zahlenfelder; **Diagrammansicht** wechselt zurück. Enter im Diagramm öffnet diese Ansicht und fokussiert den Y-Wert des ausgewählten Punkts.

Die vom Benutzer vorgegebene MSI-Afterburner-Ansicht dient als visuelle Referenz für das V/F-Diagramm. Seine Bedienidee ist zusätzlich im [offiziellen MSI-Leitfaden](https://www.msi.com/blog/msi-afterburner-overclocking-undervolting-guide) beschrieben. Omen Hub++ implementiert den Editor mit eigenem Rust-/Win32-Code; dafür werden keine MSI-Binärdateien, Treiber oder Assets übernommen oder benötigt. Diese Referenz legt die Kurvenansicht fest, keine neue globale Produktidentität.

## Diagramm lesen

**Quadrate: Entwurf** bezeichnet die bearbeitete Kurve; **Gepunktet: beim Laden** bezeichnet ihren geladenen Ausgangsentwurf. Bei einem gespeicherten GPU-Kurvenprofil ist dies die geladene Profilkurve, ansonsten die beim Laden gelesene GPU-Kurve. Lüftervorlagen sind gespeicherte oder vorgegebene Entwürfe und kein ausgelesener Hardwarezustand.

Der ausgewählte Punkt ist als größeres blaues Quadrat mit horizontaler und vertikaler Hilfslinie markiert. **Ausgewählt** nennt Spannung und Frequenz beziehungsweise Temperatur und Lüfterwert. Diese Linien folgen der Punktauswahl; sie zeigen keinen aktuellen GPU-Betriebspunkt. Ein markierter GPU-Spannungsbereich erhält ein durchgehendes farbiges Band und eine Punktzahl.

Die Achsen sind mit mV/MHz beziehungsweise °C/% beschriftet. Rasterabstände passen sich an Fenstergröße und Zoom an. **Ganze Kurve** berücksichtigt die geladenen und bearbeiteten Frequenzen. Zoomen oder Verschieben verändert nur den sichtbaren Ausschnitt; alle Kurvenpunkte bleiben im Entwurf und im gespeicherten Profil erhalten.

## Bedienung

Auf dem freigegebenen Referenzgerät enthält GPU V/F alle **127 tatsächlich gelesenen Graphics-Spannungsanker**. Spannung, Reihenfolge und Punktzahl bleiben fest; nur Frequenzziele sind bearbeitbar. Frequenzen müssen entlang steigender Spannung monoton nicht fallend sein und alle tatsächlichen Treibergrenzen einhalten. Das Spannungsfeld ist deshalb schreibgeschützt, GPU-Punkte lassen sich weder hinzufügen noch entfernen.

| GPU-Mausaktion | Wirkung auf den Entwurf |
| --- | --- |
| Punkt ziehen | Frequenz des Punkts vertikal ändern; bei einem Punkt im markierten Bereich den Bereich gemeinsam verschieben |
| Shift + Punkt ziehen | Gesamte Kurve oder markierten Bereich auf Basis des **geladenen Ausgangsentwurfs** verschieben |
| Alt + Punkt ziehen | Gesamte Kurve oder markierten Bereich auf Basis des **aktuellen Entwurfs** gleichmäßig verschieben |
| Strg + Punkt ziehen | Alle Frequenzen um den gegenüberliegenden Randpunkt neigen; dieser Randpunkt bleibt fest |
| Shift + Ziehen im freien Diagramm | Einen zusammenhängenden Spannungsbereich markieren |
| Doppelklick | Bereichsauswahl aufheben |

Shift verwendet dabei wieder die Form der geladenen Ausgangskurve. Alt erhält die Form des aktuellen Entwurfs. Ein gemeinsamer Offset wird für den gesamten betroffenen Bereich einheitlich an Treiber- und Nachbargrenzen begrenzt. Eine Neigung, die einen Punkt oder die Frequenzordnung verletzt, wird vollständig abgelehnt. Mausbewegungen werden vom festen Anfangszustand des Ziehens berechnet; ein abgeschlossener Drag zählt als ein Verlaufsschritt.

**Ab hier Plateau** setzt den Wert aus dem Frequenzfeld für den ausgewählten Punkt und alle höheren Anker. Shift + Enter arbeitet direkt mit der ausgewählten Frequenz: Es flacht den markierten Bereich ab oder erzeugt ohne Bereichsauswahl ein Plateau ab dem Punkt. Die Änderung muss für sämtliche betroffenen Anker gültig sein.

**CPU-Lüfter / GPU-Lüfter** behalten getrennte Entwürfe und Bearbeitungsverläufe beim Wechsel der Ansicht. Punkte lassen sich ziehen, numerisch bearbeiten, hinzufügen und entfernen. Temperaturen bleiben zwischen 0 und 110 °C streng geordnet, Lüfterwerte zwischen 0 und 100 % nicht fallend; mindestens zwei und höchstens 32 Punkte bleiben erhalten. Eine Vorlage ist ausdrücklich ein Entwurf.

**Kurve im Profil** legt fest, welche Kurven gespeichert werden. Eine Bearbeitung wählt die betreffende Kurve automatisch aus. **Zurücksetzen** oder Strg + D führt die aktive Kurve zum geladenen Ausgangsentwurf zurück und ist selbst rückgängig machbar. Es verändert keine Firmware.

## Tastatur und Ansicht

Die Kurvenbefehle gelten bei Fokus im Diagramm. Strg + S und F1 werden auch im gesamten Editor erkannt. F1 oder **Tastenkürzel** öffnet die native Hilfe. Rückgängig/Wiederholen und Zoom haben zusätzlich sichtbare Schaltflächen; die Punktauswahl und Zahlenfelder bieten eine weitere Alternative zur Maus.

| Eingabe | Wirkung |
| --- | --- |
| Links / Rechts, Pos1 / Ende | Punkt auswählen beziehungsweise zum ersten/letzten Punkt wechseln |
| Oben / Unten | Frequenz um 1 MHz beziehungsweise Lüfterwert um 1 Prozentpunkt ändern; bei markiertem GPU-Bereich den Bereich gemeinsam verschieben |
| Strg + Oben / Unten | Y-Wert um 10 Schritte ändern |
| Shift + Oben / Unten bei GPU V/F | Gesamte Kurve oder markierten Bereich auf Basis des geladenen Ausgangsentwurfs verschieben; mit Strg in 10-MHz-Schritten |
| Alt + Oben / Unten bei GPU V/F | Gesamte Kurve oder markierten Bereich auf Basis des aktuellen Entwurfs verschieben; mit Strg in 10-MHz-Schritten |
| Shift + Links / Rechts, Shift + Pos1 / Ende | Zusammenhängende GPU-Auswahl erweitern |
| Strg + A | Alle GPU-Punkte markieren |
| Shift + Enter | GPU-Auswahl abflachen oder Plateau ab dem Punkt bilden |
| Enter | Profil-/Werteansicht öffnen und Y-Zahlenfeld bearbeiten |
| Strg + Z | Rückgängig |
| Strg + Y oder Strg + Shift + Z | Wiederholen |
| Esc | Aktives Ziehen abbrechen; sonst Bereichsauswahl aufheben |
| Strg + D | Geladenen Entwurf der aktiven Kurve zurücksetzen |
| Strg + S | Profil speichern, ohne Hardwareänderung |
| Mausrad oder + / − | X-Achse um Mausposition beziehungsweise ausgewählten Punkt zoomen |
| Strg + Mausrad oder Strg mit + / − | Y-Achse zoomen |
| Shift + Mausrad | Sichtbaren Ausschnitt horizontal verschieben |
| Strg + 0 oder **Ganze Kurve** | Vollständige Kurve einpassen |
| Strg + Links / Rechts bei Lüftern | Temperatur um 1 °C ändern; mit Shift um 10 °C |
| Shift + Oben / Unten bei Lüftern | Lüfterwert um 10 Prozentpunkte ändern |
| Einfg / Entf bei Lüftern | Punkt hinzufügen beziehungsweise entfernen |

Jede Kurve besitzt bis zu **64 Rückgängig-Schritte**. Ein ganzer Drag belegt einen Schritt; Esc beim Ziehen stellt dessen Anfangszustand wieder her. Eine neue Änderung nach Rückgängig verwirft den Wiederholen-Zweig. Laden eines Profils beginnt mit dessen neuem Ausgangsentwurf und einem leeren Verlauf.

## Speichern

Ein vorhandenes Profil behält seine anderen Einstellungen, etwa den VRAM-Offset. Beim Speichern werden die übrigen Parameter frisch aus dem Profilspeicher gelesen und erhalten. Zwischenzeitlich in einem anderen Fenster geänderte Kurven werden nicht still überschrieben. GPU-V/F und globaler Core-Offset sind im selben Profil ausgeschlossen; beim Speichern einer aktiven GPU-V/F-Kurve entfernt der Editor den globalen Core-Offset aus diesem Profil.

## Hardwarestatus

**Bearbeiten und Speichern verändern keine Hardware.** Der Editor enthält keine SET-/UAC-Aktion. Das Profilfenster mit **Anwenden & Startoptionen …** bietet die getrennte, geprüfte Anwendung freigegebener Parameter.

GPU-V/F-Frequenzsteuerung ist auf 8E35/F.13 / RTX 5070 Laptop / NVIDIA 617.14 / VBIOS 98.06.2a.40.da freigegeben. Die Spannungsanker werden nicht geschrieben. Es gibt kein hartes physisches Spannungslimit und noch keinen Last-Stabilitätsnachweis.

**Lüfterkurven lassen sich bearbeiten und speichern, aber noch nicht auf die HP-Hardware anwenden.** Prozentwerte sind noch nicht in modellbezogene PWM-/RPM-Payloads übersetzt; aktueller Zustand, Readback und Restore sind nicht validiert. Ein Profil mit solchen Settings wird vollständig vor dem ersten Hardware-Schreibzugriff gesperrt. CPU-Temperatur und Package Power bleiben ohne geprüften Sensoradapter unbekannt.

Ein fester GPU-Spannungs-/Takt-Lock, die Afterburner-Taste L und OC Scanner sind noch nicht unterstützt. Die native Hilfe benennt diese Grenzen. Eine markierte Spannung oder ein Plateau bedeutet keine physisch erzwungene Spannungsobergrenze. Unleashed, direkte Spannungsänderungen und weitere ungeprüfte Hardwareparameter erhalten durch die neue Bedienung keine Schreibfreigabe.

## Implementierung und Prüfung

`core/curve_editor.rs` enthält plattformfreie Editing-/Grenzregeln, Bereichsauswahl, atomare Transformationen und Verlauf. `core/curve_input.rs` übersetzt abstrakte Tastaturbefehle in dieselben Editing-Regeln. `core/curve_view.rs` hält Zoom, Pan und adaptive Rasterabstände getrennt von Kurvendaten und Treibergrenzen. `app/curve_widget.rs` besitzt den wiederverwendbaren Canvas, native Eingabe, GDI-Zeichnung und Objekt-Lifetimes. `app/curve_ui.rs` verwaltet native Controls, Hintergrund-Laden, Profilzusammenführung und Speicherung. Der Renderer läuft ereignisgesteuert und zeichnet zwischen Benutzeraktionen nicht periodisch.

`scripts/smoke-curves-ui.ps1` prüft ausschließlich einen eigenen Testprozess: gleichmäßiges Shift-Ziehen aller 127 GPU-Punkte, Bereichsauswahl und Abflachen, Strg + Z/Y, Zoom/Einpassen, Zahlenfelder, CPU-Lüfter-Drag, Hinzufügen/Entfernen, separate GPU-Lüfterkurve, Speichern/Neuladen und Erhalt der übrigen Parameter. GPU-/VRAM-Offsets und alle programmierten Graphics-Deltas bleiben unverändert. Der Test prüft außerdem, dass gespeicherte Lüfter-Entwürfe keine Hardware-Schreibfreigabe erhalten.

Die visuelle Abschlussprüfung trägt **SHIP** für die Diagramme und die abgebildeten Client-Controls in der großen/kleinen GPU-Ansicht, der CPU-Lüfteransicht und einem ausdrücklich ungespeicherten/unangewendeten GPU-Beispiel. Die Bilder stammen aus der eigenen nativen Client-Zeichnung über GDI/`WM_PRINT`, ohne Desktopaufnahme. Der ausgelassene Render der vollständigen Zahlenansicht bildet versteckte EDIT-/COMBOBOX-Texte unvollständig ab und gehört nicht zu dieser Freigabe. Titelleiste, vollständige Zahlenansicht, mehrere DPI-Stufen und eine umfassende Barrierefreiheitsprüfung sind damit nicht abgenommen. Details, Test- und Build-Messungen: [VALIDATION.md](VALIDATION.md).
