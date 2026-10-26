# NvpwrControl-Experiment: einstellbare GPU-TGP in Omen Hub++

Stand: **26. Oktober 2026**. Diese Datei ist ein Analyse- und Umsetzungsplan. Die aktuelle Runde umfasst die statische Projekt-/Archivprüfung und eine lesende Hardwarezuordnung mit dem vorhandenen NVIDIA-Werkzeug. Es wurde noch kein Fork angelegt. Programme, Installationsskripte oder Treibermodule aus NvpwrControl wurden nicht ausgeführt.

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

### 1. Nutzbarer User-Mode-Pfad und Rechte klären

- Für v2.3.0 nach veröffentlichten Schnittstellen, Quellcode und Lizenz suchen beziehungsweise diese vom Rechteinhaber beziehen, falls verfügbar. Es wurden keine Nachrichten versandt oder Käufe vorgenommen.
- Prüfen, ob dessen TGP-Pfad tatsächlich über normale NVIDIA-Schnittstellen arbeitet und ohne einen zusätzlich geladenen unsignierten oder anderweitig als Bypass verwendeten Treiber auskommt.
- Direkte C/C++-Übernahme oder Rust-Portierung erst auf geklärter Lizenz-/Freigabebasis planen. Ein öffentlicher GitHub-Fork ersetzt diese Prüfung nicht: [GitHub-Lizenzhinweise](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository).
- **Ergebnis:** nachvollziehbare Backend-/Rechteentscheidung. Falls ausschließlich die alte Kernel-Speicheränderung verfügbar ist, erfüllt sie die aktuelle User-Mode-/Secure-Boot-Vorgabe nicht und wird nicht in den Produktpfad integriert.

### 2. Experiment isolieren

- Nach dem Plan einen Referenz-Fork des verlinkten Repositories unter dem eigenen GitHub-Konto anlegen; Ursprung und Rechtehinweise erhalten.
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

**Der Ansatz ist untersuchenswert, aber noch kein nachgewiesen kompatibles Backend für unseren OMEN.** Für die aktuelle Vorgabe hat die neuere User-Mode-Methode Vorrang. Die ersten offenen Punkte sind der Zugang zu deren technischem Vertrag und die Lizenzfrage. Erst danach lässt sich entscheiden, ob ein vollständiger Rust-Pfad oder ein kleiner lizenzierter C/C++-User-Mode-Shim tatsächlich umsetzbar ist. Der alte Testsignierungs-Kernelpfad wird durch diesen Plan nicht als Lösung freigegeben.
