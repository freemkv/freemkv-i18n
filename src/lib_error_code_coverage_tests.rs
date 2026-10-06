use super::error_codes::LIBFREEMKV_ERROR_CODES;
use super::*;

// EVERY error code libfreemkv can raise must have a non-empty English string.
#[test]
fn libfreemkv_error_codes_all_have_english_strings() {
    let en = english_catalog();
    let mut missing = Vec::new();
    for (code, name) in LIBFREEMKV_ERROR_CODES {
        let key = format!("error.E{code}");
        match lookup_in(en, &key) {
            Some(s) if !s.trim().is_empty() => {}
            _ => missing.push(format!("E{code} ({name})")),
        }
    }
    assert!(
        missing.is_empty(),
        "{} libfreemkv error code(s) have no English string in en.json, so they \
             render to the user as the literal key path: {:?}",
        missing.len(),
        missing
    );
}

// The reverse direction: an error.EXXXX key in en.json matching no
// libfreemkv code is a typo, or a retired code translators still maintain.
#[test]
fn en_json_has_no_error_key_for_a_code_that_does_not_exist() {
    let en = english_catalog();
    let known: Vec<u32> = LIBFREEMKV_ERROR_CODES.iter().map(|(c, _)| *c).collect();
    let mut orphans = Vec::new();
    for key in en["error"].as_object().expect("error block").keys() {
        if let Some(digits) = key.strip_prefix('E')
            && let Ok(code) = digits.parse::<u32>()
            && !known.contains(&code)
        {
            orphans.push(key.clone());
        }
    }
    assert!(
        orphans.is_empty(),
        "en.json has error string(s) for code(s) libfreemkv cannot raise: {orphans:?}"
    );
}

// Catches the checked-in list going stale against real libfreemkv. Only runs where a
// sibling libfreemkv checkout exists (never this crate's own CI).
#[test]
fn libfreemkv_code_list_has_not_drifted() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../libfreemkv/src/error.rs")
        .canonicalize();
    let Ok(source) = source else {
        eprintln!("libfreemkv checkout not present; skipping error-code drift check");
        return;
    };
    let Ok(text) = std::fs::read_to_string(&source) else {
        eprintln!("libfreemkv error.rs unreadable; skipping error-code drift check");
        return;
    };

    let mut actual: Vec<(u32, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub const E_") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(": u16 = ") else {
            continue;
        };
        if let Ok(code) = value.trim_end_matches(';').trim().parse::<u32>() {
            actual.push((code, format!("E_{name}")));
        }
    }
    actual.sort();
    assert!(
        !actual.is_empty(),
        "parsed no codes out of {} — the const shape changed and this test \
             would otherwise pass vacuously",
        source.display()
    );

    let mut checked_in: Vec<(u32, String)> = LIBFREEMKV_ERROR_CODES
        .iter()
        .map(|(c, n)| (*c, n.to_string()))
        .collect();
    checked_in.sort();

    assert_eq!(
        checked_in,
        actual,
        "src/error_codes.rs is out of date with {}. Regenerate it:\n    \
             ci/sync-error-codes.sh\nthen add an English string (and a line in \
             every locale) for any new code.",
        source.display()
    );
}
