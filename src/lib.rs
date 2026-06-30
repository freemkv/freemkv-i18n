// freemkv-i18n — i18n string loader (shared across the freemkv toolchain)
// AGPL-3.0 — freemkv project
//
// English is compiled into the binary (always available).
// Other languages loaded from disk at runtime — drop a JSON file, done.
//
// Language priority:
//   1. --language flag (set via set_language() before init())
//   2. LC_ALL / LC_MESSAGES / LANG env var (POSIX precedence)
//   3. English fallback
//
// Search paths for locale files:
//   1. <binary dir>/locales/xx.json (next to the binary)
//   2. ./locales/xx.json (working directory)
//   3. ~/.config/freemkv/locales/xx.json
//   4. /usr/share/freemkv/locales/xx.json
//
// To add a language: create locales/xx.json (copy en.json structure) and
// place it in any search path. No code changes needed.

use serde_json::Value;
use std::sync::OnceLock;

static STRINGS: OnceLock<Value> = OnceLock::new();
static LANG_OVERRIDE: OnceLock<String> = OnceLock::new();

// ── Shipped languages (auto-generated from locales/*.json by build.rs) ─────
include!(concat!(env!("OUT_DIR"), "/locales_generated.rs"));

/// Set language override from --language flag. Call before init().
///
/// Once `init()`/`get()` has locked in the active locale (`STRINGS`), this
/// override is dead: the language is already chosen. A call at that point is a
/// caller-ordering bug, so make it visible instead of silently no-opping.
pub fn set_language(lang: &str) {
    if STRINGS.get().is_some() {
        debug_assert!(
            false,
            "set_language(\"{lang}\") called after strings were initialized; the override is ignored"
        );
        eprintln!("warning: --language ignored (set after locale was initialized)");
        return;
    }
    let _ = LANG_OVERRIDE.set(lang.to_string());
}

/// Initialize strings for the current locale.
/// Priority: bundled locale → disk locale → English fallback.
pub fn init() {
    let code = detect_language();
    let json = if let Some(data) = bundled_locale(&code) {
        // Shipped language — compiled in
        serde_json::from_str(data).expect("invalid bundled locale")
    } else if let Some(v) = load_locale_file(&code) {
        // Community language — loaded from disk
        v
    } else {
        // Fallback to English. Notify the user when they explicitly requested a
        // locale that isn't available (VAL-3). Plain English is intentional here:
        // we cannot use strings::get/fmt because STRINGS isn't set yet.
        if code != "en" {
            eprintln!("freemkv: locale '{code}' not found, using English");
        }
        serde_json::from_str(LOCALE_EN).expect("invalid en.json")
    };
    let _ = STRINGS.set(json);
}

/// Get a string by dotted path (e.g. "disc.scanning", "error.no_drive").
/// Returns the path itself if not found — makes missing translations visible.
pub fn get(path: &str) -> String {
    let strings = STRINGS.get_or_init(|| serde_json::from_str(LOCALE_EN).expect("invalid en.json"));
    lookup(strings, path)
}

/// Get a string and replace {key} placeholders with values.
pub fn fmt(path: &str, args: &[(&str, &str)]) -> String {
    let mut s = get(path);
    for (key, val) in args {
        s = s.replace(&format!("{{{}}}", key), val);
    }
    s
}

/// Look up the localized message for a libfreemkv error code.
///
/// Error strings live under the dotted path `error.E<code>` (e.g. code `7022`
/// → key `error.E7022`), matching how `libfreemkv::Error::code()` values are
/// keyed in the locale files. The crate stays free of any `libfreemkv`
/// dependency by taking the bare numeric code rather than the `Error` type.
///
/// Falls back to the key path (`error.E<code>`) if the code has no string —
/// same miss behavior as [`get`], so a missing translation is visible rather
/// than silent.
pub fn error_message(code: u32) -> String {
    get(&format!("error.E{code}"))
}

/// Returns the bundled (compiled-in) locale JSON for `code`, if that language
/// is shipped with the crate. Exposed so downstream contract tests can validate
/// locale coverage against the exact bundled data without re-bundling the
/// files themselves (and without relying on a sibling-directory path that only
/// exists in a local checkout).
pub fn bundled_locale_json(code: &str) -> Option<&'static str> {
    bundled_locale(code)
}

// ── Internal ───────────────────────────────────────────────────────────────

fn detect_language() -> String {
    if let Some(lang) = LANG_OVERRIDE.get() {
        return normalize_code(lang);
    }
    // POSIX precedence: LC_ALL overrides every other locale variable, then
    // the category-specific LC_MESSAGES, then LANG as the fallback default.
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() && val != "C" && val != "POSIX" {
                return normalize_code(&val);
            }
        }
    }
    "en".to_string()
}

/// Try to load xx.json from search paths.
fn load_locale_file(code: &str) -> Option<Value> {
    let filename = format!("{}.json", code);

    // 1. Next to the binary
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("locales").join(&filename);
            if let Some(v) = try_load(&path) {
                return Some(v);
            }
        }
    }

    // 2. Working directory
    let path = std::path::PathBuf::from("locales").join(&filename);
    if let Some(v) = try_load(&path) {
        return Some(v);
    }

    // 3. ~/.config/freemkv/locales/
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::PathBuf::from(home)
            .join(".config/freemkv/locales")
            .join(&filename);
        if let Some(v) = try_load(&path) {
            return Some(v);
        }
    }

    // 4. /usr/share/freemkv/locales/
    let path = std::path::PathBuf::from("/usr/share/freemkv/locales").join(&filename);
    if let Some(v) = try_load(&path) {
        return Some(v);
    }

    None
}

fn try_load(path: &std::path::Path) -> Option<Value> {
    let data = std::fs::read_to_string(path).ok()?;
    // A missing file is fine (the loader tries the next search path), but a
    // file that exists yet fails to parse is almost certainly an operator
    // mistake — surface it instead of silently falling back to English.
    match serde_json::from_str(&data) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!(
                "freemkv: ignoring invalid locale file {}: {}",
                path.display(),
                e
            );
            None
        }
    }
}

/// "fr_FR.UTF-8" → "fr". Inputs are untrusted (the `--language` CLI flag and
/// the `LC_*`/`LANG` env vars), so this must never panic. The two-letter code is
/// taken by *character*, not byte, and validated as ASCII letters — anything
/// else (multibyte leading chars, digits, punctuation) falls back to English.
fn normalize_code(s: &str) -> String {
    // Strip any territory/codeset/modifier suffix (`fr_FR.UTF-8@euro` → `fr`)
    // before taking the language part.
    let lang = s
        .trim()
        .split(['_', '.', '-', '@', ':'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let code: String = lang.chars().take(2).collect();
    if code.chars().count() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        code
    } else {
        "en".to_string()
    }
}

fn lookup(strings: &Value, path: &str) -> String {
    let mut node = strings;
    for part in path.split('.') {
        match node.get(part) {
            Some(v) => node = v,
            None => return path.to_string(),
        }
    }
    match node.as_str() {
        Some(s) => s.to_string(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract `{word}` placeholders from a format string, matching exactly how
    /// `fmt` substitutes them: a balanced single `{...}` with no nested braces.
    /// Escaped/doubled braces (`{{`, `}}`) are skipped so a literal `{{val}}`
    /// does not register a malformed `{{val}` placeholder.
    fn placeholders(s: &str) -> Vec<String> {
        let bytes = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'{' {
                // Doubled `{{` is an escape, not a placeholder — skip both.
                if bytes.get(i + 1) == Some(&b'{') {
                    i += 2;
                    continue;
                }
                if let Some(rel_end) = s[i + 1..].find('}') {
                    let inner = &s[i + 1..i + 1 + rel_end];
                    // A real placeholder has no nested brace inside it.
                    if !inner.contains('{') {
                        out.push(format!("{{{}}}", inner));
                    }
                    i += 1 + rel_end + 1;
                    continue;
                }
            }
            i += 1;
        }
        out
    }

    /// Collect all dotted key paths from a JSON value (e.g. "app.usage", "error.E1000").
    fn collect_keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                if v.is_object() {
                    collect_keys(v, &path, out);
                } else {
                    out.push(path);
                }
            }
        }
    }

    fn verify_locale(code: &str, data: &str) {
        let locale: Value = serde_json::from_str(data)
            .unwrap_or_else(|e| panic!("{}.json: invalid JSON: {}", code, e));

        let en: Value = serde_json::from_str(LOCALE_EN).unwrap();
        let mut en_keys = Vec::new();
        collect_keys(&en, "", &mut en_keys);

        let mut locale_keys = Vec::new();
        collect_keys(&locale, "", &mut locale_keys);

        // Every English key must exist in the locale
        let mut missing = Vec::new();
        for key in &en_keys {
            if !locale_keys.contains(key) {
                missing.push(key.clone());
            }
        }
        assert!(
            missing.is_empty(),
            "{}.json missing {} keys: {:?}",
            code,
            missing.len(),
            missing
        );

        // Every {placeholder} in English must appear in the translation
        for key in &en_keys {
            let en_val = lookup(&en, key);
            let locale_val = lookup(&locale, key);
            for placeholder in placeholders(&en_val) {
                assert!(
                    locale_val.contains(&placeholder),
                    "{}.json key '{}': missing placeholder {} (got: '{}')",
                    code,
                    key,
                    placeholder,
                    locale_val
                );
            }
        }
    }

    #[test]
    fn normalize_code_does_not_panic_on_multibyte() {
        // Regression: byte-slicing `s[..2]` panicked on a leading multibyte
        // char (e.g. `--language あ`, `LC_ALL=€a`). Untrusted input must never
        // panic — it must fall back to English (or the ASCII language part).
        for input in ["あx", "€a", "Ⓐb", "😀x", "あ", "", ".", "_", "@", "ñ"] {
            let code = normalize_code(input);
            assert!(
                code == "en" || (code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic())),
                "normalize_code({input:?}) = {code:?}: must be 'en' or a 2-letter ASCII code"
            );
        }
    }

    #[test]
    fn normalize_code_extracts_language_part() {
        assert_eq!(normalize_code("fr_FR.UTF-8"), "fr");
        assert_eq!(normalize_code("de"), "de");
        assert_eq!(normalize_code("PT_BR"), "pt");
        assert_eq!(normalize_code("en-US"), "en");
        assert_eq!(normalize_code("es.UTF-8@modifier"), "es");
        // Non-letters fall back rather than producing a bogus code.
        assert_eq!(normalize_code("12"), "en");
        assert_eq!(normalize_code("x"), "en");
    }

    #[test]
    fn placeholders_skips_doubled_braces() {
        assert_eq!(placeholders("hi {name}!"), vec!["{name}".to_string()]);
        assert_eq!(placeholders("a {x} b {y}"), vec!["{x}", "{y}"]);
        // Doubled braces are escapes, not placeholders.
        assert!(placeholders("{{val}}").is_empty());
        assert_eq!(placeholders("{{lit}} {real}"), vec!["{real}".to_string()]);
        assert!(placeholders("no placeholders").is_empty());
    }

    #[test]
    fn locale_en_loads() {
        let _: Value = serde_json::from_str(LOCALE_EN).expect("en.json invalid");
    }

    #[test]
    fn open_failed_key_exists_and_fills_placeholders() {
        // Regression: `error.open_failed` was referenced by the drive-info /
        // CLI device-open path but never defined in any locale. A missing key
        // makes `lookup` return the dotted path verbatim, so `fmt` had no
        // `{device}`/`{error}` to substitute and the user saw the bare string
        // "error.open_failed". Guard that the key exists with both placeholders.
        let en: Value = serde_json::from_str(LOCALE_EN).expect("en.json invalid");
        let val = lookup(&en, "error.open_failed");
        assert_ne!(
            val, "error.open_failed",
            "error.open_failed missing from en.json (lookup returned the key verbatim)"
        );
        let ph = placeholders(&val);
        assert!(
            ph.contains(&"{device}".to_string()) && ph.contains(&"{error}".to_string()),
            "error.open_failed must contain {{device}} and {{error}} placeholders, got: '{}'",
            val
        );

        // And the full fmt() path produces a clean message with the values
        // substituted and no leftover placeholders or bare key.
        let out = fmt(
            "error.open_failed",
            &[("device", "/dev/sg0"), ("error", "permission denied")],
        );
        assert!(
            out.contains("/dev/sg0"),
            "device not substituted: '{}'",
            out
        );
        assert!(
            out.contains("permission denied"),
            "error not substituted: '{}'",
            out
        );
        assert!(!out.contains("{device}") && !out.contains("{error}"));
        assert_ne!(out, "error.open_failed");
    }

    #[test]
    fn locale_es_loads() {
        verify_locale("es", LOCALE_ES);
    }

    #[test]
    fn locale_fr_loads() {
        verify_locale("fr", LOCALE_FR);
    }

    #[test]
    fn locale_de_loads() {
        verify_locale("de", LOCALE_DE);
    }

    #[test]
    fn locale_it_loads() {
        verify_locale("it", LOCALE_IT);
    }

    #[test]
    fn locale_pt_loads() {
        verify_locale("pt", LOCALE_PT);
    }

    #[test]
    fn locale_nl_loads() {
        verify_locale("nl", LOCALE_NL);
    }

    #[test]
    fn bundled_locale_json_returns_shipped_languages() {
        // The seven shipped languages must be compiled in; an unknown code
        // returns None (it would be sought on disk at runtime instead).
        for code in ["en", "es", "fr", "de", "it", "pt", "nl"] {
            let data =
                bundled_locale_json(code).unwrap_or_else(|| panic!("{code} should be bundled"));
            let _: Value =
                serde_json::from_str(data).unwrap_or_else(|e| panic!("{code}.json invalid: {e}"));
        }
        assert!(bundled_locale_json("zz").is_none());
    }

    #[test]
    fn error_message_returns_real_string_not_key_path() {
        // E7022 ("no key source has a decryption key for this disc") is a known
        // code present in en.json. error_message must return its actual string,
        // not the `error.E7022` miss sentinel, and must keep the {hash}
        // placeholder so the caller can substitute the disc id.
        let msg = error_message(7022);
        assert_ne!(
            msg, "error.E7022",
            "error_message(7022) returned the key path — the string is missing or the key format is wrong"
        );
        assert!(
            msg.contains("{hash}"),
            "error_message(7022) should preserve the {{hash}} placeholder, got: '{}'",
            msg
        );

        // E1000 (drive not found) is another known code; sanity-check it too.
        assert_ne!(error_message(1000), "error.E1000");

        // An unknown code falls back to the key path (same miss behavior as get).
        assert_eq!(error_message(999_999), "error.E999999");
    }
}
