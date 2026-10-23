# Rückgabe der Lüftersteuerung auf 8E35/F.13

Stand: **23. Oktober 2026**. HP OMEN 16-ap0xxx, BIOS F.13, Ryzen 9 8940HX, RTX 5070 Laptop, installierter OMEN Hub **1101.2609.11.0**. Diese Beobachtungen erweitern keine Freigabe auf andere Firmware oder Modelle.

## Konkreter Firmwarepfad

Die DSDT wurde ausschließlich gelesen, mit gültiger Länge und ACPI-Prüfsumme lokal gesichert und mit ACPICA iASL 20240927 statisch disassembliert. Sie identifiziert **HPQOEM / 8E35**; SHA-256 **`08558148f65c7fbf88ad8d61ff7d94330e6569a29a3d9d6fa44452539decd7ee`**. Rohdaten und disassemblierter Herstellercode bleiben ignoriert unter `.local/`.

- **0x20008 / 0x10:** Der vermeintliche Lüfteranzahl-GET setzt zusätzlich **OMCC**. Eine Beobachtungsprobe darf diesen Heartbeat nicht wiederholen.
- **0x20008 / 0x1A:** Eingabebyte 1 schreibt den Leistungsmodus **HPCM**, Byte 2 die Ein-Bit-Vorgabe **FAMC**. Dies ist kein isolierter Auto-Schalter. Ein Modus-Original darf nicht geraten werden.
- **0x20008 / 0x2E:** Die ersten beiden Bytes des 128-Byte-Herstellerblocks setzen die CPU-/GPU-Level **SRP1/SRP2**. Rückgabe 0 bestätigt den Befehl; eine gemessene Drehzahl ist kein vollständiges Originalbild der Zuständigkeit.
- **0x20008 / 0x2D:** Tachometerwerte werden auf 100 RPM gerundet. Das sind keine exakten physischen Sollwerte.

Die erneut dekompilierte aktuelle Hub-Version wählt im Netzbetrieb weiterhin Software-Regelung. Ihr BIOS-Auto-Pfad verlangt Akkubetrieb, Thermal-Auto und eine zusätzliche Modellfreigabe. Die passende Khalilah-Geräteressource blieb bytegleich. Gespeicherte Auto-Auswahl und WMI-Erfolg sind somit kein Ownership-GET.

Der öffentliche [Linux-Treiber hp-wmi](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/hp/hp-wmi.c) führt Board 8E35 und beschreibt einen Rückfall nach ungefähr 120 Sekunden ohne User-Defined-Heartbeat. Seine separate automatische Null-Level-Route wird hier nicht übernommen: Diese Semantik ist für den konkreten 128-Byte-Herstellerpfad auf F.13 nicht bestätigt.

## Reale begrenzte Proben

### Watchdog, 22:36…22:39 CEST

Der eigene Hintergrundhelfer wurde regulär beendet. Der Original-Hub war bereits vor dieser Probe nicht mehr vorhanden; sein Fehlen wurde protokolliert. Nach zehn Sekunden mit **48/48** wurden **150 Sekunden** lang keine weiteren Heartbeats oder Lüfterlevel gesendet. HP-Windows-Dienste und MSI Afterburner liefen weiter. Es gab keinen Neustart, Lastgenerator, Leistungs-/Spannungs-SET oder Treiberstart.

Zunächst blieben die Lüfter bei **4800/4800 Firmware-RPM**. Etwa **122 Sekunden nach Beginn der Regelphase**, also ungefähr zwei Minuten nach dem ersten Heartbeat, sanken sie auf 4600/4600, danach 3700/3800 und schließlich **2300/2100 RPM**. Abschließend: **63,5 °C Tctl / 48 °C GPU / 36 °C IR**. Dies belegt einen autonomen Rückfall; sofortige Übergabe und Erhalt sämtlicher Leistungsparameter sind damit nicht bewiesen.

Der Original-Hub wurde über die registrierte Paketaktivierung wieder gestartet. Alle 127 GPU-Anker/Deltas, globale Offsets, CO **−20** auf allen 16 Kernen, Original-Hub-Konfiguration und eigene Profil-/Start-/Lüfterdateien blieben erhalten. Danach wurde der gespeicherte CPU-Slow-Wert **66 W** gelesen; die vorherige Probe ohne Hub hatte 55 W geliefert. Ein präziser Zeitpunkt einer Limitänderung wird daraus nicht abgeleitet.

### Explizite FAMC-Vorgabe, 22:48 CEST

Für den vom Benutzer festgelegten Unleashed-Ausgangs-/Endmodus wurde der aus der konkreten DSDT belegte Vier-Byte-Payload untersucht: bekannter Modus **04**, FAMC **1**. Der Heartbeat blieb aktiv; gleichzeitig wurden ausschließlich Maximalziele **48/48** angefordert. Innerhalb etwa neun Sekunden sanken die Firmware-RPM von 4800/4800 auf 2600/2700. Die Vorgabe hat damit eine tatsächliche Wirkung unabhängig vom Watchdog.

Bei **86 °C Tctl** beendete die feste konservative **85-°C-Abbruchbedingung** die Probe nach etwa 13 Sekunden. Die Grenze wurde nicht erhöht. Der bekannte Software-Payload mit FAMC **0** und maximaler Kühlung wurde wieder gesetzt, anschließend der Original-Hub gestartet. Metadaten, GPU-Kurve, globale Offsets, CPU-CO und Original-Hub-Konfiguration sind erhalten; CPU-Slow war danach wieder 66 W. Dies ist ausdrücklich **keine Freigabe eines unbeaufsichtigten BIOS-Auto-Betriebs**.

Ein begrenzter zusätzlicher EC-Leseversuch mit dem vorhandenen signierten PawnIO-LPC-Modul erhielt innerhalb der Frist keine Antwort und wurde verworfen. Er enthielt das ACPI-EC-READ-Protokoll, keine EC-WRITE-Operation. Dieser Pfad wird nicht in die Runtime eingebaut. Ghidra wurde zusätzlich für statische Analyse des vorhandenen HP-Lesetreibers eingesetzt; dessen Binärdatei wird nicht übernommen oder durch die App geladen.

## Konservative Freigabe in Omen Hub++

Der direkte FAMC-Umschalter bleibt außerhalb der produktiven App. Die Anwendung sendet dafür keinen neuen Modus-Payload und verändert keine CPU-/GPU-Leistungsgrenze. Der bereits qualifizierte Softwarepfad wird abgesichert:

1. Vor einem Heartbeat-/Level-Versuch übernimmt die eigene Zustandsmaschine Verantwortung. Ein Transportfehler kann einen möglichen Commit nicht als sicheren Leerlauf tarnen.
2. Bei Auto, fehlender/ungültiger Kurve und regulärem Ende wird nach eigener Regelung einmal **48/48** angefordert. Danach folgen keine weiteren Heartbeats oder Level. Eine niedrige eigene Vorgabe bleibt so nicht absichtlich bis zum Watchdog stehen.
3. Der Status heißt **`handover_pending`**. Auch verstrichene Zeit wird nicht als Ownership-GET ausgegeben. Fehler behalten ihre explizite Fehlerursache.
4. Ein positiv identifizierter Original-Hub beendet die eigene Verantwortung ohne zusätzlichen SET. Fehler der Prozessaufnahme gelten als unbekannte Zuständigkeit.
5. Erneute eigene Regelung erfordert wieder Tabellen-/Topologieprüfung und neuen Heartbeat. Reguläres Ende gibt CCD-Zuordnungen weiterhin frei.

Tests behandeln fehlgeschlagene Freigabe, erneutes Aktivieren, wiederholte Pause ohne weitere Befehle und Vorrang des Original-Hubs. Buildbezogene reale Prüfung und Paketdaten: [VALIDATION.md](VALIDATION.md).

## Weiter offen

Benötigt werden verlässlicher lesender Leistungsmodus/FAMC, thermisch abgesicherte direkte Umschaltung bei CPU-Last und Erhalt eigener Leistungsprofile. Reale AC-, Suspend-/Resume- und Neustartübergänge sowie Betrieb ohne HP-Hardwaredienste bleiben gesondert zu prüfen. Die konservative Freigabe bestätigt keinen vollständigen OMEN-Hub-Ersatz.
