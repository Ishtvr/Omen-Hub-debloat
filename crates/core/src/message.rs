//! User-facing messages from layers without UI. Core and platform return a
//! language key with its arguments as `⟦key␟arg…⟧`; the app replaces each
//! marker with the text from its language files, so no layer hardcodes
//! display language.
use std::fmt::Display;

pub const OPEN: char = '⟦';
pub const CLOSE: char = '⟧';
pub const SEPARATOR: char = '\u{1f}';

/// The marker for `key` with `args`, for an error or status string.
pub fn msg(key: &str, args: &[&dyn Display]) -> String {
    let mut text = String::from(OPEN);
    text.push_str(key);
    for arg in args {
        text.push(SEPARATOR);
        // Arguments are data; drop marker characters so they cannot nest.
        text.extend(
            arg.to_string()
                .chars()
                .filter(|c| ![OPEN, CLOSE, SEPARATOR].contains(c)),
        );
    }
    text.push(CLOSE);
    text
}

/// Replaces every marker in `text` with `translate(key, args)`.
pub fn render(text: &str, translate: impl Fn(&str, &[&str]) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len_utf8()..];
        let Some(end) = after.find(CLOSE) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let mut parts = after[..end].split(SEPARATOR);
        let key = parts.next().unwrap_or_default();
        out.push_str(&translate(key, &parts.collect::<Vec<_>>()));
        rest = &after[end + CLOSE.len_utf8()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markers_round_trip_inside_other_text() {
        let error = format!("Steuerung: {}", msg("curve.max_points", &[&56, &"a⟧b"]));
        let shown = render(&error, |key, args| format!("<{key}:{}>", args.join(",")));
        assert_eq!(shown, "Steuerung: <curve.max_points:56,ab>");
        assert_eq!(
            render("plain ⟦broken", |_, _| String::new()),
            "plain ⟦broken"
        );
    }
}
