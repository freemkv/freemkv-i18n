// Drift guard for keys freemkv looks up with a compiled-in English fallback. The CI job
// `freemkv-key-drift` points FREEMKV_SRC at a sparse checkout of freemkv's `src/`; unset, the
// drift test is skipped (like `libfreemkv_code_list_has_not_drifted`).

use serde_json::Value;
use std::path::Path;

// Calls whose first two arguments are (catalog key, English fallback). `g` is ui.rs's local
// `get_or` closure for menu items.
const CALLEES: &[&str] = &["get_or(", "fmt_or(", "PendingDiag::new(", "g("];

/// Every `(key, english)` pair in `src` passed to one of [`CALLEES`]. The key may be a string
/// literal or a `const NAME: &str = "..."` declared in the same file.
fn extract(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for callee in CALLEES {
        let mut from = 0;
        while let Some(pos) = src[from..].find(callee) {
            let at = from + pos;
            from = at + callee.len();
            // Whole-word match: `g(` must not be the tail of `log(` or `.g(`.
            let prev = src[..at].chars().next_back();
            if prev
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || (c == '.' && *callee == "g("))
            {
                continue;
            }
            let rest = &src[from..];
            let Some((key, rest)) = key_arg(src, rest) else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix(',') else {
                continue;
            };
            let Some((english, _)) = string_lit(rest.trim_start()) else {
                continue;
            };
            if is_key(&key) {
                out.push((key, english));
            }
        }
    }
    out
}

fn is_key(s: &str) -> bool {
    let mut parts = s.split('.');
    parts.clone().count() >= 2
        && parts.all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

// The first argument: a string literal, or an identifier bound by `const IDENT: &str = "...";`.
fn key_arg<'a>(file: &str, rest: &'a str) -> Option<(String, &'a str)> {
    let rest = rest.trim_start();
    if rest.starts_with('"') {
        return string_lit(rest);
    }
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let ident = &rest[..end];
    if ident.is_empty() {
        return None;
    }
    let decl = format!("const {ident}: &str =");
    let at = file.find(&decl)?;
    let (key, _) = string_lit(file[at + decl.len()..].trim_start())?;
    Some((key, &rest[end..]))
}

// A plain Rust string literal at the start of `s`, unescaped; returns it and what follows.
fn string_lit(s: &str) -> Option<(String, &str)> {
    let body = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &body[i + 1..])),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '0' => out.push('\0'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '\'' => out.push('\''),
                '\n' => {
                    // Line continuation: skip the newline and the next line's indentation.
                    let tail = chars.as_str();
                    let skip = tail.len() - tail.trim_start().len();
                    for _ in 0..tail[..skip].chars().count() {
                        chars.next();
                    }
                }
                'u' => {
                    let tail = chars.as_str().strip_prefix('{')?;
                    let close = tail.find('}')?;
                    out.push(char::from_u32(
                        u32::from_str_radix(&tail[..close], 16).ok()?,
                    )?);
                    for _ in 0..close + 2 {
                        chars.next();
                    }
                }
                _ => return None,
            },
            _ => out.push(c),
        }
    }
    None
}

fn lookup<'a>(catalog: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.')
        .try_fold(catalog, |node, part| node.get(part))?
        .as_str()
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir)
        .expect("read freemkv src dir")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn freemkv_fallback_keys_ship_in_english_with_the_same_text() {
    let Some(src) = std::env::var_os("FREEMKV_SRC") else {
        eprintln!("FREEMKV_SRC not set; skipping the freemkv key drift check");
        return;
    };
    let en_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales/en.json");
    let en: Value = serde_json::from_str(&std::fs::read_to_string(en_path).unwrap()).unwrap();
    let mut files = Vec::new();
    rust_files(Path::new(&src), &mut files);
    files.sort();
    let mut found = 0;
    let mut problems = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for (key, english) in extract(&text) {
            found += 1;
            match lookup(&en, &key) {
                None => problems.push(format!("missing {key} ({})", file.display())),
                Some(v) if v != english => problems.push(format!(
                    "{key}: en.json {v:?} != freemkv fallback {english:?} ({})",
                    file.display()
                )),
                Some(_) => {}
            }
        }
    }
    assert!(
        found > 0,
        "extracted no keys from {src:?}; the call shape changed"
    );
    assert!(
        problems.is_empty(),
        "{} freemkv fallback key(s) drifted from en.json:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn the_extractor_reads_every_call_shape() {
    let src = r#"
        let a = strings::get_or("gui.menu.cut", "Cut");
        let b = crate::strings::fmt_or(
            "share.zip_failed",
            "Could not build \
             the zip: {error}",
            &[("error", "x")],
        );
        diags.push(PendingDiag::new("error.log_file_needs_value", "needs \"a\" path\u{2026}"));
        const KEY: &str = "error.title_changed";
        fmt_or(KEY, "Title {num} changed", &args);
        item(g("gui.menu.paste", "Paste"), None);
        dialog("gui.menu.not_a_call", "x");
        let n = log("gui.menu.log_is_not_g", "x");
        get_or(path, "not a const key");
        get_or("not a key", "x");
    "#;
    let mut got = extract(src);
    got.sort();
    let want = [
        ("error.log_file_needs_value", "needs \"a\" path\u{2026}"),
        ("error.title_changed", "Title {num} changed"),
        ("gui.menu.cut", "Cut"),
        ("gui.menu.paste", "Paste"),
        ("share.zip_failed", "Could not build the zip: {error}"),
    ]
    .map(|(k, e)| (k.to_string(), e.to_string()));
    assert_eq!(got, want);
}
