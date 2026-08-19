// The runtime locale-resolution path, driven through the public API.
//
// Nothing had ever called `set_language`, `set_locale` or `init`. The whole
// startup path — the one that decides which language a user sees — was
// reachable only by running the real binary, and every unit test in the crate
// worked on catalogs it constructed by hand. That is causally why the
// `zh-Hans-CN` fallback bug shipped: the catalogs were verified in exquisite
// detail and the code that chooses between them was verified not at all.
//
// It lives in an integration test rather than beside the unit tests because
// `STRINGS` and `LANG_OVERRIDE` are process-wide statics. An integration test
// binary is its own process, and this file holds exactly ONE test, so the
// sequence below is deterministic: no other test can install a catalog or claim
// the one-shot `--language` override underneath it. Adding a second `#[test]`
// here would reintroduce the race, so don't — extend this one.

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
    freemkv_i18n::init();
    assert_eq!(
        freemkv_i18n::get(KEY),
        bundled("de", KEY),
        "--language de did not select the German catalog"
    );

    // ── 2. The bug a user actually felt ────────────────────────────────────
    // macOS hands a process the full BCP-47 tag. `zh-Hans-CN` normalizes to
    // `zh-hans-cn`, which no catalog matches; the fallback then has to try
    // `zh-hans` — which IS compiled into this binary — before `zh` and English.
    // It used to skip straight from the full tag to `zh`, and since no `zh.json`
    // ships, a Simplified Chinese user got an English UI with nothing logged.
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
    for tag in ["xx-YY", "C", "POSIX", "", "auto"] {
        freemkv_i18n::set_locale(tag);
        assert_eq!(
            freemkv_i18n::get(KEY),
            english,
            "{tag:?} should have resolved to English"
        );
    }

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
