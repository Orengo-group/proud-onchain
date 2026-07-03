#!/usr/bin/env bash
# scripts/lint.sh
#
# Runs cargo clippy across the entire Soroban contract workspace.
# Treats warnings as errors to enforce a clean lint baseline.
#
# Usage:
#   bash scripts/lint.sh
#
# The script exits with a non-zero status if any clippy warning or error
# is found, ensuring CI catches regressions.

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()  { echo -e "\033[0;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[OK]\033[0m    $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Locate contracts directory
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$REPO_ROOT/contracts"

[[ -d "$CONTRACTS_DIR" ]] || error "contracts/ directory not found at $CONTRACTS_DIR"

# ---------------------------------------------------------------------------
# Verify cargo and clippy
# ---------------------------------------------------------------------------
if ! command -v cargo &>/dev/null; then
  error "cargo not found. Install Rust: https://rustup.rs"
fi

if ! cargo clippy --version &>/dev/null 2>&1; then
  error "clippy not found. Install it: rustup component add clippy"
fi

info "clippy: $(cargo clippy --version 2>/dev/null)"

# ---------------------------------------------------------------------------
# Run clippy across workspace
# ---------------------------------------------------------------------------
cd "$CONTRACTS_DIR"

info "Running clippy across all contracts..."

cargo clippy \
  --all-targets \
  --all-features \
  -- \
  -D warnings

CLIPPY_EXIT=$?

if [[ $CLIPPY_EXIT -ne 0 ]]; then
  error "Linting failed. Fix the clippy warnings listed above."
fi

ok "Linting passed — no warnings found."
