# NvpwrControl-Experiment: einstellbare GPU-TGP in Omen Hub++

Stand: **26. Oktober 2026, fortgeschriebene Untersuchung**. Auf ausdrücklichen Nutzerwunsch bleibt die aktuelle Phase bei der Untersuchung verschiedener Ansätze und der Überarbeitung dieses Plans. App-Code und Hardware-Schreibpfade werden nicht implementiert. Eine leere Codex-Worktree wurde vorbereitet; dort gibt es keine App-Codeänderung und noch keinen GitHub-Referenzfork. Programme, Installationsskripte oder Treibermodule aus NvpwrControl wurden nicht ausgeführt.

## Ziel und feste Vorgaben

Ein isolierter Entwicklungs-Fork soll untersuchen, ob sich die GPU-TGP über einen nachvollziehbaren NvpwrControl-ähnlichen Pfad in Watt einstellen und vollständig zurücksetzen lässt. Zielgerät ist zunächst unser eigener OMEN, nicht pauschal jede RTX-50-GPU.

- **TPM und Secure Boot bleiben aktiviert.** Die Anwendungsprozesse laufen im Usermode; Administratorrechte/UAC sind bei Bedarf erlaubt.
- Der untersuchte Produktpfad benötigt weder Testsignierung noch abgeschaltete Treibersignaturprüfung, eine zusätzliche Testzertifikat-Vertrauensstellung oder einen Sicherheits-Bypass.
- Bevorzugt wird ein User-Mode-Steuerpfad über die bereits vorhandenen NVIDIA-Treiber. Der öffentliche alte Nvpwr-Kerneltreiber ist damit keine direkt verwendbare Produktlösung.
- Rust hat Vorrang. C/C++ kommt nur als eng begrenzte, lizenzrechtlich geklärte User-Mode-Implementierung hinter einer festen C-ABI infrage. Eine Sprachänderung macht einen Kernel-Patcher nicht zu Usermode.
- Native/offline, bestehende Tray-Budgetvorgabe und vollständige Original-/Zielnachweise bleiben Produktziele. Die normale Installation erhält durch die Analyse keine zusätzlichen Treiber oder Dienste.

## Geprüfte Referenzen

Repository: [LevinAi-arch/rtx-5070ti-laptop-160w-power-limit](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit).

| Referenz | Ergebnis |
| --- | --- |
| `main` und Tag `v2.3.0` | Beide verweisen bei der Prüfung auf Commit `93f55e954e4dc3db1aefe5fb3bd4f0a1f8d283c3`. |
| [Release v2.3.0](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit/releases/tag/v2.3.0) | Autor beschreibt eine User-Mode-TGP-Steuerung ohne eigenen unsignierten Nvpwr-Treiber. Der Release hat keine GitHub-Download-Assets und verweist auf eine kostenpflichtige Pro-Ausgabe. Als eigene Referenz nennt der Autor einen Lenovo Legion Pro 5 mit RTX 5070 Ti Laptop. |
| Öffentlich nachvollziehbarer Code | Im geprüften Baum liegen C/C++-Quellen des v1.8.0-Zweigs mit eigenem Kernel-Treiber. Ein separater v2.3.0-User-Mode-TGP-Backendquelltext ist dort nicht erkennbar. Die root-EXE ist blobgleich zur EXE im mitgelieferten v1.8.0-Verzeichnis. |
| [Release v1.8.0](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit/releases/tag/v1.8.0) | Öffentliches ZIP mit Quelltexten und vorgefertigten Dateien; SHA-256 `DB42DD704B5DF477CB309BFAAFB27CFA0A691D0747509D93FD6C1D183D82BA37` geprüft. Der Tag verweist auf `ab8d6d0a3b05245dd6ad87afa29943337f06f2d5`. |
| Root-ZIP „v1.8.0 RTX 40/50“ | Separates Binärpaket ohne C/C++-Quellen; SHA-256 `3CB163D54ADEB314A2EDFC5C2277CBFE21A473CE5070CE369C54FA3EB15095DD`, zusätzlich mit dem gepinnten Git-Blob verglichen. |
| Versionszuordnung | Release-ZIP und aktueller Repository-Stand sind bei `driver.c`, `nvpwr_ioctl.h` und `main.cpp` nach normalisiertem Textvergleich nicht identisch. Vorhandene SYS-Kopien haben außerdem unterschiedliche Git-Blobs. Quellen, Paket und Binärdatei dürfen daher nicht als ein einheitlich reproduzierbarer Build behandelt werden. |
| Lizenz | Repository-Metadaten: `license: null`. Im vollständigen Baum, den beiden ZIP-Inventaren und den geprüften Quell-/Dokumentdateien wurde keine ausdrückliche Nutzungslizenz gefunden. Der README-Abschnitt „License & Disclaimer“ enthält keine erkennbare Codefreigabe. |

Fremdquellen und Archive liegen ausschließlich unter ignorierter `.local/research/nvpwrcontrol/`. In Git gelangen dieser eigene Plan und knappe Schnittstellenbeobachtungen, keine fremden Binärdateien oder Quellkopien.

## Technischer Befund des öffentlichen alten Pfads

Der gelesene [C-Treiber](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit/blob/93f55e954e4dc3db1aefe5fb3bd4f0a1f8d283c3/NvpwrControl_61692_v1_8_0_unified_blackwell_tuner/NvpwrControl_61692_v1_8_0_unified_blackwell_tuner/driver/driver.c) arbeitet an laufenden NVIDIA-Power-Policy-Objekten. Zusammengehörige Basis-/Boost-/Obergrenzen werden geändert und danach intern zurückgelesen. Dieser Ansatz liegt unterhalb unseres bisherigen öffentlichen NVML-Wattlimit-Adapters und ist grundsätzlich ein anderer Ansatzpunkt für dessen fehlenden Original-GET.

Wesentliche Abgrenzungen:

1. Der alte Pfad prüft das Kernel-Image für **616.92**. Er enthält buildabhängige Adressen, Layoutannahmen und Code-Erkennung. Unser **617.42** ist damit nicht automatisch kompatibel.
2. Für die normale **RTX 5070 Laptop** akzeptiert die untersuchte Funktion `IsSupportedTarget` bei kohärenter OEM-Basis 115 W Ziele von **120 bis 140 W** in 5-W-Schritten. Die 5070-Ti-Klasse hat einen anderen Bereich. Die Root-README-Tabelle stellt dies teilweise anders dar; für die Analyse ist die gepinnte Implementierung maßgeblich.
3. Der Code verwendet ein festes 25-W-Boostmodell. Unsere tatsächlichen Policy-Komponenten sind damit noch nicht identifiziert. Die Differenz zwischen NVML-Default und effektivem Limit belegt kein identisches internes Modell.
4. Die Rücksetzung nutzt einen während der Treibersitzung erfassten Zustand. `DriverUnload` stellt NVIDIA ausdrücklich **nicht** automatisch zurück. Ein Helfer-Neustart ist daher keine erwiesene Wiederherstellung.
5. Die [gemeinsame Schnittstelle](https://github.com/LevinAi-arch/rtx-5070ti-laptop-160w-power-limit/blob/93f55e954e4dc3db1aefe5fb3bd4f0a1f8d283c3/NvpwrControl_61692_v1_8_0_unified_blackwell_tuner/NvpwrControl_61692_v1_8_0_unified_blackwell_tuner/shared/nvpwr_ioctl.h) besitzt eigene Status-/Set-Versionen. Der zusätzliche XMG-v8-Vertrag ist ein anderer Geräte-/Nachrichtenvertrag und kein TGP-Backend für unseren HP.

Die Kernelmethode kann nicht durch einen Rust- oder C++-User-Mode-Wrapper direkt übernommen werden. Windows trennt den Adressraum einer Anwendung vom Kernel: [Microsoft-Dokumentation](https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/user-mode-and-kernel-mode). Die vom Autor angekündigte neuere User-Mode-Methode muss deshalb eigenständig nachvollzogen werden.

## Unser Zielgerät

Lesende Zuordnung während dieser Analyse:

| Eigenschaft | Wert |
| --- | --- |
| Gerät | HP OMEN 16-ap0xxx, Board 8E35, BIOS F.13 |
| GPU | RTX 5070 Laptop, **ohne Ti** |
| PCI-ID | `10DE:2D18` |
| Subsystem | `103C:8E35` |
| NVIDIA-Treiber | `617.42` |
| VBIOS | `98.06.2a.40.da` |
| Öffentliche NVML-Werte | Default 80 W, Bereich 5…115 W; früher separat effektiv 115 W gelesen |
| Fehlender Wert | Konfiguriertes/angefordertes Original-Limit ist im bisherigen öffentlichen Pfad nicht verfügbar |

Diese Angaben rechtfertigen weder 160 W noch eine übernommene 5070-Ti-Freigabe. Technisches erstes Ziel ist ein nachgewiesener, veränderbarer und zurücksetzbarer Grenzwert unter Einhaltung der Boot-/User-Mode-Vorgaben.

## Vorgehen

### Aktueller Untersuchungsschwerpunkt: eigene Nachbildung

Der Nutzer möchte den technischen Weg vom alten Kernelansatz zu einem User-Mode-Programm selbst nachvollziehen. Der Zugriff auf v2.3.0-Quellen ist deshalb **keine Voraussetzung für unsere weitere Untersuchung**. Die Pro-Ausgabe bleibt eine mögliche Vergleichsreferenz. Es wird nach einem eigenständig belegbaren Steuerweg über vorhandene NVIDIA-/OEM-Schnittstellen gesucht. Die Lizenzprüfung bleibt Voraussetzung für eine direkte Übernahme fremder Implementierung, nicht für das Dokumentieren eigener Schnittstellenbeobachtungen.

Ein Kernel-Funktionszeiger aus `Nvpwr.sys` ist kein im Usermode aufrufbarer Einstieg. Für jede Wirkung muss eine erreichbare, zugelassene Schnittstelle des vorhandenen Treibers gefunden werden. Eine Übersetzung der C-Syntax nach Rust allein löst diese Grenze nicht.

### Vergleich der möglichen Wege

| Weg | Vorhandene Belege | Offene Frage / nächster Nachweis | Einordnung |
| --- | --- | --- | --- |
| **A: NVIDIA-Control-Transport rückwärts verfolgen** | Ghidra zeigt in der exakt gehashten 617.42-Implementierung eine Transportkette der Power-Policy-Funktionen mit einem administrativen Zweig zu `\\.\NvAdminDevice`. Der DOS-Geräteeintrag ist auf unserem Rechner sichtbar. | Welche der Nvpwr-Wirkungen sind über diesen bestehenden NVIDIA-Endpunkt tatsächlich als User-Mode-Control erreichbar? Paketvertrag, Zugriffsrechte, ausgewählter Transportzweig und zugehöriger Kernel-Handler müssen zugeordnet werden. | **Höchste Untersuchungpriorität.** Ein realer vorhandener Einstieg, aber noch kein nachgewiesener TGP-Writer. Es wurde kein manueller IOCTL an dieses Gerät gesendet. |
| **B: Private NVAPI-Power-Policies** | `ClientPowerPoliciesGetInfo`, `GetStatus` und der Setter sind statisch vorhanden. Die Getter wurden über die passenden 617.42-Adressen und die belegten Versionsgrößen lesend untersucht. | Info v1/v2 und Status v1 liefern als normaler Benutzer Erfolg, aber keine gültigen Einträge. Privilegien-/Modellfilter, weitere Policy-Varianten und mögliche andere User-Mode-Einstiege untersuchen. Prozentwerte benötigen außerdem einen belegten Wattbezug. | **Direkt prüfbar, bisher keine nutzbare klassische Policy auf unserem Gerät.** Funktionspräsenz und Erfolgscode reichen nicht für eine Freigabe. |
| **C: Modernes öffentliches NVML** | Der v2-Power-Setter ist exportiert. Grenzen, Default und effektiver Wert sind über öffentliche Abfragen/Felder teilweise lesbar. | Scope-/Versionsunterschiede und einen tatsächlichen konfigurierten Originalwert klären. Der bisherige Limit-GET und das angeforderte Limit bleiben auf unserem Gerät nicht unterstützt; der GPU-Base-Scope wurde nicht akzeptiert. | **Sauberster API-Fallback, sofern vollständig verfügbar.** Ein vorhandener v2-Setter behebt nicht automatisch die fehlende Rücksetzungsinformation und beweist keine Erhöhung über OEM. |
| **D: HP-/NVPCF-Budgetsteuerung** | Eigene HP-WMI-Aufrufe für Leistungsmodi, cTGP-/PPAB-Vorgaben und gemeinsame Budgetsteuerung sind bereits untersucht. Das ist grundsätzlich aus dem Usermode erreichbar. | Existiert auf 8E35 ein tatsächlich numerischer GPU-Budget- oder TGP-Vertrag, oder werden nur feste Modi und Boost-Vorgaben angeboten? NVIDIA- und HP-Schreiberzuständigkeit abgleichen. | **OEM-nahe Ergänzung oder Fallback.** Ein Moduswechsel, CPU-Limit oder AC-/P-State-Schalter ist allein noch kein frei einstellbares GPU-Wattlimit. |
| **E: Neuere NvpwrControl-User-Mode-Methode als Referenz** | v2.3.0 beschreibt nach Autorangabe User-Mode-TGP-Steuerung ohne eigenen unsignierten Nvpwr-Treiber. | Die Methode ist in den geprüften öffentlichen Quellen nicht nachvollziehbar. Nur rechtmäßig verfügbare Quellen, Schnittstellendokumentation oder ein erlaubter Vergleich können die Zuordnung bestätigen. | **Optionaler Informationsgewinn, keine harte Abhängigkeit und keine bestätigte Lösung für unseren HP.** |
| **F: Regulär zugelassener signierter Kernel-Helper** | Eine User-Mode-App könnte technisch mit einem durch die aktuelle Windows-Treiberrichtlinie zugelassenen Kernel-Backend kommunizieren. Das wäre ein anderer Aufbau als die alte Testsignierungsvariante. | Erfordert eine zusätzliche Kernelkomponente, passende Zulassung/Signierung und eigene Geräte-/Lifetime-Nachweise. Ein selbstsigniertes Zertifikat oder Rust statt C genügt nicht. | **Architekturreserve, keine reine User-Mode-Nachbildung und nicht Teil der jetzigen Umsetzung.** TPM/Secure Boot und die aktuell geltende [Windows-Treiberrichtlinie](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy) müssten dabei erfüllt bleiben. |

A und B können am Ende denselben Herstellerpfad benutzen; sie sind unterschiedliche Untersuchungsrichtungen, keine bereits bewiesenen unabhängigen Lösungen. Die öffentlichen NVIDIA-Linux-Quellen dienen bei A als [Semantik-/Bezeichnungsreferenz](https://github.com/NVIDIA/open-gpu-kernel-modules), nicht als Beweis einer identischen Windows-ABI. Bisher gelesene öffentliche Power-/PMGR-/Perf-Header liefern für sich keinen vollständigen numerischen Windows-TGP-Schreibvertrag.

### Was inzwischen konkret untersucht wurde

- Die tatsächlich aufgelösten privaten Power-Getter liegen in der erwarteten NVIDIA-Implementierung mit SHA-256 `C6A2014763C3662512B5D420DFC7632B777338881A13AEAB3F21D8033EAE6C71`. Die relativen Einstiegspunkte stimmen mit der statischen Interface-Tabelle überein.
- Ghidra bestätigt für die untersuchten Getter Info v1 mit 184 Byte, Info v2 mit 2248 Byte und Status v1 mit 72 Byte. Die reinen GET-Proben verwenden begrenzte Frames mit zusätzlichen Kontrollbereichen; diese blieben unverändert.
- Als **nicht erhöhter Benutzer**: Info v1 liefert Status 0, Valid 0 und Count 0; Status v1 liefert Status 0 und Count 0. Die zusätzliche Info-v2-Abfrage liefert ebenfalls Status 0 mit leerem gültigem Policy-Angebot. Diese Antworten enthalten keinen konfigurierten Originalwert und kein freigegebenes Regelintervall.
- Statisch verfolgt wurde der Weg der Power-Policy-Wrapper in einen NVIDIA-Control-Dispatcher. In einem Zweig sind ein administrativer Gerätezugang und `DeviceIoControl` belegt. Daraus folgt noch nicht, dass genau dieser Zweig bei unseren Getteraufrufen gewählt wurde oder dass die alten Kernel-Direktänderungen darüber erreichbar sind.
- Die Sichtbarkeit von `NvAdminDevice` wurde nur über den DOS-Gerätenamensraum geprüft. Der Endpunkt wurde in dieser Probe nicht geöffnet und es wurde kein eigener Control-Puffer an ihn übergeben.
- Für NVML wurde lediglich die Präsenz des v2-Setter-Exports festgestellt. **Kein Setter wurde aufgerufen.** Ebenso wurden keine Nvpwr-Module geladen, keine Dienste gestoppt und keine Boot-/Signaturoptionen geändert.
- Der tatsächliche aktuelle TPM-/Secure-Boot-Zustand wurde in dieser Untersuchungsrunde nicht neu abgenommen. Dass die zukünftige Lösung diese Funktionen aktiviert lassen muss, bleibt eine feste Vorgabe.

Rohbelege liegen ignoriert in `.local/research/nvpwrcontrol/`; zusätzliche Ghidra-Decompilationen bleiben in `.local/research/native/mcp-evidence/`. Die App-Implementierung wurde dafür nicht verändert.

### Zuordnungsbuch: Kernelwirkung zu möglichem User-Mode-Vertrag

| Wirkung des alten Backends | Was für eine User-Mode-Nachbildung gefunden werden muss |
| --- | --- |
| Basis und dynamischen Anteil verändern | Lesbarer Originalzustand und ein typisierter Hersteller-Control mit belegten Einheiten und Modellgrenzen; keine pauschale Übernahme des 25-W-Modells. |
| MAX und obere Sättigungsgrenze konsistent anpassen | Ein Vertrag, der die zusammengehörigen Begrenzungen gültig aktualisiert. Direktes Schreiben an Kerneladressen ist keine User-Mode-Alternative. |
| NVIDIA-Generator auslösen | Belegter Commit-/Eligibility-/Policy-Aufruf, der die neue Vorgabe tatsächlich übernimmt; reine Anzeige- oder Cacheänderung reicht nicht. |
| Aktuellen Zustand prüfen | Angeforderte, effektive und weitere begrenzende Werte unterscheiden. Leere Policy-Listen und Defaultwerte sind keine Original-Snapshots. |
| Original vollständig zurückgeben | Gleiche GPU-/Treibersitzung, sämtliche eigenen Änderungen und ein eigener Restore-Vertrag. Prozessende oder Helferentladen dürfen nicht als Restore angenommen werden. |

### Unmittelbar nächste Untersuchungsschritte

1. **Transport- und Rechtezuordnung vervollständigen:** Administrative NVIDIA-Transportzweige den Power-Operationen zuordnen; lesenden Standard-/Administratorvergleich planen und nur belegte Frames verwenden. Keine Control-Code-Suche durch blindes Ausprobieren.
2. **Weitere erreichbare Policy-Klassen prüfen:** Die klassische v1/v2-Familie hat bisher keine Daten geliefert. Prüfen, ob das eine Modell-/Rechteauswahl ist oder ob ein anderer angebotener Power-/NVPCF-Vertrag benötigt wird.
3. **NVML v2 und OEM-Verträge abgrenzen:** Verfügbare Scopes, Einheiten und Originalwerte mit den bestehenden Ergebnissen vergleichen. Nur belegte Grenzwertsteuerung als TGP-Ansatz einstufen; reine Performance-/Boost-Präferenzen gesondert halten.
4. **Für jeden Kandidaten ein Ergebnisblatt erstellen:** Einstieg, Datenrichtung, erforderliche Rechte, Geräte-/Buildbindung, Snapshot, Wiederherstellung, Boot-Kompatibilität sowie belegte und weiterhin unbekannte Punkte.
5. **Go/No-Go für einen späteren Prototyp:** Erst wenn wenigstens ein Kandidat einen lesbaren Originalzustand und einen plausibel zugehörigen zugelassenen Schreib-/Rücksetzungsvertrag bietet, wird ein separater Implementierungsschritt geplant. Fehlt das, wird der Befund dokumentiert statt ein funktionierender User-Mode-Regler behauptet.

## Spätere Umsetzung — erst nach einem tragfähigen Untersuchungsbefund

### 1. Gefundenen User-Mode-Pfad und Wiederverwendungsrechte festhalten

- Den durch unsere Untersuchung belegten Herstellervertrag und dessen Geräte-/Buildgrenzen als Grundlage wählen. Ein Zugriff auf Pro-Quellcode ist dafür nicht zwingend nötig; deren mögliche Methode bleibt unbestätigt.
- Für jede direkte Übernahme fremder C/C++-Implementierung oder deren Portierung vorher Lizenz/Freigabe klären. Ein öffentlicher GitHub-Fork ersetzt diese Prüfung nicht: [GitHub-Lizenzhinweise](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository).
- **Ergebnis:** eigenständig nachvollziehbare Backend-/Rechteentscheidung. Der alte Testsignierungs-Kernelweg wird nicht stillschweigend als User-Mode-Lösung eingesetzt.

### 2. Experiment isolieren

- Nach Abschluss der Untersuchung gegebenenfalls einen Referenz-Fork des verlinkten Repositories unter dem eigenen GitHub-Konto anlegen; Ursprung und Rechtehinweise erhalten.
- Für die eigentliche Omen-Hub++-Integration einen eigenen Zweig **`codex/nvpwrcontrol-tdp`** mit getrennter Worktree verwenden. Basis ist der aktuelle Omen-Hub++-Stand `ca79bf3db1b0dd5b73ad95a6c56a39cdd020308a`.
- Keine binären Upstream-Artefakte in normale Release-Pakete kopieren. Abhängigkeiten und Nachweise getrennt erfassen.

### 3. Diagnose zunächst ohne SET

- Einen kleinen Rust-Prototyp für den nachgewiesenen User-Mode-Zugang schreiben. Er darf keinen eigenen Treiber automatisch installieren oder starten.
- Aktive GPU, Subsystem, VBIOS, geladene Treiber-/Bibliotheksidentität und die Epochengültigkeit des zurückgegebenen Zustands erfassen.
- Basis, Boostanteil, Obergrenzen, angeforderten und effektiven Zustand getrennt lesen. Nur verfügbare Werte interpretieren; Default und effektives Limit bleiben andere Größen als ein konfiguriertes Original.
- Private Schnittstellen müssen für **617.42** belegt sein. Die alten Kernel-Adressen oder der feste 25-W-Anteil werden nicht lediglich umetikettiert.
- **Ergebnis:** reproduzierbarer lesender Nachweis für unser Gerät mit TPM/Secure Boot aktiv. Noch keine Zusage eines schreibbaren TGP-Reglers.

### 4. Rust-Module und feste Verträge umsetzen

| Modul | Aufgabe | Sprache |
| --- | --- | --- |
| `omen-core::gpu_policy` | Semantischer Snapshot, Zustandsmodell, Zielprüfung und Rücksetzungsregeln | Rust, ohne Windows-Abhängigkeit |
| `platform::windows::nvpwr_user` | Fester User-Mode-Treiber-/API-Vertrag, Lebensdauer, Privilegien, Parser | Rust mit enger FFI-Grenze |
| Optionaler Backend-Shim | Nur belegte, lizenzierte Aufrufe, falls ein verfügbarer User-Mode-Teil nicht sinnvoll direkt portiert werden kann | Kleine C/C++-Bibliothek mit C-ABI |
| Bestehender Profilcontroller | Original-Capture, Journal, einmaliger SET, unabhängiger Readback und Restore | Rust |
| Vorhandenes Presetfeld | Wattvorgabe sowie klarer Original-/Ziel-/Effektivstatus | Bestehende native Rust-Oberfläche |

Das bestehende öffentliche NVML-Backend bleibt ein eigener Adapter. Der neue Adapter wird erst bei einem belegten Vertrag angeboten. Fremde interne Adressen oder beliebige IOCTL-Kommandos dürfen nicht aus Profilen oder der UI stammen. Rücksetzungsdaten binden GPU und Treibersitzung; alte Kernelzeiger sind keine über Neustart gültigen Wiederherstellungsdaten.

### 5. Erst Rücksetzung, dann kleine echte Änderung

- Zuerst gespeicherte Nachrichten-/Zustandsfälle und ABI-Tests prüfen: falsche Version, gemischter Zustand, geänderte Sitzung, verlorene Antwort, fremde Änderungen und misslungene Rückgabe.
- Den ersten echten Versuch innerhalb der ursprünglichen Gerätehülle planen, vorzugsweise mit einer kleinen Senkung und unmittelbarer vollständiger Rücksetzung.
- Der unveränderte alte 5070-Writer ist dafür ungeeignet: Er erlaubt erst Werte oberhalb 115 W. Außerdem ist ein gleiches Gesamtlimit kein No-op, wenn sich Basis oder Boostaufteilung ändern. Ein neuer User-Mode-Pfad braucht dafür einen eigenständig nachgewiesenen Vertrag.
- AC-/Temperaturdaten, unveränderte Core-/VRAM-/V/F-Steuerbilder, konkurrierende HP-/Tuning-Writer und alle zusammengehörenden Power-Werte prüfen.
- **Ergebnis:** echter SET/Readback/Original-Restore auf unserem OMEN; keine Verwechslung einer Limitanzeige mit tatsächlicher Leistungsaufnahme.

### 6. Profile, Lebenszyklus und optional erweiterter Bereich

- Danach Presets, Windows-Anmeldung, App-Abbruch, Standby/Resume und Treiber-/GPU-Neustart separat integrieren und prüfen.
- Vollständige Rückgabe muss vor Helfer-Neustart oder Aufgabe der eigenen Zuständigkeit bestätigt sein. Eine unklare Rücksetzung bleibt sichtbar.
- Erst nach dem Grundpfad getrennt über Werte oberhalb des OEM-Bereichs entscheiden und dafür passende Geräte-, Versorgungs-, Temperatur- und Lastnachweise erheben. Fehlende Hotspot-/VRAM-/VRM-Daten werden nicht erfunden.
- Kontrollieren, dass TPM und Secure Boot in der Zielkonfiguration weiterhin aktiviert sind und keine Signatur-/Boot-Sicherheitsoption für den Betrieb geändert werden muss.
- Normale Cargo-Prüfungen, Release-Build und die gesamten residenten Tray-Prozesse gehören zur abschließenden Abnahme. Die heutige Analyse ersetzt keinen dieser Nachweise.

## Entscheidung nach der Analyse

**Der Ansatz ist untersuchenswert, aber noch kein nachgewiesen kompatibles schreibbares Backend für unseren OMEN.** Die Untersuchung wird unabhängig von der Verfügbarkeit einer Pro-Ausgabe fortgeführt. Vorrang haben die Zuordnung vorhandener NVIDIA-Control-Transporte und die Suche nach einem tatsächlich angebotenen Policy-Vertrag. Die klassischen privaten Getter sind aufrufbar, haben in unseren nicht erhöhten Proben jedoch keine nutzbaren Policies geliefert. Ein administrativer Gerätezugang ist statisch und als Geräteobjekt sichtbar, aber noch nicht als TGP-Writer belegt. Rust-/C++-Implementierung und Hardware-Schreibtests bleiben nach der letzten Nutzeranweisung zurückgestellt. TPM und Secure Boot bleiben feste Anforderungen.
