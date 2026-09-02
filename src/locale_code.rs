// Locale-tag arithmetic shared by build.rs (`include!`, hence `std`-only, no
// file-scope `use`) and lib.rs. Canonicalizes to `lang[-script][-region]`,
// e.g. `pt_BR.UTF-8` -> `pt-br`. See docs/locale-code.md for full rationale.
pub(crate) fn normalize_code(s: &str) -> String {
    // Drop codeset/modifier (`.UTF-8`, `@euro`), keep language[_-subtags].
    let base = s.trim().split(['.', '@', ':']).next().unwrap_or("");
    let mut parts = base.split(['_', '-']).filter(|p| !p.is_empty()).peekable();
    let mut lang = parts.next().unwrap_or("").to_ascii_lowercase();
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return "en".to_string();
    }
    // BCP-47 extended-language subtag (`zh-yue`, `zh-cmn`): promote to primary
    // language so `yue` falls through to English, not `zh` Simplified.
    // See docs/locale-code.md for the full incident writeup.
    if let Some(next) = parts.peek()
        && next.len() == 3
        && next.chars().all(|c| c.is_ascii_alphabetic())
    {
        lang = parts.next().unwrap_or_default().to_ascii_lowercase();
    }
    // Fold ISO 639-2/639-3 codes and macrolanguage members onto the shipped
    // two-letter code, so `--language deu` reaches `de.json`.
    lang = fold_language(&lang);

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

// Fold a primary-language subtag onto the two-letter code the crate ships a
// catalog under (ISO 639-2/3, macrolanguage members, deprecated aliases).
// Unknown codes pass through unchanged. See docs/locale-code.md for detail.
fn fold_language(lang: &str) -> String {
    match lang {
        // Macrolanguage members / deprecated aliases → the shipped base code.
        "nb" | "nn" | "nob" | "nno" | "nor" => "no",
        "mo" => "ro",
        "in" => "id",
        // ISO 639-2/639-3 three-letter → two-letter, shipped languages only.
        "cat" => "ca",
        "ces" | "cze" => "cs",
        "dan" => "da",
        "deu" | "ger" => "de",
        "ell" | "gre" => "el",
        "eng" => "en",
        "spa" => "es",
        "fin" => "fi",
        "fra" | "fre" => "fr",
        "hun" => "hu",
        "ind" => "id",
        "ita" => "it",
        "jpn" => "ja",
        "kor" => "ko",
        "nld" | "dut" => "nl",
        "pol" => "pl",
        "por" => "pt",
        "ron" | "rum" => "ro",
        "rus" => "ru",
        "slk" | "slo" => "sk",
        "swe" => "sv",
        "tur" => "tr",
        "ukr" => "uk",
        "vie" => "vi",
        "zho" | "chi" | "cmn" => "zh",
        other => other,
    }
    .to_string()
}

// The codes to try, most specific first: every prefix of the tag, longest
// first (`zh-hans-cn` -> `zh-hans` -> `zh`), so a `lang-script` catalog is
// found whether or not a region is attached. See docs/locale-code.md.
pub(crate) fn fallback_chain(code: &str) -> Vec<String> {
    let parts: Vec<&str> = code.split('-').filter(|p| !p.is_empty()).collect();
    let mut out = Vec::with_capacity(parts.len());
    for n in (1..=parts.len()).rev() {
        let candidate = parts[..n].join("-");
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    if out.is_empty() {
        out.push(code.to_string());
    }
    out
}

// The on-disk filenames that may hold the catalog for an internal code:
// the internal form, the underscore form, and BCP-47 canonical case, in
// both separators (four names at worst). See docs/locale-code.md.
pub(crate) fn locale_filenames(code: &str) -> Vec<String> {
    let parts: Vec<&str> = code.split('-').filter(|p| !p.is_empty()).collect();
    // BCP-47 canonical case: language lowercase, 4-letter script Titlecase,
    // 2-letter region UPPERCASE, numeric region unchanged.
    let canonical: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                p.to_ascii_lowercase()
            } else if p.len() == 4 && p.chars().all(|c| c.is_ascii_alphabetic()) {
                let mut s = p[..1].to_ascii_uppercase();
                s.push_str(&p[1..].to_ascii_lowercase());
                s
            } else if p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()) {
                p.to_ascii_uppercase()
            } else {
                p.to_string()
            }
        })
        .collect();

    let mut out = Vec::with_capacity(4);
    for stem in [
        parts.join("-"),
        parts.join("_"),
        canonical.join("-"),
        canonical.join("_"),
    ] {
        let name = format!("{stem}.json");
        if !stem.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}
