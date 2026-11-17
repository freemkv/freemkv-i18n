// The runtime locale-resolution path, driven through the public API. One `#[test]` only:
// STRINGS/LANG_OVERRIDE are process-wide statics, so a second test here would race it.

use serde_json::Value;

/// Read a key straight out of a bundled catalog, with no fallback of any kind,
/// so the assertions below compare against ground truth rather than against
/// whatever the loader happened to return.
fn bundled(code: &str, key: &str) -> String {
    let raw =
        freemkv_i18n::bundled_locale_json(code).unwrap_or_else(|| panic!("{code} is not bundled"));
    let json: Value = serde_json::from_str(raw).expect("bundled catalog parses");
    let mut node = &json;
    for part in key.split('.') {
        node = node
            .get(part)
            .unwrap_or_else(|| panic!("{code}: no key {key}"));
    }
    node.as_str().expect("string leaf").to_string()
}

#[test]
fn the_startup_and_live_switch_paths_select_the_right_catalog() {
    // A key every catalog has, whose value differs per language.
    const KEY: &str = "app.opt_quiet";
    let english = bundled("en", KEY);

    // ── 1. --language, applied before anything reads a string ──────────────
    // This is the only chance to exercise it: `LANG_OVERRIDE` is a OnceLock and
    // `set_language` refuses (and debug-asserts) once a catalog is installed.
    freemkv_i18n::set_language("de");
    // init must re-resolve from the override, replacing an installed catalog.
    freemkv_i18n::set_locale("fr");
    assert_eq!(freemkv_i18n::get(KEY), bundled("fr", KEY));
    freemkv_i18n::init();
    assert_eq!(
        freemkv_i18n::get(KEY),
        bundled("de", KEY),
        "--language de did not select the German catalog"
    );

    // ── 2. The bug a user actually felt ────────────────────────────────────.
    let simplified = bundled("zh-hans", KEY);
    let traditional = bundled("zh-hant", KEY);
    for tag in ["zh-Hans-CN", "zh_Hans_SG.UTF-8", "zh-Hans"] {
        freemkv_i18n::set_locale(tag);
        let got = freemkv_i18n::get(KEY);
        assert_ne!(got, english, "{tag} silently fell back to English");
        assert_eq!(got, simplified, "{tag} must land on the Simplified catalog");
    }
    for tag in ["zh-Hant-TW", "zh-Hant-HK", "zh_TW"] {
        freemkv_i18n::set_locale(tag);
        assert_eq!(
            freemkv_i18n::get(KEY),
            traditional,
            "{tag} must land on the Traditional catalog"
        );
    }

    // ── 3. Region → base language, and the live switch ─────────────────────
    freemkv_i18n::set_locale("pt-BR");
    assert_eq!(freemkv_i18n::get(KEY), bundled("pt-br", KEY));
    freemkv_i18n::set_locale("pt-PT"); // no pt-pt catalog ships
    assert_eq!(freemkv_i18n::get(KEY), bundled("pt", KEY));
    freemkv_i18n::set_locale("fr_FR.UTF-8");
    assert_eq!(freemkv_i18n::get(KEY), bundled("fr", KEY));

    // ── 4. Unknown tags, and the POSIX C locale, end at English ────────────
    for tag in ["xx-YY", "C", "POSIX", ""] {
        freemkv_i18n::set_locale(tag);
        assert_eq!(
            freemkv_i18n::get(KEY),
            english,
            "{tag:?} should have resolved to English"
        );
    }

    // "auto" re-detects the same way startup does, not just landing on
    // English: LANG_OVERRIDE was pinned to "de" in step 1 and never resets,
    // so "auto" here must reach German, exactly like init() would.
    freemkv_i18n::set_locale("auto");
    assert_eq!(
        freemkv_i18n::get(KEY),
        bundled("de", KEY),
        "auto did not re-detect through the same path as startup"
    );

    // ── 5. fmt and error_message go through the same installed catalog ─────
    freemkv_i18n::set_locale("de");
    let out = freemkv_i18n::fmt("app.unknown_option", &[("opt", "--nope")]);
    assert!(out.contains("--nope"), "placeholder not substituted: {out}");
    assert!(!out.contains("{opt}"), "placeholder left behind: {out}");
    let msg = freemkv_i18n::error_message(1000);
    assert_ne!(msg, "error.E1000", "E1000 rendered as its own key path");
    // E1000's string embeds `{detail}` (a device path). The detail-less entry
    // point must NOT leak the literal placeholder; the detail-aware one fills it
    // from the same installed `de` catalog.
    let de_raw = bundled("de", "error.E1000");
    assert!(
        de_raw.contains("{detail}"),
        "fixture drift: de E1000 should carry a {{detail}} placeholder, got {de_raw:?}"
    );
    assert!(
        !msg.contains("{detail}"),
        "error_message(1000) leaked a {{detail}} placeholder: {msg}"
    );
    let filled = freemkv_i18n::error_message_with(1000, "/dev/sr0");
    assert_eq!(
        filled,
        de_raw.replace("{detail}", "/dev/sr0"),
        "error_message_with must substitute the caller's detail into the catalog string"
    );
    assert!(!filled.contains("{detail}"));

    // E9087 must preserve the caller's track/count detail in every locale,
    // even if all catalogs accidentally agree on the wrong placeholder name.
    let detail = "subtitle track 37 requested; available tracks: 2";
    for code in freemkv_i18n::SHIPPED_CODES {
        freemkv_i18n::set_locale(code);
        let filled = freemkv_i18n::error_message_with(9087, detail);
        assert!(
            filled.contains(detail),
            "{code}: E9087 dropped the caller's track/count detail: {filled}"
        );
        let bare = freemkv_i18n::error_message(9087);
        assert_ne!(bare, "error.E9087", "{code}: E9087 is missing");
        assert!(
            !bare.contains(['{', '}']),
            "{code}: E9087 leaked a placeholder without detail: {bare}"
        );
    }

    // ── 6. A locale with no catalog anywhere still returns a real message ──
    // Not the key path, and not a panic: the per-key English fallback covers
    // it. A missing key must never take the process down.
    freemkv_i18n::set_locale("sw"); // Swahili does not ship
    assert_eq!(freemkv_i18n::get(KEY), english);
    assert_eq!(
        freemkv_i18n::get("no.such.key.anywhere"),
        "no.such.key.anywhere"
    );
}
