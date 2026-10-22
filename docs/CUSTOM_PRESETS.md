# Leistungsprofile und Custom Presets

**Leistungsprofile** öffnet den nativen Tab **Custom Presets**, alternativ `presets-ui [id]`. `gpu-ui` bleibt ein kompatibler Einstieg. Der Editor bündelt gespeicherte Vorgaben; er ist kein Nachweis, dass alle angebotenen Parameter auf dem Gerät schreibbar sind.

## Bedienung

1. Ein gespeichertes Preset auswählen oder **Neu** wählen und einen Namen eingeben.
2. Gewünschte Gruppen aufklappen. **GPU**, **CPU** und **Windows** besitzen eigene Abschnittsüberschriften. Nur angehakte Einstellungen gehören zum Preset; ein nicht enthaltener Wert verändert diese Hardwarefunktion beim späteren Apply nicht.
3. **Speichern** sichert ausschließlich Metadaten. **Anwenden** wird erst für ein gespeichertes, vollständig unterstütztes Preset verfügbar und fordert gezielt Windows-UAC an. Ein einziges unqualifiziertes Setting sperrt das gesamte Apply, statt einen Teil anzuwenden.
4. **Bei Windows-Start aktivieren** ist erst nach erfolgreichem Apply genau dieser Revision und vollständiger Startup-Prüfung freigegeben. Die Aktion richtet bei Bedarf den festen geschützten Anmeldungseinstieg ein. Ein veralteter ausgewählter Eintrag erscheint ausdrücklich mit ungültiger Freigabe; er gilt nicht als aktiv validiert.
5. **Löschen** entfernt das Preset aus den Auswahllisten und archiviert seine exakten JSON-Bytes mit Hash unter `deleted-profiles`. Eine zugehörige Startup-Auswahl wird deaktiviert. Das Hardware-Recoveryjournal bleibt erhalten; Löschen ist kein Restore.

Wird ein Presetwechsel mit ungespeicherten Änderungen abgelehnt, bleiben Auswahl, Entwurf und Speicherziel beim bisher geladenen Preset. Eine angenommene Verwerfung lädt das gewählte Preset neu.

## Einstellungsgruppen

| Bereich | Gruppen und gespeicherte Optionen |
| --- | --- |
| GPU | GPU-/VRAM-Takt-Offsets, vollständige GPU-Leistungskurve, manuelles Max-Fan und die ausdrücklich gewünschte Dynamic-Boost-Vorgabe (Hardwareadapter noch ungeprüft); ein globaler Core-Offset ist bei aktiver individueller V/F-Kurve ausdrücklich gesperrt |
| CPU | CPU-Temperatur, Slow- und Fast-Limit (PPT), TDC/EDC, PBO, gemeinsamer oder einzelner Curve Optimizer sowie Intel-Leistungsgrenzen. Einzelnes STAPM, feste CPU-Takte und direkte Spannungen sind entfernt. |
| Windows | Eigener Abschnitt mit Prozessor-Boost, Min/Max, EPP, Core Parking und CCD-Affinität (der Hintergrunddienst pinnt das Vordergrundprogramm) sowie Energie-/Anzeigevorgaben |

Super Resolution und Radeon Graphics Options erscheinen wie gewünscht nicht im Editor. Das versionierte Datenformat behält ältere Feature-Schlüssel, damit bereits vorhandene Profile beim Bearbeiten nicht gekürzt werden. Der Katalog umfasst jetzt **91 typisierte Schlüssel**; das ist eine Formatbreite und keine Zahl funktionsfähiger Hardwareadapter.

**GPU-Leistungskurve → Bearbeiten …** öffnet den bestehenden nativen Chart auch für ein neues Preset ohne Namen und bei ungespeicherten Änderungen. Der eigene Chart bekommt ein validiertes vollständiges Kurvenmodell aus dem zuvor gelesenen GPU-Zustand und bearbeitet es im Speicher. **Übernehmen** kehrt mit dem Kurvenentwurf zurück; Name, VRAM- und andere ungespeicherte Werte bleiben erhalten. **Abbrechen** verwirft nur Änderungen dieses Chart-Aufrufs. Beide Aktionen schreiben keine Profil-Datei und keine Hardware. Erst **Speichern** im Preset persistiert die Kurve. Während der Chart offen ist, ist sein Besitzerfenster gegen parallele Bearbeitung gesperrt; danach kehrt der Fokus zurück. `gpu-curve-ui` bleibt als separater gespeicherter Kurveneditor verfügbar; **Lüfterkurve** öffnet weiterhin nur CPU-/GPU-Lüfterentwürfe.

Im GPU-Bereich fehlen direkte GPU-/VRAM-Spannungen und FPS-Limiter. Dynamic Boost ist als HP-PPAB-Vorgabe verfügbar; cTGP bleibt dabei erhalten. Der globale Core-Offset ist bei einer aktiven individuellen V/F-Kurve gesperrt. Legacy-Daten werden nur beim ausdrücklichen Speichern bereinigt: zehn entfernte GPU- und 25 APU-/iGPU-Schlüssel, einschließlich des nicht zuverlässig steuerbaren einzelnen STAPM-Reglers. Vollständige GPU-Kurven und andere versteckte Einstellungen bleiben erhalten. Aktuelle Steuerungsverträge und Nachweise: [SYSTEM_CONTROLS.md](SYSTEM_CONTROLS.md).

## Daten und Grenzen

Felddefinitionen und Layout sind getrennt: `app/preset_fields.rs` beschreibt Gruppen, Typen, Einheiten und Vorgaben; `app/preset_ui.rs` enthält native Interaktion und Hintergrundarbeit. `core/preset.rs` konvertiert Werte ohne Gleitkommarundung, zum Beispiel **45,125 W → 45125 mW**. Er führt unabhängig bearbeitete Felder mit der frisch gelesenen Profilrevision zusammen und weist gleichzeitige Konflikte zurück. Unveränderte vollständige GPU-Kurven und nicht sichtbare Settings bleiben erhalten. Geräte-/Treiberbindungen werden nicht blind auf eine andere GPU übertragen.

Eingabefelder zeigen Entwurfsvorgaben, keine Messwerte. Nicht unterstützte Einstellungen blockieren das gesamte Apply. Qualifizierte privilegierte Funktionen können im unelevierten Editor gewählt werden; erst der feste erhöhte Prozess erfasst und prüft sämtliche vollständigen Originale vor dem ersten SET. Allein Speichern autorisiert keinen Anmeldestart. Windows-Energiemodus benötigt den Plan Ausbalanciert, der im selben Preset gewählt werden kann. Bei der Anmeldung gelten weiterhin Geräte-/Treiberbindung und die exakt autorisierte Profilrevision; ein realer Neustartnachweis steht aus.

Anwendung und Original-Restore verwenden die vorhandene atomare Transaktionsengine, keinen frei programmierbaren Kernel-/Kommandoweg. Ein offenes Journal schaltet den Apply-Button auf **Wiederherstellen**. Die geschützte Anmeldeaufgabe wird nur nach ausdrücklicher Freigabe eingerichtet; sie startet ausschließlich die feste eigene Binary mit `profile-startup-apply`. Sie enthält weder Netzwerkaktion noch Hintergrund-Updater. Reale Neustart- und Power-Lifecycle-Proben bleiben offen.

Die Settings-Ansicht unterstützt Mausrad, Seiten-/Anfang-/Ende-Nachrichten, Ziehen des dunklen Scrollgriffs und automatisches Nachscrollen fokussierter nativer Felder. Name, Auswahl und untere Aktionen bleiben außerhalb des scrollbaren Bereichs. Die Checkboxen behalten ihren nativen Typ, Namen, Auto-Toggle und Tastaturverhalten; die Zeichnung folgt dem dunklen System. Geprüft sind Clientgrößen **1084×861** und **904×681** bei 96 DPI, keine umfassende DPI-/Assistive-Tech-Zertifizierung.

Bedien- und Speicherprüfungen: `scripts/smoke-presets-ui.ps1`, `scripts/smoke-controls-navigation.ps1` und `scripts/smoke-preset-curve-ui.ps1`. Letzterer öffnet den Chart über das echte Preset-Control, prüft neue/dirty Presets, Cancel/Übernehmen/Reopen und Save sowie zehn entfernte GPU- und 26 APU/iGPU-Schlüssel ohne Hardwareänderung und mit Erhalt beider Boost-Schlüssel. Hardwareverträge und tatsächliche Messergebnisse: [GPU_PROFILES.md](GPU_PROFILES.md), [FAN_CONTROL.md](FAN_CONTROL.md), [VALIDATION.md](VALIDATION.md).
