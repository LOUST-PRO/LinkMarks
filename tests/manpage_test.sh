#!/usr/bin/env bash
# manpage_test.sh — bash-side smoke for docs/man/linkmarks.1.
#
# Validates that the manpage renders cleanly with `groff -man` on this
# host. If groff is not installed, the script logs a skip and exits 0
# (CI installs groff explicitly, see .github/workflows/ci-smoke.yml).
#
# Exit codes:
#   0  groff rendered cleanly (or groff missing — skip)
#   1  groff returned non-zero (broken manpage)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MANPAGE="$REPO_ROOT/docs/man/linkmarks.1"

fail() { printf 'manpage_test.sh: FAIL: %s\n' "$*" >&2; exit 1; }
pass() { printf 'manpage_test.sh: pass: %s\n' "$*"; }
skip() { printf 'manpage_test.sh: SKIP: %s\n' "$*"; exit 0; }

[[ -f "$MANPAGE" ]] || fail "missing $MANPAGE"

if ! command -v groff >/dev/null 2>&1; then
  skip "groff not installed; CI installs it via apt-get"
fi

echo "=== check: groff -man -Tutf8 render ==="
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

if ! groff -man -Tutf8 "$MANPAGE" > "$TMP"; then
  fail "groff exited non-zero for $MANPAGE"
fi

LINES="$(wc -l < "$TMP")"
if [[ "$LINES" -lt 50 ]]; then
  fail "render produced only $LINES lines — likely a broken manpage"
fi

# Spot-check: the NAME and SYNOPSIS sections must appear in the output.
grep -q "linkmarks" "$TMP" || fail "render does not mention 'linkmarks'"
grep -q "SYNOPSIS" "$TMP" || fail "render missing SYNOPSIS section"

pass "groff rendered $LINES lines OK"

echo "manpage_test.sh: all checks passed."
exit 0
