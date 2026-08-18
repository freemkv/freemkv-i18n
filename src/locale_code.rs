// Locale-tag arithmetic, shared by the RUNTIME loader (`lib.rs`) and the BUILD
// script (`build.rs`) — the two used to derive a locale's internal code with
// separate, subtly different code. `build.rs` lower-cased the filename stem and
// swapped `_` for `-`; the runtime ran the full `normalize_code`, which also
// infers a Chinese script from the region. A `zh_TW.json` dropped into
// `locales/` therefore registered under the code `zh-tw`, which nothing ever
// asks for (the runtime turns `zh_TW` into `zh-hant`), so the file was compiled
// into the binary and then permanently unreachable.
//
// There is exactly one derivation now, and `build.rs` includes this file
// verbatim (`include!`) rather than depending on the crate it is building. That
// is why everything here is `std`-only with no `use` statements at file scope:
// it has to compile inside a build script as well as inside the library.

/// Canonicalize a locale string to an internal tag `lang[-script][-region]`,
/// all lowercase, hyphen-joined: `pt_BR.UTF-8` → `pt-br`, `zh_Hant` → `zh-hant`,
/// `en-US` → `en-us`, `de` → `de`. Region/script are PRESERVED (no collapse to
/// two letters), so `pt-BR` ≠ `pt` and Simplified ≠ Traditional.
///
/// Inputs are untrusted (the `--language` CLI flag and the `LC_*`/`LANG` env
/// vars), so this must never panic — it iterates by *character*, validates
/// ASCII, and falls back to English on anything malformed. That fallback also
/// gives the POSIX `C`/`POSIX` locales the right answer for free: neither is a
/// 2–3 letter language subtag, so both land on English, which is precisely what
/// `LC_ALL=C` is asking for.
///
/// Chinese without an explicit script infers one from the region (tw/hk/mo →
/// hant, else hans) and defaults to Simplified, so `zh_CN` → `zh-hans`,
/// `zh_TW` → `zh-hant`, bare `zh` → `zh-hans`.
pub(crate) fn normalize_code(s: &str) -> String {
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

/// The codes to try, most specific first, for an already-normalized tag.
///
/// This exists because the fallback used to be exactly two steps — the full tag
/// and then `code.split('-').next()`, the FIRST subtag — which silently skipped
/// the `lang-script` level in the middle. macOS hands the process
/// `zh-Hans-CN`; that normalizes to `zh-hans-cn`, which no catalog matches, and
/// the old chain jumped straight to `zh`, which no catalog matches either
/// because the crate ships `zh-hans.json` and `zh-hant.json`, not `zh.json`.
/// The result was English: the two Chinese catalogs shipped in the binary were
/// unreachable from the tag the operating system actually emits, and a
/// Simplified Chinese user saw an English UI with nothing logged to say why.
///
/// The chain is now every prefix of the tag, longest first — `zh-hans-cn` →
/// `zh-hans` → `zh`, `sr-latn-rs` → `sr-latn` → `sr`, `pt-br` → `pt`. A
/// `lang-script` catalog is found whether or not a region is attached, and a
/// bare `zh.json` still works if someone ships one.
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

/// The on-disk filenames that may hold the catalog for an internal code.
///
/// The internal code is always lowercase and hyphen-joined, but an operator
/// dropping a file into `locales/` writes the tag the way the rest of the world
/// writes it — `pt-BR.json`, `zh_Hans_CN.json`. The loader used to build one
/// lowercase name and `read_to_string` it, which finds nothing on any
/// case-sensitive filesystem, so a perfectly good `locales/pt-BR.json` was
/// invisible on Linux while working on macOS and Windows purely because those
/// filesystems fold case. `build.rs` already accepted both spellings when
/// bundling, so the two halves of the crate disagreed about the same file.
///
/// Rather than read the directory (which costs a syscall per search path even
/// when nothing is there, and drags in its own platform differences), derive the
/// small set of spellings that are actually plausible: the internal form, the
/// underscore form, and BCP-47 canonical case (`lang-Script-REGION`) in both
/// separators. Four names at worst, and a one-shot startup path stats them.
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
