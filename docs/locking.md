# Locking design — `STRINGS`, `INIT_LOCK`, and lock poisoning

## `INIT_LOCK`

Serializes the catalog-INSTALLING paths — lazy first-use `get`, `init`,
`set_locale` — with each other and with `set_language`. It is deliberately
NOT on the `get`/`fmt` fast path (a catalog already installed): a reader then
takes only the `STRINGS` read lock and never touches this. It exists to close
two windows the `STRINGS` lock alone cannot, because resolution reads the
environment / `--language` override OUTSIDE the `STRINGS` write lock:

* a `set_language` override arriving WHILE another thread is mid-resolve —
  that thread would install an environment-derived catalog and the override
  would be silently dropped even though `set_language` reported success;
* two `set_locale` calls whose resolves complete in the opposite order from
  which they were called, so the last WRITER wins rather than the last
  CALLER. Holding this lock across resolve-then-install makes the last
  caller to acquire it the last to install.

## Lock access (`read_strings` / `write_strings`)

A poisoned `RwLock` must not take the process down. `.unwrap()` on a lock
turns one panic anywhere in the program into a panic in EVERY later call to
`get`, including the panic handler's own attempt to render a message — the
user then loses the real error and sees a lock-poisoning backtrace instead.
The data behind this lock is a parsed JSON catalog: a panic cannot leave it
half-written into an inconsistent state, so recovering the guard is safe and
is strictly better than aborting the run over a message lookup.

## `set_language` — why a read guard alone isn't enough

Take `INIT_LOCK` for the whole check-then-set. Every catalog-installing path
(lazy `get`, `init`, `set_locale`) holds it across resolve-then-install, so
this cannot interleave with an in-flight resolve: either the override is set
BEFORE any resolve begins, or a catalog is already installed and the override
is genuinely too late. A brief read guard alone did NOT close this — a
`get()` that had already resolved from the environment (outside the lock)
but not yet installed would still ignore the override, so the user passed
`--language de`, got their system locale, and nothing warned.

## `set_locale` — last caller wins, and why resolution stays off `STRINGS`

Hold `INIT_LOCK` across resolve-then-install so the LAST CALLER wins. Two
concurrent `set_locale`s used to resolve outside any shared lock and race to
the write lock, so whichever resolve happened to FINISH last installed last —
last-writer-wins, not last-caller-wins, and a GUI that fired a `de` switch
after an `fr` one could settle on French. With `INIT_LOCK` the caller that
acquires it later also resolves and installs later.

Resolution still runs off the `STRINGS` write lock: it stats up to four
directories and may read and parse a file off disk, and every `get` in the
process would block on that if it happened under the writer. `INIT_LOCK` is a
separate mutex the read fast path never touches, so serializing switches here
does not stall readers.
