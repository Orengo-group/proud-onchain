# PROUD Smart Contracts

This workspace contains all Stellar Soroban smart contracts for the PROUD on-chain layer.

## Workspace layout

```text
contracts/
├── Cargo.toml                    # Workspace manifest
├── rewards/                      # Rule-based reward distribution to students
├── identity_registry/            # NFC UID ↔ student wallet mapping
├── campaign_vault/               # Sponsor-funded reward pool per campaign
└── achievement_certificate/      # Verifiable on-chain achievement records
```

## Contract responsibilities

| Contract | Purpose |
|---|---|
| `rewards` | Evaluates eligibility criteria (attendance %, grade threshold) and distributes token rewards |
| `identity_registry` | Registers NFC UIDs to student addresses; admin-controlled |
| `campaign_vault` | Accepts sponsor deposits per campaign and releases funds on trigger |
| `achievement_certificate` | Issues and verifies on-chain academic certificates |

## Building

```bash
cd contracts/
cargo build --target wasm32-unknown-unknown --release
```

## Testing

```bash
cd contracts/
cargo test
```

## Adding a new contract

1. Create a new folder under `contracts/`:

```bash
mkdir -p contracts/<name>/src
```

2. Add a `Cargo.toml` (copy from an existing contract and rename).
3. Add `contracts/<name>` to the `members` list in `contracts/Cargo.toml`.
4. Implement your contract in `contracts/<name>/src/lib.rs`.

## Deploying to testnet

See [docs/soroban-cli-setup.md](../docs/soroban-cli-setup.md) for CLI setup, then:

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/<name>.wasm \
  --network testnet \
  --source my-key
```
