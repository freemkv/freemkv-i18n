// Drift guard for keys freemkv looks up with a compiled-in English fallback. The CI job
// `freemkv-key-drift` points FREEMKV_SRC at a sparse checkout of freemkv's `src/`; unset, the
// drift test is skipped. Every call site must parse or be on ALLOWLIST: nothing is skipped silently.

use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Str(String),
    Ident(String),
    Punct(String),
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    line: usize,
}

// Rust source to tokens, dropping comments. Char literals become Punct("'"); only strings,
// identifiers and punctuation matter here.
fn lex(src: &str) -> Result<Vec<Token>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    let push = |out: &mut Vec<Token>, tok: Tok, line: usize| out.push(Token { tok, line });
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && next == Some('*') {
            let mut depth = 0;
            loop {
                match (c.get(i), c.get(i + 1)) {
                    (Some('/'), Some('*')) => (depth, i) = (depth + 1, i + 2),
                    (Some('*'), Some('/')) => {
                        (depth, i) = (depth - 1, i + 2);
                        if depth == 0 {
                            break;
                        }
                    }
                    (Some(ch), _) => {
                        line += usize::from(*ch == '\n');
                        i += 1;
                    }
                    (None, _) => return Err(format!("line {line}: unterminated block comment")),
                }
            }
        } else if let Some(hashes_at) = raw_string_start(&c, i) {
            let start_line = line;
            let hashes = c[hashes_at..].iter().take_while(|&&h| h == '#').count();
            let mut j = hashes_at + hashes + 1;
            let mut s = String::new();
            loop {
                match c.get(j) {
                    None => return Err(format!("line {start_line}: unterminated raw string")),
                    Some('"')
                        if c[j + 1..]
                            .iter()
                            .take(hashes)
                            .filter(|&&h| h == '#')
                            .count()
                            == hashes =>
                    {
                        j += 1 + hashes;
                        break;
                    }
                    Some(&ch) => {
                        line += usize::from(ch == '\n');
                        s.push(ch);
                        j += 1;
                    }
                }
            }
            push(&mut out, Tok::Str(s), start_line);
            i = j;
        } else if ch == '"' || (ch == 'b' && next == Some('"')) {
            let start_line = line;
            let (s, j, lines) = cooked_string(&c, if ch == 'b' { i + 1 } else { i })
                .map_err(|e| format!("line {start_line}: {e}"))?;
            line += lines;
            push(&mut out, Tok::Str(s), start_line);
            i = j;
        } else if ch == '\'' {
            // A char literal ('a', '\n', '\u{2026}') or a lifetime ('a).
            if next == Some('\\') {
                let mut j = i + 3;
                while j < c.len() && c[j] != '\'' {
                    j += 1;
                }
                i = j + 1;
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
            push(&mut out, Tok::Punct("'".into()), line);
        } else if ch.is_alphanumeric() || ch == '_' {
            let mut j = i;
            while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_') {
                j += 1;
            }
            push(&mut out, Tok::Ident(c[i..j].iter().collect()), line);
            i = j;
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            let p = if ["::", "=>", "->"].contains(&two.as_str()) {
                two
            } else {
                ch.to_string()
            };
            i += p.chars().count();
            push(&mut out, Tok::Punct(p), line);
        }
    }
    Ok(out)
}

// Index of the first `#` (or the `"`) of a raw string starting at `i` (`r"`, `r#"`, `br"`).
fn raw_string_start(c: &[char], i: usize) -> Option<usize> {
    if i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_') {
        return None;
    }
    let at = match (c.get(i), c.get(i + 1)) {
        (Some('r'), _) => i + 1,
        (Some('b'), Some('r')) => i + 2,
        _ => return None,
    };
    let hashes = c[at..].iter().take_while(|&&h| h == '#').count();
    (c.get(at + hashes) == Some(&'"')).then_some(at)
}

// A `"..."` literal at `c[i]`, unescaped: returns the text, the index after it and the newlines
// it spans.
fn cooked_string(c: &[char], i: usize) -> Result<(String, usize, usize), String> {
    let (mut s, mut j, mut lines) = (String::new(), i + 1, 0);
    loop {
        let ch = *c.get(j).ok_or("unterminated string")?;
        j += 1;
        match ch {
            '"' => return Ok((s, j, lines)),
            '\n' => {
                lines += 1;
                s.push('\n');
            }
            '\\' => {
                let e = *c.get(j).ok_or("unterminated escape")?;
                j += 1;
                match e {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    '0' => s.push('\0'),
                    '\\' | '"' | '\'' => s.push(e),
                    // Continuation (LF or CRLF): skip the line break and the next line's indent.
                    '\n' | '\r' => {
                        while c.get(j).is_some_and(|w| w.is_whitespace()) {
                            lines += usize::from(c[j] == '\n');
                            j += 1;
                        }
                        lines += usize::from(e == '\n');
                    }
                    'x' => {
                        let hex: String = c.get(j..j + 2).ok_or("short \\x")?.iter().collect();
                        s.push(char::from(
                            u8::from_str_radix(&hex, 16).map_err(|e| e.to_string())?,
                        ));
                        j += 2;
                    }
                    'u' => {
                        let close = c[j..].iter().position(|&x| x == '}').ok_or("bad \\u")?;
                        let hex: String = c[j + 1..j + close].iter().collect();
                        let v = u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                        s.push(char::from_u32(v).ok_or("bad \\u value")?);
                        j += close + 1;
                    }
                    other => return Err(format!("unknown escape \\{other}")),
                }
            }
            _ => s.push(ch),
        }
    }
}

// Drop `use` statements and every item under `#[cfg(test)]`.
fn strip_tests_and_uses(toks: Vec<Token>) -> Vec<Token> {
    let is = |t: &Token, s: &str| matches!(&t.tok, Tok::Ident(x) | Tok::Punct(x) if x == s);
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let attr = ["#", "[", "cfg", "(", "test", ")", "]"];
        if toks.len() - i >= attr.len() && attr.iter().enumerate().all(|(k, s)| is(&toks[i + k], s))
        {
            i = item_end(&toks, i + attr.len());
        } else if is(&toks[i], "use") {
            while i < toks.len() && !is(&toks[i], ";") {
                i += 1;
            }
            i += 1;
        } else {
            out.push(toks[i].clone());
            i += 1;
        }
    }
    out
}

// Index after the item starting at `i`: its `;`, or the `}` closing its first top-level `{`.
fn item_end(toks: &[Token], mut i: usize) -> usize {
    let mut depth = 0i32;
    while i < toks.len() {
        if let Tok::Punct(p) = &toks[i].tok {
            match p.as_str() {
                "(" | "[" => depth += 1,
                ")" | "]" => depth -= 1,
                ";" if depth == 0 => return i + 1,
                "{" if depth == 0 => {
                    let mut braces = 0;
                    while i < toks.len() {
                        match &toks[i].tok {
                            Tok::Punct(p) if p == "{" => braces += 1,
                            Tok::Punct(p) if p == "}" => {
                                braces -= 1;
                                if braces == 0 {
                                    return i + 1;
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    toks.len()
}

#[derive(Debug, Default)]
struct Extracted {
    /// `(key, english, line)` from call sites whose arguments resolved.
    pairs: Vec<(String, String, usize)>,
    /// `(enclosing fn, line)` of call sites whose arguments did not.
    unparsed: Vec<(String, usize)>,
    /// `const NAME: &[(&str, &str)]` tables: `NAME -> [(key, english)]`.
    pair_tables: HashMap<String, Vec<(String, String)>>,
    /// `"english" => "key"` match arms per fn: `fn -> [(key, english)]`.
    match_tables: HashMap<String, Vec<(String, String)>>,
}

struct Block {
    parent: Option<usize>,
    func: String,
    consts: HashMap<String, Vec<String>>,
    aliases: Vec<String>,
}

const CALLEES: &[&str] = &["get_or", "fmt_or"];

fn extract(src: &str) -> Result<Extracted, String> {
    let toks = strip_tests_and_uses(lex(src)?);
    let is = |i: usize, s: &str| matches!(toks.get(i).map(|t| &t.tok), Some(Tok::Ident(x) | Tok::Punct(x)) if x == s);
    let ident = |i: usize| match toks.get(i).map(|t| &t.tok) {
        Some(Tok::Ident(x)) => Some(x.clone()),
        _ => None,
    };
    let string = |i: usize| match toks.get(i).map(|t| &t.tok) {
        Some(Tok::Str(x)) => Some(x.clone()),
        _ => None,
    };
    let mut ex = Extracted::default();
    let mut blocks = vec![Block {
        parent: None,
        func: String::new(),
        consts: HashMap::new(),
        aliases: Vec::new(),
    }];
    let mut cur = 0;
    let mut pending_fn: Option<String> = None;
    let mut skip_until = 0;
    // Innermost visible binding of `name` via `pick`, or an error if one block binds it twice.
    let resolve =
        |blocks: &Vec<Block>, mut b: usize, name: &str| -> Result<Option<String>, String> {
            loop {
                if let Some(v) = blocks[b].consts.get(name) {
                    return match v.as_slice() {
                        [one] => Ok(Some(one.clone())),
                        _ => Err(format!("ambiguous const {name} in one scope")),
                    };
                }
                match blocks[b].parent {
                    Some(p) => b = p,
                    None => return Ok(None),
                }
            }
        };
    let aliased = |blocks: &Vec<Block>, mut b: usize, name: &str| loop {
        if blocks[b].aliases.iter().any(|a| a == name) {
            return true;
        }
        match blocks[b].parent {
            Some(p) => b = p,
            None => return false,
        }
    };
    let mut i = 0;
    while i < toks.len() {
        let line = toks[i].line;
        match &toks[i].tok {
            Tok::Punct(p) if p == "{" => {
                let func = pending_fn
                    .take()
                    .unwrap_or_else(|| blocks[cur].func.clone());
                blocks.push(Block {
                    parent: Some(cur),
                    func,
                    consts: HashMap::new(),
                    aliases: Vec::new(),
                });
                cur = blocks.len() - 1;
            }
            Tok::Punct(p) if p == "}" => cur = blocks[cur].parent.unwrap_or(0),
            Tok::Punct(p) if p == ";" => pending_fn = None,
            Tok::Ident(k) if k == "fn" && ident(i + 1).is_some() => {
                pending_fn = ident(i + 1);
                // `"english" => "key"` arms anywhere in this fn's body form its match table.
                if let Some(name) = ident(i + 1) {
                    let end = item_end(&toks, i);
                    let arms: Vec<_> = (i..end)
                        .filter(|&j| is(j + 1, "=>"))
                        .filter_map(|j| Some((string(j + 2)?, string(j)?)))
                        .collect();
                    if !arms.is_empty() {
                        ex.match_tables.insert(name, arms);
                    }
                }
            }
            Tok::Ident(k) if (k == "const" || k == "static") && !is(i + 1, "fn") => {
                let n = if is(i + 1, "mut") { i + 2 } else { i + 1 };
                if let Some(name) = ident(n).filter(|_| is(n + 1, ":")) {
                    let end = item_end(&toks, n);
                    let eq = (n..end).find(|&j| is(j, "=")).unwrap_or(end);
                    if let (Some(v), true) = (string(eq + 1), eq + 2 == end - 1) {
                        blocks[cur].consts.entry(name).or_default().push(v);
                    } else if is(eq + 1, "&") && is(eq + 2, "[") {
                        let rows: Vec<_> = (eq..end)
                            .filter(|&j| {
                                is(j, "(") && is(j + 2, ",") && (is(j + 4, ")") || is(j + 5, ")"))
                            })
                            .filter_map(|j| Some((string(j + 1)?, string(j + 3)?)))
                            .collect();
                        if !rows.is_empty() {
                            ex.pair_tables.insert(name, rows);
                        }
                    }
                    i = end;
                    continue;
                }
            }
            // `let g = |k, fallback| ...get_or(k, fallback);` makes `g` a callee in this block.
            Tok::Ident(k) if k == "let" && is(i + 2, "=") && is(i + 3, "|") => {
                let end = item_end(&toks, i);
                let wraps =
                    (i..end).any(|j| ident(j).is_some_and(|x| CALLEES.contains(&x.as_str())));
                if let (true, Some(name)) = (wraps, ident(i + 1)) {
                    blocks[cur].aliases.push(name);
                    skip_until = end;
                }
            }
            Tok::Ident(name) if i >= skip_until => {
                let pending = is(i + 1, "::") && is(i + 2, "new") && name == "PendingDiag";
                let bare = !is(i.wrapping_sub(1), ".") && !is(i.wrapping_sub(1), "::");
                let callee = CALLEES.contains(&name.as_str())
                    || pending
                    || (bare && aliased(&blocks, cur, name));
                if callee && !is(i.wrapping_sub(1), "fn") {
                    let open = if pending { i + 3 } else { i + 1 };
                    let parsed = if is(open, "(") {
                        let args = call_args(&toks, open);
                        // A string literal or a visible const, optionally borrowed.
                        let arg = |n: usize| -> Result<Option<String>, String> {
                            let a = args.get(n).map(Vec::as_slice).unwrap_or_default();
                            let a = match a {
                                [amp, rest @ ..] if amp.tok == Tok::Punct("&".into()) => rest,
                                _ => a,
                            };
                            match a {
                                [
                                    Token {
                                        tok: Tok::Str(s), ..
                                    },
                                ] => Ok(Some(s.clone())),
                                [
                                    Token {
                                        tok: Tok::Ident(x), ..
                                    },
                                ] => resolve(&blocks, cur, x),
                                _ => Ok(None),
                            }
                        };
                        match (arg(0)?, arg(1)?) {
                            (Some(k), Some(e)) if is_key(&k) => Some((k, e)),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match parsed {
                        Some((k, e)) => ex.pairs.push((k, e, line)),
                        None => ex.unparsed.push((blocks[cur].func.clone(), line)),
                    }
                    if pending {
                        i += 3;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(ex)
}

// The comma-separated argument token lists of the call whose `(` is at `open`.
fn call_args(toks: &[Token], open: usize) -> Vec<Vec<Token>> {
    let (mut args, mut arg, mut depth) = (Vec::new(), Vec::new(), 0);
    for t in &toks[open + 1..] {
        match &t.tok {
            Tok::Punct(p) if ["(", "[", "{"].contains(&p.as_str()) => depth += 1,
            Tok::Punct(p) if [")", "]", "}"].contains(&p.as_str()) => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Tok::Punct(p) if p == "," && depth == 0 => {
                args.push(std::mem::take(&mut arg));
                continue;
            }
            _ => {}
        }
        arg.push(t.clone());
    }
    if !arg.is_empty() {
        args.push(arg);
    }
    args
}

fn is_key(s: &str) -> bool {
    s.split('.').count() >= 2
        && s.split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// How an unparseable call site is covered instead.
enum Covered {
    /// Its (key, english) pairs are the rows of this `const NAME: &[(&str, &str)]`.
    PairTable(&'static str),
    /// Its pairs are this fn's `"english" => "key"` match arms.
    MatchTable(&'static str),
    /// A wrapper whose own callers are checked directly.
    Forwarder,
}

// I005 (review fixup): (file, fn) alone let a second unparseable call in an allowlisted
// fn hide under the first entry; an exact line was brittle (unrelated edits renumber it).
// The fn's expected COUNT of unparseable call sites is the stable, still-tight signal.
/// `(file, enclosing fn, expected unparsed-call-site count, coverage, reason)`.
const ALLOWLIST: &[(&str, &str, usize, Covered, &str)] = &[
    (
        "cli_entry.rs",
        "usage",
        1,
        Covered::PairTable("TRACK_SINK_URL_LINES"),
        "loops over the usage.url.* track-sink table",
    ),
    (
        "ui.rs",
        "format_label",
        1,
        Covered::MatchTable("format_key"),
        "gui.format.* keys come from format_key's match",
    ),
    (
        "cli_entry.rs",
        "render",
        1,
        Covered::Forwarder,
        "PendingDiag renders the key/english its ::new sites passed",
    ),
    (
        "strings.rs",
        "fmt_or",
        1,
        Covered::Forwarder,
        "fmt_or delegates to get_or",
    ),
];

/// The ALLOWLIST lookup (I005): matched on `(file, fn)`, then validated against the
/// entry's expected unparsed-call-site COUNT. A second unparseable call added to an
/// allowlisted fn changes the count, so it is not silently covered by the fn's entry.
fn allowlist_check(file: &str, func: &str, actual_count: usize) -> Result<usize, String> {
    let Some(n) = ALLOWLIST
        .iter()
        .position(|(f, fun, ..)| *f == file && *fun == func)
    else {
        return Err(format!(
            "{file} fn {func}: {actual_count} unparseable call site(s), not allowlisted"
        ));
    };
    let expected = ALLOWLIST[n].2;
    if actual_count != expected {
        return Err(format!(
            "{file} fn {func}: {actual_count} unparseable call site(s) now, \
             ALLOWLIST expects {expected}"
        ));
    }
    Ok(n)
}

// I005 / N6 (IB1): a NEW allowlist, separate from the call-site ALLOWLIST above (X4-1).
// Empty: KU-F1 rendered `keys.hddvd_unverified` and ST-F1 the five ST-I2 `stop.*` keys,
// each removing its entry (§8.1 R11). A new en.json key freemkv does not render yet goes here.
/// `(key, reason)` for every en.json key deliberately not (yet) used by freemkv.
const ORPHAN_ALLOWLIST: &[(&str, &str)] = &[];

/// §8.1 R11's assumption, made real (review fixup): checking `checked` cannot prove an
/// ORPHAN_ALLOWLIST key is unused, because freemkv renders `keys.*` through bare
/// `strings::get(key)` calls that never land in `checked` (the extractor only recognizes
/// the get_or/fmt_or/PendingDiag::new call shapes, plus its two allowlisted tables). So
/// this searches the raw source text instead: any file containing the literal `"<key>"`
/// renders that key by *some* call shape, extractor-parseable or not — the allowlist
/// entry is stale, and that is a failure, not a warning.
///
/// `files` is `(display name, raw source text)` for every `.rs` file under FREEMKV_SRC.
fn stale_orphan_allowlist_entries(
    allowlist: &[(&str, &str)],
    files: &[(String, String)],
) -> Vec<String> {
    allowlist
        .iter()
        .filter_map(|(key, why)| {
            let quoted = format!("\"{key}\"");
            let at = files.iter().find_map(|(name, text)| {
                text.lines()
                    .enumerate()
                    .find(|(_, l)| l.contains(&quoted))
                    .map(|(i, _)| format!("{name}:{}", i + 1))
            })?;
            Some(format!(
                "stale orphan-allowlist entry {key}: literal {quoted} appears in \
                 freemkv src at {at} ({why}); remove it"
            ))
        })
        .collect()
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
    let mut checked: Vec<(String, String, String)> = Vec::new();
    let mut problems = Vec::new();
    let mut used = vec![false; ALLOWLIST.len()];
    let mut raw_sources: Vec<(String, String)> = Vec::new();
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(file).unwrap();
        let ex = match extract(&text) {
            Ok(ex) => ex,
            Err(e) => {
                problems.push(format!("{}: {e}", file.display()));
                raw_sources.push((name, text));
                continue;
            }
        };
        for (k, e, line) in ex.pairs {
            checked.push((k, e, format!("{name}:{line}")));
        }
        // Group by fn: ALLOWLIST now matches on (file, fn, expected COUNT), not a line.
        let mut by_func: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
        for (func, line) in ex.unparsed {
            by_func.entry(func).or_default().push(line);
        }
        for (func, lines) in by_func {
            match allowlist_check(&name, &func, lines.len()) {
                Err(e) => problems.push(e),
                Ok(n) => {
                    used[n] = true;
                    let rows = match ALLOWLIST[n].3 {
                        Covered::PairTable(t) => ex.pair_tables.get(t),
                        Covered::MatchTable(f) => ex.match_tables.get(f),
                        Covered::Forwarder => continue,
                    };
                    match rows {
                        Some(rows) => {
                            for (k, e) in rows {
                                checked.push((
                                    k.clone(),
                                    e.clone(),
                                    format!("{name} fn {func} table"),
                                ));
                            }
                        }
                        None => {
                            problems.push(format!("{name} fn {func}: allowlisted table not found"))
                        }
                    }
                }
            }
        }
        raw_sources.push((name, text));
    }
    for (n, (f, fun, _, _, why)) in ALLOWLIST.iter().enumerate() {
        if !used[n] {
            problems.push(format!("stale allowlist entry {f} fn {fun} ({why})"));
        }
    }
    assert!(
        checked.len() > 40,
        "only {} keys extracted from {src:?}; the call shape changed",
        checked.len()
    );
    for (key, english, at) in &checked {
        match lookup(&en, key) {
            None => problems.push(format!("missing {key} ({at})")),
            Some(v) if v != english => problems.push(format!(
                "{key}: en.json {v:?} != freemkv fallback {english:?} ({at})"
            )),
            Some(_) => {}
        }
    }

    // §8.1 R11: see stale_orphan_allowlist_entries' doc for why this greps raw source
    // text instead of using `checked`.
    for problem in stale_orphan_allowlist_entries(ORPHAN_ALLOWLIST, &raw_sources) {
        problems.push(problem);
    }

    assert!(
        problems.is_empty(),
        "{} freemkv fallback key problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

// I005 guard, per spec (this file's ALLOWLIST doc comment); do not change without
// re-widening the match back to a bare (file, fn). A fn's expected call-site COUNT is
// the tight signal now; a second unparseable call raises the count and must still fail.
#[test]
fn allowlist_matches_by_expected_call_count_not_line() {
    let (file, func, expected) = (ALLOWLIST[2].0, ALLOWLIST[2].1, ALLOWLIST[2].2);
    assert_eq!((file, func, expected), ("cli_entry.rs", "render", 1));
    assert_eq!(allowlist_check(file, func, expected), Ok(2));
    // The I005 case: a second unparseable call site appears in the same fn.
    assert!(allowlist_check(file, func, expected + 1).is_err());
    // Not allowlisted at all.
    assert!(allowlist_check(file, "no_such_fn", 1).is_err());
}

// §8.1 R11 guard (review fixup): `checked` never has a `keys.*` key (bare
// `strings::get`, unparsed here), so the old check could never catch a real stale entry.
#[test]
fn a_literal_key_in_freemkv_src_fails_as_stale() {
    // A synthetic entry, so the self-test does not depend on the live list.
    let allowlist = [("keys.hddvd_unverified", "until freemkv renders it")];
    let clean = [("other.rs".to_string(), "fn f() {}".to_string())];
    assert!(
        stale_orphan_allowlist_entries(&allowlist, &clean).is_empty(),
        "not rendered anywhere: not stale"
    );

    let now_rendered = [(
        "keys.rs".to_string(),
        "fn open_image() {\n    strings::get(\"keys.hddvd_unverified\")\n}".to_string(),
    )];
    let stale = stale_orphan_allowlist_entries(&allowlist, &now_rendered);
    assert_eq!(stale.len(), 1);
    assert!(stale[0].contains("keys.hddvd_unverified"), "{stale:?}");
    assert!(stale[0].contains("keys.rs:2"), "{stale:?}");
}

#[cfg(test)]
fn keys(ex: &Extracted) -> Vec<(&str, &str)> {
    let mut v: Vec<_> = ex
        .pairs
        .iter()
        .map(|(k, e, _)| (k.as_str(), e.as_str()))
        .collect();
    v.sort();
    v
}

#[test]
fn the_extractor_reads_every_call_shape() {
    let src = "
        fn a() {
            let x = strings::get_or(\"gui.menu.cut\", \"Cut\");
            let y = crate::strings::fmt_or(
                \"share.zip_failed\",
                \"Could not build \\
                 the zip: {error}\",
                &[(\"error\", \"x\")],
            );
            diags.push(PendingDiag::new(\"error.log_file_needs_value\", \"needs a path\"));
            let g = |k: &str, fallback: &str| crate::strings::get_or(k, fallback);
            item(g(\"gui.menu.paste\", \"Paste\"), None);
            log(\"gui.menu.log_is_not_g\", \"x\");
        }
        fn get_or(key: &str, english: &str) -> String { String::new() }
        impl PendingDiag { fn new(key: &str, english: &str) -> Self { todo!() } }
    ";
    let ex = extract(src).unwrap();
    assert_eq!(
        keys(&ex),
        [
            ("error.log_file_needs_value", "needs a path"),
            ("gui.menu.cut", "Cut"),
            ("gui.menu.paste", "Paste"),
            ("share.zip_failed", "Could not build the zip: {error}"),
        ]
    );
    assert!(ex.unparsed.is_empty(), "{:?}", ex.unparsed);
}

#[test]
fn the_extractor_decodes_raw_strings_and_escapes() {
    let src = "fn a() {
        get_or(r\"gui.a.raw\", r#\"say \"hi\"\"#);
        get_or(\"gui.a.hex\", \"\\x41\\u{2026}\\t\");
        get_or(\"gui.a.crlf\", \"one \\\r\n    two\");
    }";
    assert_eq!(
        keys(&extract(src).unwrap()),
        [
            ("gui.a.crlf", "one two"),
            ("gui.a.hex", "A\u{2026}\t"),
            ("gui.a.raw", "say \"hi\""),
        ]
    );
}

#[test]
fn consts_resolve_in_their_own_scope() {
    let src = "
        static TOP: &'static str = \"gui.a.top\";
        fn one() { const KEY: &str = \"gui.a.one\"; fmt_or(KEY, \"One\", &[]); }
        fn two() { const KEY: &str = \"gui.a.two\"; fmt_or(KEY, \"Two\", &[]); get_or(TOP, \"Top\"); }
    ";
    assert_eq!(
        keys(&extract(src).unwrap()),
        [
            ("gui.a.one", "One"),
            ("gui.a.top", "Top"),
            ("gui.a.two", "Two")
        ]
    );
    let dup = "fn f() { const K: &str = \"gui.a\"; const K: &str = \"gui.b\"; get_or(K, \"x\"); }";
    assert!(extract(dup).unwrap_err().contains("ambiguous const K"));
}

#[test]
fn comments_tests_and_uses_are_not_call_sites() {
    let src = "
        use crate::strings::{fmt_or, get_or};
        // get_or(\"gui.a.line_comment\", \"x\")
        /* get_or(\"gui.a.block\", /* nested */ \"x\") */
        fn real() { get_or(\"gui.a.real\", \"Real\"); }
        #[cfg(test)]
        mod tests { fn t() { get_or(\"gui.a.test_only\", \"x\"); get_or(key, \"x\"); } }
    ";
    let ex = extract(src).unwrap();
    assert_eq!(keys(&ex), [("gui.a.real", "Real")]);
    assert!(ex.unparsed.is_empty(), "{:?}", ex.unparsed);
}

#[test]
fn unresolvable_sites_are_reported_and_tables_are_read() {
    let src = "
        const LINES: &[(&str, &str)] = &[(\"usage.url.a\", \"A\"), (\"usage.url.b\", \"B\",)];
        fn usage() { for (key, english) in LINES { get_or(key, english); } }
        fn format_key(c: &str) -> Option<&'static str> {
            Some(match c { \"Whole disc\" => \"gui.format.iso\", _ => return None })
        }
        fn label(c: &str) -> String { get_or(\"not a key\", c) }
        fn passed(f: fn(&str, &str) -> String) {}
        fn later() { passed(get_or); }
    ";
    let ex = extract(src).unwrap();
    assert!(ex.pairs.is_empty());
    let funcs: Vec<_> = ex.unparsed.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(funcs, ["usage", "label", "later"]);
    assert_eq!(
        ex.pair_tables["LINES"],
        [("usage.url.a", "A"), ("usage.url.b", "B")].map(|(k, e)| (k.into(), e.into()))
    );
    assert_eq!(
        ex.match_tables["format_key"],
        [("gui.format.iso".to_string(), "Whole disc".to_string())]
    );
}
