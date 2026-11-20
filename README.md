[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![CI](https://github.com/freemkv/freemkv-i18n/actions/workflows/ci.yml/badge.svg)](https://github.com/freemkv/freemkv-i18n/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/freemkv/freemkv-i18n/branch/dev/graph/badge.svg)](https://codecov.io/gh/freemkv/freemkv-i18n)

# freemkv-i18n

Translated strings for the freemkv toolchain. The catalogs are compiled into the crate, and a catalog found on disk can replace a bundled one without a rebuild.

## Locales and keys

Each language is one JSON file in `locales/`, named after its locale code: `de.json`, `pt-br.json`, `zh-hans.json`, `es-419.json`. `build.rs` bundles every `locales/*.json` at compile time, so there is no list to update. The crate ships 29 catalogs.

`locales/en.json` is the source of truth. Keys are dotted paths into nested JSON objects, such as `disc.scanning`, and values are strings. A `{name}` in a value is a placeholder that `fmt` fills in. libfreemkv error messages live under `error.E<code>` and are looked up with `error_message(code)` or `error_message_with(code, detail)`.

## Using the crate

```rust
freemkv_i18n::get("disc.scanning");
freemkv_i18n::fmt("app.unknown_option", &[("opt", "--nope")]);
```

- `get(path)` returns the string for a key.
- `fmt(path, args)` does the same and replaces each `{name}` placeholder.
- `set_language(code)` is the one-time override for a `--language` flag. Call it before the first lookup.
- `set_locale(code)` changes the language while the program runs. `"auto"` detects it again from the environment.
- `init()` picks the language at startup. `get` and `fmt` call it on first use, so calling it yourself is optional.

Without an override, the language comes from `LC_ALL`, `LC_MESSAGES` or `LANG`, plus `LANGUAGE` as a priority list. The C and POSIX locales mean English.

## Fallback

Codes are normalized first, so `pt_BR.UTF-8` becomes `pt-br`. Then the most specific catalog wins: `zh-hans-cn`, then `zh-hans`, then `zh`, and finally English. A key that is missing or blank in the chosen language shows the English text. Only a key that is in no catalog shows as its own path. A locale that cannot be found at all prints a notice and uses English.

For each code, the bundled catalog is tried first, then these directories in order:

1. `locales/` next to the executable
2. `~/.config/freemkv/locales/` (`%USERPROFILE%` on Windows)
3. `/usr/share/freemkv/locales/` (`%PROGRAMDATA%\freemkv\locales` on Windows)
4. `locales/` in the working directory

## Adding a language

1. Copy `locales/en.json` to `locales/<code>.json`, for example `sw.json` or `pt-br.json`.
2. Translate the values. Keep every key and every `{placeholder}` exactly as it is in English, and leave no value empty.
3. Run the tests. They fail if a key is missing or extra, if a placeholder differs from English, or if a value is blank.

When you add a key, put it in `en.json` and in every other catalog in the same change, using the English text until it is translated.

## Development

```bash
cargo test
```

`src/error_codes.rs` lists libfreemkv's error codes. After a code is added in libfreemkv, run `ci/sync-error-codes.sh` to regenerate it, then add the message to every locale.

## License

MIT. See [LICENSE](LICENSE).
