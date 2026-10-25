# Standby- und Neustartprüfung

## Implementierter Ablauf

Seit dem Softwarestand vom 25. Oktober sperrt ein aktiver oder unklarer HPOmenCap die neue Anwendung von PPT-/Slow-Profilen schon vor Capture/SET. Eine getrennte eigene Übernahme-/Recovery-Engine ist vorbereitet, aber noch nicht automatisch aktiv. Sie ersetzt weder den unabhängigen Rückkehrwächter noch die gekoppelte Kühlungs-/Power-Abnahme. Echte Prepare-/Standby-/Neustartchecks verlangen unverändert ein aktuell vollständig bestätigtes Profil. Umfang: [HP_CONTROL.md](HP_CONTROL.md).

Das Referenzgerät 8E35/F.13 unterstützt laut lokalem `powercfg /a` **Modern Standby (S0, Netzwerk verbunden)** und Ruhezustand. Der Dashboard-Prozess registriert sich ausdrücklich für Suspend/Resume, den Bildschirmzustand seiner Benutzersitzung und Netzteilwechsel. Die Abonnements werden gemeinsam besessen, auf Fehler geprüft und beim Ende freigegeben. Grundlage sind die öffentlichen Microsoft-Verträge für [RegisterSuspendResumeNotification](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registersuspendresumenotification) und [GUID_SESSION_DISPLAY_STATUS](https://learn.microsoft.com/en-us/windows/win32/power/power-setting-guids).

Ein ausgeschalteter Bildschirm pausiert die Dashboard-Sensoren. Automatisches Aufwachen hebt diese Pause nicht auf; erst ein aktiver oder gedimmter Bildschirm gibt die Abfragen wieder frei. Suspend und Bildschirmzustand sind unabhängig. Netzteilwechsel setzt Messbaselines zurück, hebt aber keine Pause auf. Die plattformunabhängige Richtlinie liegt in `crates/core/src/power.rs`.

Das Fenster veröffentlicht Zustandsänderungen sofort an den Arbeiter. Eine CPU-/GPU-Aufnahme, die einen solchen Wechsel überlappt, wird nicht angezeigt. Bei einer noch offenen CPU-Anfrage wird ausschließlich deren bekannte Sitzungs-/Sequenzantwort abgewartet und verworfen, anschließend ausdrücklich pausiert. Die Antwort wird niemals als Bestätigung eines späteren Befehls verwendet. Alte Intervalle werden nicht über einen Stromzustandswechsel hinweg weitergerechnet. Der bestehende Timeout bleibt begrenzt; ein tatsächlicher Treiber-/Verbindungsfehler bleibt sichtbar.

Diese Dashboard-Pause ist keine Aussage über die Firmware-Lüfterzuständigkeit, Tastatur-Hotkeys, andere Programme oder die tatsächliche Tiefschlafdauer des gesamten Rechners. Der eigene Lüfterhelfer behält seinen getrennten Kühlungsauftrag. Vollständiger Ersatzbetrieb und tatsächliche Schlafübergänge benötigen eigene Messungen.

## Lesende Vorprüfung

Die neue Release-Binary bietet zwei zusätzliche CLI-Abfragen:

```powershell
& 'C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe' profile-startup-check
& 'C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe' profile-verify-current <profil-id>
```

`profile-startup-check` prüft Auswahl, abgeschlossene Profilrevision, geschützte Installation und Freigabe. Die Abfrage muss aus der installierten Version beziehungsweise einer identischen Binary laufen: eine abweichende Build-Identität ist ausdrücklich kein gültiger Startup-Beleg. Sie aktiviert, autorisiert und deaktiviert nichts.

`profile-verify-current` benötigt für die Hardware-GETs eine erhöhte Windows PowerShell. Sie vergleicht alle Ziele des exakt abgeschlossenen Profils mit den aktuellen Werten, einschließlich vollständigem GPU-Kurvenbild und CPU-Steuerwerten. Eine Abweichung wird gemeldet; die Abfrage repariert sie nicht. SMU-Abweichungen enthalten den angeforderten Wert und das tatsächlich gelesene Steuerbild.

Jede zukünftige Ausführung von `profile-startup-apply` speichert Zeitpunkt und Prozess-ID im lokalen Ergebnis. Zusammen mit Windows-Bootzeit, geplantem Aufgabenlauf und vollständiger Zielprüfung lässt sich eine neue Ausführung von einer alten Erfolgsdatei unterscheiden.

Für explizite Schreibdiagnosen bietet `profile-apply-trace <profil-id>` dieselbe Transaktion und Wiederherstellung wie `profile-apply`, ergänzt um lokale CPU-Readback-Beobachtungen. Dieser Befehl **wendet das Profil an**; er gehört nicht zur lesenden Vorprüfung. Normale Anwendungen sammeln keinen Trace. Nach einem einzelnen SMU-Limit-SET werden ausschließlich feste GETs für höchstens zwei Sekunden beobachtet; der Wert muss mindestens 100 ms übereinstimmen. Ungültige Antworten oder Transport-/Identitätsfehler brechen sofort ab, der SET wird nicht wiederholt. Auch die Rückkehr zum vollständigen Originalbild wird begrenzt beobachtet. Ein bestandener Momentabgleich belegt keine spätere Beständigkeit.

## Reproduzierbare Checkpoints

`scripts/check-power-lifecycle.ps1` erstellt ausschließlich lokale Diagnose-Checkpoints. Es ruft weder Standby noch Neustart auf, wendet kein Profil an und ändert keine Freigabe, Dienste oder Treiber. Eine erhöhte Windows PowerShell ist nur für die festen Hardwarelesezugriffe notwendig. Das Dashboard muss auf Deutsch laufen und darf für die eindeutige Prozesszuordnung nur einmal geöffnet sein.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\check-power-lifecycle.ps1' -Phase Prepare

# Anschließend den von Prepare ausgegebenen Ordner benutzen:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\check-power-lifecycle.ps1' -Phase AfterStandby -RunDirectory '<Prüfordner>'

powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'D:\Omen Gaming Hub Rewrite\scripts\check-power-lifecycle.ps1' -Phase AfterReboot -RunDirectory '<Prüfordner>'
```

Die Vorbereitung verlangt ein aktuell bestätigtes Profil, eine passende Startup-Freigabe und verfügbare Dashboard-Sensoren aller 32 logischen Prozessoren. Jeder Lauf erhält einen eigenen Unterordner; frühere Proben bleiben erhalten. Die Prüfung liest zusätzlich feste CPU-/GPU-Diagnosen. Änderungen von Profilrevision oder Build zwischen Checkpoints machen den Vergleich ungültig.

Für `AfterStandby` müssen Bootidentität und App-/Leser-PIDs erhalten bleiben. Windows muss nach dem Vorbereitungspunkt eine geordnete S0-Eintritts-/Austrittsfolge melden, und das Dashboard muss neue Power-Ereignisse sowie wieder verfügbare Sensoren zeigen. Die lokal installierten Ereignisdefinitionen ordnen Kernel-Power **506/507** dem S0-Eintritt/Austritt zu. Simulierte Fensternachrichten allein genügen nicht. Für `AfterReboot` müssen sich Bootzeit und der nachgewiesene Aufgaben-/Profilstart auf den neuen Boot beziehen; eine alte Erfolgsdatei genügt nicht. Deep-Idle-/DRIPS-Anteile werden damit nicht gemessen.

`scripts/test-power-lifecycle-evidence.ps1` prüft 18 künstliche Belegkombinationen: unter anderem fehlende/vertauschte/zu alte oder nach dem Checkpoint liegende S0-Ereignisse, unvollständige Vorbereitung, fehlende/geänderte Reader-PID, nicht chronologische Checkpoints, alte Startup-Ergebnisse, fehlgeschlagene Aufgaben und abweichende Builds/Profile. Diese Tests lösen keine Stromzustandsänderung aus und zählen nicht als echte Standby-/Neustartabnahme.

## Ergänzung vom 25. Oktober 2026: Dienstisolation

Nach gesonderter Nutzerfreigabe wurde **HPOmenCap einmal kurz angehalten**. Die identifizierte Originaloberfläche samt Hintergrundprozess war geschlossen, UXTU/Afterburner liefen nicht und die eigene Lüfterregelung war nach bestätigten **4800/4800 RPM** pausiert. Die abgefragten Treiberzustände und Starttypen stimmten vor/nach Dienststopp überein. Mit dem unveränderten Release E95616E4… wurde das unveränderte Profil „Test“ einmal angewendet und vollständig bestätigt. Slow **70 W** und PPT **75 W** blieben in sechs anschließenden GET-Stichproben bis ungefähr zwölf Sekunden nach dem Apply bestehen; Tctl fiel dabei von 68 auf 60,875 °C. Die festen Temperatur-/Lüfterlesezugriffe funktionierten bei gestopptem Dienst weiter.

Anschließend wurden die vollständigen zuvor erfassten Hardwarebilder restauriert, das vorherige Recovery-Journal und die Startup-Auswahl byteidentisch erhalten, der Dienst und die eigene Lüfteraufgabe gestartet sowie der Original-Hub wieder geöffnet. Der gesonderte Rückkehrhelfer musste nicht eingreifen. Es wurde kein zweites Profil-Apply für einen Vergleich und kein realer Schlaf-/Neustart ausgeführt.

Dies stützt die Hypothese einer Beeinflussung durch **HPOmenCap bzw. seinen Lifecycle** gegenüber den Rücksetzern bei weiterlaufendem Dienst am Vortag. Es identifiziert weder einen konkreten Setter noch eine alleinige Ursache. Kurze feste GETs und einmaliges Apply bei gestopptem Dienst beweisen keinen vollständigen HP-Ersatz, laufenden eigenen Lüfterbetrieb ohne Dienst oder langfristige Profilstabilität. Es gibt noch keinen automatischen Dienststopp in Omen Hub++. Eine kontrollierte Übernahme und die tatsächliche Standby-/Neustartabnahme bleiben offen. Zeitpunkte, Messwerte, Wiederherstellung und Freigabeumfang: [VALIDATION.md](VALIDATION.md).

## Befund vom 24. Oktober 2026

Das unveränderte Nutzerprofil **„Test“** enthält Slow-PPT **70 W**, PPT **75 W**, Tctl-Limit **85 °C**, CO **−18**, eine vollständige GPU-Kurve und Dynamic Boost. Gelesener Ausgangszustand war unter anderem Slow **66 W** und PPT **80 W**. Die bestehende geschützte Freigabe gehört zur installierten älteren Binary A0F4113A…; eine neue Freigabe wurde nicht erteilt.

Die folgenden Proben waren vom Nutzer ausdrücklich freigegeben. Der Original-Hub-Hintergrundprozess wurde zeitweise geschlossen, UXTUs Logon-Aufgabe während der Isolation deaktiviert und anschließend in ihren vorherigen Zustand zurückgestellt. **Kein Herstellerdienst und kein Treiber wurde angehalten oder geändert.**

| Zeitpunkt CEST | Beobachtung | Aussagegrenze |
| --- | --- | --- |
| 16:50 | Slow 70 W wurde beim ersten Readback abgewiesen; vollständiger Rollback ohne Fehler. | Kein Beleg einer allgemeinen 66-W-Grenze. |
| 18:58 | Ohne laufendes UXTU und nach zwei Wartefenstern von je 15 s bestand Slow 70 W, PPT 75 W wurde als 80 W gelesen und zurückgesetzt. | Ein geschlossener Original-Hub und UXTU-Prozess allein erklären den Rücksetzer nicht. |
| 19:16 | Trace-Build 82D02B1D… bestätigte alle sieben Ziele. Slow 70 W und PPT 75 W stimmten jeweils in drei Beobachtungen über mindestens 100 ms überein; auch der abschließende Gesamtvergleich bestand. Rund fünf Sekunden später wurde Slow 66 W gelesen. | Erfolgreiche unmittelbare Anwendung, **keine beständige Profilwirkung**. Diese spätere Abweichung ist vom fehlgeschlagenen Apply mit automatischem Rollback zu unterscheiden. |
| 19:44 | Die eigene Lüfterregelung wurde sauber bei angeforderter voller Kühlung pausiert. Die geforderten Drehzahlen waren noch nicht beide erreicht; Abbruch vor Profil-SET. | Keine Schreibprobe aus diesem Versuch. |
| 19:49 | Nach bestätigten ≥4000 RPM beider Lüfter und bei pausierter eigener Regelung bestand das Apply. Bereits nach etwa 1,23 s wurden wieder PPT 80 W und Slow 66 W gelesen. CPU-Tctl 64,25 °C. Danach explizite vollständige Wiederherstellung und Neustart des eigenen Lüfterhelfers. | Ein periodischer eigener Lüfter-Heartbeat ist für den Rücksetzer nicht notwendig. Kein zusätzlicher Heartbeat wurde als Gegenprobe gesendet. |
| 20:07 | Bei gleicher Kühlungsabsicherung scheiterte PPT 75 W erneut am 80-W-Readback; Rollback ohne Fehler, eigener Lüfterhelfer wieder gestartet. | Die vorbereitete Untersuchung bei durchgehend offenem Controller kam nicht zur Ausführung. Die unqualifizierte `--hold`-Variante ist nicht Bestandteil der finalen CLI. |

**UXTU:** Die lokale Version 26.3.1.0 speichert `AutoReapply=True`, Intervall **15 s** und `ApplyOnStart=True`. Ihr Timer kann Einstellungen beim laufenden Programm erneut anwenden. Die gespeicherten Befehle enthalten Temperatur, Curve Optimizer und Windows-/Leistungsoptionen, aber keine ausdrücklichen Slow-/Fast-PPT-Zahlen. Bei den isolierten Proben war weder ein UXTU-Prozess noch ein zugehöriger Dienst aktiv; die geplante Aufgabe besitzt einen Anmeldetrigger, keine periodische Wiederholung. Das ist ein möglicher konkurrierender Controller beim nächsten Start, jedoch kein Nachweis als Ursache der hier ohne Prozess beobachteten Rücksetzer. Die Nutzereinstellungen und der ursprüngliche aktivierte Aufgabenstatus wurden erhalten.

**Weitere Eingrenzung:** HP-/AMD-Hintergrunddienste liefen weiter. Statische Ghidra-Analyse der identifizierten HP-AMD-RPC-Komponente zeigte einen Request-Dispatcher und leere Start-/Stop-Einstiege; daraus ergibt sich kein belegter autonomer periodischer PPT-Setter. Der Diensthost und seine übrigen Module sind damit nicht ausgeschlossen. Firmware-/Treiberpolitik, weitere Controller und Sitzungseffekte bleiben offene Ursachen. Es wird keine periodische SET-Schleife eingeführt, um einen unbekannten Controller zu überstimmen.

Der feste RSMU-GET `0xDA`, den [ZenStates-Core/Zen4Settings](https://github.com/irusanov/ZenStates-Core/blob/master/Hardware/Smu/Settings/Zen4Settings.cs) als `GetPboFusedApuSlowLimit` benennt, lieferte **85000**, `GetFusedPpt` **160000**. Der zusätzliche Wert bleibt `fused_slow_raw`, keine laufende Vorgabe und keine automatisch verwendete Obergrenze. Die kleinere ältere Probe **55 → 54 → 55 W** ist ebenfalls keine Beständigkeitsfreigabe für das vollständige Nutzerprofil.

Nach bestätigter vollständiger Wiederherstellung wurden die zurückgesetzten Diagnosejournale archiviert und der vorherige vollständige Nutzer-Recovery-Checkpoint byteidentisch erhalten. Auch die bestehende Startup-Auswahl blieb byteidentisch. Das gibt das ursprüngliche Recovery wieder, behauptet aber keine aktuell aktiven 70/75 W. Profil, Startup-Freigabe und installierte Binary wurden nicht auf einen schwächeren Prüfzustand umgestellt. Die vorbereitete 66-W-Profilkopie wurde nicht importiert oder angewendet. Der Original-Hub wurde nach den Softwareprüfungen um 20:56 CEST wie gewünscht wieder gestartet; der eigene Lüfterhelfer meldete anschließend `waiting_for_hub`.

Ein echtes Standby-/Neustart-Prepare verlangt weiterhin ein vollständig aktuell bestätigtes Profil. **Diese Voraussetzung ist nicht erfüllt.** Es wurde kein realer Schlaf- oder Neustartübergang ausgelöst. Buildbezogene Softwareprüfungen und Speicher: [VALIDATION.md](VALIDATION.md). Rohdaten, proprietäre Dateien und dekompilierte Quellen verbleiben ausschließlich in ignorierten lokalen Analyseordnern.
