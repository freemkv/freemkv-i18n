use super::*;
use std::cell::RefCell;

// Extract {word} placeholders by driving the PRODUCTION scanner rather
// than reimplementing it, so this can't drift from what fmt substitutes.
fn placeholders(s: &str) -> Vec<String> {
    let found = RefCell::new(Vec::new());
    substitute(s, |name| {
        found.borrow_mut().push(format!("{{{name}}}"));
        None
    });
    found.into_inner()
}

// Build a `get_var`-shaped closure over explicit pairs, shared by every
// test that fakes the environment for locale_from_env /
// locale_candidates_from_env.
fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |var: &str| {
        owned
            .iter()
            .find(|(k, _)| k == var)
            .map(|(_, v)| v.to_string())
    }
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
    let locale: Value =
        serde_json::from_str(data).unwrap_or_else(|e| panic!("{}.json: invalid JSON: {}", code, e));

    let en = english_catalog();
    let mut en_keys = Vec::new();
    collect_keys(en, "", &mut en_keys);

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

    // Placeholders must match EXACTLY, in both directions, against the RAW catalogs (not
    // through the production fallback).
    for key in &en_keys {
        let en_val = lookup_in(en, key)
            .unwrap_or_else(|| panic!("en.json key '{key}' is not a non-empty string"));
        let locale_val = lookup_in(&locale, key).unwrap_or_else(|| {
            panic!("{code}.json key '{key}': not a string (number, object, bool or null)")
        });
        assert!(
            !locale_val.trim().is_empty(),
            "{code}.json key '{key}': empty value — renders as a blank message, \
                 which reads as success"
        );

        let mut en_ph = placeholders(&en_val);
        let mut locale_ph = placeholders(&locale_val);
        en_ph.sort();
        en_ph.dedup();
        locale_ph.sort();
        locale_ph.dedup();
        assert_eq!(
            en_ph, locale_ph,
            "{code}.json key '{key}': placeholders differ from en.json \
                 (en: '{en_val}', {code}: '{locale_val}')"
        );
    }
}

#[test]
fn normalize_code_does_not_panic_on_multibyte() {
    // Regression: byte-slicing s[..2] panicked on a leading multibyte char.
    for input in ["あx", "€a", "Ⓐb", "😀x", "あ", "", ".", "_", "@", "ñ", "de"] {
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
    // The POSIX locales are not language subtags, so they land on English
    // without needing a special case — see `locale_from_env`.
    assert_eq!(normalize_code("C"), "en");
    assert_eq!(normalize_code("C.UTF-8"), "en");
    assert_eq!(normalize_code("POSIX"), "en");
}

/// Catches the missing `lang-script` rung: a chain that goes straight from
/// the full tag to the first subtag skips `zh-hans` entirely.
#[test]
fn fallback_chain_walks_every_prefix_longest_first() {
    assert_eq!(
        fallback_chain("zh-hans-cn"),
        ["zh-hans-cn", "zh-hans", "zh"]
    );
    assert_eq!(
        fallback_chain("zh-hant-hk"),
        ["zh-hant-hk", "zh-hant", "zh"]
    );
    assert_eq!(
        fallback_chain("sr-latn-rs"),
        ["sr-latn-rs", "sr-latn", "sr"]
    );
    assert_eq!(fallback_chain("pt-br"), ["pt-br", "pt"]);
    assert_eq!(fallback_chain("es-419"), ["es-419", "es"]);
    assert_eq!(fallback_chain("de"), ["de"]);
}

// The bug a Simplified Chinese user actually felt: macOS hands the process
// zh-Hans-CN, no zh.json ships, and a two-step fallback jumped straight
// over the compiled-in zh-hans.json to silent English.
#[test]
fn three_subtag_tags_reach_their_script_catalog() {
    let hans: Value = serde_json::from_str(bundled_locale_json("zh-hans").unwrap()).unwrap();
    let hant: Value = serde_json::from_str(bundled_locale_json("zh-hant").unwrap()).unwrap();

    for tag in ["zh-Hans-CN", "zh_Hans_SG", "zh-Hans"] {
        let got = resolve_catalog_tagged(&normalize_code(tag)).0;
        assert_eq!(got, hans, "{tag} must resolve to the Simplified catalog");
        assert_ne!(
            got,
            *english_catalog(),
            "{tag} silently fell back to English"
        );
    }
    for tag in ["zh-Hant-TW", "zh-Hant-HK", "zh_Hant_MO"] {
        let got = resolve_catalog_tagged(&normalize_code(tag)).0;
        assert_eq!(got, hant, "{tag} must resolve to the Traditional catalog");
    }
    // Region-only tags still land on their base language, and a tag with no
    // catalog anywhere still ends at English.
    let pt: Value = serde_json::from_str(bundled_locale_json("pt").unwrap()).unwrap();
    assert_eq!(resolve_catalog_tagged(&normalize_code("pt-PT")).0, pt);
    assert_eq!(
        resolve_catalog_tagged(&normalize_code("xx-YY")).0,
        *english_catalog()
    );
}

// Catches LC_ALL=C being treated as unset; with it set, LC_MESSAGES and
// LANG must not be consulted at all.
#[test]
fn lc_all_overrides_every_other_locale_variable() {
    // The headline case: an explicit `LC_ALL=C` must win, and `C` means
    // English. It used to fall through and print German.
    assert_eq!(
        normalize_code(&locale_from_env(env(&[
            ("LC_ALL", "C"),
            ("LC_MESSAGES", "de_DE.UTF-8"),
            ("LANG", "fr_FR.UTF-8"),
        ]))),
        "en"
    );
    assert_eq!(
        normalize_code(&locale_from_env(env(&[
            ("LC_ALL", "POSIX"),
            ("LANG", "de_DE.UTF-8"),
        ]))),
        "en"
    );
    // A real LC_ALL still wins over the others.
    assert_eq!(
        locale_from_env(env(&[("LC_ALL", "de_DE.UTF-8"), ("LANG", "fr_FR")])),
        "de_DE.UTF-8"
    );
    // An EMPTY variable is "unset" in POSIX, so the next one is consulted.
    assert_eq!(
        locale_from_env(env(&[("LC_ALL", ""), ("LC_MESSAGES", "it_IT")])),
        "it_IT"
    );
    assert_eq!(
        locale_from_env(env(&[("LC_MESSAGES", ""), ("LANG", "nl_NL")])),
        "nl_NL"
    );
    // Nothing set at all → English.
    assert_eq!(locale_from_env(env(&[])), "en");
}

// Catches the loader building only a lowercase filename: an operator's
// locales/pt-BR.json would be invisible on a case-sensitive filesystem.
#[test]
fn locale_filenames_cover_the_spellings_operators_actually_write() {
    assert_eq!(
        locale_filenames("pt-br"),
        ["pt-br.json", "pt_br.json", "pt-BR.json", "pt_BR.json"]
    );
    assert_eq!(
        locale_filenames("zh-hans-cn"),
        [
            "zh-hans-cn.json",
            "zh_hans_cn.json",
            "zh-Hans-CN.json",
            "zh_Hans_CN.json"
        ]
    );
    // A numeric region has no case to canonicalize.
    assert_eq!(locale_filenames("es-419"), ["es-419.json", "es_419.json"]);
    // A bare language has exactly one spelling — no wasted stat calls.
    assert_eq!(locale_filenames("de"), ["de.json"]);
}

// Catches search path 2 being gated on $HOME (unset on stock Windows) and
// path 3 being a hardcoded /usr/share that means nothing there.
#[test]
fn search_dirs_are_ordered_and_survive_a_missing_home() {
    let dirs = locale_search_dirs_from(
        Some(PathBuf::from("/opt/freemkv/bin")),
        Some(PathBuf::from("/opt/testhome")),
        Some(PathBuf::from("/usr/share/freemkv/locales")),
    );
    assert_eq!(
        dirs,
        [
            PathBuf::from("/opt/freemkv/bin/locales"),
            PathBuf::from("/opt/testhome/.config/freemkv/locales"),
            PathBuf::from("/usr/share/freemkv/locales"),
            PathBuf::from("locales"),
        ]
    );
    // No home and no system dir must not drop the two that remain.
    assert_eq!(
        locale_search_dirs_from(Some(PathBuf::from("/opt/bin")), None, None),
        [PathBuf::from("/opt/bin/locales"), PathBuf::from("locales")]
    );

    // The live wiring must produce the working directory entry everywhere,
    // and must not offer a POSIX system path on Windows.
    let live = locale_search_dirs();
    assert!(live.contains(&PathBuf::from("locales")));
    #[cfg(windows)]
    assert!(
        !live.iter().any(|d| d.starts_with("/usr")),
        "/usr/share is meaningless on Windows: {live:?}"
    );
    #[cfg(not(windows))]
    assert!(live.contains(&PathBuf::from("/usr/share/freemkv/locales")));
}

/// Catches `read_to_string(..).ok()?`, which made an unreadable file look
/// exactly like an absent one and left the user reading "locale not found"
/// about a file sitting right there.
#[test]
fn an_unreadable_locale_file_is_reported_and_an_absent_one_is_not() {
    use std::io::{Error, ErrorKind};
    let path = Path::new("/tmp/freemkv/locales/sw.json");

    assert!(
        locale_read_diagnostic(path, &Error::from(ErrorKind::NotFound)).is_none(),
        "a file that is simply not there is the normal case for three of \
             the four search paths and must stay silent"
    );
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::IsADirectory,
        ErrorKind::InvalidData,
    ] {
        let msg = locale_read_diagnostic(path, &Error::from(kind))
            .unwrap_or_else(|| panic!("{kind:?} must be reported, not swallowed"));
        assert!(
            msg.contains("sw.json"),
            "diagnostic must name the file: {msg}"
        );
    }

    // And end to end against the filesystem: a directory where a catalog
    // was expected is a read error, not a missing file, and still yields
    // None so the search continues.
    let dir = std::env::temp_dir().join("freemkv-i18n-try-load-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("xx.json")).expect("create test dir");
    assert!(try_load(&dir.join("xx.json")).is_none());
    std::fs::write(dir.join("yy.json"), r#"{"app":{"opt_quiet":"q"}}"#).unwrap();
    assert_eq!(
        try_load(&dir.join("yy.json")).and_then(|v| lookup_in(&v, "app.opt_quiet")),
        Some("q".to_string())
    );
    std::fs::write(dir.join("zz.json"), "{ not json").unwrap();
    assert!(try_load(&dir.join("zz.json")).is_none());
    assert!(try_load(&dir.join("absent.json")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

// The size cap rejects an oversized locale file as InvalidData (never
// NotFound), so try_load reports it rather than silently swallowing it or
// reading an unbounded file into memory. Tested with a tiny cap.
#[test]
fn an_oversized_locale_file_is_a_capped_read_error_not_a_miss() {
    let dir = std::env::temp_dir().join("freemkv-i18n-cap-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test dir");
    let path = dir.join("big.json");
    std::fs::write(&path, vec![b'x'; 33]).unwrap();
    assert_eq!(
        read_capped(&path, 32).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData,
        "a file over the cap must be an InvalidData read error"
    );
    std::fs::write(&path, vec![b'x'; 32]).unwrap();
    assert!(
        read_capped(&path, 32).is_ok(),
        "exactly at the cap is accepted"
    );
    assert_eq!(
        read_capped(&dir.join("absent.json"), 32)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound,
        "a missing file is NotFound, never the cap's InvalidData"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Catches the per-key English fallback being completely silent, and
/// catches a "log it" fix that floods instead: each distinct key is
/// reported exactly once, so the line count equals the size of the gap.
#[test]
fn a_silently_substituted_english_string_is_reported_once_per_key() {
    assert!(
        report_missing_translation("test.first_key"),
        "the first substitution for a key must be reported"
    );
    assert!(
        !report_missing_translation("test.first_key"),
        "repeats of the same key must not flood the terminal"
    );
    assert!(
        report_missing_translation("test.second_key"),
        "a different key is a different gap and must be reported"
    );
}

/// Catches the English catalog being re-parsed on every miss, and catches
/// the short-circuit being wrong: when English is already active, a miss
/// must return the path, exactly as a second walk of the same tree would.
#[test]
fn the_english_catalog_is_parsed_once_and_skipped_when_active() {
    assert!(
        std::ptr::eq(english_catalog(), english_catalog()),
        "en.json must be parsed once and cached, not re-parsed per miss"
    );

    let sparse: Value = serde_json::json!({ "error": { "E9999": "present" } });
    assert!(lookup_or_english(&sparse, "error.E9053", false).contains("mkv://"));
    assert_eq!(
        lookup_or_english(&sparse, "error.E9053", true),
        "error.E9053",
        "with English active the fallback is a second walk of the same tree"
    );
}

// Catches sequential String::replace over the accumulating result: a
// substituted value that itself looks like a placeholder must not be
// rewritten by the next argument's pass.
#[test]
fn substitution_is_single_pass_over_the_original() {
    let out = fmt(
        "error.open_failed",
        &[("device", "{error}"), ("error", "permission denied")],
    );
    assert!(
        out.contains("{error}"),
        "a value that looks like a placeholder must be copied out verbatim: '{out}'"
    );
    assert!(out.contains("permission denied"));
    assert_eq!(
        out.matches("permission denied").count(),
        1,
        "the substituted value was rescanned: '{out}'"
    );

    // A placeholder with no argument is left visible; an unbalanced or
    // nested brace is a literal.
    assert_eq!(substitute("a {x} b", |_| None), "a {x} b");
    assert_eq!(substitute("a {x} b", |_| Some("Z")), "a Z b");
    assert_eq!(substitute("{unclosed", |_| Some("Z")), "{unclosed");
    assert_eq!(substitute("{a{b}", |_| Some("Z")), "{aZ");
    assert_eq!(substitute("", |_| Some("Z")), "");
}

#[test]
fn placeholders_matches_what_fmt_substitutes() {
    assert_eq!(placeholders("hi {name}!"), vec!["{name}".to_string()]);
    assert_eq!(placeholders("a {x} b {y}"), vec!["{x}", "{y}"]);
    assert!(placeholders("no placeholders").is_empty());
    // Every placeholder the helper reports must be one `fmt` really
    // substitutes — the two used to be separate parsers with different
    // rules about doubled braces.
    for s in ["{a}{b}", "{{x}}", "{a{b}", "{unclosed", "plain"] {
        for ph in placeholders(s) {
            let name = &ph[1..ph.len() - 1];
            assert_ne!(
                substitute(s, |n| (n == name).then_some("SUBSTITUTED")),
                s,
                "helper reported {ph} in {s:?} but fmt does not substitute it"
            );
        }
    }
}

#[test]
fn every_shipped_locale_has_full_key_coverage() {
    // Auto-covers every locale file so a new catalog can't ship with
    // missing/invented placeholders or a non-string/blank value; supersedes
    // the hand-picked per-language tests that used to sit here.
    assert!(
        SHIPPED_CODES.len() > 1,
        "build.rs bundled nothing but English"
    );
    for code in SHIPPED_CODES {
        if *code == "en" {
            continue;
        }
        let data = bundled_locale_json(code)
            .unwrap_or_else(|| panic!("{code} in SHIPPED_CODES but not bundled"));
        verify_locale(code, data);
    }
}

/// KU-F1i: the keydb-fixing texts are front-end-neutral, because the freemkv-library
/// server shows them too: no CLI command in any shipped locale.
#[test]
fn a_keydb_fix_names_no_cli_command() {
    for code in SHIPPED_CODES {
        let v: Value = serde_json::from_str(bundled_locale_json(code).unwrap()).unwrap();
        for err in ["E7000", "E7013", "E7026", "E8001", "E8002", "E8004"] {
            let text = lookup_in(&v, &format!("error.{err}")).unwrap_or_default();
            assert!(
                !text.contains("update-keys") && !text.contains("freemkv "),
                "{code} error.{err} names a CLI command: {text}"
            );
        }
    }
}

// freemkv compares the answer to drive.submit_prompt against drive.submit_affirmative, so
// each prompt must offer that token. drive.zip_failed was superseded by share.zip_failed.
#[test]
fn every_submit_prompt_offers_its_affirmative_token() {
    assert!(lookup_in(english_catalog(), "drive.zip_failed").is_none());
    for code in SHIPPED_CODES {
        let v: Value =
            serde_json::from_str(bundled_locale_json(code).expect("bundled")).expect("json");
        let yes = lookup_in(&v, "drive.submit_affirmative").expect("affirmative");
        let prompt = lookup_in(&v, "drive.submit_prompt").expect("prompt");
        assert!(
            prompt.contains(&format!("[{yes}/")),
            "{code}: {prompt:?} does not offer {yes:?}"
        );
    }
}

#[test]
fn locale_en_loads() {
    let _: Value = serde_json::from_str(LOCALE_EN).expect("en.json invalid");
}

// The placeholder-leak fix: error_message must DROP every unfilled
// placeholder and tidy what that leaves; error_message_with fills
// {detail} and drops the rest. Nothing braces-shaped may reach the user.
#[test]
fn error_message_drops_unfilled_placeholders_and_with_fills_detail() {
    // The drop-all-then-tidy path, driven directly so the checks do not
    // hinge on which catalog another test in this binary installed.
    let drop = |s: &str| tidy_empty_slots(&substitute(s, |_| Some("")));
    assert_eq!(
        drop("Could not read the disc at sector {detail}. Clean it."),
        "Could not read the disc at sector. Clean it."
    );
    assert_eq!(drop("Drive reset failed: {detail}"), "Drive reset failed");
    assert_eq!(
        drop("The drive is not ready ({detail}). Insert a disc."),
        "The drive is not ready. Insert a disc."
    );
    assert_eq!(
        drop("Title {detail} doesn't exist."),
        "Title doesn't exist."
    );
    assert_eq!(
        drop("Key database download failed (HTTP {detail}). Try again."),
        "Key database download failed (HTTP). Try again."
    );
    // The sibling placeholders: E7022's `(id: {hash})` wrapper and
    // E9067/E9068's quoted `'{path}'` must vanish cleanly, not leave `(id:)`
    // or `''` behind.
    assert_eq!(
        drop("No key source has a decryption key for this disc (id: {hash})."),
        "No key source has a decryption key for this disc."
    );
    assert_eq!(
        drop("A file name is too long. Shorten it and try again: '{path}'."),
        "A file name is too long. Shorten it and try again."
    );
    assert_eq!(drop("plain sentence."), "plain sentence.");

    // Full-width CJK parentheses (ja/zh wrap the value in （）/（id：）, not
    // ASCII parens). An ASCII-only tidy would leave a bare `（）`/`（id：）`.
    assert_eq!(
        drop("ドライブの準備ができていません（{detail}）。挿入してください。"),
        "ドライブの準備ができていません。挿入してください。"
    );
    assert_eq!(
        drop("ダウンロードに失敗しました（HTTP {detail}）。"),
        "ダウンロードに失敗しました（HTTP）。"
    );
    assert_eq!(
        drop("このディスクの復号キーを持つキーソースがありません（id：{hash}）。"),
        "このディスクの復号キーを持つキーソースがありません。"
    );

    // Public entry points. error_message_with fills {detail}; error_message
    // and the {hash}/{path} codes leak nothing.
    let filled = error_message_with(1000, "/dev/sr0");
    assert!(
        filled.contains("/dev/sr0") && !filled.contains("{detail}"),
        "error_message_with must substitute the detail: {filled}"
    );
    for code in [1000u32, 7022, 9067, 9068] {
        let bare = error_message(code);
        assert!(
            !bare.contains('{'),
            "error_message({code}) leaked a placeholder: {bare}"
        );
    }
}

// Three-letter ISO 639-2/3 codes and macrolanguage members fold onto the
// shipped two-letter catalog codes (deleting the fold_language call sends
// deu/nb to English despite de.json/no.json shipping).
#[test]
fn normalize_code_folds_three_letter_and_macrolanguage_codes() {
    assert_eq!(normalize_code("deu"), "de");
    assert_eq!(normalize_code("ger"), "de"); // 639-2/B alias
    assert_eq!(normalize_code("fra"), "fr");
    assert_eq!(normalize_code("fre"), "fr");
    assert_eq!(normalize_code("spa"), "es");
    assert_eq!(normalize_code("nld"), "nl");
    assert_eq!(normalize_code("jpn"), "ja");
    assert_eq!(normalize_code("rus"), "ru");
    assert_eq!(normalize_code("deu_DE"), "de-de");
    // Norwegian: nb/nn have no catalog of their own; the shipped one is `no`.
    assert_eq!(normalize_code("nb"), "no");
    assert_eq!(normalize_code("nn"), "no");
    assert_eq!(normalize_code("nb_NO.UTF-8"), "no-no");
    assert_eq!(normalize_code("mo"), "ro"); // deprecated Moldovan alias
    // An unshipped 3-letter code is passed through untouched (→ English).
    assert_eq!(normalize_code("swa"), "swa");
}

// Extended-language subtags resolve to the extlang, not the macro
// language's default script (else zh-yue/Cantonese would silently
// become Simplified Mandarin).
#[test]
fn normalize_code_handles_extended_language_subtags() {
    assert_eq!(normalize_code("zh-yue"), "yue");
    assert_eq!(normalize_code("zh_yue_HK"), "yue-hk");
    // A Mandarin extlang folds to `zh` and picks up Simplified by default.
    assert_eq!(normalize_code("zh-cmn"), "zh-hans");
    assert_eq!(normalize_code("zh-cmn-Hant"), "zh-hant");
    // A 3-DIGIT subtag is a region, never an extlang.
    assert_eq!(normalize_code("es-419"), "es-419");
}

// GNU LANGUAGE precedence: a colon-separated priority list that overrides
// a real POSIX selection but is IGNORED for the C/POSIX locale.
#[test]
fn gnu_language_overrides_a_real_locale_but_not_the_c_locale() {
    // Headline: LANGUAGE overrides a real LANG, and the WHOLE colon list is
    // preserved in order (a "first entry only" reading would drop `en`).
    assert_eq!(
        locale_candidates_from_env(env(&[("LANGUAGE", "de:en"), ("LANG", "en_US.UTF-8")])),
        vec!["de", "en"]
    );
    // Empty entries are skipped; each entry is normalized.
    assert_eq!(
        locale_candidates_from_env(env(&[("LANGUAGE", ":fr:pt_BR"), ("LANG", "en_US.UTF-8")])),
        vec!["fr", "pt-br"]
    );
    // LC_ALL=C must NOT be overridden — parseable-output contract.
    assert_eq!(
        locale_candidates_from_env(env(&[("LC_ALL", "C"), ("LANGUAGE", "de:en")])),
        vec!["en"]
    );
    // LANGUAGE with no locale variable set is ignored (that is the C locale).
    assert_eq!(
        locale_candidates_from_env(env(&[("LANGUAGE", "de")])),
        vec!["en"]
    );
    // An empty LANGUAGE falls back to the POSIX selection.
    assert_eq!(
        locale_candidates_from_env(env(&[("LANGUAGE", ""), ("LANG", "it_IT")])),
        vec!["it-it"]
    );
}

// The GNU LANGUAGE list is tried entry by entry: a leading entry with no
// catalog must not short-circuit to English before a later, shipped entry.
#[test]
fn language_priority_list_tries_each_entry_before_english() {
    let de: Value = serde_json::from_str(bundled_locale_json("de").unwrap()).unwrap();
    let (got, is_en) = resolve_catalog_for_candidates(&["sw".to_string(), "de".to_string()]);
    assert_eq!(
        got, de,
        "sw:de must reach German, not stop at the first miss"
    );
    assert!(!is_en);
    // Every candidate missing → English.
    let (fallback, en_flag) = resolve_catalog_for_candidates(&["sw".to_string(), "xx".to_string()]);
    assert_eq!(fallback, *english_catalog());
    assert!(en_flag);
}

// A blank locale value is a translation hole that reads as success; it
// must not render as an empty message in place of the English text.
#[test]
fn a_blank_locale_value_is_treated_as_missing() {
    let blank: Value = serde_json::json!({ "error": { "E9053": "   " } });
    let got = lookup_or_english(&blank, "error.E9053", false);
    assert!(
        got.contains("mkv://"),
        "a blank value must fall back to English, got {got:?}"
    );
    // Blank with English already active degrades to the path, not "".
    let both: Value = serde_json::json!({ "x": "" });
    assert_eq!(lookup_or_english(&both, "x", true), "x");
}

#[test]
fn open_failed_key_exists_and_fills_placeholders() {
    // Regression: error.open_failed was referenced by the device-open path
    // but never defined in any locale, so it rendered as the bare key.
    // Guard that it exists with both placeholders.
    let val = lookup_or_english(english_catalog(), "error.open_failed", true);
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
fn bundled_locale_json_returns_every_shipped_language() {
    // Every code build.rs bundled must resolve to parseable JSON, and an
    // unknown code must return None (it would be sought on disk at runtime
    // instead). This used to name seven languages by hand; 29 bundle.
    for code in SHIPPED_CODES {
        let data = bundled_locale_json(code).unwrap_or_else(|| panic!("{code} should be bundled"));
        let _: Value =
            serde_json::from_str(data).unwrap_or_else(|e| panic!("{code}.json invalid: {e}"));
    }
    assert!(bundled_locale_json("zz").is_none());
    // build.rs derives the bundled code with the same `normalize_code` the
    // runtime resolves with, so every shipped code is reachable from a tag.
    for code in SHIPPED_CODES {
        assert_eq!(
            normalize_code(code),
            **code,
            "{code} is bundled under a code no tag normalizes to"
        );
    }
}

// Catches.unwrap() on the catalog lock.
#[test]
fn a_poisoned_catalog_lock_does_not_take_the_process_down() {
    let poisoner = std::thread::spawn(|| {
        let _guard = write_strings();
        panic!("simulated panic while holding the catalog lock");
    });
    assert!(poisoner.join().is_err(), "the poisoning thread must panic");
    assert!(STRINGS.is_poisoned(), "the lock must actually be poisoned");

    // Both accessors, and the whole public read path on top of them, keep
    // working.
    drop(read_strings());
    drop(write_strings());
    assert_ne!(
        get("error.open_failed"),
        "error.open_failed",
        "a lookup after a poisoned lock must still return a real message"
    );
}

// Catches a second --language being dropped into `let _ = ...set(..)`
// with nothing said about it.
#[test]
fn a_second_language_override_is_reported_not_swallowed() {
    let msg = language_override_conflict("fr", Some("de")).expect("conflict must be reported");
    assert!(msg.contains("fr") && msg.contains("de"), "{msg}");
    // Asking twice for the same language changed nothing, so there is
    // nothing to warn about.
    assert_eq!(language_override_conflict("de", Some("de")), None);
    assert_eq!(language_override_conflict("de", None), None);
}

// The empty-candidate-list edge of the "not found" diagnostic: it must
// fall through quietly rather than panic on .first() or print a
// diagnostic naming nothing.
#[test]
fn resolve_catalog_for_candidates_with_no_candidates_falls_back_to_english_silently() {
    let (got, is_en) = resolve_catalog_for_candidates(&[]);
    assert_eq!(got, *english_catalog());
    assert!(is_en);
}

/// A GNU `LANGUAGE` value that is present but resolves to no usable
/// candidates (every entry empty, e.g. a bare `:`) must fall through to the
/// POSIX selection instead of returning an empty candidate list.
#[test]
fn language_var_with_only_empty_entries_falls_through_to_posix_selection() {
    assert_eq!(
        locale_candidates_from_env(env(&[("LANGUAGE", ":"), ("LANG", "it_IT")])),
        vec!["it-it"]
    );
}

// load_locale_file against the real, unmocked search path: it must find a
// catalog dropped in ./locales, not just the bundled/compiled-in ones.
#[test]
fn load_locale_file_finds_a_catalog_via_the_real_working_directory_search_path() {
    let path = Path::new("locales/ztx.json");
    std::fs::write(path, r#"{"app":{"opt_quiet":"disk-loaded"}}"#).expect("write test locale file");
    let result = load_locale_file("ztx");
    let _ = std::fs::remove_file(path);
    let v = result.expect("must find the file just written under ./locales");
    assert_eq!(
        lookup_in(&v, "app.opt_quiet"),
        Some("disk-loaded".to_string())
    );
}

#[test]
fn error_message_returns_real_string_not_key_path() {
    // E7022's English string names the disc by {hash}, a value this
    // bare-code entry point lacks, so the placeholder (and its `(id: …)`
    // wrapper) must be DROPPED, never leaked as literal `{hash}`.
    let msg = error_message(7022);
    assert_ne!(
        msg, "error.E7022",
        "error_message(7022) returned the key path — the string is missing or the key format is wrong"
    );
    assert!(
        !msg.contains('{'),
        "error_message(7022) leaked a placeholder to the user, got: '{msg}'"
    );
    assert!(
        !msg.contains("(id:)") && !msg.contains("（id：）"),
        "error_message(7022) left an empty (id:) wrapper: '{msg}'"
    );
    assert!(
        msg.contains("decryption key"),
        "error_message(7022) lost its real wording: '{msg}'"
    );

    // E1000 (drive not found) is another known code; sanity-check it too.
    assert_ne!(error_message(1000), "error.E1000");

    // An unknown code falls back to the key path (same miss behavior as get).
    assert_eq!(error_message(999_999), "error.E999999");
}

// Key-SOURCE failure codes must not render as E7022's "no key source has
// a key" wording — that sent an operator hunting a VUK during an HTTP 502
// outage that was never a missing-key problem.
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
