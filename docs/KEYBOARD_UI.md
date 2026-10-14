# Tastaturbeleuchtungseditor

**Beleuchtung …** im Dashboard öffnet einen nativen Editor für vier Tastaturzonen. Alternativ startet `omen-hub-pp.exe keyboard-ui [id]` das Fenster mit einer optionalen gespeicherten Profil-ID. Der Editor trennt den Farbentwurf und gespeicherte Profilwerte vom tatsächlich gelesenen Firmwarezustand.

Schreibsupport gilt derzeit ausschließlich für HP-Board **8E35**, BIOS **F.13**, Capability **[1,0,0,0] / FourZoneWithNumpad**. Andere Boards, BIOS-Versionen, Ein-Zonen-, MCU- oder Einzeltastenvarianten sind nicht durch diesen Adapter freigegeben. Die normale Oberfläche bleibt uneleviert; der BIOS-Provider benötigt hier auch für das Lesen Windows-UAC.

## Ein Profil bearbeiten und anwenden

1. Ein vorhandenes Profil auswählen oder mit **Neu** einen Entwurf beginnen. Ein neues Profil startet mit weißen Entwurfsfarben; diese sind keine gelesenen Hardwarewerte.
2. Einen Namen und vier RGB-Farben eingeben. Die Hexfelder akzeptieren sechs Hex-Zeichen mit optionalem `#`, zum Beispiel `3399FF`. **Farbe wählen …** öffnet den Windows-Farbdialog. Die vier Farbfelder zeigen den Entwurf.
3. Unter **Anzeige** auswählen, ob eigene statische Farben aktiviert werden, die Firmware-Voreinstellung verwendet wird oder die Aktivierung unverändert bleibt. Die letzte Option speichert Farben ohne Modus-Setting. **Firmwarefarben übernehmen** kopiert nur die zuletzt gelesenen RGB-Werte in den Entwurf; die Modusauswahl bleibt wie eingestellt.
4. **Speichern** oder Strg+S schreibt Profilmetadaten. Dies verändert keine Hardware. Vorhandene GPU-, CPU-, Lüfter- und andere Settings bleiben erhalten. Hat ein anderes Fenster die Beleuchtungssettings oder den gleichzeitig bearbeiteten Namen geändert, verlangt der Editor ein erneutes Laden.
5. **Firmware prüfen …** fordert Windows-Freigabe an, liest Capability sowie vollständige Farb-/Helligkeitsbilder und prüft das gesamte gespeicherte Profil. Pro Zone erscheint `Firmware: #RRGGBB`. Die Herkunftszeile nennt eigene Farben oder Firmware-Voreinstellung. Ohne gespeichertes Profil lässt sich der aktuelle Zustand lesen und in einen neuen Entwurf übernehmen.
6. **Gesamtes Profil anwenden …** wird erst nach Save und erfolgreicher Prüfung verfügbar. Die Aktion überträgt das gesamte ausgewählte Profil, also auch darin enthaltene GPU-Settings. Ein fester UAC-Child prüft das Profil erneut, speichert die Originale vor dem ersten SET dauerhaft und kontrolliert den vollständigen Zielzustand per Readback.
7. **Original wiederherstellen …** ist verfügbar, wenn das letzte aufgezeichnete Originalbild Beleuchtungssettings enthält und zu der ausgewählten Profil-ID gehört. Der erhöhte Child prüft diese Zuordnung nochmals. Restore stellt die gesamte aufgezeichnete Profiltransaktion wieder her, einschließlich gegebenenfalls enthaltener GPU-Settings, und deaktiviert die Anmeldungsauswahl.

Ein gespeichertes Profil mit nicht unterstützten CPU-, Windows-, Lüfter-, MUX- oder Unleashed-Settings bleibt als Ganzes nicht anwendbar. Die Beleuchtungsaktion überspringt solche Settings nicht. Änderungen am Entwurf sperren Apply bis zum erneuten Save und Check. Eine noch offene Recovery sperrt ein weiteres Apply.

## Herkunft und Fehlerzustände

**Firmware: nicht gelesen** bedeutet, dass für die aktuelle Ansicht kein gültiger GET-Snapshot vorliegt. Die Hexfelder und Farbfelder sind stets Entwurf oder Profilwerte. Ein GET-Snapshot ist eine Momentaufnahme der ausdrücklichen Prüfung; der Editor besitzt keinen periodischen Sensor- oder Beleuchtungstimer.

Profilwechsel und Speichern verwerfen den angezeigten Firmware-Snapshot. Vor Firmwareprüfung, Apply und Restore wird er bereits beim Start der Aktion entfernt. Nach UAC-Abbruch, Timeout oder einem Fehler bleibt daher kein älteres Farbbild als aktueller Hardwarezustand stehen. Nach erfolgreichem Apply/Restore zeigen die Ergebniszeilen den verifizierten Abschluss; eine neue Firmwareprüfung liest die Werte für die Zonenanzeige erneut.

Die Ergebnisprüfung verbindet Prozess-Exitcode, feste Operation und Anfrage-Nonce mit dem persistierten Protokoll. Windows-Abbruch, fehlende Capability, Datentyp-/Längenfehler, veränderte Steuerbilder und Profilblocker erscheinen im Statusfeld. Nach 45 Sekunden meldet die UI einen weiter laufenden Steuerprozess. Das Schließen des Fensters beendet einen bereits gestarteten Hardwareprozess nicht; dessen eigener Ablauf führt Prüfung und Wiederherstellung zu Ende. Ein offenes Journal bleibt die Grundlage der Recovery.

## Freigegebene Hardwarewirkung

RGB schreibt ein vollständiges 128-Byte-Farbbild, in dem nur die zwölf RGB-Bytes der vier Zonen geändert werden. Alle übrigen Farbbytes und das vollständige Helligkeitsbild müssen erhalten bleiben. Der getrennte Modusschalter ist ausschließlich für die gemessenen Rohzustände **0x64 / 0xE4** freigegeben: Firmware-Voreinstellung beziehungsweise eigene statische Farben. Er stellt keine Helligkeitsprozente bereit.

Am 14. Oktober 2026 bestätigten kurze UAC-Proben zunächst RGB und dann RGB plus Modus mit vollständigem Original-Restore. Ein weiterer Versuch verwendete isolierten dauerhaften Profilspeicher und getrennte Apply-/Recovery-Prozesse: vier Ein-Byte-Farbänderungen, gegensätzlicher Modus, vollständiger Ziel-Readback und Original-Restore sowie unveränderte GPU-Steuerungen. Die produktiven Journal-/Startup-Dateien blieben dabei unverändert. Messdetails und Grenzen stehen in [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md), [models/8e35.md](models/8e35.md) und [VALIDATION.md](VALIDATION.md).

Die Originaldienste liefen während dieser Versuche weiter. Reboot, Resume, Hotkeys, physische Animation und unabhängige Controller-Ownership sind damit nicht nachgewiesen. Beide Beleuchtungsfeatures bleiben `startup_safe = false`; automatische Wiederanwendung ist gesperrt. Helligkeitsprozente, Aus-/Animationsmodi und Einzeltasten-RGB besitzen keine freigegebenen Schreibadapter. Der [Ersatzplan](REPLACEMENT_PLAN.md) beschreibt die weiterhin nötige Arbeit zum vollständigen Ersatz des Original-Hubs.

## Native Form und überprüfte Darstellung

Dieses Fenster erweitert die vorhandene Win32-Oberfläche: **Segoe UI** mit GDI-Schrifthöhe `-18`, Standard-`BUTTON`, `EDIT`, `STATIC` und `COMBOBOX`, Windows-Farbdialog und Systemfarben. Der Hintergrund folgt `COLOR_BTNFACE`; die Rahmen der selbst gezeichneten RGB-Felder folgen `COLOR_WINDOWTEXT`. Die Farbfelder erhalten ihre Farbe ausschließlich aus dem Entwurf, nicht aus einer neuen Produktpalette. Profilwahl und Name stehen oben, vier gleichartige Zonenzeilen darunter, dann Moduswahl, Check/Copy/Save, Apply/Restore und das Statusfeld. Das aktuelle Fenster hat eine feste Größe; andere DPI-Einstellungen sind noch nicht umfassend geprüft.

Der native Client-Render `.impeccable/review/keyboard-editor.png` mit **764×741** Pixeln zeigt Layout, Labels, Buttons und Farbfelder. Die Methode bildet Text der verborgenen `EDIT`-/`COMBOBOX`-Controls nicht vollständig ab; solche Texte, Titlebar und weitere DPI-Stufen gehören nicht zu dieser visuellen Evidenz. Der abschließende Reviewer-Verdict bewertet die einzelne Korrektur der veralteten GET-Anzeige als **resolved**, Disposition **ship**; dies ist keine vollständige Fenster-/DPI-Abnahme. Die native Real-Control-Snapshotprüfung und der bestandene Tastatur-Metadaten-Smoke ergänzen die Verhaltensabdeckung ohne Hardware-Schreibzugriff. Alle vier nativen UI-Smokes bestanden mit dem finalen Release unter Windows PowerShell 5.1. Die dunkle Identität des separaten Kurveneditors wird durch dieses Fenster nicht verändert.
