#!/usr/bin/env bash
# scripts/start-localnet.sh
#
# Sets up and starts a Soroban local sandbox for development and testing.
# Uses the Stellar CLI's sandbox mode to run a local network that does not
# require testnet funds or an internet connection.
#
# Usage:
#   bash scripts/start-localnet.sh
#
# Requirements:
#   - stellar CLI installed  (see docs/soroban-cli-setup.md)
#   - Rust + wasm32-unknown-unknown target  (see rust-toolchain.toml)
#
# See docs/local-development.md for a full walkthrough.

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()  { echo -e "\033[0;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[OK]\033[0m    $*"; }
warn()  { echo -e "\033[0;33m[WARN]\033[0m  $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Locate repo root
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$REPO_ROOT/contracts"

# ---------------------------------------------------------------------------
# Verify tools
# ---------------------------------------------------------------------------
if ! command -v stellar &>/dev/null; then
  error "stellar CLI not found. See docs/soroban-cli-setup.md for installation instructions."
fi
info "Stellar CLI: $(stellar --version)"

if ! command -v cargo &>/dev/null; then
  error "cargo not found. Install Rust: https://rustup.rs"
fi
info "Cargo: $(cargo --version)"

# ---------------------------------------------------------------------------
# Build contracts first
# ---------------------------------------------------------------------------
info "Building all contracts for wasm32-unknown-unknown..."
cd "$CONTRACTS_DIR"
cargo build --target wasm32-unknown-unknown --release --quiet
ok "Contracts built successfully."

# ---------------------------------------------------------------------------
# Configure local sandbox identity (creates if not already present)
# ---------------------------------------------------------------------------
LOCAL_IDENTITY="local-admin"

if ! stellar keys address "$LOCAL_IDENTITY" &>/dev/null 2>&1; then
  info "Creating local sandbox identity '$LOCAL_IDENTITY'..."
  stellar keys generate "$LOCAL_IDENTITY" --no-fund
  ok "Identity '$LOCAL_IDENTITY' created."
else
  ok "Identity '$LOCAL_IDENTITY' already exists."
fi

LOCAL_ADDRESS=$(stellar keys address "$LOCAL_IDENTITY")
info "  Address: $LOCAL_ADDRESS"

# ---------------------------------------------------------------------------
# Print next steps
# ---------------------------------------------------------------------------
echo ""
ok "Local sandbox is ready!"
echo ""
info "Run tests without testnet funds:"
info "  cd contracts/ && cargo test"
echo ""
info "Build WASM artifacts:"
info "  cd contracts/ && cargo build --target wasm32-unknown-unknown --release"
echo ""
info "Deploy a contract to the sandbox (when stellar CLI sandbox is available):"
info "  stellar contract deploy \\"
info "    --wasm target/wasm32-unknown-unknown/release/rewards.wasm \\"
info "    --source $LOCAL_IDENTITY \\"
info "    --network local"
echo ""
info "Invoke a deployed contract function:"
info "  stellar contract invoke \\"
info "    --id <CONTRACT_ID> \\"
info "    --source $LOCAL_IDENTITY \\"
info "    --network local \\"
info "    -- distribute"
echo ""
info "See docs/local-development.md for a full walkthrough."
