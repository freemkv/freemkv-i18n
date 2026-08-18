// Scan locales/ directory and generate bundled locale code.
// Drop a new xx.json in locales/ → next build picks it up automatically.

use std::fs;
use std::path::Path;

// The SAME locale-tag arithmetic the runtime loader uses, included verbatim
// rather than reimplemented. A build script cannot depend on the crate it is
// building, and the previous hand-rolled derivation here (lowercase, `_` → `-`)
// only CLAIMED to match `normalize_code`: it lacked the Chinese script
// inference, so a `zh_TW.json` would have been bundled under the code `zh-tw`,
// which the runtime never asks for — the runtime turns `zh_TW` into `zh-hant`,
// so the file would ship inside the binary and be permanently unreachable.
// `src/locale_code.rs` is std-only precisely so it can be pulled in here.
#[allow(dead_code)]
mod locale_code {
    include!("src/locale_code.rs");
}

fn main() {
    let locales_dir = Path::new("locales");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("locales_generated.rs");

    let mut includes = Vec::new();
    let mut match_arms = Vec::new();
    let mut codes: Vec<(String, String)> = Vec::new();

    if locales_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(locales_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let filename = entry.file_name().to_string_lossy().to_string();
            let stem = filename.trim_end_matches(".json");
            // Internal code: exactly what `normalize_code` would produce for the
            // tag, so the code a file is bundled under is always a code the
            // runtime can actually resolve to. The const name is sanitized to a
            // valid Rust identifier from the STEM (not the code) so it stays
            // stable: `pt-br.json` → `LOCALE_PT_BR`.
            let code = locale_code::normalize_code(stem);
            let const_name = format!("LOCALE_{}", stem.to_uppercase().replace(['-', '.'], "_"));

            // Two files normalizing to one code means one of them can never be
            // loaded. That is silent data loss in a generated table, so refuse
            // to build instead. It also catches a stray non-locale `*.json`
            // (`README.json` normalizes to `en`) shadowing a real catalog.
            if let Some((dup, other)) = codes.iter().find(|(c, _)| *c == code) {
                panic!(
                    "locales/: {filename} and {other} both normalize to the locale code {dup:?}; \
                     one of them would be unreachable at runtime"
                );
            }

            includes.push(format!(
                "const {}: &str = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/locales/{}\"));",
                const_name, filename
            ));
            match_arms.push(format!("        \"{}\" => Some({}),", code, const_name));
            codes.push((code, filename));
        }
    }

    let codes_arr = codes
        .iter()
        .map(|(c, _)| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");

    let generated = format!(
        "{}\n\nfn bundled_locale(code: &str) -> Option<&'static str> {{\n    match code {{\n{}\n        _ => None,\n    }}\n}}\n\n/// Every locale code compiled into the crate (derived from locales/*.json).\npub const SHIPPED_CODES: &[&str] = &[{}];\n",
        includes.join("\n"),
        match_arms.join("\n"),
        codes_arr,
    );

    fs::write(&out_path, generated).unwrap();

    println!("cargo:rerun-if-changed=locales");
    println!("cargo:rerun-if-changed=src/locale_code.rs");
}
