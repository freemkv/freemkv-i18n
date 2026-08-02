// freemkv-i18n — i18n string loader (shared across the freemkv toolchain)
// MIT — freemkv project
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
use std::sync::{OnceLock, RwLock};

// The active catalog. An `RwLock` (not `OnceLock`) so a running GUI can switch
// language live via `set_locale`; the CLI still sets it exactly once at
// startup. Reads (`get`/`fmt`) take the read lock; a language switch takes the
// write lock briefly. `None` until first use, then lazily initialized.
static STRINGS: RwLock<Option<Value>> = RwLock::new(None);
static LANG_OVERRIDE: OnceLock<String> = OnceLock::new();

// ── Shipped languages (auto-generated from locales/*.json by build.rs) ─────
include!(concat!(env!("OUT_DIR"), "/locales_generated.rs"));

/// Set language override from --language flag. Call before init().
///
/// Once `init()`/`get()` has locked in the active locale (`STRINGS`), this
/// override is dead: the language is already chosen. A call at that point is a
/// caller-ordering bug, so make it visible instead of silently no-opping.
pub fn set_language(lang: &str) {
    if STRINGS.read().unwrap().is_some() {
        debug_assert!(
            false,
            "set_language(\"{lang}\") called after strings were initialized; use set_locale to switch live"
        );
        eprintln!("warning: --language ignored (set after locale was initialized)");
        return;
    }
    let _ = LANG_OVERRIDE.set(lang.to_string());
}

/// Switch the active locale at runtime — the live path a GUI language change
/// uses. Unlike [`set_language`] (a one-shot pre-init override), this
/// re-resolves and swaps the catalog in place, so subsequent `get`/`fmt` calls
/// return the new language without a restart. `code` is any locale string
/// (`"de"`, `"pt-BR"`, `"auto"`); it is normalized and resolved the same way as
/// startup (full tag → base language → English). Safe to call repeatedly.
pub fn set_locale(code: &str) {
    let resolved = resolve_catalog(&normalize_code(code));
    *STRINGS.write().unwrap() = Some(resolved);
}

/// Initialize strings for the current locale (CLI startup). Idempotent-ish: it
/// (re)resolves from the environment / `--language` override and installs the
/// catalog. `get`/`fmt` also initialize lazily, so calling this is optional.
pub fn init() {
    let json = resolve_catalog(&detect_language());
    *STRINGS.write().unwrap() = Some(json);
}

/// Resolve a locale code to a catalog with the region-aware fallback: try the
/// full tag (`pt-br`), then the base language (`pt`), then English. So a
/// `pt-BR` user gets Brazilian strings when that catalog ships and European
/// Portuguese otherwise.
fn resolve_catalog(code: &str) -> Value {
    load_for(code)
        .or_else(|| {
            let base = base_language(code);
            (base != code).then(|| load_for(base)).flatten()
        })
        .unwrap_or_else(|| {
            // Notify the user when they explicitly requested a locale that isn't
            // available (VAL-3). Plain English is intentional here.
            if code != "en" {
                eprintln!("freemkv: locale '{code}' not found, using English");
            }
            serde_json::from_str(LOCALE_EN).expect("invalid en.json")
        })
}

/// Load one locale code: compiled-in first, then disk search paths. `None` if
/// that exact code has no catalog anywhere (the caller then tries the base
/// language, then English).
fn load_for(code: &str) -> Option<Value> {
    if let Some(data) = bundled_locale(code) {
        return Some(serde_json::from_str(data).expect("invalid bundled locale"));
    }
    load_locale_file(code)
}

/// Get a string by dotted path (e.g. "disc.scanning", "error.no_drive").
/// Returns the path itself if not found — makes missing translations visible.
pub fn get(path: &str) -> String {
    // Fast path: catalog already loaded — take the read lock and look up.
    {
        let guard = STRINGS.read().unwrap();
        if let Some(v) = guard.as_ref() {
            return lookup(v, path);
        }
    }
    // First use with no explicit init(): resolve from the environment /
    // --language override (honoring any pre-init set_language), install it, and
    // retry. The double-check under the write lock avoids a lost update if two
    // threads race here.
    {
        let mut w = STRINGS.write().unwrap();
        if w.is_none() {
            *w = Some(resolve_catalog(&detect_language()));
        }
    }
    let guard = STRINGS.read().unwrap();
    lookup(guard.as_ref().unwrap(), path)
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
        if let Ok(val) = std::env::var(var)
            && !val.is_empty()
            && val != "C"
            && val != "POSIX"
        {
            return normalize_code(&val);
        }
    }
    "en".to_string()
}

/// Try to load xx.json from search paths.
fn load_locale_file(code: &str) -> Option<Value> {
    let filename = format!("{}.json", code);

    // 1. Next to the binary
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let path = dir.join("locales").join(&filename);
        if let Some(v) = try_load(&path) {
            return Some(v);
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

/// Canonicalize a locale string to an internal tag `lang[-script][-region]`,
/// all lowercase, hyphen-joined: `pt_BR.UTF-8` → `pt-br`, `zh_Hant` → `zh-hant`,
/// `en-US` → `en-us`, `de` → `de`. Region/script are PRESERVED (no more
/// collapse to two letters), so `pt-BR` ≠ `pt` and Simplified ≠ Traditional.
///
/// Inputs are untrusted (the `--language` CLI flag and the `LC_*`/`LANG` env
/// vars), so this must never panic — it iterates by *character*, validates
/// ASCII, and falls back to English on anything malformed.
///
/// Chinese without an explicit script infers one from the region (tw/hk/mo →
/// hant, else hans) and defaults to Simplified, so `zh_CN` → `zh-hans`,
/// `zh_TW` → `zh-hant`, bare `zh` → `zh-hans`.
fn normalize_code(s: &str) -> String {
    // Drop codeset/modifier (`.UTF-8`, `@euro`), keep language[_-subtags].
    let base = s.trim().split(['.', '@', ':']).next().unwrap_or("");
    let mut parts = base.split(['_', '-']).filter(|p| !p.is_empty());
    let lang = parts.next().unwrap_or("").to_ascii_lowercase();
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return "en".to_string();
    }
    let mut script: Option<String> = None;
    let mut region: Option<String> = None;
    for p in parts {
        if p.len() == 4 && p.chars().all(|c| c.is_ascii_alphabetic()) && script.is_none() {
            script = Some(p.to_ascii_lowercase()); // hans, hant, latn, cyrl …
        } else if (p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic())
            || p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
            && region.is_none()
        {
            region = Some(p.to_ascii_lowercase()); // br, us, 419 …
        }
    }
    // Chinese script inference (only when no explicit script subtag). Folds the
    // region into the script so `zh_TW` and `zh_CN` map to the two catalogs.
    if lang == "zh" && script.is_none() {
        script = Some(match region.as_deref() {
            Some("tw") | Some("hk") | Some("mo") => "hant".to_string(),
            _ => "hans".to_string(),
        });
        region = None;
    }
    let mut out = lang;
    if let Some(sc) = script {
        out.push('-');
        out.push_str(&sc);
    }
    if let Some(rg) = region {
        out.push('-');
        out.push_str(&rg);
    }
    out
}

/// The base language subtag (`pt-br` → `pt`, `zh-hant` → `zh`).
fn base_language(code: &str) -> &str {
    code.split('-').next().unwrap_or(code)
}

fn lookup(strings: &Value, path: &str) -> String {
    if let Some(s) = lookup_in(strings, path) {
        return s;
    }
    // Per-key English fallback. The catalog-level fallback above only chooses
    // WHICH file to load; without this, a key present in en.json but missing
    // from the active locale rendered as the raw path — a user running under
    // `de` saw the literal text `error.E9053`.
    //
    // That is strictly worse than English: it is not a message in any language,
    // and it leaks an internal key into the UI. A translation gap should degrade
    // to a language the user may not read, not to something nobody can.
    if let Ok(en) = serde_json::from_str::<Value>(LOCALE_EN)
        && let Some(s) = lookup_in(&en, path)
    {
        return s;
    }
    path.to_string()
}

/// One catalog, no fallback. Returns `None` for a missing or non-string node so
/// the caller can decide what to do about it.
fn lookup_in(strings: &Value, path: &str) -> Option<String> {
    let mut node = strings;
    for part in path.split('.') {
        node = node.get(part)?;
    }
    node.as_str().map(|s| s.to_string())
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

    /// Collect all dotted key paths from a JSON value (e.g. "disc.scanning", "error.E1000").
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

        // ...and no key may exist in the locale that English does not have.
        // Parity was one-directional, so retiring a string from en.json left it
        // behind in all 29 locales with nothing to flag it: a stale entry that
        // translators keep re-translating and reviewers keep reading as live.
        // Both directions now, so a removal has to be completed everywhere.
        let mut extra = Vec::new();
        for key in &locale_keys {
            if !en_keys.contains(key) {
                extra.push(key.clone());
            }
        }
        assert!(
            extra.is_empty(),
            "{}.json has {} key(s) not present in en.json: {:?}",
            code,
            extra.len(),
            extra
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
            // A malformed leading subtag must fall back to English; a valid one
            // yields a lowercase-ASCII tag (lang plus optional -script/-region).
            assert!(
                code == "en"
                    || code
                        .split('-')
                        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric())),
                "normalize_code({input:?}) = {code:?}: must be 'en' or a clean ASCII tag"
            );
        }
    }

    #[test]
    fn normalize_code_extracts_language_part() {
        // Region and script are now PRESERVED (lowercase, hyphen-joined).
        assert_eq!(normalize_code("fr_FR.UTF-8"), "fr-fr");
        assert_eq!(normalize_code("de"), "de");
        assert_eq!(normalize_code("PT_BR"), "pt-br");
        assert_eq!(normalize_code("en-US"), "en-us");
        assert_eq!(normalize_code("es.UTF-8@modifier"), "es");
        assert_eq!(normalize_code("es-419"), "es-419");
        assert_eq!(normalize_code("zh_Hant"), "zh-hant");
        // Chinese region → script inference.
        assert_eq!(normalize_code("zh_TW"), "zh-hant");
        assert_eq!(normalize_code("zh_CN"), "zh-hans");
        assert_eq!(normalize_code("zh"), "zh-hans");
        // Non-letters fall back rather than producing a bogus code.
        assert_eq!(normalize_code("12"), "en");
        assert_eq!(normalize_code("x"), "en");
    }

    #[test]
    fn base_language_strips_subtags() {
        assert_eq!(base_language("pt-br"), "pt");
        assert_eq!(base_language("zh-hant"), "zh");
        assert_eq!(base_language("de"), "de");
    }

    #[test]
    fn every_shipped_locale_has_full_key_coverage() {
        // Auto-covers every locale file (incl. regional variants) so a new
        // catalog can't ship with missing keys or dropped placeholders.
        for code in SHIPPED_CODES {
            if *code == "en" {
                continue;
            }
            let data = bundled_locale_json(code)
                .unwrap_or_else(|| panic!("{code} in SHIPPED_CODES but not bundled"));
            verify_locale(code, data);
        }
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

    /// The key-SOURCE failure codes must each render as their own message, and
    /// none of them may render as E7022's "no key source has a key for this
    /// disc". That sentence is what an operator read during a seven-hour HTTP 502
    /// outage before going to hunt for a VUK that was never missing — the three
    /// codes exist precisely so the outage does not borrow it.
    #[test]
    fn key_service_failure_codes_do_not_reuse_the_missing_key_wording() {
        let missing_key = error_message(7022);
        let mut seen = Vec::new();
        for code in [7028u32, 7029, 7030] {
            let msg = error_message(code);
            assert_ne!(
                msg,
                format!("error.E{code}"),
                "E{code} has no English string"
            );
            assert_ne!(
                msg, missing_key,
                "E{code} must not reuse E7022's missing-key wording"
            );
            assert!(
                !seen.contains(&msg),
                "E{code} duplicates another code's text"
            );
            seen.push(msg);
        }
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    /// A key present in English but missing from the active locale must render
    /// the ENGLISH text, never the raw path.
    ///
    /// Before the per-key fallback, `lookup` returned the path itself, so a user
    /// under a non-English locale saw the literal string `error.E9053` where a
    /// message belonged. Five error codes added during the 1.6.0 audit were in
    /// exactly that state — and two of them exist because that audit split apart
    /// codes that had been reporting total failure as success, so the fix that
    /// made those failures visible would have shown an internal key.
    #[test]
    fn a_key_missing_from_the_locale_falls_back_to_english() {
        // A catalog that deliberately lacks the key.
        let sparse: Value = serde_json::json!({ "error": { "E9999": "present" } });
        let got = lookup(&sparse, "error.E9053");
        assert_ne!(
            got, "error.E9053",
            "a missing key must not render as its own path"
        );
        assert!(
            got.contains("mkv://"),
            "expected the English text for E9053, got {got:?}"
        );
        // A key missing from BOTH still degrades to the path — there is nothing
        // better to show, and silently returning empty would be worse.
        assert_eq!(lookup(&sparse, "error.E0000"), "error.E0000");
    }

    /// Every error code libfreemkv can raise must have an English string. This is
    /// the check that was absent: five codes shipped without one because nothing
    /// compared the two lists.
    #[test]
    fn the_audit_added_codes_all_have_english_strings() {
        let en: Value = serde_json::from_str(LOCALE_EN).expect("en.json parses");
        for code in ["E7027", "E9051", "E9052", "E9053", "E9054"] {
            let path = format!("error.{code}");
            let got = lookup(&en, &path);
            assert_ne!(got, path, "{code} has no English string");
            assert!(
                got.len() > 20,
                "{code} string looks like a placeholder: {got:?}"
            );
        }
    }
}
