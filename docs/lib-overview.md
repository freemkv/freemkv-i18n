# freemkv-i18n — i18n string loader (shared across the freemkv toolchain)

MIT — freemkv project. There is no README: this file plus the module docs
are the specification.

The libraries under this crate emit NUMERIC ERROR CODES and never English
text. This crate is the only place a code becomes something a person reads,
which makes its failure mode unusual: a bug here does not crash, it produces
a message. A wrong, blank or silently-English message turns a real failure
into something that reads as benign, and nothing downstream can tell.

English is compiled into the binary (always available). Other languages are
compiled in too, and additional ones can be loaded from disk at runtime —
drop a JSON file next to the binary, done.

## Language priority

1. `--language` flag (set via `set_language()` before `init()`)
2. GNU `LANGUAGE` — a colon-separated priority LIST (`LANGUAGE=de:en`), each
   entry tried in turn until one has a catalog. It sits above the POSIX vars
   but is consulted ONLY when the POSIX selection names a real locale: with
   `LC_ALL=C` (or nothing set) it is ignored, so a script asking for
   parseable output still gets English.
3. `LC_ALL` / `LC_MESSAGES` / `LANG` env var (POSIX precedence: the FIRST of
   those that is set and non-empty wins outright, even when its value is `C`
   or `POSIX` — both of which mean English. `LC_ALL=C` is how a script asks
   for parseable output and it must not be overridden by `LC_MESSAGES`.)
4. English fallback

## Catalog resolution

Catalog resolution walks every prefix of the requested tag, longest first, so
a three-subtag tag still finds a two-subtag catalog:

```
zh-Hans-CN → zh-hans-cn → zh-hans → zh → en
pt-BR      → pt-br      → pt      → en
```

## Search paths

Search paths for on-disk locale files (each tried under the internal
lowercase name, the underscore form, and BCP-47 canonical case, so an
operator's `pt-BR.json` is found on a case-sensitive filesystem):

1. `<binary dir>/locales/xx.json` (next to the binary)
2. `<home>/.config/freemkv/locales/xx.json` (home is `$HOME`, or
   `%USERPROFILE%` on Windows)
3. `/usr/share/freemkv/locales/xx.json` (`%PROGRAMDATA%\freemkv\locales` on
   Windows)
4. `./locales/xx.json` (working directory — lowest precedence, so launching
   from an arbitrary directory can't shadow the installed/user catalog)

To add a language: create `locales/xx.json` (copy `en.json` structure) and
place it in any search path. No code changes needed.

## API note — two functions look interchangeable and are not

* `set_language` is the one-shot `--language` override. It must be called
  BEFORE anything reads a string, and only takes effect once.
* `set_locale` is the live switch a GUI uses. It works at any time and can be
  called repeatedly.

`get` and `fmt` are deliberately terse crate-root names: every consumer
re-exports this crate under a module (freemkv does `pub use freemkv_i18n::*;`
from `strings.rs`), so they are read as `strings::get` / `strings::fmt` at
the call site. Renaming them would break those consumers for no gain.
