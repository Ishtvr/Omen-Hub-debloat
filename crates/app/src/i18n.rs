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

struct Texts {
    selected: HashMap<String, String>,
    reference: HashMap<String, String>,
}

fn parse(source: &str) -> HashMap<String, String> {
    serde_json::from_str(source).unwrap_or_default()
}

/// German for a German Windows display language, English otherwise.
/// `OMEN_HUB_LANG=de|en` overrides it (for screenshots and translators).
fn language() -> &'static str {
    match std::env::var("OMEN_HUB_LANG").as_deref() {
        Ok("de") => return "de",
        Ok("en") => return "en",
        _ => {}
    }
    #[cfg(windows)]
    {
        // SAFETY: Parameterless query of the user's UI language.
        let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        if id & 0x3ff == 0x07 {
            return "de";
        }
    }
    "en"
}

fn texts() -> &'static Texts {
    static TEXTS: OnceLock<Texts> = OnceLock::new();
    TEXTS.get_or_init(|| Texts {
        selected: parse(if language() == "de" { GERMAN } else { ENGLISH }),
        reference: parse(GERMAN),
    })
}

/// The text for `key` in the current language.
pub fn t(key: &str) -> &'static str {
    let texts = texts();
    if let Some(text) = texts.selected.get(key).or_else(|| texts.reference.get(key)) {
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
    args.iter()
        .enumerate()
        .fold(t(key).to_owned(), |text, (i, arg)| {
            text.replace(&format!("{{{i}}}"), &arg.to_string())
        })
}

/// `text` with every core/platform message marker (`⟦key␟arg…⟧`) replaced by
/// its translation. An argument written as `@key` is itself translated.
pub fn message(text: &str) -> String {
    omen_core::message::render(text, |key, args| {
        let args = args
            .iter()
            .map(|a| {
                a.strip_prefix('@')
                    .map_or_else(|| a.to_string(), |k| t(k).to_owned())
            })
            .collect::<Vec<_>>();
        tf(
            key,
            &args.iter().map(|a| a as &dyn Display).collect::<Vec<_>>(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(source: &str) -> BTreeSet<String> {
        parse(source).into_keys().collect()
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
        let sources = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap();
        let mut missing = vec![];
        for entry in sources {
            let path = entry.unwrap().path();
            // This module's own scanner text is not a translation call.
            if path.ends_with("i18n.rs") {
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
}
