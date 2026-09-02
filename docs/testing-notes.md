# Test suite notes

## `verify_locale` — bidirectional key parity

Parity used to be one-directional, so retiring a string from en.json left it
behind in all 29 locales with nothing to flag it: a stale entry that
translators keep re-translating and reviewers keep reading as live. Both
directions are checked now, so a removal has to be completed everywhere.

## `verify_locale` — raw-value placeholder comparison

Two holes used to sit here. The values were read through `lookup`, the
production accessor, which carries the per-key English fallback: a locale
whose leaf was a number, an object, or an empty string returned the ENGLISH
text and every assertion passed on it — the parity test could not fail on
the wrong-type/blank-value class it exists to catch. And the comparison ran
one way only, so an EXTRA `{code}` invented in a translation — which no
caller supplies and which therefore renders to the user as the literal
characters `{code}` — was invisible. Both directions, against the raw
catalog values, close both holes.

## `a_poisoned_catalog_lock_does_not_take_the_process_down`

A panic anywhere in the program while the write lock is held poisons it, and
an unwrapping accessor then turns that into a panic in EVERY later string
lookup — including the one the panic handler itself makes while trying to
render the original error. The user loses the real failure and gets a
lock-poisoning backtrace in its place. The data behind the lock is a parsed
catalog that a panic cannot leave half-written, so recovering the guard is
both safe and strictly better than aborting the run.

This test deliberately poisons the process-wide lock and leaves it poisoned:
with the fix in place nothing else notices, which is the entire point.

## `a_key_missing_from_the_locale_falls_back_to_english`

Before the per-key fallback, `lookup` returned the path itself, so a user
under a non-English locale saw the literal string `error.E9053` where a
message belonged. Five error codes added during the 1.6.0 audit were in
exactly that state — and two of them exist because that audit split apart
codes that had been reporting total failure as success, so the fix that made
those failures visible would have shown an internal key.

## `libfreemkv_error_codes_all_have_english_strings`

`error_message` takes a bare `u32` with nothing to enumerate, so the only
guard used to be a handful of hand-written code literals in tests: a new
libfreemkv code could ship, no string was ever written for it, and it
rendered to the user as the literal text `error.E12345` forever while fmt,
clippy and the test suite all stayed green. The CHANGELOG records this
happening — "five codes shipped without one because nothing compared the two
lists". At the time this check was added, eleven more codes were in that
state, including E9056/E9057 (a rip that could not be confirmed written to
disk). See docs/error-codes.md for why the list is checked in rather than
read from libfreemkv, and `libfreemkv_code_list_has_not_drifted` for how the
copy is kept honest.

## `normalize_code_does_not_panic_on_multibyte`

Regression: byte-slicing `s[..2]` panicked on a leading multibyte char (e.g.
`--language あ`, `LC_ALL=€a`). Untrusted input must never panic — it must
fall back to English (or the ASCII language part). `"de"` is a real, valid
ASCII tag mixed into the same loop so the `code.split('-').all(...)` arm of
the assertion actually runs at least once — every other input here folds to
`"en"` and short-circuits past it.
