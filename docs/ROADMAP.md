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
- [ ] Lüfter, Max-Fan und MUX: Get/Set-Paare, Rückgabecodes und Zustandsübergänge identifizieren.
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
- [ ] Beleuchtungssettings für automatisches Anwenden freigeben, erst nach belegtem Lifecycle und Controller-Ownership; derzeit `startup_safe = false`.
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
- [x] GPU-Watt und Temperatur zuerst, CPU-Auslastung, qualifizierten Windows-Takt und CPU-Hardwarewerte direkt danach vor Inventar/Profilkontrollen anzeigen.
- [x] CPU-Paketleistung mit Energie-Mittelwert/Intervall und Tctl über einen expliziten, sitzungsgebundenen UAC-Leseprozess anzeigen; Standardzustand bleibt nicht verfügbar, Pause verwirft Reader/Baseline, Rückkehr beginnt frisch.
- [x] Feste lokale CPU-IPC mit Benutzer-/SYSTEM-DACL, beiden Prozess-IDs, Sitzungs-Token, Schema/Sequenz und `Sample`/`Pause`/`Stop`; vorübergehender UAC-Launch und isoliertes begrenztes Inventar.
- [x] Deutsche Support-/Freigabe-/Verbindungs-/Messfehler und passende Wiederherstellung, getrennte letzte technische Ursache ohne Hardwarejournaländerung.
- [ ] Gemessene CPU-Boost-/Effektivtakte und CCD-/Kerntemperaturen anbinden; Windows-MHz und Tctl nicht dafür umdeuten.
- [ ] Ausgewählten UI-Entwurf nativ umsetzen; CPU und GPU gemeinsam im ersten Blickfeld, Profile danach.
- [x] Native GPU-Profile speichern, aktuellen Offset-Zustand anzeigen, anwenden und wiederherstellen; die HP-Systemmodi bleiben Vorschau.
- [x] Native V/F-Kurvenbearbeitung mit vorhandenem Spannungsanker, Plateau-Frequenzziel und Erhalt eigener vollständiger Kurven bei Name-/VRAM-Änderungen.
- [x] Gemeinsames natives Kurven-Widget mit GPU-V/F, CPU-/GPU-Lüfterentwürfen, Maus-/Tastatur-/Zahlenbedienung und eigener Profilspeicherung; Lüfter-Hardwareadapter bleibt offen.
- [x] Nativer Vier-Zonen-Beleuchtungseditor mit Hex-/Farbauswahl, Metadaten-Save, UAC-Firmwareprüfung, Gesamtprofil-Apply und profilgebundenem Original-Restore.
- [ ] Lüfterkurven auf Hardware anwenden, Max-Fan und MUX samt nötigem Neustartverhalten.
- [ ] Erweiterte Tastaturbeleuchtung mit validierter Helligkeit, Aus-/Animationszuständen und unabhängigem Power-/Hotkey-Lifecycle.
- [x] Versionierte Profile, atomarer Speicher außerhalb MSIX-AppData, kontrollierte Migration und geschützter Windows-Anmeldungseinstieg für bestätigte Revisionen.
- [ ] Tatsächlichen Reboot, Suspend/Resume, AC-Wechsel und langfristige Stabilität mit ausgewähltem Profil messen.
- [ ] Dienst nur für privilegierte Hardwarefunktionen; UI bleibt im Benutzerkonto.
- [ ] Release-Speicherbudget, CPU-Leerlauf, Startzeit und Binärgröße reproduzierbar messen.
- [x] Erstes Speicher-/CPU-Protokoll für den Diagnose-Release erstellen; aktuelles Tray-Ziel: 50 MB Working Set.
- [x] Optionale Buildvorbereitung aus signiertem öffentlichen PawnIO.Modules 0.2.11, Größen-/Hashprüfung und LGPL-Quellen/Lizenzbeilage; keine Downloads oder Treiberpakete in der Runtime.
- [ ] Hardwarematrix, Installationspaket, Abhängigkeitslizenzen und Wiederherstellungsanleitung.

Die fertiggestellte Anwendung soll keine Telemetrie, Hintergrund-Updates, Werbung, Overlays, Registrierung oder Netzwerkpflicht enthalten. Das Entfernen von HP-Komponenten ist eine spätere, getrennte Funktion mit Wiederherstellungsplan; der aktuelle Prototyp deinstalliert oder stoppt nichts.

Die CPU-Erweiterung erweitert die bestehende native Oberfläche code-first; sie führt keinen globalen Neuentwurf ein. Der Finish-Reviewer hat die einzelne Korrektur der deutschen Fehler-/Recoverytexte als `resolved` bewertet, Disposition `ship`, bei gültigen eigenen Client-/Footer-Recaptures. Aktive Fehler-/Messdarstellung, weitere DPI-Konfigurationen, vollständige Barrierefreiheit und reale Power-Übergänge sind damit nicht abgenommen. Aktuelle Release-, Test- und Speicherevidenz: [VALIDATION.md](VALIDATION.md).
