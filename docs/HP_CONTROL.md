# Kontrollierte HP-Steuerung

## Stand vom 25. Oktober 2026

Die eigene Übernahme-Engine ist als plattformunabhängige Zustandslogik, getrenntes Recovery-Journal und fester Windows-Adapter implementiert. **Die normale App hält HPOmenCap weiterhin nicht automatisch an.** Ein dauerhaft aktiver Übernahme-Schalter, automatischer Anmeldestart, unabhängiger Prozesswächter und die gekoppelte Kühlungs-/Power-Lifecycle-Abnahme sind noch offen. Die Bibliotheksschnittstelle dient der nächsten beaufsichtigten Qualifikation; sie ist keine Freigabe für unbeaufsichtigten Ersatzbetrieb.

Die vorherige ausdrücklich freigegebene Hardwareprobe bestätigte das vollständige Nutzerprofil bei kurz angehaltenem Dienst und sechs anschließende 70/75-W-Lesungen. Dieses kurze Ergebnis wird nicht auf neue native Dienstschaltvorgänge oder einen Dauerbetrieb übertragen. Die neuen Softwaretests benutzen einen simulierten Dienst und echte temporäre Dateien; am Gerät wurden nur Status und Profilvorschau gelesen. Messungen: [VALIDATION.md](VALIDATION.md).

## Schutz beim Anwenden eines Profils

Ein Profil mit **CpuPptLimitMw** oder **CpuSlowPptLimitMw** wird vor dem ersten Capture/SET vollständig abgewiesen, wenn der ursprüngliche HPOmenCap noch läuft oder sich in einem Übergang befindet. Ein Lesefehler ist unbekannte Zuständigkeit und erlaubt ebenfalls keinen neuen Leistungszugriff. Alle anderen Einstellungen desselben Profils bleiben dann ebenfalls unangewendet. Ein fehlender Dienst wird nur beim ausdrücklichen Windows-Fehler `ERROR_SERVICE_DOES_NOT_EXIST` als abwesend behandelt; Omen Hub++ verlangt damit keinen Original-Hub als Runtime-Abhängigkeit.

Unmittelbar vor jedem neuen PPT-/Slow-SET wird der Dienststatus erneut gelesen. Wird HP während der Transaktion aktiv, folgt ein Fehler mit angeforderter vollständiger Transaktionsrücksetzung, kein erneuter Dienststopp und keine wiederholte Tuning-Schleife. Das gilt für die unabhängigen SMU-Leistungsgrenzen; es ist keine vollständige Konfliktprüfung aller HP-Default-, GPU-, CO-, Windows- oder Lüfterpfade.

Hardware-Recovery bleibt getrennt: das Zurückgeben bereits erfasster Originale läuft nicht durch die Sperre für neue Profilvorgaben. Umgekehrt darf ein ausstehendes oder nicht lesbares Hardware-Recovery nicht durch Rückgabe der Dienstzuständigkeit übersprungen werden. Zunächst das Hardware-Recovery klären, anschließend den ursprünglichen Controller zurückgeben.

Eine nur vorübergehend gestoppte, weiterhin installierte OEM-Steuerung ist noch kein bestätigter Anmeldestart. Bis der eigene Startup-Übernahmeprozess qualifiziert ist, werden neue PPT-/Slow-Startup-Freigaben deshalb auch bei momentan gestopptem HPOmenCap verweigert. Bei tatsächlich entferntem Dienst bleibt unabhängiger Profilstart möglich; die übrigen Modell-/Treiber-/Profilprüfungen gelten weiter. Die lesende Startup-Bereitschaft prüft zusätzlich den aktuellen vollständigen Startup-Plan. Bestehende Nutzerauswahl/Freigaben werden durch eine reine Statusprüfung nicht verändert.

## Zustands- und Speichervertrag

`core/ownership.rs` kennt nur einen festen Gegenstand **HpOmenCap**. Eine Übernahme verlangt eine explizite Sitzungsfreigabe, einen frischen Besitzer mit Benutzer, PID, Prozess-Erzeugungszeit und Sitzungskennung sowie eine unveränderte Dienstidentität. Die PID allein genügt nicht. Abhängige Dienste, unklare Übergänge und ein ausstehendes Recovery sperren eine neue Übernahme.

Vor STOP muss `StopPending` dauerhaft geschrieben sein. Erst ein bestätigter Stillstand erlaubt `Owned`. Bei Rückgabe folgen `RestorePending` und nach bestätigtem Ausgangszustand `Restored`. War der Dienst vorher gestoppt, wird er nicht eigenmächtig gestartet. Hat eine andere Komponente diesen zuvor gestoppten Dienst inzwischen gestartet, markiert die Engine `Superseded` und respektiert die Änderung. Ein unerwarteter Neustart verliert die eigene Zuständigkeit; er führt nicht zu einem zweiten STOP.

Ein STOP-Fehler kann bereits Wirkung gehabt haben. Auch nach verlorenem API-Ergebnis oder einem Schreibfehler nach STOP versucht die Engine die Rückkehr zum ursprünglichen laufenden Dienst. Nicht bestätigte Wiederherstellung und nicht dauerhaft gespeicherter Abschluss bleiben als Fehler sichtbar. Ein verändertes Originalbild oder ein fremder Besitzer darf kein bestehendes Recovery verdrängen.

`platform/ownership_store.rs` speichert höchstens 16 KiB unter der geschützten Installation als `controller-ownership.json`. Schema, Felder und Identitäten werden geprüft; ungültige Dateien bedeuten niemals „kein Recovery“. Schreibzugriffe vergleichen die erwartete Revision, synchronisieren die Datei und ersetzen atomar. Frühere abgeschlossene Sitzungen werden bytegenau mit SHA-256-Namen unter `controller-history/` archiviert. Der Hardware-Profiljournal und dessen vollständige Originalbilder werden dafür nicht ersetzt.

## Windows-Grenze

Der native Adapter adressiert ausschließlich **HPOmenCap** auf der gemessenen Bindung **HP 8E35 / F.13 / Ryzen 9 8940HX**, mit dem identifizierten OmenCap.exe-Hash **9EECCBDD7BEAA9C33930551853A572AB1E56E2E9A72F6453DC53E190284FE0C8**. Er prüft Benutzerprozess-Diensttyp, festen DriverStore-Pfad, Konto LocalSystem, ursprüngliche Startart, Abhängigkeiten, tatsächlich laufenden Prozesspfad/PID/Erzeugungszeit und Konfigurationsdigest. Zusätzliche Argumente, Treiberdienste oder eine andere Datei werden abgewiesen. Die geprüfte Datei bleibt ohne Schreib-/Löschfreigabe geöffnet. Es gibt keine generischen Dienstnamen, beliebigen Kommandos oder Starttypänderungen aus einem Journal.

Statusleser öffnen nur Abfragerechte. Die ausdrückliche Reparatur öffnet zusätzlich START, **kein STOP**. Die vorbereitete Übernahme-Bibliothek besitzt einen getrennten maschinenweiten Lebensdauer-Mutex und kurze Hardware-Transaktionssperren. Sie prüft vor ihrer beaufsichtigten Qualifikation AC, kühle frische Sensoren, mindestens 4000 RPM beider Lüfter und bekannte konkurrierende Programme. Sie schließt oder beendet diese Programme nicht selbst. Diese Bibliotheksroute wird noch von keinem normalen UI-/Autostartpfad aufgerufen.

Die Statusbeobachtung ist begrenzt. Die zugrunde liegenden synchronen Windows-Dienstaufrufe können selbst bis zu 30 Sekunden benötigen; ein Zehn-Sekunden-Statusbudget ist deshalb keine Zehn-Sekunden-Garantie für die ganze Aktion. Verträge: [ControlService](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-controlservice), [StartServiceW](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-startservicew), [QueryServiceStatusEx](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-queryservicestatusex), [QueryServiceConfigW](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-queryserviceconfigw) und [QUERY_SERVICE_CONFIGW](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/ns-winsvc-query_service_configw).

## Diagnose und ausdrückliches Recovery

```powershell
# Reine Abfrage; kein UAC, Dienstbefehl oder Hardware-SET:
& '.\dist\Omen Hub++.exe' hp-control-status

# Nur bei tatsächlich ausstehender eigener Sitzungs-Recovery:
# Erhöhte Windows PowerShell und identisch installierte aktuelle Binary nötig.
& 'C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe' hp-control-recover
```

Die Statusabfrage unterscheidet den aktuellen Dienstzustand vom vollständigen Identitätsaudit. Fehlende Rechte bleiben ausdrücklich ein Auditfehler und werden nicht als geprüfte Identität ausgegeben. Ein unverändert installiertes älteres Release darf für die reine Journalinspektion bestehen bleiben; dadurch entsteht keine neue Freigabe.

Die Reparatur benötigt die geschützte passende Binary und ein gültiges Journal desselben Benutzers. Sie prüft Besitzerliveness über Kernel-Prozesshandle **und Erzeugungszeit**. Ein lebender oder nicht zugänglicher Besitzer blockiert; eine wiederverwendete PID gilt nicht als derselbe Prozess. Eine deaktivierte Anmeldeaufgabe verhindert diese Reparatur nicht, denn Binary-Identität und Autorun-Bereitschaft werden getrennt geprüft. Die Reparatur aktiviert weder Aufgaben noch Profile und kann keine neue Übernahme starten.

`scripts/check-hp-control.ps1` prüft dieselben lesenden Wege gegen einen ausdrücklich angegebenen Kandidatenhash. Optional vergleicht es die Profilvorschau mit dem laufenden Controller. Es hält Originale, Dienst-/Treiberzustände und Prozess-IDs vor/nach fest. Das Skript ruft keine Übernahme, Reparatur, Profilanwendung oder Installation auf.

## Noch zu qualifizieren

Vor Aktivierung eines dauerhaften App-Schalters braucht der dedizierte Besitzerprozess einen unabhängig bestätigten Rückkehrwächter und eine geprüfte Kühlungsübergabe. Reguläres Scope-Ende kann aufräumen; ein harter Prozessabbruch liefert vorerst ein dauerhaftes Recovery, keine bereits automatisch ausgeführte Rückkehr. Die vollständige native Dienstprüfung unter erhöhten Rechten, längere Profile, AC-/Display-/Suspend-/Resume-Wechsel und tatsächlicher Neustart bleiben eigene beaufsichtigte Proben. Bereits bestätigte kurzzeitige PowerShell-Isolation ist kein Ersatz für diese Abnahme.
