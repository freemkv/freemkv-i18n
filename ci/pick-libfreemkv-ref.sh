#!/usr/bin/env bash
# ST-I0 (stop-design-v5.md §5.7): the same branch-pick pattern as the `freemkv-key-drift`
# job's "Pick freemkv branch" step (ci.yml, `Fetch freemkv src/`), but for libfreemkv and
# with the J-5.5-4 fallback: a PR's base branch instead of a bare `dev`, so a PR into qa
# compares against qa. Writes `ref=<branch>` to $GITHUB_OUTPUT (or stdout if unset, for
# the standalone test below).
#
# `git ls-remote --exit-code` exits 2 for "no matching refs" specifically — any OTHER
# nonzero exit (network down, rate-limited, ...) is a real failure, not "no such branch",
# and must fail the job loudly rather than silently falling back to dev/the base branch.
set -uo pipefail

want="${GITHUB_HEAD_REF:-$GITHUB_REF_NAME}"
status=0
git ls-remote --exit-code --heads https://github.com/freemkv/libfreemkv.git "refs/heads/$want" \
  >/dev/null 2>&1 || status=$?

case "$status" in
  0) ref="$want" ;;
  2) ref="${GITHUB_BASE_REF:-dev}" ;;
  *)
    echo "::error::git ls-remote failed (exit $status) checking libfreemkv for a '$want' branch" >&2
    exit "$status"
    ;;
esac

echo "libfreemkv ref: $ref" >&2
echo "ref=$ref" >> "${GITHUB_OUTPUT:-/dev/stdout}"
