#!/usr/bin/env bash
# install.sh — install LinkMarks to a prefix directory.
#
# Default behavior (Linux + macOS):
#   - Build via `cargo install --path crates/linkmarks-cli --locked`
#     against $PREFIX/bin/linkmarks (PREFIX defaults to $HOME/.local).
#   - Validate the binary with `linkmarks --version` and
#     `linkmarks --help` (stderr captured).
#   - Print next-steps that point to the manpage.
#
# Flags:
#   --prefix PATH         Install root. Default: $HOME/.local
#                         (binaries land at $PREFIX/bin/linkmarks).
#   --binary-from PATH    Skip cargo build; copy this prebuilt binary
#                         to $PREFIX/bin/linkmarks instead.
#   --force               Overwrite an existing $PREFIX/bin/linkmarks.
#   --dry-run             Print what would happen; make no changes.
#   -h, --help            Show this help and exit.
#
# Idempotent: re-running without --force keeps the existing install
# unless the user opts in.

set -euo pipefail

print_help() {
  cat <<'EOF'
install.sh — install the LinkMarks CLI binary.

Usage:
  scripts/install.sh [OPTIONS]

Options:
  --prefix PATH      Install root (binaries go to <PREFIX>/bin).
                     Default: $HOME/.local
  --binary-from PATH Copy a prebuilt binary from PATH instead of
                     running `cargo install`.
  --force            Overwrite an existing <PREFIX>/bin/linkmarks.
  --dry-run          Print the plan; do not write or run cargo install.
  -h, --help         Show this help and exit.

Environment:
  CARGO              Override cargo binary (default: cargo).
  SKIP_BUILD         If set, equivalent to --binary-from <nothing>,
                     but the script exits 2 (refusing an empty install).

Exit codes:
  0  success (binary installed or already present)
  1  generic failure
  2  invalid arguments
  3  cargo not found and --binary-from not supplied
  4  installed binary failed smoke validation
EOF
}

# Defaults
PREFIX="${HOME}/.local"
BINARY_FROM=""
FORCE=0
DRY_RUN=0
CARGO_BIN="${CARGO:-cargo}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      [[ $# -ge 2 ]] || { echo "install.sh: --prefix requires PATH" >&2; exit 2; }
      PREFIX="$2"
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#*=}"
      shift
      ;;
    --binary-from)
      [[ $# -ge 2 ]] || { echo "install.sh: --binary-from requires PATH" >&2; exit 2; }
      BINARY_FROM="$2"
      shift 2
      ;;
    --binary-from=*)
      BINARY_FROM="${1#*=}"
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      print_help
      exit 0
      ;;
    *)
      echo "install.sh: unknown argument: $1" >&2
      echo "Run 'scripts/install.sh --help' for usage." >&2
      exit 2
      ;;
  esac
done

# Resolve absolute paths where it matters.
case "$PREFIX" in
  /*) ;;
  *) PREFIX="${PWD}/${PREFIX}" ;;
esac

TARGET_BIN="${PREFIX%/}/bin/linkmarks"

log() { printf 'install.sh: %s\n' "$*" >&2; }

# Idempotency check: skip early if the binary is already installed.
if [[ -x "$TARGET_BIN" && $FORCE -eq 0 && $DRY_RUN -eq 0 ]]; then
  log "linkmarks already installed at $TARGET_BIN — skipping (use --force to overwrite)."
  log "Run 'linkmarks --version' to verify."
  exit 0
fi

# Plan ------------------------------------------------------------------
plan=()
if [[ -n "$BINARY_FROM" ]]; then
  if [[ ! -f "$BINARY_FROM" ]]; then
    echo "install.sh: --binary-from file not found: $BINARY_FROM" >&2
    exit 1
  fi
  plan+=("copy $BINARY_FROM -> $TARGET_BIN")
else
  plan+=("cargo install --path crates/linkmarks-cli --locked --root $PREFIX")
fi
plan+=("create $PREFIX/bin if missing")
plan+=("smoke: $TARGET_BIN --version")
plan+=("smoke: $TARGET_BIN --help")

log "plan:"
for line in "${plan[@]}"; do
  log "  - $line"
done

if [[ $DRY_RUN -eq 1 ]]; then
  log "dry-run: nothing was changed."
  exit 0
fi

# Action ----------------------------------------------------------------
mkdir -p "$PREFIX/bin"

if [[ -n "$BINARY_FROM" ]]; then
  log "copying $BINARY_FROM -> $TARGET_BIN"
  install -m 0755 "$BINARY_FROM" "$TARGET_BIN"
else
  if ! command -v "$CARGO_BIN" >/dev/null 2>&1; then
    echo "install.sh: cargo not found in PATH (CARGO=$CARGO_BIN)." >&2
    echo "install.sh: re-run with --binary-from <path> to install a prebuilt binary." >&2
    exit 3
  fi
  # Find the workspace root from this script's location so the script
  # works regardless of where the user invokes it from.
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
  log "running: $CARGO_BIN install --path $WORKSPACE_ROOT/crates/linkmarks-cli --locked --root $PREFIX"
  ( cd "$WORKSPACE_ROOT" && "$CARGO_BIN" install \
      --path crates/linkmarks-cli \
      --locked \
      --root "$PREFIX" )
fi

# Validate --------------------------------------------------------------
if [[ ! -x "$TARGET_BIN" ]]; then
  echo "install.sh: $TARGET_BIN is missing or not executable after install" >&2
  exit 4
fi

log "validating $TARGET_BIN --version"
if ! "$TARGET_BIN" --version >/tmp/lm-version.out 2>/tmp/lm-version.err; then
  echo "install.sh: --version failed:" >&2
  cat /tmp/lm-version.err >&2 || true
  exit 4
fi

log "validating $TARGET_BIN --help"
if ! "$TARGET_BIN" --help >/tmp/lm-help.out 2>/tmp/lm-help.err; then
  echo "install.sh: --help failed:" >&2
  cat /tmp/lm-help.err >&2 || true
  exit 4
fi

VERSION_LINE="$(head -n1 /tmp/lm-version.out || true)"
log "ok: $VERSION_LINE"
log "installed at: $TARGET_BIN"

# PATH advisory ---------------------------------------------------------
case ":${PATH}:" in
  *":${PREFIX}/bin:"*)
    log "$PREFIX/bin is already on PATH."
    ;;
  *)
    log "NOTE: $PREFIX/bin is not on PATH. Add this to your shell rc:"
    log "  export PATH=\"$PREFIX/bin:\$PATH\""
    ;;
esac

# Next-steps ------------------------------------------------------------
cat <<EOF

install.sh: next steps

  # Initialize the local store + config (XDG paths).
  linkmarks init

  # Import from your Chromium-family browser (Chrome, Brave, Edge, Arc, ...).
  linkmarks import --source=chrome --path ~/.config/google-chrome/Default/Bookmarks

  # List bookmarks deterministically.
  linkmarks list --source=store --format=table

  # Export to Netscape HTML for sharing.
  linkmarks export --format=netscape --output ./bookmarks.html

  # Dedupe (dry-run; pass --apply to commit).
  linkmarks dedupe

  # Browse interactively in the terminal.
  linkmarks tui

  # Full documentation:
  man -l docs/man/linkmarks.1
EOF

exit 0
