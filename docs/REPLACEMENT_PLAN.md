# Vollständiger Ersatz des OMEN Gaming Hub

Der Benutzer hat am 13. Oktober 2026 klargestellt: **Omen Hub++ soll den originalen Hub vollständig ersetzen.** Die Zielengine soll keine laufenden HP-Omen-Controller oder deren proprietäre RPC-Wrapper benötigen. Windows-, Firmware- und reguläre Gerätetreiber bleiben technische Plattformgrundlagen; ihre Entfernung ist kein Nachweis für einen vollständigen Ersatz.

Dieser Plan trennt den eigenen Diagnose-/Steuertransport vom noch unbewiesenen vollständigen Ersatzbetrieb. NVIDIA-Takt-/Kurvensteuerung, begrenzte Vier-Zonen-Farben-/Modussteuerung und der manuelle Max-Fan-Override sind am Referenzgerät mit Readback/Restore und persistierter Recovery validiert. Herstellerdienste wurden nicht gestoppt und keine Treiber installiert. Der vorhandene Windows-/Firmwarepfad wurde mit weiterhin laufenden HP-Controllern geprüft.

| Bereich | Eigener Weg | Stand / fehlender Nachweis |
| --- | --- | --- |
| Systeminventar / Netzteil / Akku | Windows-WMI und Power-API | Implementiert |
| GPU-Watt / Temperatur / Takt | Öffentliche NVML / NVAPI | Implementiert; regulärer NVIDIA-Treiber erforderlich |
| CPU-Auslastung / Windows-MHz | Windows-Zeitdifferenzen / Power-API | Implementiert; kein Watt-/Boost-Ersatzwert |
| CPU-Paketwatt / Temperatur | Eigener fester PawnIO-Leseadapter, öffentliche AMD-Energiezähler und Tctl | Auf HP8E35/F.13/8940HX mit bereits laufendem PawnIO2.2.0 und signiertem Modul0.2.11 gemessen; expliziter UAC-Leseprozess, frische Baseline nach Pause; andere Geräte/Versionen offen |
| Tastaturfarbe / Anzeigemodus | Eigener BIOS-WMI-Transport | Vier Zonen und statisch ein/aus auf 8E35/F.13 qualifiziert; neue direkte Zonen-UI mit Readback/Restore, Ein/Aus vom Benutzer sichtbar bestätigt; Helligkeit/Animationen und unabhängiger Lifecycle offen |
| Tastatureffekte / RGB pro Taste | Fähigkeitsabhängig WMI oder eigener HID-Adapter | Get-/Set-/Persistenzunterschiede gefunden; kein vollständiger Restore-Vertrag für alle Varianten |
| HP-Leistungsprofile / GPU-Leistungsrahmen | Modellbezogene BIOS-WMI-Engine | Statische Befunde und begrenzter Statusreader; keine Write-Freigabe |
| Keep-alive / AC / Standby | Ein eigener Controller mit geprüftem Lifecycle | Reine Policy und Windows-Ereignisse vorhanden; Firmware-Heartbeat noch nicht ausgeführt/validiert |
| Lüfterzustand / Max-Fan | Eigener direkter BIOS-WMI-Adapter | GETs, manueller Override und vollständiger Readback/Original-Restore mit persistierter Recovery auf 8E35/F.13 qualifiziert; Netzbetrieb erforderlich, Startup gesperrt; maximale Nenndrehzahl und unabhängiger Lifecycle offen |
| Eigene Lüfterkurven / BIOS-Automatik | Koordinierter modellbezogener Thermikcontroller | Kurvenentwürfe und Policy vorhanden; Leistungs-/Lüfterzuständigkeit, Modus-Readback/Restore, zusätzliche thermische Sensoranforderung und Grenzwerte noch nicht qualifiziert |
| MUX / Displaymodi / Tastaturmakros | Eigene validierte Geräteadapter | Weitere Analyse erforderlich; bisher keine Steuerung |
| Profilpersistenz / Tray / Autostart | Eigene lokale Konfiguration und Benutzer-App | Versionierte Profile, dauerhaftes Originaljournal und geschützter fester Anmeldungseinstieg implementiert; Task mit GPU-Profil geprüft, kein echter Reboot; Beleuchtungssettings und Max-Fan nicht für Startup freigegeben |

## Welche heutigen Aufgaben übernommen werden müssen

Die HP-Hintergrundmodule enthalten mehr als die sichtbaren Schalter. Beleuchtung wird nach Profilwechseln oder Entsperren erneut angewendet; animierte Varianten reagieren auch auf Resume, Displayzustand und Hotkeys. Leistungskontrolle enthält Plattformbedingungen und Heartbeat-Verhalten. CPU/GPU-Verteilung und Thermik sind Teil dieser Bedingungen. Ein vollständiger Ersatz braucht diese belegten Abläufe in einer eigenen Engine, zusätzlich zu den einzelnen Gerätebefehlen.

Die Hersteller-Softwarekurven koordinieren CPU-, GPU- und zusätzliche IR-/Oberflächensensoranforderungen, Glättung/Hysterese und modellabhängige Grenzen. Eigene CPU-/GPU-Entwürfe ersetzen diesen Sicherheitsweg noch nicht. Der qualifizierte Max-Fan-Schalter übernimmt weder diese Regelung noch den gekoppelten Leistungs-/BIOS-Lüftermodus; sein Aus-Zustand gibt nur den Override frei. Details: [FAN_CONTROL.md](FAN_CONTROL.md).

Die relevante eigene Architektur besteht aus einem serialisierten Controller, einem Zustandsmodell und typisierten Plattformadaptern. Vor einem Apply werden Modell/BIOS, Capabilities, Versorgung und Originalzustand geprüft. Nach einem Apply wird tatsächlich gelesen; bei Teilfehlern wird der belegte Ausgangszustand wiederhergestellt. Resume und Geräteverlust invalidieren alte Handles und Zustandsannahmen. Normale Getter und ein als Heartbeat verwendeter, möglicherweise zustandswirksamer Aufruf werden getrennt behandelt.

Für privilegierte Funktionen kann ein eigener kleiner Controller im Dienst oder ein ausdrücklich erhöht gestarteter Prozess nötig werden. Die Benutzeroberfläche bleibt in der Benutzersitzung. Die Sperre beim Lesen der BIOS-Instanz wurde als Berechtigungsbefund bestätigt: Einzelne erhöhte Leseprozesse erreichen Capability, Farben und Helligkeit, während die HP-Dienste weiterlaufen. Es wurde damit keine dauerhafte Exklusivsperre nachgewiesen. Ob diese Dienste nach einem Umstieg entbehrlich sind, bleibt getrennt zu prüfen.

## Heutige HP-Komponenten als Analysequellen

- **HPOmenCap:** Host für verschiedene SDK-/HID-/Tuningfunktionen. Ein eigener Adapter übernimmt die notwendigen Geräteoperationen; der Host ist kein geplantes Runtime-Erfordernis.
- **HPSysInfoCap / WMIService:** HP-RPC-Zwischenweg zu Windows-WMI. Die native Analyse bestätigt `ROOT\WMI` / `hpqBIntM`; die eigene Abbildung vermeidet diesen RPC-Weg.
- **HpReadHWData:** Separater Kerneltransport für verschiedene Hardwarezugriffe. Der beobachtete Handle ist pro Transaktion exklusiv und über `Global\Access_PCI` koordiniert. Für direktes BIOS-WMI ist die Notwendigkeit dieses Treibers nicht belegt.
- **OMENLighting:** Vorhandener virtueller HID-/VHF-Lampentreiber, der BIOS-WMI nutzt. Eine eigene direkte Farbsteuerung und die optionale Windows-Lampenintegration sind getrennte Wege. Ein notwendiger eigener Lampentreiber wird daraus nicht abgeleitet.
- **HPAppHelperCap und weitere gemeinsame HP-Dienste:** Ihre Installation allein erklärt weder ihre Notwendigkeit für Omen Hub++ noch ihre gefahrlose Entfernung. Gemeinsame Funktionen anderer HP-Software sind gesondert zuzuordnen.

## Nachweis für den späteren Umstieg

Capabilities und klassischer Tastaturzustand sind auf 8E35/F.13 über den eigenen direkten Windows-WMI-Adapter gelesen. Farb-/Modustransaktionen mit vollständigem Rücklesen und Wiederherstellung, auch aus einem persistierten Journal in einem neuen Prozess, sind inzwischen gemessen. Es wurden keine HP-RPC-/SDK-Wrapper geladen; vorhandene HP-Dienste wurden jedoch nicht gestoppt. Entsperren, Helligkeitstasten und Power-Übergänge brauchen eigene Prüfungen. Erst ein separat geplanter Umstieg zeigt den Betrieb ohne ursprüngliche HP-Omen-Writer. Ein erfolgreich gesendeter Befehl oder eine geschlossene Herstelleroberfläche genügt dafür nicht.

Seit 15. Oktober sind auch die festen Lüfter-GETs sowie manuelles Max-Fan-Ein, vollständiger Schalter-Readback und Original-Restore direkt über BIOS-WMI belegt. Ein neuer Prozess verwarf ein manipuliertes Tabellenbild und restaurierte anschließend aus dem dauerhaften Journal. Die kurzen Proben zeigen tatsächliches Hochlaufen, keine eingeschwungene maximale Nenndrehzahl oder Langzeitstabilität. HP-Controller blieben aktiv; Reboot, Standby/Resume, AC-Übergänge und Ersatzbetrieb ohne diese Writer bleiben für diesen Pfad offen. UI-Speicherung und UI-Smoke sind getrennte Metadatenprüfungen und kein weiterer Hardwarebeweis.

Auswahl und Unterstützung anderer Boards bleiben explizit. Der vorhandene Hub-Code und ein generischer Modulname ersetzen keine Modellfreigabe. Details zum ersten Beleuchtungspfad stehen in [KEYBOARD_LIGHTING.md](KEYBOARD_LIGHTING.md).
