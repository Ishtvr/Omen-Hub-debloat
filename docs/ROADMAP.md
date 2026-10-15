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
- [x] Feste GPU-Status-/SystemDesignData-GETs auf 8E35/F.13 messen; D-State 0 bleibt ohne erfundene Bedeutung erhalten, Original-Hub laut Benutzer Unleashed.
- [ ] Bedeutung der tatsächlichen GPU-Status-Antwort auf 8E35/F.13 und Zuordnung zum Originalprofil klären.
- [ ] HSA/CAP-RPC bis zum tatsächlichen WMI-/EC-/Treiberaufruf vollständig verfolgen.
- [x] Zuordnete 8E35/Khalilah25C1-Ressource finden: PL1, TPP, Thermal-/IR-Parameter, 30000-ms-Heartbeat und Unleashed-Support; Laufzeitwirkung bleibt separat zu validieren.
- [x] Feste Lüfter-GETs auf 8E35/F.13 lesen: vollständiger Max-Fan-Schalter, CPU-/GPU-Drehzahlen, Zuordnung, gekoppelte Tabelle und rohe Zusatzsensorantwort.
- [x] IR-Selektor 0 auf 25C1 bis zur Celsius-Ausgabe verfolgen; gespeicherte Auto-Auswahl von Software-/BIOS-Zuständigkeit unterscheiden. Einmalige CPU-/GPU-/IR-/Lüfterdiagnose mit expliziten Alters-/Berechtigungsfehlern implementiert und gelesen.
- [x] Max-Fan-Get/Set-Paar ohne OMEN-DLL implementieren und manuelles Ein-/Readback-/Original-Restore sowie persistierte Recovery in getrennten Prozessen messen; HP-Dienste liefen weiter.
- [ ] Gekoppelte Leistungs-/BIOS-Lüfterzuständigkeit, zusätzliche thermische Sensoranforderung und vollständige Modus-Readback-/Restore-Semantik für eigene Kurven qualifizieren; Max-Fan ist kein Ersatzbeweis.
- [ ] MUX: Get/Set-Paare, Rückgabecodes und Zustandsübergänge identifizieren.
- [ ] Originalzustände, Gaming-Hub-Verhalten und Lastmessungen vergleichen: Netzteil, BIOS, Temperaturen, CPU-Watt, GPU-Watt, Akkudrain und Throttle-Gründe.
- [ ] Verhalten nach Standby, Netzteilwechsel und Ausfall eines Controllers dokumentieren.

Abnahme: identifizierte Calls, belegte Payloads und modellbezogene Messungen; kein Raten von EC-Adressen. Keine fremden C#-Implementierungen übernehmen.

## Phase 2: Rust-Hardware-Engine — Policy-Grundlage vorhanden

- [x] Hardware-unabhängige Modelle und transparente Profilvorschau.
- [x] Lüfterinterpolation mit Behandlung fehlender/ungültiger Sensoren.
- [x] Heartbeat-Lifecycle mit Entschärfung bei AC-Wechsel, Suspend, Resume und Fehler.
- [x] Optionaler lesender CPU-Adapter mit exakter Identitäts-/Hashprüfung des bereits laufenden PawnIO 2.2.0 und signiertem AMDFamily17 0.2.11; query-only SCM, kein Installations-/Treiberstart- oder Tuningpfad.
- [ ] Ein einzelner privilegierter Controller mit exklusivem Gerätezugriff; keine konkurrierenden HP-/Drittanbieter-Schleifen.
- [x] Typisierter lesender HP-WMI-Transport mit festen GETs, Begrenzung auf 8E35/F.13 sowie strengen Datentyp-/Längen-/Rückgabecode-Prüfungen; Größe-0-Eingabe am Gerät validiert.
- [x] Typisierter direkter HP-BIOS-Tastaturadapter für Capability `[1,0,0,0]`: vollständige RGB-Bilder, exakte Modusbytes 0x64/0xE4, dauerhafte Originale, Readback und Recovery; übrige Steuerbytes erhalten.
- [x] Typisierter direkter Max-Fan-Adapter für 8E35/F.13: Netzbetrieb, Tabellenhash, Topologie, Thermalpolicy und vollständige Schalterbytes vor SET prüfen; boolesches Ziel, vollständiger Readback und persistierter Original-Restore, fremde Zustände verwerfen.
- [ ] Beleuchtungssettings für automatisches Anwenden freigeben, erst nach belegtem Lifecycle und Controller-Ownership; derzeit `startup_safe = false`.
- [ ] Max-Fan bei echten AC-, Fehler-, Standby-/Resume- und Reboot-Übergängen sowie ohne ursprüngliche HP-Controller prüfen; automatischer Profilstart bleibt `startup_safe = false`. Eingeschwungene maximale Nenndrehzahl und Dauerstabilität separat messen.
- [ ] Eigene Lüfterkurven erst mit qualifizierter Modusübernahme/Restore und koordiniertem thermischem Sicherheitsweg anwenden; Software-Level und zustandswirksamen FanCount-/Heartbeat-Pfad separat qualifizieren.
- [x] Typisierte Gesamt-Transaktionen mit 85 semantischen Profil-Features, dauerhaften Originalbildern und Sperren bei nicht unterstützten Settings.
- [x] NVIDIA-Core-/VRAM-Offsets auf dem Referenzgerät: GET/SET, exakter Readback, Teilfehler-Rollback und persistierte Recovery.
- [x] Exakte NVIDIA-Graphics-V/F-ABI mit Ghidra und lesenden Proben belegen; alle 127 Frequenz-Deltas schreiben, zurücklesen und vollständig restaurieren.
- [x] Kurvenziele von direkten mV-Offsets trennen; komplexe Preflight-Prüfung, einzelne immutable Spannungsanker und vollständige gespeicherte Kurven.
- [x] Gemessene VRAM-Setter-/Graphics-Kurven-Kopplung abbilden: Dependency-Reihenfolge, vollständige Originale und Erhalt der Graphics-Einstellungen.
- [ ] Physische Undervolting-Wirkung und längere Stabilität unter Last messen; direkte GPU-/VRAM-Spannung nur mit eigenem belegtem Steuervertrag.
- [ ] Zusätzliche Modelle, OEM-Power-/Voltage-/CPU-/Windows-Adapter jeweils mit eigener Gerätevalidierung.
- [ ] Modellbezogener Heartbeat; keine universell angenommene 30-Sekunden-Schleife.
- [ ] `libryzenadj` via dynamischer FFI: STAPM, Fast/Slow PPT, Temperatur; Familien-Support vorher prüfen.
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
- [ ] Gemessene CPU-Boost-/Effektivtakte und CCD-/Kerntemperaturen anbinden; Windows-MHz und Tctl nicht dafür umdeuten.
- [x] Vom Benutzer bestätigte moderne, saubere dunkle Oberfläche nativ umsetzen: GPU und CPU gemeinsam im ersten Blickfeld, prominente Editoraktionen danach; gemeinsame Felder und Buttons in GPU-, Beleuchtungs- und Kurveneditor.
- [x] Dunkle Design-Tokens und native Control-/Darstellungsgrenzen in DESIGN.md, Sidecar und Surface-Briefs festhalten; Diagrammpalette und feste GPU-Anker behalten ihre eigene Regel.
- [x] Dashboard-Kindcontrols im Tray freigeben und bei Rückkehr mit der bisherigen HP-Vorschauauswahl neu aufbauen; CPU-Freigabe und neue Energie-Baseline unabhängig im Worker behalten.
- [x] Sichtbare NVIDIA-Lesesitzung auf einem wegwerfbaren Thread mit ursprünglicher FFI-/Library-Ownership, begrenztem Sample-Warten und gesundem Stop/Cleanup führen; Hersteller-Hang bleibt als gesonderter Störfall offen. CPU-Treiber-Hashprüfung bei identischer Begrenzung/Identität in 8-KiB-Blöcken streamen.
- [x] Native GPU-Profile speichern, aktuellen Offset-Zustand anzeigen, anwenden und wiederherstellen; die HP-Systemmodi bleiben Vorschau.
- [x] Native V/F-Kurvenbearbeitung mit vorhandenem Spannungsanker, Plateau-Frequenzziel und Erhalt eigener vollständiger Kurven bei Name-/VRAM-Änderungen.
- [x] Gemeinsames natives Kurven-Widget mit GPU-V/F, CPU-/GPU-Lüfterentwürfen, Maus-/Tastatur-/Zahlenbedienung und eigener Profilspeicherung; Hardwareadapter für eigene Lüfterkurven bleibt offen.
- [x] **Max-Fan im Profil: Nicht ändern / Ein / Aus** im nativen GPU-Editor: nur Metadaten speichern, Apply bei ungespeicherten Änderungen sperren, vollständige Custom-GPU-Kurve erhalten, ausdrückliches UAC-Apply/Restore; Startup mit Max-Fan sperren.
- [x] Nativer Vier-Zonen-Beleuchtungseditor mit Hex-/Farbauswahl, Metadaten-Save, UAC-Firmwareprüfung, Gesamtprofil-Apply und profilgebundenem Original-Restore.
- [ ] Lüfterkurven auf Hardware anwenden und MUX samt nötigem Neustartverhalten qualifizieren; Max-Fan-Lifecycle bleibt eine eigene offene Prüfung.
- [ ] Erweiterte Tastaturbeleuchtung mit validierter Helligkeit, Aus-/Animationszuständen und unabhängigem Power-/Hotkey-Lifecycle.
- [x] Versionierte Profile, atomarer Speicher außerhalb MSIX-AppData, kontrollierte Migration und geschützter Windows-Anmeldungseinstieg für bestätigte Revisionen.
- [ ] Tatsächlichen Reboot, Suspend/Resume, AC-Wechsel und langfristige Stabilität mit ausgewähltem Profil messen.
- [ ] Dienst nur für privilegierte Hardwarefunktionen; UI bleibt im Benutzerkonto.
- [ ] Release-Speicherbudget, CPU-Leerlauf, Startzeit und Binärgröße reproduzierbar messen.
- [x] Erstes Speicher-/CPU-Protokoll für den Diagnose-Release erstellen; aktuelles Tray-Ziel: 50 MB Working Set.
- [x] Optionale Buildvorbereitung aus signiertem öffentlichen PawnIO.Modules 0.2.11, Größen-/Hashprüfung und LGPL-Quellen/Lizenzbeilage; keine Downloads oder Treiberpakete in der Runtime.
- [ ] Hardwarematrix, Installationspaket, Abhängigkeitslizenzen und Wiederherstellungsanleitung.

Die fertiggestellte Anwendung soll keine Telemetrie, Hintergrund-Updates, Werbung, Overlays, Registrierung oder Netzwerkpflicht enthalten. Das Entfernen von HP-Komponenten ist eine spätere, getrennte Funktion mit Wiederherstellungsplan; der aktuelle Prototyp deinstalliert oder stoppt nichts.

Am 14. Oktober 2026 bestätigte der Benutzer die moderne, saubere dunkle Anwendung und erlaubte mehr als 50 MB bei geöffnetem Fenster. Das **50-MB-Tray-Budget zählt weiter alle verbleibenden Prozesse, auch den pausierten CPU-Reader**. Die Gestaltung bleibt code-first und vollständig nativ; reale Messungen, gespeicherte Entwürfe, HP-Vorschau und explizite UAC-Aktionen bleiben getrennt. Frühere abgegrenzte Finish-Verdicts für Kurven, Beleuchtung und CPU-Fehlertexte sind historische Evidenz, keine Abnahme des neuen gesamten Fensters. Aktuelle Clientcapturen und visuelle Grenzen stehen in den Surface-Briefs; buildbezogene Tests, aktueller Finish-Verdict und Speicherproben in [VALIDATION.md](VALIDATION.md). Weitere DPI-Konfigurationen, dynamischer High-Contrast-Wechsel, vollständige Barrierefreiheit und reale Power-Übergänge bleiben zu prüfen.

Der frühere dunkle Finish-Verdict lautet **ship / G1 resolved**: Die einzelne GPU-Helfer-/Capability-Textkorrektur verwendet gemeinsam 14 Pixel bei erhaltenen Fakten und Aktionsmaßen. Der Reviewer öffnete alle fünf gültigen Clientcapturen erneut; der Verdict bleibt auf diese Korrektur im ursprünglichen abgebildeten Clientumfang begrenzt. Aktive CPU-/Fehlerzustände, Titlebars, andere DPI-/High-Contrast-/Accessibility-Konfigurationen, reale Power-Übergänge und die spätere Telemetrie-/Hash-Lifetime-Änderung sind damit nicht abgenommen.

Die Max-Fan-Erweiterung erhielt am 15. Oktober im eigenen frischen Finish-Review **ship**, ohne materielle Korrekturen. Geprüft wurde der reale native GPU-Client mit 754×771 Pixeln; Dropdown im geöffneten Zustand, Titlebar, andere DPI und vollständige Barrierefreiheit bleiben außerhalb dieses Verdicts. Der Metadaten-Smoke prüft die drei Profilzustände, Dirty-/Apply-Sperre, Startup-Sperre und Erhalt aller 127 Custom-Kurvenpunkte ohne Hardwarewrites. Tatsächliche Max-Fan-Proben und die Grenzen zu Nenndrehzahl, Power-Lifecycle und vollständigem HP-Ersatz stehen getrennt in [FAN_CONTROL.md](FAN_CONTROL.md). Das bestehende DESIGN.md und sein Sidecar bleiben für diese gewöhnliche Erweiterung unverändert. Finale Tests und Speicherproben der aktuellen Binary stehen in [VALIDATION.md](VALIDATION.md); ältere Buildwerte werden nicht übertragen.
