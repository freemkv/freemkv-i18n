# Loader diagnostics — unreadable files, missing-translation reporting, per-key fallback

## `locale_read_diagnostic`

The read used to be `.ok()?`, which collapsed two different situations into
one: a file that exists but cannot be read (wrong permissions, a directory
where a file was expected, a broken symlink target, non-UTF-8 bytes) was
indistinguishable from a file that was never there, and the user was told the
locale was "not found" while looking straight at it. The very next branch
already reports an unparseable file; this makes the two symmetrical.

## `report_missing_translation`

The substitution itself used to be completely silent: a locale missing a
translation produced a perfectly ordinary-looking English sentence in the
middle of, say, a German UI, and nothing anywhere recorded that a translation
was missing — the exact shape this project treats as a defect in its own
right, a fallback that hides what it is covering for. Per-key deduplication
keeps a progress loop from flooding the terminal without hiding the size of
the gap: the number of lines IS the number of untranslated keys in use.

## `lookup_or_english`

The catalog-level fallback in `resolve_catalog_tagged` only chooses WHICH
file to load; without this per-key fallback, a key present in en.json but
missing from the active locale rendered as the raw path — a user running
under `de` saw the literal text `error.E9053`. That is strictly worse than
English: it is not a message in any language, and it leaks an internal key
into the UI.

A BLANK value counts as a miss, not a hit: an override file whose key maps to
`""` (or only whitespace) used to be a "successful" lookup that rendered as
an empty message — reads as success while saying nothing, with no
diagnostic. It is now treated exactly like an absent key.

`active_is_english` short-circuits the second walk when the catalog in hand
IS the English one, where it can only miss again.
