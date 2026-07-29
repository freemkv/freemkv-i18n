// Scan locales/ directory and generate bundled locale code.
// Drop a new xx.json in locales/ → next build picks it up automatically.

use std::fs;
use std::path::Path;

fn main() {
    let locales_dir = Path::new("locales");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("locales_generated.rs");

    let mut includes = Vec::new();
    let mut match_arms = Vec::new();
    let mut codes = Vec::new();

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
            // Internal code: lowercase, hyphen-joined (matches normalize_code):
            // `pt-BR.json`/`pt_BR.json` → `pt-br`. Const name sanitized to a
            // valid Rust identifier: `pt-br` → `LOCALE_PT_BR`.
            let code = stem.to_lowercase().replace('_', "-");
            let const_name = format!("LOCALE_{}", stem.to_uppercase().replace(['-', '.'], "_"));

            includes.push(format!(
                "const {}: &str = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/locales/{}\"));",
                const_name, filename
            ));
            match_arms.push(format!("        \"{}\" => Some({}),", code, const_name));
            codes.push(code);
        }
    }

    let codes_arr = codes
        .iter()
        .map(|c| format!("\"{}\"", c))
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
}
