# Localization

The app includes German (`de.json`, reference) and English (`en.json`) under `crates/app/lang/`. Repository documentation uses English for current guides; archived measurement records retain their original language. Documentation language does not change the app's language selection.

## Selection and keys

German Windows display language selects German; other Windows display languages select English. `OMEN_HUB_LANG=de` or `OMEN_HUB_LANG=en` overrides the choice for a process. A missing selected-language key falls back to German, then to its literal key.

| Pattern | Purpose |
| --- | --- |
| `field.<Feature>`, `info.<Feature>` | Setting label and its explanation |
| `choice.<Choice>` | Choice text |
| `group.*`, `section.*` | Collapsible groups and GPU/CPU/Windows sections |
| `dash.*`, `preset.*`, `curve.*`, `widget.*`, `lighting.*`, `mode.*` | Window/action/status copy |
| `curve.err.*`, `cpu.err.*`, `windows.err.*` | User-facing lower-layer message keys |

Numbered placeholders `{0}`, `{1}`, etc. may be reordered. Core/platform return bounded message markers `⟦key␟argument…⟧`; the app renders them through `i18n::message`. An argument starting `@key` is itself translated. Technical adapter diagnostics remain unchanged and separate from user-facing recovery copy.

## Adding a language

1. Copy the reference JSON and translate values while retaining keys/placeholders.
2. Embed it in `i18n.rs` and map the supported language selection.
3. Run the workspace tests. Existing tests compare language keys, referenced keys and label/info coverage for preset fields.

New copy must distinguish measured state, selected preference, draft, unavailable, pending and confirmed application. Do not describe a watt limit as power draw, CO steps as exact mV, or metadata Save as hardware Apply. Current misleading fan quick-status copy is a tracked implementation issue in [Project review](PROJECT_REVIEW.md); this documentation change does not alter language files.
