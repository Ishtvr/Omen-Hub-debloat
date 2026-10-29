# Localization

The app includes German (`de.json`, reference) and English (`en.json`) under `crates/app/lang/`, with 543 matching keys in each catalog. Repository documentation uses English for current guides; archived measurement records retain their original language. Documentation language does not change the app's language selection.

## Selection and keys

Language precedence is an explicit `--lang de|en` prefix, then the process's `OMEN_HUB_LANG=de|en`, then Windows display language (German → German, otherwise English). The choice is immutable for that app process; it does not alter Windows language or profile data.

```powershell
.\target\release\omen-hub-pp.exe --lang de
.\target\release\omen-hub-pp.exe --lang en --tray
.\target\release\omen-hub-pp.exe --lang en presets-ui
.\target\release\omen-hub-pp.exe --lang de --help
```

Separately launched control windows and fixed UAC actions explicitly carry the chosen language. The fixed scheduled-task actions and machine protocols stay unchanged. Native Windows dialogs, consent prompts and vendor/OS diagnostic text can follow the installed Windows resources; their original technical details are not machine-translated.

A missing selected-language key falls back to German, then to its literal key. Tests reject missing/duplicate/empty entries and mismatched numbered placeholders before shipping.

| Pattern | Purpose |
| --- | --- |
| `field.<Feature>`, `info.<Feature>` | Setting label and its explanation |
| `choice.<Choice>` | Choice text |
| `group.*`, `section.*` | Collapsible groups and GPU/CPU/Windows sections |
| `dash.*`, `preset.*`, `curve.*`, `widget.*`, `lighting.*`, `mode.*` | Window/action/status copy |
| `curve.err.*`, `cpu.err.*`, `windows.err.*` | User-facing lower-layer message keys |

Numbered placeholders `{0}`, `{1}`, etc. may be reordered. They are substituted once from the template, never again from a user's name or diagnostic argument containing placeholder-looking text. Core/platform return bounded message markers `⟦key␟argument…⟧`; the app renders them through `i18n::message`. An argument starting `@key` is itself translated.

Profile, preflight, recovery and support errors use language-neutral keys. UI presentation decodes known JSON error/blocker/rollback fields before rendering, so escaped markers and raw JSON frames do not become status text. Unexpected native/provider diagnostics retain their exact data under a translated **Technical details** label. Control-flow signals such as the shared hardware-busy code must remain independent of localized prose.

Display numbers use a decimal comma in German and a point in English. Driver units, JSON and profile persistence remain invariant. Both input forms continue to parse exact milliwatts; translating a display number must not change a saved target.

## Voice and terminology

Use concise, professional language with a small amount of familiar PC-enthusiast terminology. Prefer concrete effects and conditional performance claims. Do not add hype, jokes or slang to recovery/permission errors.

| German | English | Use |
| --- | --- | --- |
| Preset | Preset | Saved group of optional settings |
| Boost / Boost-Spielraum | Boost / boost headroom | Explain conditional clock/power behavior |
| Undervolting | Undervolting | Tuning intent, not a stability promise |
| Offset / Powerlimit | Offset / power limit | Use alongside units and actual control scope |
| Idle / Leerlauf | Idle | Light technical use; ordinary instructions can say Leerlauf |
| VRAM / Numpad | VRAM / numpad | Familiar component names |

Keep German labels natural: Takt, Auslastung, Leistungsprofil, Anwenden and Wiederherstellen remain preferable to forced English everywhere. Registered names and established terms such as Dynamic Boost, Curve Optimizer, PBO, Tctl and MUX stay consistent. CO steps are not fixed mV; Max-Fan Off is not confirmed BIOS handover; a stored MUX preference is not confirmed GPU routing.

## Adding a language

1. Copy the reference JSON and translate values while retaining keys/placeholders.
2. Embed it in `i18n.rs` and map the supported language selection.
3. Run the workspace tests. They check key parity, duplicate/empty entries, placeholders, referenced keys, preset label/info coverage, direct display literals, literal argument preservation, exact localized units and structured errors in both languages.

New copy must distinguish measured state, selected preference, draft, unavailable, pending and confirmed application. Fan quick-status wording now confirms selection only; fresh matching helper-status confirmation remains a separate [review item](PROJECT_REVIEW.md). No localization change grants new hardware support.
