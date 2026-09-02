# `locale_code.rs` — design rationale

Locale-tag arithmetic, shared by the runtime loader (`lib.rs`) and the build
script (`build.rs`) — the two used to derive a locale's internal code with
separate, subtly different code. `build.rs` lower-cased the filename stem and
swapped `_` for `-`; the runtime ran the full `normalize_code`, which also
infers a Chinese script from the region. A `zh_TW.json` dropped into
`locales/` therefore registered under the code `zh-tw`, which nothing ever
asks for (the runtime turns `zh_TW` into `zh-hant`), so the file was compiled
into the binary and then permanently unreachable.

There is exactly one derivation now, and `build.rs` includes this file
verbatim (`include!`) rather than depending on the crate it is building. That
is why everything here is `std`-only with no `use` statements at file scope:
it has to compile inside a build script as well as inside the library.

## `normalize_code`

Inputs are untrusted (the `--language` CLI flag and the `LC_*`/`LANG` env
vars), so this must never panic — it iterates by *character*, validates
ASCII, and falls back to English on anything malformed. That fallback also
gives the POSIX `C`/`POSIX` locales the right answer for free: neither is a
2-3 letter language subtag, so both land on English, which is precisely what
`LC_ALL=C` is asking for.

Chinese without an explicit script infers one from the region (tw/hk/mo →
hant, else hans) and defaults to Simplified, so `zh_CN` → `zh-hans`,
`zh_TW` → `zh-hant`, bare `zh` → `zh-hans`.

### Extended-language subtag promotion

BCP-47 extended-language subtag: a three-letter subtag immediately after
the primary language (`zh-yue` Cantonese, `zh-cmn` Mandarin) whose
canonical form is that subtag standing alone — `zh-yue` IS `yue`. It used
to be dropped silently (neither a 4-letter script nor a 2-letter/3-digit
region, so the loop below ignored it), after which the Chinese script
inference folded the bare `zh` onto the Simplified catalog: a Cantonese
tag rendered as Simplified Mandarin — a different language — with nothing
logged. Promoting the extlang to the primary language makes `yue` fall
through to English (no catalog ships) while `cmn` folds to `zh` like any
other Mandarin tag. A 3-digit subtag (`es-419`) is a region, not an
extlang, so the alphabetic check is what keeps that case intact.

## `fold_language`

Fold a primary-language subtag onto the two-letter code the crate ships a
catalog under. Covers the ISO 639-2/B, 639-2/T and 639-3 three-letter codes
for the shipped languages (`deu`/`ger` → `de`, `fra`/`fre` → `fr`, …), plus
macrolanguage members and deprecated aliases that have no catalog of their
own (`nb`/`nn` Norwegian Bokmål/Nynorsk → `no`, `cmn` Mandarin → `zh`, the
legacy `mo` → `ro` and `in` → `id`). An unknown code is returned unchanged,
so a language the crate does not ship still resolves through the normal
fallback chain to English rather than being mangled into a wrong catalog.

## `fallback_chain`

This exists because the fallback used to be exactly two steps — the full tag
and then `code.split('-').next()`, the first subtag — which silently skipped
the `lang-script` level in the middle. macOS hands the process
`zh-Hans-CN`; that normalizes to `zh-hans-cn`, which no catalog matches, and
the old chain jumped straight to `zh`, which no catalog matches either
because the crate ships `zh-hans.json` and `zh-hant.json`, not `zh.json`.
The result was English: the two Chinese catalogs shipped in the binary were
unreachable from the tag the operating system actually emits, and a
Simplified Chinese user saw an English UI with nothing logged to say why.

The chain is now every prefix of the tag, longest first — `zh-hans-cn` →
`zh-hans` → `zh`, `sr-latn-rs` → `sr-latn` → `sr`, `pt-br` → `pt`. A
`lang-script` catalog is found whether or not a region is attached, and a
bare `zh.json` still works if someone ships one.

## `locale_filenames`

The internal code is always lowercase and hyphen-joined, but an operator
dropping a file into `locales/` writes the tag the way the rest of the world
writes it — `pt-BR.json`, `zh_Hans_CN.json`. The loader used to build one
lowercase name and `read_to_string` it, which finds nothing on any
case-sensitive filesystem, so a perfectly good `locales/pt-BR.json` was
invisible on Linux while working on macOS and Windows purely because those
filesystems fold case. `build.rs` already accepted both spellings when
bundling, so the two halves of the crate disagreed about the same file.

Rather than read the directory (which costs a syscall per search path even
when nothing is there, and drags in its own platform differences), derive the
small set of spellings that are actually plausible: the internal form, the
underscore form, and BCP-47 canonical case (`lang-Script-REGION`) in both
separators. Four names at worst, and a one-shot startup path stats them.
