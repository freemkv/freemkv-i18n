# Changelog

## [1.6.7] — 2026-08-21

### Changed

- Version aligned to 1.6.7 for the unified release. No functional changes to
  this crate; the release was driven by autorip (per-webhook event selection,
  a progress bar per moved artifact, and move-queue / webhook-error fixes —
  see the autorip 1.6.7 notes).

## [1.6.6] — 2026-08-20

### Changed

- Version aligned to 1.6.6 for the unified release. No functional changes
  to this crate; the release was driven by autorip (webhooks may now target
  private/LAN addresses — see the autorip 1.6.6 notes).

## [1.6.5] — 2026-08-20

### Added

- **Twenty-two strings the freemkv CLI was still printing in English**,
  found by a frontend audit and added to all 29 locales (which the parity
  test requires). They cover the nine `--help` URL-scheme lines and their
  header, the `--language` flag's help line, the `info` "File:" label for
  the container arm, the five argv pre-pass diagnostics that run before the
  catalog is even loaded (bad `--log-level`, `--log-file` and `--language`
  values), and a macOS keydb-busy note the Windows shell already read from
  the catalog.

- **The `--share` drive-profile submit prompt, now fail-closed.** The
  consent flow gained its own keys and the prompt reads `[y/N]`: a bare
  Enter DECLINES, so a drive profile — which carries the drive serial
  unless `--mask` — is never posted to the public tracker on a stray
  keypress. Every locale previously advertised `[Y/n]`, a default-YES the
  parser no longer honours.

- **A string for `E6019`** (a UDF file whose allocation descriptors resolve
  but yield no usable extent). libfreemkv added the code this cycle; without
  a string it would have rendered as the literal text `error.E6019`. Added
  to all 29 locales.

### Fixed

- **Simplified and Traditional Chinese were unreachable from the locale tag
  macOS actually emits.** Resolution tried the full tag and then the FIRST
  subtag, with no `lang-script` step in between: `zh-Hans-CN` became
  `zh-hans-cn` (no catalog), then `zh` (no catalog — the crate ships
  `zh-hans.json` and `zh-hant.json`, not `zh.json`), then English. Both Chinese
  catalogs were compiled into every binary and could not be reached from a
  standard BCP-47 tag. The chain now walks every prefix of the tag, longest
  first, which also fixes `sr-Latn-RS` and any other three-subtag tag.
- **`LC_ALL=C` no longer loses to `LC_MESSAGES`.** An explicitly-set `LC_ALL`
  whose value was `C` or `POSIX` was treated as unset, so
  `LC_ALL=C LC_MESSAGES=de_DE.UTF-8` printed German — the opposite of what
  POSIX says, of what the code's own comment claimed, and of what a script
  asking for parseable output wants.
- **Eleven libfreemkv error codes had no English string** and rendered to the
  user as the literal text `error.E9056`. Among them were the two codes that
  report a rip could not be confirmed written to disk. The suite's code list is
  now compared against the catalog mechanically.
- **An operator's `locales/pt-BR.json` is found on Linux.** The loader built one
  lowercase filename and read it exact-case, while `build.rs` accepted both
  spellings when bundling; the two halves of the crate disagreed about the same
  file. Both now share one derivation, which also stops a `zh_TW.json` from
  being bundled under a code nothing can ask for.
- **A locale file that exists but cannot be read is reported**, instead of
  producing "locale not found" — a misleading message about a file the operator
  is looking straight at.
- **A translation served from English says so**, once per key. The per-key
  fallback was completely silent, which is the shape this project treats as a
  defect in its own right.
- **Placeholder substitution is single-pass.** A disc label containing the
  literal text `{size}` was being rewritten by the next argument's pass.
- **The per-user locale directory exists on Windows** (`%USERPROFILE%`, and
  `%PROGRAMDATA%` in place of `/usr/share`), delivering the cross-platform
  search path the crate documents.
- **A poisoned catalog lock no longer takes the process down**, a second
  `--language` is reported rather than dropped, and the `--language` check and
  the override write are no longer separated by a window an `init()` can land
  in. Locale resolution no longer does file I/O while holding the write lock.

### Changed

- The locale parity test asserts against the RAW catalogs and compares
  placeholders in BOTH directions. It previously read values through the
  production accessor, whose per-key English fallback meant a locale value that
  was blank, a number or an object silently returned English and passed every
  assertion — the test could not fail on the class it exists to catch.
- The English catalog is parsed once per process rather than on every missed
  key, and is not consulted at all when English is already the active locale.
- The runtime resolution path (`set_language`, `init`, `set_locale`, the disk
  search, the fallback chain) has tests. It had none, which is causally why the
  Chinese bug shipped unnoticed.

## [1.6.4] — 2026-08-15

### Added

- **The destination-is-source refusal message**, so the CLI can explain why it
  will not decrypt an image over its own source file. Added to all 29 locales,
  which the parity test requires.

## [1.6.3] — 2026-08-10

### Changed

- **No functional change.** This crate ships alongside the rest of freemkv at a
  matching version. Its build and release checks were updated; the strings it
  loads and the way it loads them are untouched.

## [1.6.2] — 2026-08-08

Version sync with the workspace. No functional change in this crate.

## [1.6.1] — 2026-08-07

Version sync with the workspace. No functional change in this crate.

## [1.6.0] — 2026-08-03

### Added

- **Strings for the three key-service failure codes** (`E7028` unreachable,
  `E7029` token rejected, `E7030` rate limited), so an outage no longer reads
  as "this disc has no key". Each ends by saying explicitly that it does not
  mean the disc has no key.
- **Strings for the three per-track-kind picker rows** (video / audio /
  subtitle only). Translated for de, fr, es, it, nl and pt, reusing each
  locale's own established wording for that family; the remaining catalogs
  carry the English text, which is what a user sees anyway via the per-key
  fallback, and gives a translator something to find.

### Note

- This release grew the bundled catalogs from seven locales to **29**. Entries
  below that were written earlier in the cycle say "all seven locales" — read
  those as "all locales bundled at the time", not as the shipped count.

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
