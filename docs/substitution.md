# Placeholder substitution — `fmt`, `substitute`, `tidy_empty_slots`

## `fmt` — single-pass substitution

Substitution is a SINGLE pass over the catalog string: a value that itself
contains `{something}` is copied out verbatim and never rescanned. The old
implementation ran `String::replace` once per argument over the ACCUMULATING
result, so a disc label containing the literal text `{size}` was rewritten by
the next argument's pass — disc-derived text was being interpreted as format
syntax.

There is deliberately no `{{` escape: the catalogs are translator-authored
JSON, no catalog uses one, and inventing an escape would silently rewrite any
translation that ever contains a doubled brace.

## `error_message` / `error_message_with` — why placeholders are dropped, not leaked

Twenty-three error codes have an English string that embeds a placeholder —
`{detail}` (a device path, a sector, an HTTP status), `{hash}` (E7022's disc
id), or `{path}` (E9067/E9068's file name) — that only a caller holding the
originating error can fill. `error_message` has none of those values, so a
raw lookup would render the literal characters `{detail}`/`{hash}`/`{path}`
to the user: exactly the "reads as benign / actually broken" failure this
crate exists to prevent, and the one a bare-code caller such as autorip's
`error_message(err.code())` hit. It therefore DROPS every unfilled
placeholder (and tidies what that leaves) rather than leaking it.
`error_message_with` is the companion that fills `{detail}` when the caller
has it, and still drops any other unfilled placeholder the same way.

## `tidy_empty_slots` — cleaning up after a drop

Collapses the artifacts a dropped placeholder leaves behind so the message is
clean and brace-free in every locale: an empty bracket or quote (`()`, `''`,
and their full-width `（）` forms, which the CJK catalogs use), a label-only
parenthetical like `(id:)` left when E7022's `{hash}` value goes, a space
before `.,:;!?)`/`）`, a space after `(`/`（`, a dangling trailing `:`, and
runs of spaces. Language-neutral — it only moves whitespace, brackets, quotes
and punctuation, never letters — so it cannot corrupt a translation.
