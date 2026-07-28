# Changelog

## [1.6.0] — UNRELEASED

### Fixed

- **`usage.flag.title` (the live `-t` help line)** reworded in all seven bundled
  locales. It previously read "Default: all." — stale since 1.6.0. It now states
  the 1.6.0 behaviour: `-t` defaults to the main title (title 1), `-t all` selects
  every title, and `-t N` is 1-based and repeatable.

### Added

- **`usage.flag.audio` / `usage.flag.subtitles`** help strings (all seven locales)
  for the `-a`/`--audio` and `-s`/`--subtitles` stream-selection flags: a
  comma-separated language list (names or ISO codes, case-insensitive) or
  `all`/`none`, defaulting to keep-all. The `usage.flag.share` line now notes that
  `--share` is `info disc://`-only and that `-s` on a rip means `--subtitles`.
- **`error.unknown_language`** (all seven locales, placeholders `{tag}` and
  `{available}`) plus **`error.stream_none`** — the CLI's stream-selection error is
  now localized instead of hardcoded English.

### Removed

- **Dead scaffolding keys** from all seven locales: the orphaned `app.*` block
  (`app.usage`, `app.commands`, `app.cmd_*`, `app.opt_device/output/title/keydb/
  list/raw/share/mask`, `app.rip_options`, `app.examples`, `app.drive_info_options`,
  `app.global_options`, `app.unknown_command`), and `usage.subcmd.verify`,
  `usage.subcmd.remux`, `usage.synopsis_3`. All had zero code references. The three
  still-used `app.*` keys (`app.unknown_option`, `app.opt_quiet`, `app.opt_verbose`)
  are kept.

## [1.5.2] — 2026-07-22

Version sync with the workspace. No functional change.

## [1.5.0] — 2026-07-19

### Added

- **`E7026` string** (AACS 2.1 FMTS variant key missing) in all seven bundled
  locales — closes a gap where the error had a code but no user-facing text, so the
  CLI rendered the key path instead of a message.

Version sync with the workspace; inherits libfreemkv 1.5.0.

## [1.4.1] — 2026-07-14

Version sync with the workspace; inherits libfreemkv 1.4.1.

## [1.4.0] — 2026-07-13

Version sync with the workspace; inherits libfreemkv 1.4.0.

## [1.3.2] — 2026-07-10

Version sync with the workspace; inherits libfreemkv 1.3.2.

## [1.3.1] — 2026-07-10

### Licensing

- **Relicensed to the MIT License, from 1.3.1 onwards** (releases up to and
  including 1.3.0 remain under AGPL-3.0).

Version sync with the workspace; inherits libfreemkv 1.3.1.

## [1.3.0] — 2026-07-08

Version sync with the rest of the freemkv toolchain. No functional change to the
string loader or the bundled locales.
