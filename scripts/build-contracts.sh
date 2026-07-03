#!/usr/bin/env bash
# scripts/build-contracts.sh
#
# Builds all Soroban contracts in the workspace and outputs WASM artifacts.
# Artifacts are written to:
#   contracts/target/wasm32-unknown-unknown/release/<contract_name>.wasm
#
# Usage:
#   bash scripts/build-contracts.sh
#
# Requirements:
#   - Rust with wasm32-unknown-unknown target (see rust-toolchain.toml)
#   - stellar CLI optional — only needed for deploy steps
#
# The script exits with a non-zero status if any contract fails to build.

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()  { echo -e "\033[0;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[OK]\033[0m    $*"; }
warn()  { echo -e "\033[0;33m[WARN]\033[0m  $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Locate contracts directory
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$REPO_ROOT/contracts"

[[ -d "$CONTRACTS_DIR" ]] || error "contracts/ directory not found at $CONTRACTS_DIR"

# ---------------------------------------------------------------------------
# Verify Rust toolchain
# ---------------------------------------------------------------------------
if ! command -v cargo &>/dev/null; then
  error "cargo not found. Install Rust: https://rustup.rs"
fi
info "Rust toolchain: $(rustc --version)"
info "Cargo: $(cargo --version)"

# ---------------------------------------------------------------------------
# Verify wasm32 target is installed
# ---------------------------------------------------------------------------
if ! rustup target list --installed 2>/dev/null | grep -q "wasm32-unknown-unknown"; then
  warn "wasm32-unknown-unknown target not installed. Installing..."
  rustup target add wasm32-unknown-unknown
fi
ok "Target wasm32-unknown-unknown is available."

# ---------------------------------------------------------------------------
# Build all contracts
# ---------------------------------------------------------------------------
info "Building all contracts in $CONTRACTS_DIR ..."
cd "$CONTRACTS_DIR"

cargo build \
  --target wasm32-unknown-unknown \
  --release

BUILD_EXIT=$?

if [[ $BUILD_EXIT -ne 0 ]]; then
  error "Build failed with exit code $BUILD_EXIT"
fi

# ---------------------------------------------------------------------------
# Show output artifacts
# ---------------------------------------------------------------------------
ARTIFACTS_DIR="$CONTRACTS_DIR/target/wasm32-unknown-unknown/release"

echo ""
ok "Build succeeded! WASM artifacts:"
find "$ARTIFACTS_DIR" -maxdepth 1 -name "*.wasm" | sort | while read -r wasm; do
  size=$(du -sh "$wasm" 2>/dev/null | cut -f1)
  info "  $size  $wasm"
done

echo ""
info "Deploy a contract to testnet:"
info "  stellar contract deploy \\"
info "    --wasm $ARTIFACTS_DIR/<contract>.wasm \\"
info "    --source <KEY_ALIAS> \\"
info "    --network testnet"
echo ""
info "See docs/local-development.md for a full build-deploy-invoke walkthrough."
