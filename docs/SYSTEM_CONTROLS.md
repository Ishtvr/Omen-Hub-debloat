# Sichere Steuerungsprüfung am Referenzgerät

**Wächter-Software 25. Oktober, 15:49 CET:** Der vorbereitete native Übernahmepfad verlangt jetzt bestätigten unabhängigen Rückkehrwächter vor STOP. Besitzerabbruch, Kanalverlust und 60-Sekunden-Qualifikationsfrist erlauben nur die Rückkehr derselben Sitzung; der Helfer öffnet ausschließlich START auf den fest geprüften Benutzerdienst. Neun echte Prozessfälle mit Dateisimulation bestanden zusätzlich zu 209 Rust-Tests. Am Gerät wurden nur Status/Preflight gelesen. Neue automatische Übernahme und native Dienstschalt-/Kühlungsfreigabe bleiben offen. [HP_CONTROL.md](HP_CONTROL.md).

**Software-Schutz 25. Oktober, 03:59 CET:** Neue Profile mit unabhängigem CPU-PPT/Slow werden bei aktivem, wechselndem oder nicht lesbarem HPOmenCap-Zustand vor Capture/SET vollständig gesperrt. Vor dem eigentlichen Limit-SET wird erneut geprüft; ein späterer Konflikt löst Fehler/Rollback aus. Neue PPT-/Slow-Startup-Freigaben werden bis zur Qualifikation des Anmelde-Übernahmepfads bei installiertem OEM-Dienst auch nach temporärem Stopp gesperrt. Hardware-Original-Recovery bleibt zugänglich und muss vor Rückgabe der Dienstzuständigkeit abgeschlossen sein. Dies verändert keine Treiber, Leistungswerte oder Dienste durch bloßes Öffnen/Prüfen. Die neue native Übernahme-Engine ist softwareseitig vorbereitet, aber noch ohne normale automatische Aktivierung oder Hardware-Abnahme ihres START/STOP-Pfads. [HP_CONTROL.md](HP_CONTROL.md).

**Ergänzung 25. Oktober:** Der Nutzer erlaubte gesondert eine einmalige **HPOmenCap**-Isolation. Bei angehaltenem Dienst, geschlossenem Original-Hub/UXTU und bestätigter voller Kühlung bestand das unveränderte 70/75-W-Profil das vollständige Apply; sechs nachfolgende CPU-GETs bis ungefähr zwölf Sekunden nach dem Apply bestätigten beide Limits. Vollständiger Original-Restore sowie Wiederherstellung von Dienst, Kühlung, Hub, Recovery-Journal und Startup-Auswahl sind abgeschlossen. Keine direkten Treiberaktionen, kein echter Schlaf-/Neustart. Der Vergleich zum Vortag stützt einen Diensteinfluss, beweist aber noch keine alleinige Ursache oder längerfristige Beständigkeit. Details: [VALIDATION.md](VALIDATION.md).

**Ergänzung 24. Oktober:** Das ausdrücklich freigegebene Nutzerprofil Slow **70 W** / Fast/PPT **75 W** bestand in einer späteren Trace-Probe den vollständigen unmittelbaren Readback, fiel danach jedoch auf **66/80 W** zurück. Andere Versuche scheiterten bereits während des Apply und wurden vollständig zurückgesetzt. Der Rücksetzer trat auch ohne laufendes UXTU/Original-Hub-Hintergrundprogramm und bei pausierter eigener Lüfterregelung auf; Herstellerdienste liefen weiter. UXTUs gespeicherte automatische Wiederanwendung alle 15 s ist kein Beweis als Ursache dieser isolierten Proben. Der zusätzliche Fuse-GET lieferte 85000 und begründet keine automatisch gültige Schreibgrenze. Frühere kleinere Proben werden nicht als beständige 70/75-W-Freigabe umgedeutet. Vollständiger alter Recovery-Checkpoint und Startup-Auswahl bleiben erhalten; echte Power-Lifecycle-Abnahme ist offen. Details: [POWER_LIFECYCLE.md](POWER_LIFECYCLE.md).

**Ergänzung 23. Oktober:** Die Freigabe des Lüfterdienstes fordert nach eigener Regelung zunächst einmal 48/48 an und stoppt anschließend Heartbeat/Level. Ein realer Firmware-Rückfall nach ungefähr zwei Minuten ist beobachtet. Der direkte HPCM/FAMC-Payload wurde in einer kurzen Probe an der konservativen Temperaturgrenze abgebrochen und bleibt außerhalb der Runtime. Befund und Grenzen: [FAN_HANDOVER.md](FAN_HANDOVER.md). Die folgenden Proben gehören zu ihrem datierten Stand vom 22. Oktober.

Stand: **22. Oktober 2026**. HP **8E35 / F.13**, Ryzen 9 **8940HX**, SMU **0x546F00**, PM-Tabelle **0x540208**, RTX 5070 Laptop mit NVIDIA **617.42**. Die Prüfungen sind kurze Funktionsproben mit Rücklesen und Wiederherstellung. Sie sind keine Freigabe beliebiger OC-Werte oder ein Laststabilitätstest.

Der Benutzer erlaubte ausdrücklich kleine, sichere Schreibproben sowie das vorübergehende Schließen von OMEN Hub und UXTU. Er legte **Unleashed** als Anfangs-/Endzustand der HP-Modustests fest. Der CPU-GET bestätigte **−20 Curve-Optimizer-Schritte auf allen 16 physischen Kernen**. Geprüft wird −19 und anschließend die vollständige Rückkehr zu −20; Schritte sind keine Millivolt.

## Korrigierte CPU-Verträge

Die ursprüngliche Rohprobe vom 16. Oktober und die neue Probe liefern beide dezimal **5505544**, also **0x540208**. Der bisher im Adapter verwendete Wert 0x540108 war ein Übertragungsfehler. Er hatte reguläre Limit-Captures vor dem ersten SET blockiert; die alten direkten Kalibrierungsproben sind davon zu unterscheiden.

| CPU-Vorgabe | Fester RSMU-Befehl | Readback |
| --- | --- | --- |
| Fast/PPT | 0x56 | PM-Tabelle, Zelle 2, mW |
| Slow/PPT | 0xCB | PM-Tabelle, Zelle 4, mW |
| TDC | 0x57 | PM-Tabelle, Zelle 8, mA |
| EDC | 0x58 | PM-Tabelle, Zelle 62, mA |
| Tctl-Limit | 0x59 | PM-Tabelle, Zelle 10, °C |
| CO gemeinsam/einzeln | 0x07 / 0x06 | 0xD5 für jeden Kern |
| PBO-Scalar | 0x5B | 0x6D, vollständiger f32-Wert |

Der direkte Slow-Vertrag und die Skalierung sind in den primären Quellen [ZenStates-Core: Zen4Settings](https://github.com/irusanov/ZenStates-Core/blob/master/Hardware/Smu/Settings/Zen4Settings.cs) und [SetSmuLimit](https://github.com/irusanov/ZenStates-Core/blob/master/Hardware/Smu/Commands/SetSmuLimit.cs) beschrieben. Diese Interface-Beobachtungen ersetzen keine Board-Probe. Am **05:17 CEST** bestand Build **6FF1F7A145532F7FE392ABAF3B741B29D350070A7B7B7581907B5C4052A42E8F** die feste Folge **55 → 54 → 55 W**; ein neuer Adapter verglich danach alle sieben unabhängigen CPU-Steuerbilder sowie die vollständigen GPU-Steuerwerte. Journal und Startup-Datei blieben erhalten.

Der vorherige Versuch, unabhängig STAPM und Slow über den gemeinsamen HP-Befehl 0x29 zu steuern, war nicht zuverlässig. Beispielsweise wurde für das gewünschte Paar 79/54 W tatsächlich 80/54 W zurückgelesen. Die Transaktion schlug fehl und stellte ihre Originale wieder her. **Der einzelne STAPM-Regler ist deshalb entfernt und gesperrt.** Slow verwendet jetzt ausschließlich 0xCB. Die festen HP-Modi verwenden weiter die separat belegten Hersteller-Payloads für ihren gemeinsamen PL1.

Neue CPU-Limit-Journale tragen den eigenen Typ `rsmu_limit` und die gemessene Tabellenversion. Alte HP-Limitbilder werden weiter dekodiert, aber nicht als Beleg für einen neuen Transport übernommen. Die Rücksicherung verweigert außerdem falsche Feature-/Bildkombinationen, unvollständige Kernlisten, ungültige PBO-Bitmuster und andere Firmwareversionen. Ein Profil darf CO für alle Kerne oder einzelne Kerne enthalten; beides gemeinsam wird vor dem Schreiben abgewiesen.

Die PM-Tabellenauflösung, Aktualisierung und Kopie halten jetzt dieselbe gemeinsame PCI-Sperre. Nur diese beobachtende Lesesequenz darf dreimal mit kurzem Abstand wiederholt werden; Tuning-SETs werden nicht blind wiederholt.

## Windows-Zustände und vollständige Originale

Die Windows-Prozessorrichtlinien schreiben beide Stromquellen. Ein bereits passender AC-Wert darf einen abweichenden DC-Wert nicht verdecken. Readback und Recovery vergleichen daher beide Werte. Eine fehlgeschlagene Teiländerung darf nur Original-/eigene Zielwerte restaurieren; fremde Werte bleiben gesperrt.

Ein Profil mit Energiesparplan und Prozessorrichtlinien erfasst die Originale der **Zielplan-Richtlinien**, aktiviert den Zielplan zuerst und schreibt erst danach die Richtlinien. Beim Restore werden auch die ursprünglichen Werte eines wieder inaktiven Zielplans geprüft. Die Wiederherstellung eines inaktiven Plans aktiviert diesen nicht nebenbei.

Windows-Energiemodus und Energiesparplan sind unterschiedliche Einstellungen. Für den Modus wird der Plan **Ausbalanciert** benötigt; eine unpassende Kombination wird vor dem ersten SET mit einer konkreten Meldung blockiert. Der Benutzer kann beide Einstellungen im selben Preset auswählen. Diese Bedingung entspricht der [Microsoft-Beschreibung des Performance-Sliders](https://learn.microsoft.com/en-us/windows-hardware/customize/desktop/customize-power-slider).

Das Original des Modus stammt jetzt aus `PowerGetActualOverlayScheme`. `PowerGetEffectiveOverlayScheme` ist ein zusätzlicher Apply-Readback, weil der wirksame Zustand durch den Plan oder Energy Saver abweichen kann. Ein altes Journal mit ausschließlich dem effektiven Overlay wird nicht als konfigurierter Originalzustand ausgegeben. Die verwendeten Export-Signaturen sind unter anderem in der primären Implementierung [G-Helper PowerNative](https://github.com/seerge/g-helper/blob/main/app/Mode/PowerNative.cs) nachvollziehbar.

## Lüfter und CCDs

Der Max-Schalter ignoriert jetzt eine gespeicherte leisere Kurve und setzt beide Ziellevel auf **48**. Fehlende CPU-/IR-Werte, ungültige GPU-Temperaturen und Schutzanforderungen führen zu voller Kühlung. Der Lüfterpfad liest nur die benötigte CPU-Temperatur; optionale Effektivtakt-Warnungen verwerfen diesen gültigen Tctl-Wert nicht mehr.

Der Dienst prüft vor dem ersten Heartbeat/Level-SET die vollständige bekannte Lüftertabelle und Zuordnung. Bei pausierter Regelung oder laufendem Original-Hub lädt er keine CPU-/NVIDIA-Sensorleser. Ein eigener Stop-Event beendet die Schleife regulär, damit die CCD-Zuordnungen freigegeben werden können. Der Installer verwendet diesen Weg und verhindert einen erzwungenen Austausch bei gespeicherter CCD-Einschränkung.

Ein isolierter Proof mit denselben Core-/Platform-Quellen, SHA-256 **06724712D49A36B28510A8BFD55CADEACB68C9727373F42493EF585427E5FE30**, führte am **05:02 CEST** eine konservative Kurve und anschließend Max aus. Die gehaltenen Max-Statuswerte waren **48/48**, die gemessenen Firmware-Drehzahlen stiegen bis **4800/4800 RPM**. Beide abschließenden HP-Limitfelder waren wieder 80/80 W, die GPU-Steuerwerte unverändert. Das ist eine kurze reale Funktionsprobe, keine Zertifizierung des gesamten thermischen Regelvertrags.

Nach dem Stop wurden 36 Sekunden lang keine weiteren Software-Level gesendet; die RPM blieben in dieser Stichprobe bei 4800. Daraus wird **keine bestätigte Übergabe an die BIOS-Automatik** abgeleitet. Der Status „Auto“ beschreibt wahrheitsgemäß die pausierte eigene Regelung. Ein Modus-/Ownership-GET, reale Standby-/AC-Übergänge und der unabhängige Dauerbetrieb bleiben offen.

Die CCD-Probe veränderte ausschließlich einen eigens gestarteten, schlafenden Kindprozess. Derselbe produktive Enforcer und die Windows-Prozess-APIs setzten **0xFFFF**, **0xFFFF0000** und anschließend wieder dessen ursprüngliche Maske **0xFFFFFFFF**. Auch `release` stellte die Originalmaske wieder her. Andere Benutzerprogramme wurden dabei nicht umgeordnet.

## Speicher und eindeutige Anzeige

Der Lüfterdienst verwendet nur den öffentlichen [NVAPI-Thermalvertrag](https://docs.nvidia.com/nvapi/group__gputhermal.html), v2-Struktur mit höchstens drei Sensoren und GPU-Ziel 1. Mehrere NVIDIA-Geräte oder fehlende/ungültige Kurvensensorwerte führen nicht zu einem geratenen Temperaturmix, sondern zu voller Kühlung. Die allgemeine NVAPI-/NVML-Dashboard-Telemetrie bleibt im kurzlebigen Leseprozess. Max fordert bedingungslos 48/48 an und benötigt dafür keine CPU-/GPU-Lesebibliothek. Bei fehlender oder ungültiger Kurve werden die Leser ebenso freigegeben wie bei expliziter Pause und Original-Hub-Wartezustand.

Die vollständige aktive Tray-Stichprobe liegt bei **47,60 MB**, die Pause nach Sensoraktivierung bei **44,88 MB**, jeweils einschließlich Dashboard, pausiertem CPU-Reader und eigenem Lüfterhelfer. Vorherige aktive Versuche mit allgemeiner Telemetrie bzw. NVML-only lagen über 50 MB und wurden ersetzt. Es wurde kein Working-Set-Trimming eingesetzt. Genaue Build-/Zeit-/Prozessabgrenzung: [VALIDATION.md](VALIDATION.md).

Strukturelle Dateivalidierung und Anwendungs-Preflight sind getrennt: ältere widersprüchliche Presets bleiben zum Korrigieren lesbar, dürfen aber keine Hardwareänderung starten. Die beiden gespeicherten Dynamic-Boost-Namen werden in der UI als eine Vorgabe angezeigt; explizite Änderung/Abwahl erfasst vorhandene Aliase zusammen und respektiert parallele Änderungen. Speichern einer Lüfterkurve meldet die Speicherung und den nötigen Kurvenmodus, keine unbestätigte aktive Regelung.

## Wiederholbare Prüfungen

`scripts/check-system-controls.ps1` verlangt Administratorrechte und den ausdrücklich angegebenen SHA-256 der geprüften Binary. Es verweigert laufende konkurrierende Tuning-Anwendungen. `-CoolCpu` kann den Windows-Boost des aktiven Plans vorübergehend abschalten; AC/DC-Originale werden vorab in der ignorierten Prüfablage gesichert und abschließend exakt zurückgelesen. Eine zu heiße CPU bleibt ein Blocker; die Temperaturgrenze wird für einen Test nicht gelockert.

Die festen CPU-Proben senken Grenzen um 1 W, 1 A oder 1 °C. PBO wird bei unverändertem 1× tatsächlich geschrieben und zurückgelesen. Nach jeder CPU-Probe liest eine neue Sitzung alle sieben unabhängigen CPU-Steuerbilder; alle Proben prüfen zusätzlich die vollständige GPU-Kurve und globalen Offsets. Jede abgeschlossene Probe darf nur ihr eigenes vollständig restauriertes Journal durch den vorherigen Checkpoint ersetzen. Ein unvollständiges Recovery bleibt sichtbar.

Die weiteren Proben betreffen Windows-Boost, Min/Max, EPP, Core-Parking, Modus/Plan, Bildwiederholrate, gespeicherte CCD-Auswahl, Dynamic Boost, die MUX-Startvorgabe, Max-Fan und klassische Beleuchtung. **MUX-Readback ist kein physisch getesteter GPU-Wechsel.** Die tatsächliche Verdrahtung und automatische Profilaktivierung nach einem echten Neustart verlangen eine eigene reale Neustartprobe.

Rohdaten, fremde Binärdateien und lokale Proof-Artefakte liegen ausschließlich in ignorierten Verzeichnissen. Buildbezogene Endergebnisse stehen in [VALIDATION.md](VALIDATION.md); die Zusammenfassung des aktuellen Supports in [HARDWARE_MATRIX.md](HARDWARE_MATRIX.md).
