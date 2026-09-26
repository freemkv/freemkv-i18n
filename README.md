[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![CI](https://github.com/freemkv/freemkv-i18n/actions/workflows/ci.yml/badge.svg)](https://github.com/freemkv/freemkv-i18n/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/freemkv/freemkv-i18n/branch/dev/graph/badge.svg)](https://codecov.io/gh/freemkv/freemkv-i18n)

# freemkv-i18n

i18n string loader for the freemkv toolchain (bundled + on-disk locales).

## Usage

Use `set_language` for a one-time startup override before initialization;
use `set_locale` for live language changes. `get` looks up a string and `fmt`
substitutes its named arguments. English remains the fallback catalog.

Add translations by matching the keys and placeholders in `locales/en.json`.
Build API documentation with `cargo doc --no-deps --open` for locale resolution
and catalog loading. Run `cargo test --tests` to check substitutions and fallbacks.

## License

MIT — see [LICENSE](LICENSE).
