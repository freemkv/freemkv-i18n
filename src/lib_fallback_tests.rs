use super::*;

// A key present in English but missing from the active locale must render the ENGLISH text,
// never the raw path.
#[test]
fn a_key_missing_from_the_locale_falls_back_to_english() {
    // A catalog that deliberately lacks the key.
    let sparse: Value = serde_json::json!({ "error": { "E9999": "present" } });
    let got = lookup_or_english(&sparse, "error.E9053", false);
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
    assert_eq!(
        lookup_or_english(&sparse, "error.E0000", false),
        "error.E0000"
    );
}
