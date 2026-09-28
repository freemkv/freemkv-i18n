#!/usr/bin/env bash
# ST-I0 (stop-design-v5.md §5.7): the same branch-pick pattern as the `freemkv-key-drift`
# job's "Pick freemkv branch" step (ci.yml, `Fetch freemkv src/`), but for libfreemkv and
# with the J-5.5-4 fallback: a PR's base branch instead of a bare `dev`, so a PR into qa
# compares against qa. Writes `ref=<branch>` to $GITHUB_OUTPUT (or stdout if unset, for
# the standalone test below).
set -euo pipefail

want="${GITHUB_HEAD_REF:-$GITHUB_REF_NAME}"
if git ls-remote --exit-code --heads https://github.com/freemkv/libfreemkv.git "refs/heads/$want" >/dev/null 2>&1; then
  ref="$want"
else
  ref="${GITHUB_BASE_REF:-dev}"
fi
echo "libfreemkv ref: $ref" >&2
echo "ref=$ref" >> "${GITHUB_OUTPUT:-/dev/stdout}"
