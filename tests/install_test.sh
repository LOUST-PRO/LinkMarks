#!/usr/bin/env bash
# install_test.sh — bash-side smoke for scripts/install.sh.
#
# This is intentionally NOT a `cargo test`. Bash, not Rust.
#
# Validates:
#   1. Syntax: `bash -n scripts/install.sh` exits 0.
#   2. Help:   `bash scripts/install.sh --help` exits 0 and prints
#              the expected banner.
#   3. Dry-run: `scripts/install.sh --dry-run --prefix /tmp/lm-dryrun-test`
#              exits 0 and creates no files.
#
# Exit codes:
#   0  all checks passed
#   1  at least one check failed

set -euo pipefail

# Resolve repo root from this script's location.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INSTALL_SH="$REPO_ROOT/scripts/install.sh"

fail() { printf 'install_test.sh: FAIL: %s\n' "$*" >&2; exit 1; }
pass() { printf 'install_test.sh: pass: %s\n' "$*"; }

[[ -f "$INSTALL_SH" ]] || fail "missing $INSTALL_SH"

echo "=== check 1/3: bash -n syntax ==="
if ! bash -n "$INSTALL_SH"; then
  fail "bash -n failed for $INSTALL_SH"
fi
pass "syntax OK"

echo "=== check 2/3: --help ==="
HELP_OUT="$(bash "$INSTALL_SH" --help 2>&1)" || fail "--help exit non-zero"
echo "$HELP_OUT" | grep -q "install.sh" \
  || fail "--help banner missing 'install.sh'"
echo "$HELP_OUT" | grep -q "LinkMarks CLI binary" \
  || fail "--help banner missing 'LinkMarks CLI binary'"
pass "--help OK"

echo "=== check 3/3: --dry-run ==="
DRY_PREFIX="/tmp/lm-dryrun-test-$$"
mkdir -p "$DRY_PREFIX"
DRY_OUT="$(bash "$INSTALL_SH" --dry-run --prefix "$DRY_PREFIX" 2>&1)" || fail "--dry-run exit non-zero"
echo "$DRY_OUT" | grep -q "dry-run: nothing was changed" \
  || fail "--dry-run banner missing"
if [[ -e "$DRY_PREFIX/bin/linkmarks" ]]; then
  fail "--dry-run created $DRY_PREFIX/bin/linkmarks (should not)"
fi
# Clean up the (intentionally empty) prefix dir.
rmdir "$DRY_PREFIX" 2>/dev/null || true
pass "--dry-run OK"

echo "install_test.sh: all checks passed."
exit 0
