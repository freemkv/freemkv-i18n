# tests/runtime.rs — why this test exists and why it is a single test

Nothing had ever called `set_language`, `set_locale` or `init`. The whole
startup path — the one that decides which language a user sees — was
reachable only by running the real binary, and every unit test in the crate
worked on catalogs it constructed by hand. That is causally why the
`zh-Hans-CN` fallback bug shipped: the catalogs were verified in exquisite
detail and the code that chooses between them was verified not at all.

It lives in an integration test rather than beside the unit tests because
`STRINGS` and `LANG_OVERRIDE` are process-wide statics. An integration test
binary is its own process, and this file holds exactly ONE test, so the
sequence below is deterministic: no other test can install a catalog or claim
the one-shot `--language` override underneath it. Adding a second `#[test]`
here would reintroduce the race, so don't — extend this one.

## The bug a user actually felt

macOS hands a process the full BCP-47 tag. `zh-Hans-CN` normalizes to
`zh-hans-cn`, which no catalog matches; the fallback then has to try
`zh-hans` — which IS compiled into this binary — before `zh` and English.
It used to skip straight from the full tag to `zh`, and since no `zh.json`
ships, a Simplified Chinese user got an English UI with nothing logged.
