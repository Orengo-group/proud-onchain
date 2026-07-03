#!/usr/bin/env bash
# scripts/format.sh
#
# Runs cargo fmt across the entire Soroban contract workspace.
#
# Usage:
#   bash scripts/format.sh           # auto-format all files
#   bash scripts/format.sh --check   # check formatting without modifying files (used in CI)
#
# The script exits with a non-zero status if formatting fails (--check mode)
# or if cargo fmt encounters an error.

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()  { echo -e "\033[0;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[OK]\033[0m    $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
CHECK_MODE=false
for arg in "$@"; do
  case "$arg" in
    --check) CHECK_MODE=true ;;
    *) error "Unknown argument: $arg. Usage: $0 [--check]" ;;
  esac
done

# ---------------------------------------------------------------------------
# Locate contracts directory
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$REPO_ROOT/contracts"

[[ -d "$CONTRACTS_DIR" ]] || error "contracts/ directory not found at $CONTRACTS_DIR"

# ---------------------------------------------------------------------------
# Verify cargo and rustfmt
# ---------------------------------------------------------------------------
if ! command -v cargo &>/dev/null; then
  error "cargo not found. Install Rust: https://rustup.rs"
fi

if ! cargo fmt --version &>/dev/null 2>&1; then
  error "rustfmt not found. Install it: rustup component add rustfmt"
fi

info "rustfmt: $(cargo fmt --version 2>/dev/null || rustfmt --version)"

# ---------------------------------------------------------------------------
# Run formatting
# ---------------------------------------------------------------------------
cd "$CONTRACTS_DIR"

if $CHECK_MODE; then
  info "Checking formatting across workspace (--check mode)..."
  if cargo fmt --all -- --check; then
    ok "All files are correctly formatted."
  else
    error "Formatting check failed. Run 'bash scripts/format.sh' to auto-format."
  fi
else
  info "Auto-formatting all contracts in workspace..."
  cargo fmt --all
  ok "Formatting complete."
fi
