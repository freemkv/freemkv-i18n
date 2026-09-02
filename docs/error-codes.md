# `LIBFREEMKV_ERROR_CODES` — why this is a checked-in copy

This file is generated; do not edit by hand. Regenerate with:

    ci/sync-error-codes.sh

`LIBFREEMKV_ERROR_CODES` lists every error code libfreemkv can raise, as
(code, constant name) pairs.

It is a checked-in copy rather than a dependency: libfreemkv git-deps
freemkv-unlock, which is private and unpublished, so depending on it would
make this crate unbuildable in its own CI — and it would invert the
layering, since libfreemkv emits codes and this crate is what turns them
into text.

The copy is not trusted to stay correct on its own. See
`libfreemkv_error_codes_all_have_english_strings` (every listed code must
have an English string — runs in CI) and `libfreemkv_code_list_has_not_drifted`
(this list must equal the real one — runs wherever a sibling libfreemkv
checkout exists).
