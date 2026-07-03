#!/usr/bin/env bash
# scripts/configure-testnet.sh
#
# Reads configs/testnet.toml (or testnet.example.toml as a fallback) and
# registers the Stellar testnet network + identity with the Soroban CLI.
#
# Usage:
#   cp configs/testnet.example.toml configs/testnet.toml
#   # Edit configs/testnet.toml with your key alias
#   bash scripts/configure-testnet.sh
#
# Requirements:
#   - stellar CLI installed (see docs/soroban-cli-setup.md)
#   - Rust + wasm32-unknown-unknown target

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()  { echo -e "\033[0;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[OK]\033[0m    $*"; }
warn()  { echo -e "\033[0;33m[WARN]\033[0m  $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Locate config file
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="$REPO_ROOT/configs/testnet.toml"
EXAMPLE_FILE="$REPO_ROOT/configs/testnet.example.toml"

if [[ ! -f "$CONFIG_FILE" ]]; then
  warn "configs/testnet.toml not found — using example file."
  warn "Copy it and fill in your values: cp configs/testnet.example.toml configs/testnet.toml"
  CONFIG_FILE="$EXAMPLE_FILE"
fi

[[ -f "$CONFIG_FILE" ]] || error "No config file found at $CONFIG_FILE"

# ---------------------------------------------------------------------------
# Parse TOML values (simple grep — no extra dependencies required)
# ---------------------------------------------------------------------------
parse_toml() {
  local key="$1"
  grep -E "^\s*${key}\s*=" "$CONFIG_FILE" | head -1 | sed 's/.*=\s*"\(.*\)".*/\1/'
}

RPC_URL=$(parse_toml "rpc_url")
PASSPHRASE=$(parse_toml "network_passphrase")
NETWORK_NAME=$(parse_toml "network_name")
KEY_ALIAS=$(parse_toml "key_alias")

[[ -n "$RPC_URL" ]]      || error "rpc_url not found in $CONFIG_FILE"
[[ -n "$PASSPHRASE" ]]   || error "network_passphrase not found in $CONFIG_FILE"
[[ -n "$NETWORK_NAME" ]] || error "network_name not found in $CONFIG_FILE"
[[ -n "$KEY_ALIAS" ]]    || error "key_alias not found in $CONFIG_FILE"

# ---------------------------------------------------------------------------
# Check stellar CLI is installed
# ---------------------------------------------------------------------------
if ! command -v stellar &>/dev/null; then
  error "stellar CLI not found. See docs/soroban-cli-setup.md for installation."
fi

info "Using Stellar CLI: $(stellar --version)"

# ---------------------------------------------------------------------------
# Register the testnet network
# ---------------------------------------------------------------------------
info "Registering network '$NETWORK_NAME'..."
stellar network add \
  --global "$NETWORK_NAME" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" 2>/dev/null || true

ok "Network '$NETWORK_NAME' registered (or already exists)."

# ---------------------------------------------------------------------------
# Verify the key alias exists; guide user if not
# ---------------------------------------------------------------------------
if stellar keys address "$KEY_ALIAS" &>/dev/null; then
  PUBLIC_KEY=$(stellar keys address "$KEY_ALIAS")
  ok "Identity '$KEY_ALIAS' found: $PUBLIC_KEY"
else
  warn "Key alias '$KEY_ALIAS' not found. Generating a new testnet keypair..."
  stellar keys generate --global "$KEY_ALIAS" --network "$NETWORK_NAME"
  PUBLIC_KEY=$(stellar keys address "$KEY_ALIAS")
  ok "Generated key '$KEY_ALIAS': $PUBLIC_KEY"
  info "Funding account via Friendbot..."
  stellar keys fund "$KEY_ALIAS" --network "$NETWORK_NAME" || warn "Friendbot funding failed — fund manually at https://friendbot.stellar.org"
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo ""
ok "Testnet configuration complete!"
info "  Network  : $NETWORK_NAME"
info "  RPC URL  : $RPC_URL"
info "  Identity : $KEY_ALIAS ($PUBLIC_KEY)"
echo ""
info "Next steps:"
info "  cd contracts/ && cargo build --target wasm32-unknown-unknown --release"
info "  stellar contract deploy --wasm target/wasm32-unknown-unknown/release/rewards.wasm --network $NETWORK_NAME --source $KEY_ALIAS"
