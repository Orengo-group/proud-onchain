#!/usr/bin/env bash
# scripts/deploy-achievement-certificate.sh
#
# Deploys the achievement_certificate contract to a Stellar network and
# initializes it with the provided admin address.
#
# Usage:
#   bash scripts/deploy-achievement-certificate.sh <NETWORK> <ADMIN_ADDRESS>
#
# Arguments:
#   NETWORK       - Stellar network to deploy to (e.g. "testnet", "standalone")
#   ADMIN_ADDRESS - Stellar address to set as contract admin
#
# Examples:
#   bash scripts/deploy-achievement-certificate.sh testnet GABC...
#   bash scripts/deploy-achievement-certificate.sh standalone GABC...
#
# Requirements:
#   - stellar CLI installed (see docs/soroban-cli-setup.md)
#   - Contracts must be built first: bash scripts/build-contracts.sh
#   - The source account must be funded and have the stellar CLI key configured

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()  { echo -e "\033[0;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[OK]\033[0m    $*"; }
warn()  { echo -e "\033[0;33m[WARN]\033[0m  $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*" >&2; exit 1; }

usage() {
  echo "Usage: $0 <NETWORK> <ADMIN_ADDRESS>"
  echo ""
  echo "Arguments:"
  echo "  NETWORK       Stellar network (e.g. testnet, standalone)"
  echo "  ADMIN_ADDRESS Stellar address to set as contract admin"
  echo ""
  echo "Example:"
  echo "  $0 testnet GABC1234567890ABCDEFGHIJKLMNOPQRSTUV"
  exit 1
}

# ---------------------------------------------------------------------------
# Validate arguments
# ---------------------------------------------------------------------------
if [[ $# -ne 2 ]]; then
  usage
fi

NETWORK="$1"
ADMIN_ADDRESS="$2"

if [[ -z "$NETWORK" || -z "$ADMIN_ADDRESS" ]]; then
  error "Both NETWORK and ADMIN_ADDRESS are required."
fi

# ---------------------------------------------------------------------------
# Locate repo root and WASM artifact
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$REPO_ROOT/contracts"
WASM_PATH="$CONTRACTS_DIR/target/wasm32-unknown-unknown/release/achievement_certificate.wasm"

if [[ ! -f "$WASM_PATH" ]]; then
  error "WASM artifact not found at $WASM_PATH. Run 'bash scripts/build-contracts.sh' first."
fi

# ---------------------------------------------------------------------------
# Verify stellar CLI is installed
# ---------------------------------------------------------------------------
if ! command -v stellar &>/dev/null; then
  error "stellar CLI not found. See docs/soroban-cli-setup.md for installation."
fi

info "Stellar CLI: $(stellar --version)"
info "Network: $NETWORK"
info "Admin address: $ADMIN_ADDRESS"

# ---------------------------------------------------------------------------
# Deploy the contract
# ---------------------------------------------------------------------------
info "Deploying achievement_certificate contract..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --source "$ADMIN_ADDRESS" \
  --network "$NETWORK" \
  2>&1)

# Extract contract ID from output (strip any surrounding whitespace)
CONTRACT_ID=$(echo "$CONTRACT_ID" | tr -d '[:space:]')

if [[ -z "$CONTRACT_ID" ]]; then
  error "Failed to deploy contract. Check your network configuration and source account."
fi

ok "Contract deployed with ID: $CONTRACT_ID"

# ---------------------------------------------------------------------------
# Initialize the contract
# ---------------------------------------------------------------------------
info "Initializing contract with admin: $ADMIN_ADDRESS"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$ADMIN_ADDRESS" \
  --network "$NETWORK" \
  -- initialize \
  --admin "$ADMIN_ADDRESS"

ok "Contract initialized successfully."

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
ok "Achievement Certificate Contract deployed and initialized!"
info "  Contract ID : $CONTRACT_ID"
info "  Network     : $NETWORK"
info "  Admin       : $ADMIN_ADDRESS"
echo ""
info "Next steps:"
info "  - Issue certificates: bash scripts/issue-certificate.sh $CONTRACT_ID $NETWORK"
info "  - View contract docs: docs/contracts/achievement-certificate.md"
info "  - Store the contract ID for future operations."
