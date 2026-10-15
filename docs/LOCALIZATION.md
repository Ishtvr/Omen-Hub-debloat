# Lokalisierung

Alle sichtbaren Texte der App stehen in `crates/app/lang/<sprache>.json`; der Code verweist nur auf Schlüssel. Deutsch (`de.json`) ist die Referenzsprache, Englisch (`en.json`) die zweite.

## Sprache wählen

- Windows-Anzeigesprache Deutsch → Deutsch, sonst Englisch.
- `OMEN_HUB_LANG=de` oder `OMEN_HUB_LANG=en` erzwingt eine Sprache (für Screenshots und Übersetzer).
- Fehlt ein Schlüssel in der gewählten Sprache, gilt Deutsch; fehlt er überall, erscheint der Schlüssel selbst.

## Schlüssel

| Bereich | Muster | Beispiel |
| --- | --- | --- |
| Preset-Einstellung | `field.<Feature>` und `info.<Feature>` | `field.CpuStapmLimitMw`, `info.CpuStapmLimitMw` |
| Auswahlwert | `choice.<Choice>` | `choice.BestPerformance` |
| Gruppen/Abschnitte | `group.*`, `section.*` | `group.limits` |
| Fenster | `dash.*`, `preset.*`, `curve.*`, `widget.*`, `lighting.*`, `mode.*` | `curve.fan_hint` |
| Meldungen aus core/platform | `curve.err.*`, `cpu.err.*`, `windows.err.*`, … | `curve.err.max_points` |

Platzhalter heißen `{0}`, `{1}`, … und dürfen in jeder Sprache anders angeordnet sein. `info.*` ist der Text hinter dem runden **i** neben jeder Einstellung: was sie ändert und was das bewirkt.

## Meldungen ohne UI-Schicht

`core` und `platform` haben keine Oberfläche. Sie liefern nutzerrelevante Meldungen als Marker `⟦schlüssel␟arg…⟧` (`omen_core::message::msg`). Die App ersetzt jeden Marker beim Anzeigen (`i18n::message`, eingebaut in die Text-Helfer). Ein Argument `@schlüssel` wird selbst übersetzt, z. B. ein Feldname. Technische Diagnosedetails der Hardware-Adapter (englisch, für Logs und `cpu-sensor-status`) bleiben unverändert; die App zeigt dafür eigene übersetzte Meldungen.

## Neue Sprache

1. `de.json` nach `<code>.json` kopieren und alle Werte übersetzen.
2. Datei in `i18n.rs` per `include_str!` einbinden und in `language()` der Windows-Sprach-ID zuordnen.
3. `cargo test` prüft, dass alle Sprachdateien dieselben Schlüssel haben, jeder im Code verwendete Schlüssel existiert und jede Preset-Einstellung Name und Erklärung hat.
