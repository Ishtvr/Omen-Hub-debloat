# Leistungsprofile und Custom Presets

**Leistungsprofile** öffnet den nativen Tab **Custom Presets**, alternativ `presets-ui [id]`. `gpu-ui` bleibt ein kompatibler Einstieg. Der Editor bündelt gespeicherte Vorgaben; er ist kein Nachweis, dass alle angebotenen Parameter auf dem Gerät schreibbar sind.

## Bedienung

1. Ein gespeichertes Preset auswählen oder **Neu** wählen und einen Namen eingeben.
2. Gewünschte Gruppen aufklappen. **GPU** ist visuell von **CPU und System** getrennt. Nur angehakte Einstellungen gehören zum Preset; ein nicht enthaltener Wert verändert diese Hardwarefunktion beim späteren Apply nicht.
3. **Speichern** sichert ausschließlich Metadaten. **Anwenden** wird erst für ein gespeichertes, vollständig unterstütztes Preset verfügbar und fordert gezielt Windows-UAC an. Ein einziges unqualifiziertes Setting sperrt das gesamte Apply, statt einen Teil anzuwenden.
4. **Bei Windows-Start aktivieren** ist erst nach erfolgreichem Apply genau dieser Revision und vollständiger Startup-Prüfung freigegeben. Die Aktion richtet bei Bedarf den festen geschützten Anmeldungseinstieg ein. Ein veralteter ausgewählter Eintrag erscheint ausdrücklich mit ungültiger Freigabe; er gilt nicht als aktiv validiert.
5. **Löschen** entfernt das Preset aus den Auswahllisten und archiviert seine exakten JSON-Bytes mit Hash unter `deleted-profiles`. Eine zugehörige Startup-Auswahl wird deaktiviert. Das Hardware-Recoveryjournal bleibt erhalten; Löschen ist kein Restore.

Wird ein Presetwechsel mit ungespeicherten Änderungen abgelehnt, bleiben Auswahl, Entwurf und Speicherziel beim bisher geladenen Preset. Eine angenommene Verwerfung lädt das gewählte Preset neu.

## Einstellungsgruppen

| Bereich | Gruppen und gespeicherte Optionen |
| --- | --- |
| GPU | NVIDIA-Takt-/VRAM-Offsets, GPU-Leistungskurve, Dynamic Boost, Power-/Temperatur-/Spannungsvorgaben, Leistungsmodus, Max-Fan |
| CPU | Temperatur, Leistungslimits (PPT/STAPM), Stromlimits/VRM, Precision Boost Overdrive und Boost-Profil, Curve Optimizer, zwei CCD-Gruppen mit jeweils acht Kernwerten, Takt/Spannung und Intel-Vorgaben |
| System | Windows-Prozessorleistung (Boost, Min/Max, EPP, Core Parking), Energieschema/-modus, Display-Bildwiederholrate, MUX und CCD-Affinität |

Super Resolution und Radeon Graphics Options erscheinen wie gewünscht nicht im Editor. Das versionierte Datenformat behält ältere Feature-Schlüssel, damit bereits vorhandene Profile beim Bearbeiten nicht gekürzt werden. Der Katalog umfasst jetzt **91 typisierte Schlüssel**; das ist eine Formatbreite und keine Zahl funktionsfähiger Hardwareadapter.

**GPU-Leistungskurve → Bearbeiten …** öffnet nur den GPU-Kurveneditor für das gespeicherte Preset. Ein ungespeicherter Entwurf muss zuerst gespeichert werden. Nach der Rückkehr werden gespeicherte Änderungen neu geladen. Der Dashboard-Einstieg **Lüfterkurve** bietet nur CPU-/GPU-Lüfterentwürfe. Die bestehende kombinierte CLI-Workbench `curve-ui` bleibt verfügbar. Kurvenbearbeitung und Speicherung sind Metadatenaktionen.

Der UI-Name **Dynamic Boost** bezeichnet die bisherige `gpu_unleashed`-Profiloption; der Serialisierungsschlüssel bleibt kompatibel. Der gekoppelten HP-Leistungssteuerung fehlt weiterhin eine vollständige Modell-/Lifecycle-Qualifikation. Die Umbenennung aktiviert weder Unleashed noch einen versprochenen TGP. **Quiet (Default)**, **Balanced (Default)** und **Performance (Default)** stehen im Dashboard unter **Leistungsmodus**, gefolgt von den eigenen Presets. Die drei Defaults bleiben dort ausdrücklich nicht verfügbar. Eine bestätigte eigene Presetauswahl startet dagegen den festen Gesamt-Apply; bloßes Öffnen oder Wiederaufbauen des Dropdowns schreibt nichts.

## Daten und Grenzen

Felddefinitionen und Layout sind getrennt: `app/preset_fields.rs` beschreibt Gruppen, Typen, Einheiten und Vorgaben; `app/preset_ui.rs` enthält native Interaktion und Hintergrundarbeit. `core/preset.rs` konvertiert Werte ohne Gleitkommarundung, zum Beispiel **45,125 W → 45125 mW**. Er führt unabhängig bearbeitete Felder mit der frisch gelesenen Profilrevision zusammen und weist gleichzeitige Konflikte zurück. Unveränderte vollständige GPU-Kurven und nicht sichtbare Settings bleiben erhalten. Geräte-/Treiberbindungen werden nicht blind auf eine andere GPU übertragen.

Eingabefelder zeigen beim Erstellen deaktivierte Entwurfsvorgaben, keine gemessenen CPU-/GPU-Sensorwerte. Nicht unterstützte Adapter tragen **Noch nicht verfügbar**. CPU-SMU/Intel-Tuning, Windows-Energie-Tuning, MUX, Dynamic Boost, direkte GPU-/VRAM-Spannung und eigene Lüfterkurven besitzen noch keine Schreibfreigabe. Bereits qualifizierte Core-/VRAM-Takt-Offsets, vollständige V/F-Frequenzkurven und manuelles Max-Fan bleiben an ihre bestehenden Grenzen gebunden. Beleuchtung wird im eigenen direkten Fenster bedient; vorhandene Beleuchtungssettings bleiben beim Preset-Save erhalten und sperren weiterhin Startup.

Anwendung und Original-Restore verwenden die vorhandene atomare Transaktionsengine, keinen frei programmierbaren Kernel-/Kommandoweg. Ein offenes Journal schaltet den Apply-Button auf **Wiederherstellen**. Die geschützte Anmeldeaufgabe wird nur nach ausdrücklicher Freigabe eingerichtet; sie startet ausschließlich die feste eigene Binary mit `profile-startup-apply`. Sie enthält weder Netzwerkaktion noch Hintergrund-Updater. Reale Neustart- und Power-Lifecycle-Proben bleiben offen.

Die Settings-Ansicht unterstützt Mausrad, Seiten-/Anfang-/Ende-Nachrichten, Ziehen des dunklen Scrollgriffs und automatisches Nachscrollen fokussierter nativer Felder. Name, Auswahl und untere Aktionen bleiben außerhalb des scrollbaren Bereichs. Die Checkboxen behalten ihren nativen Typ, Namen, Auto-Toggle und Tastaturverhalten; die Zeichnung folgt dem dunklen System. Geprüft sind Clientgrößen **1084×861** und **904×681** bei 96 DPI, keine umfassende DPI-/Assistive-Tech-Zertifizierung.

Bedien- und Speicherprüfungen: `scripts/smoke-presets-ui.ps1` und `scripts/smoke-controls-navigation.ps1`. Hardwareverträge und tatsächliche Messergebnisse: [GPU_PROFILES.md](GPU_PROFILES.md), [FAN_CONTROL.md](FAN_CONTROL.md), [VALIDATION.md](VALIDATION.md).
