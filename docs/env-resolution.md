# Environment locale resolution — `locale_from_env`, `locale_candidates_from_env`

## POSIX precedence (`locale_from_env`)

`LC_ALL` overrides every other locale variable, then the category-specific
`LC_MESSAGES`, then `LANG` as the fallback default. The FIRST of those that
is set and non-empty wins outright.

It used to skip a variable whose value was `C` or `POSIX`, which made an
explicitly-set `LC_ALL=C` behave as if it were unset and let `LC_MESSAGES`
take over: `LC_ALL=C LC_MESSAGES=de_DE.UTF-8 freemkv …` printed German. That
is backwards — `LC_ALL=C` is the standard way to demand the unlocalized,
parseable output a script is about to grep. Nothing special is needed to
honor it now: `normalize_code` maps `C` and `POSIX` to English on its own,
because neither is a two- or three-letter language subtag.

## GNU `LANGUAGE` precedence (`locale_candidates_from_env`)

GNU `LANGUAGE` sits ABOVE the POSIX vars, but only for message translation
and only when the selected locale is a real one. It is a colon-separated
priority LIST (`LANGUAGE=de:en` means "German, then English"): every entry is
a candidate, tried in order by the resolver until one has a catalog — so
`LANGUAGE=sw:de` reaches German even though Swahili does not ship, rather
than stopping at the first miss. It is IGNORED when the POSIX selection is
the `C` / `POSIX` locale (or nothing is set at all) — that is how `LC_ALL=C`
keeps demanding parseable output even with `LANGUAGE` exported. Without any
of this, `LANGUAGE=de:en LANG=en_US.UTF-8` (the shape a German user's desktop
sets) rendered English, because `LANGUAGE` was never consulted.

## `is_posix_c_locale`

Checks the raw locale NAME, not `normalize_code`'s output, because `en_US`
also normalizes to English yet is a real locale over which `LANGUAGE` still
takes precedence.
