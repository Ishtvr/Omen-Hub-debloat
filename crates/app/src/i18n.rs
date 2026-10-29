//! UI texts live in `lang/<code>.json`; code refers to them by key only. German
//! is the reference language: a key missing from another language falls back
//! to German, and a key missing everywhere shows the key itself.
use std::{
    collections::HashMap,
    fmt::Display,
    sync::{Mutex, OnceLock},
};

const GERMAN: &str = include_str!("../lang/de.json");
const ENGLISH: &str = include_str!("../lang/en.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    German,
    English,
}

impl Language {
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "de" => Some(Self::German),
            "en" => Some(Self::English),
            _ => None,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::German => "de",
            Self::English => "en",
        }
    }

    fn number(self, value: &str) -> String {
        match self {
            Self::German => value.replace('.', ","),
            Self::English => value.to_owned(),
        }
    }
}

static LANGUAGE: OnceLock<Language> = OnceLock::new();

fn take_language_argument(args: &mut Vec<String>) -> Result<Option<Language>, String> {
    if args.first().map(String::as_str) != Some("--lang") {
        return Ok(None);
    }
    let language = args
        .get(1)
        .and_then(|code| Language::from_code(code))
        .ok_or_else(|| omen_core::message::msg("language.invalid", &[]))?;
    args.drain(..2);
    Ok(Some(language))
}

/// Select once, before any UI/helper threads start. Fixed UAC commands carry
/// this option explicitly because shell elevation need not inherit our env.
pub fn configure(args: &mut Vec<String>) -> Result<(), String> {
    if let Some(language) = take_language_argument(args)? {
        LANGUAGE
            .set(language)
            .map_err(|_| omen_core::message::msg("language.already_selected", &[]))?;
    }
    Ok(())
}

pub fn language_code() -> &'static str {
    LANGUAGE.get_or_init(detect_language).code()
}

/// Localize display numbers only. Persistence and driver units stay invariant.
pub fn number(value: &str) -> String {
    LANGUAGE.get_or_init(detect_language).number(value)
}

struct Texts {
    german: HashMap<String, String>,
    english: HashMap<String, String>,
}

fn parse(source: &str) -> HashMap<String, String> {
    serde_json::from_str(source).unwrap_or_default()
}

/// German for a German Windows display language, English otherwise.
/// `OMEN_HUB_LANG=de|en` overrides it (for screenshots and translators).
fn detect_language() -> Language {
    if let Ok(code) = std::env::var("OMEN_HUB_LANG") {
        if let Some(language) = Language::from_code(&code) {
            return language;
        }
    }
    #[cfg(windows)]
    {
        // SAFETY: Parameterless query of the user's UI language.
        let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        if id & 0x3ff == 0x07 {
            return Language::German;
        }
    }
    Language::English
}

fn texts() -> &'static Texts {
    static TEXTS: OnceLock<Texts> = OnceLock::new();
    TEXTS.get_or_init(|| Texts {
        german: parse(GERMAN),
        english: parse(ENGLISH),
    })
}

/// The text for `key` in the current language.
pub fn t(key: &str) -> &'static str {
    lookup(*LANGUAGE.get_or_init(detect_language), key)
}

fn lookup(language: Language, key: &str) -> &'static str {
    let texts = texts();
    let selected = match language {
        Language::German => &texts.german,
        Language::English => &texts.english,
    };
    if let Some(text) = selected.get(key).or_else(|| texts.german.get(key)) {
        return text.as_str();
    }
    static MISSING: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut missing = MISSING
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    missing
        .entry(key.to_owned())
        .or_insert_with(|| Box::leak(key.to_owned().into_boxed_str()))
}

/// The text for `key` with `{0}`, `{1}`, … replaced by `args`.
pub fn tf(key: &str, args: &[&dyn Display]) -> String {
    let values: Vec<_> = args.iter().map(ToString::to_string).collect();
    format_template(t(key), &values)
}

/// Substitute once from the template, never from an argument. User names and
/// native diagnostics containing `{1}` must remain literal data.
fn format_template(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let token = &after[..end];
        let replacement = (!token.is_empty() && token.bytes().all(|b| b.is_ascii_digit()))
            .then(|| {
                token
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| args.get(index))
            })
            .flatten();
        if let Some(value) = replacement {
            out.push_str(value);
        } else {
            out.push_str(&rest[start..start + end + 2]);
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Known app errors use language-neutral message keys. Opaque OS/provider
/// diagnostics retain their exact text, explicitly labeled as technical detail.
pub fn error_message(detail: &str) -> String {
    error_in(*LANGUAGE.get_or_init(detect_language), detail, 0)
}

fn error_in(language: Language, detail: &str, depth: usize) -> String {
    // A preflight child may return a JSON-encoded Plan or ApplyFailure. Decode
    // known fields first so escaped marker arguments and raw JSON don't leak
    // into a user-facing status. Native diagnostics remain exact data.
    if depth < 4 && detail.len() <= 64 * 1024 {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(detail) {
            let error = value.get("Err").unwrap_or(&value);
            let mut reasons = Vec::new();
            if let Some(reason) = error.get("error").and_then(serde_json::Value::as_str) {
                reasons.push(reason);
            }
            for field in ["blockers", "rollback_errors"] {
                if let Some(items) = error.get(field).and_then(serde_json::Value::as_array) {
                    for item in items {
                        if let Some(reason) = item
                            .as_str()
                            .or_else(|| item.get("reason").and_then(serde_json::Value::as_str))
                        {
                            reasons.push(reason);
                        }
                    }
                }
            }
            if !reasons.is_empty() {
                return reasons
                    .into_iter()
                    .map(|reason| error_in(language, reason, depth + 1))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
    if detail.contains(omen_core::message::OPEN) {
        return message_in(language, detail);
    }
    let texts = texts();
    let selected = match language {
        Language::German => &texts.german,
        Language::English => &texts.english,
    };
    if selected.values().any(|value| value == detail) {
        return detail.to_owned();
    }
    format_template(
        lookup(language, "error.technical_details"),
        &[detail.to_owned()],
    )
}

/// `text` with every core/platform message marker (`⟦key␟arg…⟧`) replaced by
/// its translation. An argument written as `@key` is itself translated.
pub fn message(text: &str) -> String {
    message_in(*LANGUAGE.get_or_init(detect_language), text)
}

fn message_in(language: Language, text: &str) -> String {
    omen_core::message::render(text, |key, args| {
        let args = args
            .iter()
            .map(|a| {
                a.strip_prefix('@')
                    .map_or_else(|| a.to_string(), |k| lookup(language, k).to_owned())
            })
            .collect::<Vec<_>>();
        format_template(lookup(language, key), &args)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(source: &str) -> BTreeSet<String> {
        strict_keys(source).expect("Language catalogs need unique, nonempty string entries")
    }

    fn strict_keys(source: &str) -> Result<BTreeSet<String>, serde_json::Error> {
        struct Keys;
        impl<'de> serde::de::Visitor<'de> for Keys {
            type Value = BTreeSet<String>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("unique, nonempty translation strings")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut keys = BTreeSet::new();
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    if value.trim().is_empty() || !keys.insert(key) {
                        return Err(serde::de::Error::custom("Empty or duplicate translation"));
                    }
                }
                Ok(keys)
            }
        }
        let mut decoder = serde_json::Deserializer::from_str(source);
        let keys = serde::de::Deserializer::deserialize_map(&mut decoder, Keys)?;
        decoder.end()?;
        Ok(keys)
    }

    #[test]
    fn languages_have_the_same_keys() {
        let german = keys(GERMAN);
        let english = keys(ENGLISH);
        assert!(!german.is_empty());
        assert_eq!(
            german.difference(&english).collect::<Vec<_>>(),
            Vec::<&String>::new(),
            "missing in en.json"
        );
        assert_eq!(
            english.difference(&german).collect::<Vec<_>>(),
            Vec::<&String>::new(),
            "missing in de.json"
        );
    }

    #[test]
    fn every_key_used_in_code_exists() {
        let german = keys(GERMAN);
        let mut sources = vec![std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src"
        ))];
        let mut missing = vec![];
        while let Some(path) = sources.pop() {
            if path.is_dir() {
                sources.extend(
                    std::fs::read_dir(path)
                        .unwrap()
                        .map(|entry| entry.unwrap().path()),
                );
                continue;
            }
            // This module's own scanner text is not a translation call.
            if path.extension().is_none_or(|e| e != "rs") || path.ends_with("i18n.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for call in ["t(", "tf("] {
                for (at, _) in source.match_indices(call) {
                    // `get("…")` and the like are not translation calls.
                    let before = source[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let Some(rest) = source[at + call.len()..].trim_start().strip_prefix('"')
                    else {
                        continue;
                    };
                    let key = rest.split('"').next().unwrap();
                    if !key.is_empty() && !german.contains(key) {
                        missing.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "{missing:#?}");
    }

    #[test]
    fn every_preset_field_has_a_label_and_an_explanation() {
        let german = keys(GERMAN);
        for field in crate::preset_fields::fields() {
            for prefix in ["field", "info"] {
                let key = format!("{prefix}.{:?}", field.feature);
                assert!(german.contains(&key), "{key}");
            }
            for choice in field.choices() {
                let key = format!("choice.{choice:?}");
                assert!(german.contains(&key), "{key}");
            }
        }
    }

    #[test]
    fn every_message_key_from_core_and_platform_exists() {
        let german = keys(GERMAN);
        let mut missing = vec![];
        let mut pending = vec![
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/src")),
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../platform/src")),
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
        ];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                // The marker module's own test uses an example key.
                if path.extension().is_none_or(|e| e != "rs")
                    || path.ends_with("message.rs")
                    || path.ends_with("i18n.rs")
                {
                    continue;
                }
                let source = std::fs::read_to_string(&path).unwrap();
                for (at, _) in source.match_indices("msg(") {
                    let before = source[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let Some(rest) = source[at + 4..].trim_start().strip_prefix('"') else {
                        continue;
                    };
                    let key = rest.split('"').next().unwrap();
                    if !german.contains(key) {
                        missing.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "{missing:#?}");
    }

    #[test]
    fn markers_are_translated_with_nested_keys() {
        let marker = omen_core::message::msg("preset.err.changed_elsewhere", &[&"@field.CpuTdcMa"]);
        assert_eq!(
            message(&format!("x {marker}")),
            format!(
                "x {}",
                tf("preset.err.changed_elsewhere", &[&t("field.CpuTdcMa")])
            )
        );
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(
            tf("test.placeholder", &[&3, &"W"]),
            t("test.placeholder")
                .replace("{0}", "3")
                .replace("{1}", "W")
        );
    }

    #[test]
    fn duplicate_empty_and_invalid_catalog_entries_fail_validation() {
        assert!(strict_keys(r#"{"key":"a","key":"b"}"#).is_err());
        assert!(strict_keys(r#"{"key":"  "}"#).is_err());
        assert!(strict_keys(r#"{"key":1}"#).is_err());
        assert!(strict_keys(r#"{"key":"a"} trailing"#).is_err());
    }

    #[test]
    fn all_translations_preserve_the_same_placeholder_contract() {
        fn placeholders(text: &str) -> BTreeSet<usize> {
            text.split('{')
                .skip(1)
                .filter_map(|part| part.split('}').next()?.parse().ok())
                .collect()
        }
        let german = parse(GERMAN);
        let english = parse(ENGLISH);
        for (key, de) in &german {
            let en = english.get(key).expect("Matching language keys");
            assert_eq!(
                placeholders(de),
                placeholders(en),
                "Placeholder mismatch: {key}"
            );
        }
    }

    #[test]
    fn arguments_with_placeholder_text_are_never_substituted_again() {
        assert_eq!(
            format_template("{0} · {1} · {0}", &["Preset {1}".into(), "90 W".into()]),
            "Preset {1} · 90 W · Preset {1}"
        );
        assert_eq!(format_template("{99} {unfinished", &[]), "{99} {unfinished");
    }

    #[test]
    fn explicit_language_is_stripped_before_dispatch_and_invalid_codes_do_not_mutate_args() {
        for (code, expected) in [("de", Language::German), ("en", Language::English)] {
            let mut args = ["--lang", code, "profile-plan", "test"]
                .map(String::from)
                .to_vec();
            assert_eq!(take_language_argument(&mut args), Ok(Some(expected)));
            assert_eq!(args, ["profile-plan", "test"]);
        }
        for invalid in [vec!["--lang"], vec!["--lang", "fr"]] {
            let mut args: Vec<_> = invalid.into_iter().map(String::from).collect();
            let original = args.clone();
            assert!(take_language_argument(&mut args).is_err());
            assert_eq!(args, original);
        }
        let mut command = vec!["profile-show".into(), "--lang".into()];
        assert_eq!(take_language_argument(&mut command), Ok(None));
    }

    #[test]
    fn localized_display_numbers_keep_exact_profile_units_in_both_languages() {
        for language in [Language::German, Language::English] {
            for value in [90_000, 90_125, -50_125] {
                let invariant = omen_core::preset::format_number(value, 1000);
                let shown = language.number(&invariant);
                assert_eq!(omen_core::preset::parse_number(&shown, 1000), Ok(value));
            }
        }
        assert_eq!(Language::German.number("12.5"), "12,5");
        assert_eq!(Language::English.number("12.5"), "12.5");
    }

    #[test]
    fn json_preflight_and_rollback_messages_render_in_the_requested_language() {
        let reason =
            omen_core::message::msg("transaction.err.bounds", &[&115_001, &5_000, &115_000]);
        let plan =
            serde_json::json!({"blockers":[{"feature":"gpu_power_limit_mw","reason":reason}]})
                .to_string();
        for (language, expected) in [
            (Language::German, "außerhalb"),
            (Language::English, "outside"),
        ] {
            let shown = error_in(language, &plan, 0);
            assert!(shown.contains(expected), "{shown}");
            assert!(
                shown.contains("115001") && !shown.contains("blockers") && !shown.contains('⟦')
            );
            let failed = serde_json::json!({"Err":{"error":reason,"rollback_errors":[omen_core::message::msg("transaction.err.recovery_incomplete", &[])]}}).to_string();
            let shown = error_in(language, &failed, 0);
            assert!(shown.contains(lookup(language, "transaction.err.recovery_incomplete")));
        }
    }

    #[test]
    fn opaque_provider_details_are_labeled_and_preserved_without_reformatting() {
        let detail = "NVML status 3 / driver {1}";
        for (language, label) in [
            (Language::German, "Technische Details:"),
            (Language::English, "Technical details:"),
        ] {
            let shown = error_in(language, detail, 0);
            assert!(shown.starts_with(label));
            assert!(shown.ends_with(detail));
        }
    }

    #[test]
    fn direct_wide_literals_are_only_platform_identifiers_or_branding() {
        let sources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut unexpected = Vec::new();
        for entry in std::fs::read_dir(sources).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "rs") || path.ends_with("i18n.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let code = source.split("#[cfg(test)]").next().unwrap();
            for (offset, _) in code.match_indices("wide(") {
                let after = code[offset + 5..].trim_start();
                if let Some(literal) = after.strip_prefix('"').and_then(|s| s.split('"').next()) {
                    if !literal.is_empty()
                        && !literal.starts_with("OmenHubPP")
                        // The i glyph is the language-neutral information icon.
                        && !["Omen Hub++", "OmenHubPlusPlusWindow", "Segoe UI", "runas", "TaskbarCreated", "i"].contains(&literal)
                    {
                        unexpected.push(format!("{}: {literal}", path.display()));
                    }
                }
            }
        }
        assert!(
            unexpected.is_empty(),
            "Use language keys for display text: {unexpected:#?}"
        );
    }
}
