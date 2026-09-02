#!/usr/bin/env bash
# Regenerate src/error_codes.rs from libfreemkv's error-code constants.
#
# This crate deliberately does NOT depend on libfreemkv. libfreemkv pulls in
# freemkv-unlock, which is private and never published, so a dependency here
# would make freemkv-i18n unbuildable in its own CI (which checks out no
# siblings) — and it would invert the layering besides: libfreemkv emits numeric
# codes, this crate renders them.
#
# So the code list is checked in, and two tests keep it honest:
#   * libfreemkv_error_codes_all_have_english_strings — runs everywhere,
#     including CI, and fails if any listed code has no English string.
#   * libfreemkv_code_list_has_not_drifted — runs wherever a sibling libfreemkv
#     checkout exists, and fails if this file no longer matches error.rs.
#
# Run this after adding an error code to libfreemkv, then write an English
# string for the new code in locales/en.json AND a line in every other locale
# (the parity test requires all 29 to agree).
#
# Usage: ci/sync-error-codes.sh [path/to/libfreemkv/src/error.rs]

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_file="${1:-$here/../libfreemkv/src/error.rs}"

if [[ ! -f "$source_file" ]]; then
    echo "error: $source_file not found." >&2
    echo "Pass the path to libfreemkv/src/error.rs as the first argument." >&2
    exit 1
fi

out="$here/src/error_codes.rs"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

{
    cat <<'HEADER'
// GENERATED FILE — do not edit by hand. Regenerate: ci/sync-error-codes.sh
// A checked-in copy of libfreemkv's error codes (this crate cannot depend on
// libfreemkv). See docs/error-codes.md for why, plus the drift/parity tests.
pub const LIBFREEMKV_ERROR_CODES: &[(u32, &str)] = &[
HEADER

    grep -oE '^[[:space:]]*pub const E_[A-Z0-9_]+: u16 = [0-9]+;' "$source_file" |
        sed -E 's/^[[:space:]]*pub const (E_[A-Z0-9_]+): u16 = ([0-9]+);/\2 \1/' |
        sort -n -k1,1 |
        awk '{ printf "    (%s, \"%s\"),\n", $1, $2 }'

    echo '];'
} >"$tmp"

mv "$tmp" "$out"
trap - EXIT

count=$(grep -c '^    (' "$out")
echo "wrote $out ($count codes from $source_file)"
