# Projektziel: Omen Hub++

Omen Hub++ soll alle praktisch nutzbaren OMEN-Hardwarefunktionen in einer nativen Rust-Anwendung bündeln und um CPU-/APU-Tuning ergänzen, wie es UXTU bietet. Vorbild für das Bedienkonzept ist G-Helper. Funktionsumfang und Schreibzugriffe werden pro Board, BIOS und CPU/GPU-Familie freigegeben.

## Phase 1: Reverse Engineering und Messbasis — begonnen

- [x] Hardware des ersten Zielgeräts und HP-WMI-Metadaten erfassen.
- [x] Installierte HP-Leistungs- und Lüftermodule mit ILSpy analysieren.
- [x] GPU-Status-/Power-Payload und Heartbeat-Einstieg statisch finden.
- [x] Eigenständige, lesende NVIDIA-Telemetrie und JSON-Diagnose erstellen.
- [x] CPU-Auslastung und qualifizierten Windows-Takt ohne zusätzlichen Treiber anbinden.
- [x] Statische CPU-Sensorpfade für 8940HX untersuchen; AMD-SDK-Init-/UnInit-Seiteneffekte und Installationsfallback erkennen und diese Runtime-Route ausschließen.
- [x] Tastaturmodellroute für 8E35, Beleuchtungs-GET-/SET-Verträge und Hintergrundaufgaben im installierten Hub untersuchen.
- [x] Native BIOS-WMI-Abbildung und virtuellen Lampentreiber mit Ghidra/PyGhidra-MCP statisch nachvollziehen.
- [x] Feste lesende Tastaturerfassung vorbereiten; unelevierter Fehler vor BIOS-Methodenaufruf nachgewiesen.
- [x] Tatsächliche Tastatur-Capability und vollständigen Farb-/Helligkeitszustand in PowerShell und nativem Rust lesen; 8E35/F.13 meldet vier Zonen mit Nummernblock.
- [x] Direkte Vier-Zonen-RGB- und statisch/Firmware-Modus-Setter auf 8E35/F.13 mit vollständigem Readback/Restore validieren; dauerhaftes Apply und Recovery in getrennten Prozessen prüfen.
- [ ] Tastatur-Lifecycle bei Reboot, Suspend/Resume und Hotkeys sowie Betrieb ohne ursprüngliche HP-Controller messen; Helligkeit/Aus/Animationen und andere Tastaturtypen separat qualifizieren.
- [ ] Notwendige Hersteller-Lifecycle-Funktionen in der eigenen Engine übernehmen und vollständigen Ersatz ohne originale HP-Omen-Controller nachweisen.
- [x] Eigene feste PawnIO-/AMD-RAPL-Leseoperationen auf 8E35/F.13/8940HX validieren: Paketenergie-Mittelwert mit Intervall, erste Baseline ohne Watt, ausdrücklich bezeichnetes Tctl, unveränderte HWCR-/GPU-Steuerbilder und vorhandene Treiberzustände.
- [ ] CPU-Reader bei tatsächlichem Standby/Resume, AC-Wechsel und längerem Sensorbetrieb prüfen; weitere CPU-/Board-/BIOS-/Treiberkombinationen jeweils separat qualifizieren.
- [x] Modern-Standby-Anmeldung, unabhängige Bildschirm-/Suspend-Pause und sichere Behandlung noch laufender Sensorantworten implementieren; lesende Lifecycle-Checkpoints mit echten Windows-/Boot-Belegen vorbereiten. Reale Abnahme und vollständiges Profil-Readback bleiben offen: [POWER_LIFECYCLE.md](POWER_LIFECYCLE.md).
- [ ] Beständigen CPU-Profilbetrieb und kontrollierte HP-Übernahme nachweisen: Am 24. Oktober fielen 70/75 W auf 66/80 W zurück; bei einmaliger ausdrücklich freigegebener HPOmenCap-Isolation am 25. Oktober hielten sechs GETs bis ungefähr zwölf Sekunden nach dem Apply beide Ziele. Vollständiger Restore bestätigt. Diensteinfluss damit gestützt, konkreter Setter/Alleinursache, laufender eigener Betrieb und Power-/Langzeitverhalten bleiben offen; kein automatischer Dienststopp und keine periodischen SETs als Umgehung.
- [x] Feste GPU-Status-/SystemDesignData-GETs auf 8E35/F.13 messen; D-State 0 bleibt ohne erfundene Bedeutung erhalten, Original-Hub laut Benutzer Unleashed.
- [ ] Bedeutung der tatsächlichen GPU-Status-Antwort auf 8E35/F.13 und Zuordnung zum Originalprofil klären.
- [ ] HSA/CAP-RPC bis zum tatsächlichen WMI-/EC-/Treiberaufruf vollständig verfolgen.
- [x] Zuordnete 8E35/Khalilah25C1-Ressource finden: PL1, TPP, Thermal-/IR-Parameter, 30000-ms-Heartbeat und Unleashed-Support; Laufzeitwirkung bleibt separat zu validieren.
- [x] Feste Lüfter-GETs auf 8E35/F.13 lesen: vollständiger Max-Fan-Schalter, CPU-/GPU-Drehzahlen, Zuordnung, gekoppelte Tabelle und rohe Zusatzsensorantwort.
- [x] IR-Selektor 0 auf 25C1 bis zur Celsius-Ausgabe verfolgen; gespeicherte Auto-Auswahl von Software-/BIOS-Zuständigkeit unterscheiden. Einmalige CPU-/GPU-/IR-/Lüfterdiagnose mit expliziten Alters-/Berechtigungsfehlern implementiert und gelesen.
- [x] Konkrete F.13-DSDT lesen: OMCC-Heartbeat, HPCM/FAMC und SRP-Level nachvollziehen; realen Rückfall ohne weitere Befehle nach ungefähr zwei Minuten beobachten. Direkte FAMC-Probe an der konservativen Temperaturgrenze beenden und vollständig zurückstellen; keine daraus abgeleitete Runtime-Freigabe.
- [x] Max-Fan-Get/Set-Paar ohne OMEN-DLL implementieren und manuelles Ein-/Readback-/Original-Restore sowie persistierte Recovery in getrennten Prozessen messen; HP-Dienste liefen weiter.
- [ ] Gekoppelte Leistungs-/BIOS-Lüfterzuständigkeit, zusätzliche thermische Sensoranforderung und vollständige Modus-Readback-/Restore-Semantik für eigene Kurven qualifizieren; Max-Fan ist kein Ersatzbeweis.
- [x] MUX: BIOS-Grafikmodus lesen/schreiben (Konfigurationsbefehl 0x52) als Preset-Einstellung und Dashboard-Schalter Hybrid/GPU only mit Rücklesen implementiert; der tatsächliche Wechsel nach Neustart ist auf 8E35 noch ungemessen.
- [ ] Originalzustände, Gaming-Hub-Verhalten und Lastmessungen vergleichen: Netzteil, BIOS, Temperaturen, CPU-Watt, GPU-Watt, Akkudrain und Throttle-Gründe.
- [ ] Verhalten nach Standby, Netzteilwechsel und Ausfall eines Controllers dokumentieren.

Abnahme: identifizierte Calls, belegte Payloads und modellbezogene Messungen; kein Raten von EC-Adressen. Keine fremden C#-Implementierungen übernehmen.

## Phase 2: Rust-Hardware-Engine — Policy-Grundlage vorhanden

- [x] Hardware-unabhängige Modelle und transparente Profilvorschau.
- [x] Lüfterinterpolation mit Behandlung fehlender/ungültiger Sensoren.
- [x] Heartbeat-Lifecycle mit Entschärfung bei AC-Wechsel, Suspend, Resume und Fehler.
- [x] Optionaler lesender CPU-Adapter mit exakter Identitäts-/Hashprüfung des bereits laufenden PawnIO 2.2.0 und signiertem AMDFamily17 0.2.11; query-only SCM, kein Installations-/Treiberstart- oder Tuningpfad.
- [ ] Ein einzelner privilegierter Controller mit exklusivem Gerätezugriff; keine konkurrierenden HP-/Drittanbieter-Schleifen.
- [x] Eigene Controller-Zustands-/Recovery-Engine mit dauerhaftem Write-ahead-Journal, festem HPOmenCap-Adapter, Prozess-Erzeugungsidentität und Konfliktsperre für neue CPU-Leistungsprofile vorbereiten; reine Geräte-Status-/Preflight-Prüfung ohne Dienstaktion bestanden. Native Übernahme wird noch nicht automatisch aktiviert: [HP_CONTROL.md](HP_CONTROL.md).
- [ ] Dedizierten Übernahmeprozess mit unabhängigem Rückkehrwächter und gekoppelter Kühlung beaufsichtigt qualifizieren; erst danach normalen UI-/Anmeldestart und echte Power-Übergänge freigeben.
- [x] Typisierter lesender HP-WMI-Transport mit festen GETs, Begrenzung auf 8E35/F.13 sowie strengen Datentyp-/Längen-/Rückgabecode-Prüfungen; Größe-0-Eingabe am Gerät validiert.
- [x] Typisierter direkter HP-BIOS-Tastaturadapter für Capability `[1,0,0,0]`: vollständige RGB-Bilder, exakte Modusbytes 0x64/0xE4, dauerhafte Originale, Readback und Recovery; übrige Steuerbytes erhalten.
- [x] Typisierter direkter Max-Fan-Adapter für 8E35/F.13: Netzbetrieb, Tabellenhash, Topologie, Thermalpolicy und vollständige Schalterbytes vor SET prüfen; boolesches Ziel, vollständiger Readback und persistierter Original-Restore, fremde Zustände verwerfen.
- [x] Beleuchtungssettings für den Anmeldestart freigegeben (16. Oktober 2026, Nutzerwunsch). Lifecycle bei Resume, Hotkeys und Controller-Ownership bleibt ungemessen.
- [ ] Max-Fan bei echten AC-, Fehler-, Standby-/Resume- und Reboot-Übergängen sowie ohne ursprüngliche HP-Controller prüfen; automatischer Profilstart ist seit 16. Oktober auf Nutzerwunsch erlaubt. Eingeschwungene maximale Nenndrehzahl und Dauerstabilität separat messen.
- [ ] Eigene Lüfterkurven laufen seit 16. Oktober über den Lüfterdienst (Dashboard-Schalter **Kurve**, Notfall-Vollkühlung, BIOS bei **Auto**); Verhalten unter Last, Modusübernahme und Lifecycle auf dem Gerät noch messen.
- [x] Typisierte Gesamt-Transaktionen mit 91 semantischen Profil-Features, dauerhaften Originalbildern und Sperren bei nicht unterstützten Settings. Formatbreite bedeutet keine Schreibfreigabe für alle Features.
- [x] NVIDIA-Core-/VRAM-Offsets auf dem Referenzgerät: GET/SET, exakter Readback, Teilfehler-Rollback und persistierte Recovery.
- [x] Exakte NVIDIA-Graphics-V/F-ABI mit Ghidra und lesenden Proben belegen; alle 127 Frequenz-Deltas schreiben, zurücklesen und vollständig restaurieren.
- [x] Kurvenziele von direkten mV-Offsets trennen; komplexe Preflight-Prüfung, einzelne immutable Spannungsanker und vollständige gespeicherte Kurven.
- [x] Gemessene VRAM-Setter-/Graphics-Kurven-Kopplung abbilden: Dependency-Reihenfolge, vollständige Originale und Erhalt der Graphics-Einstellungen.
- [ ] Physische Undervolting-Wirkung der V/F-Kurve und des Curve Optimizers sowie längere Stabilität unter Last messen. Direkte GPU-/VRAM-Spannung ist auf Nutzerwunsch verworfen.
- [ ] Zusätzliche Modelle, OEM-Power-/Voltage-/CPU-/Windows-Adapter jeweils mit eigener Gerätevalidierung.
- [ ] Modellbezogener Heartbeat; keine universell angenommene 30-Sekunden-Schleife.
- [x] CPU-Limits erneut qualifizieren: Fast über RSMU 0x56, Slow über 0xCB, TDC/EDC/Tctl mit vollständigem Readback/Restore. Die unzuverlässige unabhängige STAPM-Vorgabe ist entfernt; die frühere HP-Kopplung wird nicht als eigener CPU-Adapter weiterverwendet.
- [x] Alle UI-Texte in Sprachdateien (de/en) mit Info-Erklärung je Einstellung, siehe LOCALIZATION.md.
- [ ] Intel PL1/PL2 über einen geeigneten, signierten MSR-Backend; Privilegien/Driver-Support explizit.
- [ ] Undervolting nur bei belegter CPU-/Firmware-Unterstützung, mit eigenen Grenzen und Readback.
- [ ] NVAPI-Schreibfunktionen nur soweit öffentlich dokumentiert oder gesondert modellbezogen belegt.

Abnahme: Unleashed aktiviert den OEM-Leistungsrahmen; tatsächliche GPU-Leistung unter Last und korrekte Rückkehr aus Fehler-/Power-Zuständen belegen. Es wird weder ein universeller Wattwert noch ein VBIOS-Bypass versprochen.

## Phase 3: Oberfläche und Alltag — Diagnose-Prototyp vorhanden

- [x] Native Windows-Oberfläche mit Tray, Profilvorschau und Sensoren.
- [x] Sensortimer im Tray aussetzen; native Power-Events empfangen.
- [x] Funktionstest für Inventar, Profilvorschau, Close-to-Tray und Beenden auf dem Referenzgerät.
- [x] GPU-Watt und Temperatur zuerst, CPU-Auslastung, qualifizierten Windows-Takt und CPU-Hardwarewerte daneben im ersten Blickfeld vor Inventar/Profilkontrollen anzeigen.
- [x] CPU-Paketleistung mit Energie-Mittelwert/Intervall und Tctl über einen expliziten, sitzungsgebundenen UAC-Leseprozess anzeigen; Standardzustand bleibt nicht verfügbar, Pause verwirft Reader/Baseline, Rückkehr beginnt frisch.
- [x] Feste lokale CPU-IPC mit Benutzer-/SYSTEM-DACL, beiden Prozess-IDs, Sitzungs-Token, Schema/Sequenz und `Sample`/`Pause`/`Stop`; vorübergehender UAC-Launch und isoliertes begrenztes Inventar.
- [x] Deutsche Support-/Freigabe-/Verbindungs-/Messfehler und passende Wiederherstellung, getrennte letzte technische Ursache ohne Hardwarejournaländerung.
- [x] Effektive CPU-Takte aus Read-Only-APERF/MPERF aller 32 logischen Prozessoren auf 8E35/F.13 messen, mit eigener Baseline, Abdeckungs-/Intervallprüfung und Anzeige des höchsten Intervallmittels; Windows-MHz bleiben getrennt.
- [ ] Aktive CPU-Boost-/Spitzentakte und CCD-/Kerntemperaturen anbinden; Intervall-Effektivtakt und Tctl nicht dafür umdeuten.
- [x] CCD-Auswahl speichern und Enforcer mit echten Windows-Affinitätsaufrufen an einem eigenen Testprozess prüfen; beide CCDs und ursprüngliche Maske sowie reguläres Release bestätigt. Fremde Programme und reale Power-Übergänge bleiben getrennte Prüfungen.
- [x] ~~Fester CPU-Takt/feste Spannung~~: am 17. Oktober 2026 auf Nutzerwunsch verworfen, weil die meisten Laptops diese Kontrolle sperren; Editorfelder (AMD-Takt/-Spannung, Intel-Core-Ratios, Cache-/SA-Spannung) entfernt. Der Fokus liegt auf den UXTU-Einstellungen: STAPM/Slow/Fast, TDC/EDC, Tctl-Limit, PBO und Curve Optimizer.
- [x] Dashboard: **Anwenden**-Schaltfläche für den Leistungsmodus (Auswahl allein schreibt nichts), Einstellungen **Mit Windows starten** (Autostart im Tray) und **Beim Schließen in den Tray minimieren**.
- [x] Vom Benutzer bestätigte moderne, saubere dunkle Oberfläche nativ umsetzen: GPU und CPU gemeinsam im ersten Blickfeld, prominente Editoraktionen danach; gemeinsame Felder und Buttons in GPU-, Beleuchtungs- und Kurveneditor.
- [x] Dunkle Design-Tokens und native Control-/Darstellungsgrenzen in DESIGN.md, Sidecar und Surface-Briefs festhalten; Diagrammpalette und feste GPU-Anker behalten ihre eigene Regel.
- [x] Dashboard-Kindcontrols im Tray freigeben und bei Rückkehr mit der bisherigen HP-Vorschauauswahl neu aufbauen; CPU-Freigabe und neue Energie-Baseline unabhängig im Worker behalten.
- [x] Sichtbare NVIDIA-Lesesitzung auf einem wegwerfbaren Thread mit ursprünglicher FFI-/Library-Ownership, begrenztem Sample-Warten und gesundem Stop/Cleanup führen; Hersteller-Hang bleibt als gesonderter Störfall offen. CPU-Treiber-Hashprüfung bei identischer Begrenzung/Identität in 8-KiB-Blöcken streamen.
- [x] Custom Presets mit getrennten GPU-/CPU-/Windows-Bereichen und vollständiger Revision/Restore-Logik; vier HP-Defaults plus eigene Presets im Leistungsmodus. Native Proben für alle HP-Modi, Dynamic Boost, unabhängige RSMU-Limits, CO, PBO 1× und Windows-Steuerungen bestanden. Vollständiger Power-Lifecycle bleibt offen.
- [x] Native V/F-Kurvenbearbeitung mit vorhandenem Spannungsanker, Plateau-Frequenzziel und Erhalt eigener vollständiger Kurven bei Name-/VRAM-Änderungen.
- [x] Gemeinsames natives Kurven-Widget mit GPU-V/F, CPU-/GPU-Lüfterentwürfen, Maus-/Tastatur-/Zahlenbedienung und eigener Profilspeicherung; Hardwareadapter für eigene Lüfterkurven bleibt offen.
- [x] Max-Fan-Override sowie Kurven-/Max-Dienst kurz auf realer Hardware prüfen; vollständige Originale des Overrides und regelbedingte CPU/GPU/IR-Sicherheit. Anmeldefreigabe für geprüfte Profilrevisionen implementiert; kein daraus abgeleiteter Neustart-/Ownership-Nachweis.
- [x] Pause und regulären Stop der eigenen Lüfterregelung absichern: einmal volle Kühlung vor Ende der Befehle, explizit ausstehende Übergabe, keine erneuten SETs bei wiederholter Pause oder erkanntem Original-Hub.
- [x] Direkte Beleuchtung mit vier anklickbaren Zonen samt Nummernblock, nativem Farbwähler und Ein/Aus; eigener dauerhafter Transaktionsspeicher, voller Readback/Restore und vom Benutzer bestätigte sichtbare Ein/Aus-Wirkung.
- [x] Exakte CPU-Anzeigeeinheiten, konfliktprüfender Preset-Merge, archiviertes Löschen mit erhaltenem Hardwarejournal, abgelehnter Presetwechsel mit erhaltener Identität sowie feste Startup-Einrichtung nach vollem Preflight.
- [x] GPU-Chart aus neuen und geänderten Presets öffnen, Übernehmen/Abbrechen ohne Datei-Save; explizites Speichern bereinigt zehn alte GPU- und 25 APU/iGPU-Schlüssel. Beide Dynamic-Boost-Namen bleiben als eine sichtbare logische Vorgabe bearbeitbar, inklusive Abwahl und Konfliktschutz.
- [ ] Vollständigen Lüfter-/MUX-Lifecycle einschließlich BIOS-Ownership, Standby, AC und echtem Neustart qualifizieren. Kurve/Max besitzen eine konservative reale Funktionsprobe; MUX-Startvorgabe ist gesetzt/zurückgelesen/restauriert, der physische Wechsel bleibt offen.
- [ ] Erweiterte Tastaturbeleuchtung mit validierter Helligkeit, Animationen und unabhängigem Power-/Hotkey-Lifecycle; Ein/Aus ist auf 8E35/F.13 inzwischen bestätigt.
- [x] Versionierte Profile, atomarer Speicher außerhalb MSIX-AppData, kontrollierte Migration und geschützter Windows-Anmeldungseinstieg für bestätigte Revisionen.
- [ ] Tatsächlichen Reboot, Suspend/Resume, AC-Wechsel und langfristige Stabilität mit ausgewähltem Profil messen.
- [ ] Dienst nur für privilegierte Hardwarefunktionen; UI bleibt im Benutzerkonto.
- [ ] Release-Speicherbudget, CPU-Leerlauf, Startzeit und Binärgröße reproduzierbar messen.
- [x] Vollständiges 50-MB-Tray-Budget einschließlich aller residenten Helfer auch nach Sensoraktivierung prüfen: aktive Kurve und anschließende Pause bestanden; keine Working-Set-Trimmung. Buildbezogene Werte in VALIDATION.md.
- [x] Optionale Buildvorbereitung aus signiertem öffentlichen PawnIO.Modules 0.2.11, Größen-/Hashprüfung und LGPL-Quellen/Lizenzbeilage; keine Downloads oder Treiberpakete in der Runtime.
- [x] Hardwarematrix, Installationspaket mit Startmenü-/Desktopverknüpfung, Lizenztexte aller eingebundenen Rust-Abhängigkeiten und Wiederherstellungsanleitung ([HARDWARE_MATRIX.md](HARDWARE_MATRIX.md), [RECOVERY.md](RECOVERY.md)). Installation und Entfernen auf Windows noch nicht ausgeführt.
- [x] Eigenes App-Symbol (dunkle Kachel, Mint-Instrument, Schriftzug „Hub“) mit Versionsinformationen ohne zusätzliche Build-Abhängigkeit in die EXE eingebettet; Fenster, Taskleiste und Tray verwenden es. Darstellung bei verschiedenen DPI auf Windows noch zu prüfen.

Die fertiggestellte Anwendung soll keine Telemetrie, Hintergrund-Updates, Werbung, Overlays, Registrierung oder Netzwerkpflicht enthalten. Das Entfernen von HP-Komponenten ist eine spätere, getrennte Funktion mit Wiederherstellungsplan; der aktuelle Prototyp deinstalliert oder stoppt nichts.

Am 14. Oktober 2026 bestätigte der Benutzer die moderne, saubere dunkle Anwendung und erlaubte mehr als 50 MB bei geöffnetem Fenster. Das **50-MB-Tray-Budget zählt weiter alle verbleibenden Prozesse, auch den pausierten CPU-Reader**. Die Gestaltung bleibt code-first und vollständig nativ; reale Messungen, gespeicherte Entwürfe, HP-Vorschau und explizite UAC-Aktionen bleiben getrennt. Frühere abgegrenzte Finish-Verdicts für Kurven, Beleuchtung und CPU-Fehlertexte sind historische Evidenz, keine Abnahme des neuen gesamten Fensters. Aktuelle Clientcapturen und visuelle Grenzen stehen in den Surface-Briefs; buildbezogene Tests, aktueller Finish-Verdict und Speicherproben in [VALIDATION.md](VALIDATION.md). Weitere DPI-Konfigurationen, dynamischer High-Contrast-Wechsel, vollständige Barrierefreiheit und reale Power-Übergänge bleiben zu prüfen.

Der frühere dunkle Finish-Verdict lautet **ship / G1 resolved**: Die einzelne GPU-Helfer-/Capability-Textkorrektur verwendet gemeinsam 14 Pixel bei erhaltenen Fakten und Aktionsmaßen. Der Reviewer öffnete alle fünf gültigen Clientcapturen erneut; der Verdict bleibt auf diese Korrektur im ursprünglichen abgebildeten Clientumfang begrenzt. Aktive CPU-/Fehlerzustände, Titlebars, andere DPI-/High-Contrast-/Accessibility-Konfigurationen, reale Power-Übergänge und die spätere Telemetrie-/Hash-Lifetime-Änderung sind damit nicht abgenommen.

Die Max-Fan-Erweiterung erhielt am 15. Oktober im eigenen frischen Finish-Review **ship**, ohne materielle Korrekturen. Geprüft wurde der reale native GPU-Client mit 754×771 Pixeln; Dropdown im geöffneten Zustand, Titlebar, andere DPI und vollständige Barrierefreiheit bleiben außerhalb dieses Verdicts. Der Metadaten-Smoke prüft die drei Profilzustände, Dirty-/Apply-Sperre, Startup-Sperre und Erhalt aller 127 Custom-Kurvenpunkte ohne Hardwarewrites. Tatsächliche Max-Fan-Proben und die Grenzen zu Nenndrehzahl, Power-Lifecycle und vollständigem HP-Ersatz stehen getrennt in [FAN_CONTROL.md](FAN_CONTROL.md). Das bestehende DESIGN.md und sein Sidecar bleiben für diese gewöhnliche Erweiterung unverändert. Finale Tests und Speicherproben der aktuellen Binary stehen in [VALIDATION.md](VALIDATION.md); ältere Buildwerte werden nicht übertragen.
