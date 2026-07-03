# Local Development Guide

This guide explains how to build, test, and interact with PROUD Soroban smart contracts **without testnet funds or an internet connection** using the Stellar CLI's local tooling.

---

## Prerequisites

| Tool | How to install |
|------|---------------|
| Rust 1.81.0 | Managed automatically via `rust-toolchain.toml` |
| `wasm32-unknown-unknown` target | Included in `rust-toolchain.toml` |
| Stellar CLI ≥ 21.x | See [docs/soroban-cli-setup.md](./soroban-cli-setup.md) |

---

## Quickstart

```bash
# 1. Clone the repo (if you haven't already)
git clone https://github.com/Orengo-group/proud-onchain.git
cd proud-onchain

# 2. Start the local sandbox and build contracts
bash scripts/start-localnet.sh

# 3. Run all contract tests (no network needed)
cd contracts/
cargo test
```

---

## Localnet vs Testnet

| | Localnet | Testnet |
|---|---|---|
| Needs internet | No | Yes |
| Needs funded account | No | Yes (Friendbot) |
| State persists across runs | No (in-memory) | Yes |
| Use case | Development & unit tests | Integration testing & pre-deploy |

---

## Build Contracts

From the repo root:

```bash
bash scripts/build-contracts.sh
```

Or manually from the `contracts/` directory:

```bash
cd contracts/
cargo build --target wasm32-unknown-unknown --release
```

WASM artifacts are written to:
```
contracts/target/wasm32-unknown-unknown/release/<contract_name>.wasm
```

---

## Run Tests

Unit tests are co-located in each contract's `src/lib.rs` file and use `soroban-sdk`'s test harness — no live network required.

```bash
cd contracts/
cargo test
```

Run tests for a specific contract:

```bash
cargo test -p rewards
cargo test -p identity_registry
cargo test -p campaign_vault
cargo test -p achievement_certificate
```

Run with output visible:

```bash
cargo test -- --nocapture
```

---

## Local Sandbox Usage

The `stellar contract` commands support a `--network` flag. For local work, you can use the in-process environment via `cargo test`, or configure a local network with the Stellar CLI:

### 1. Create a local identity

```bash
stellar keys generate local-admin --no-fund
stellar keys address local-admin
# G... (public key printed)
```

### 2. Deploy a contract locally

```bash
stellar contract deploy \
  --wasm contracts/target/wasm32-unknown-unknown/release/rewards.wasm \
  --source local-admin \
  --network local
# Output: CONTRACT_ID
```

### 3. Invoke a contract function

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source local-admin \
  --network local \
  -- distribute
```

---

## Example Workflow: Rewards Contract

```bash
# Build
bash scripts/build-contracts.sh

# Run unit tests
cd contracts/
cargo test -p rewards -- --nocapture

# Deploy (requires stellar CLI sandbox)
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/rewards.wasm \
  --source local-admin \
  --network local

# Invoke placeholder function
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source local-admin \
  --network local \
  -- distribute
```

---

## Formatting and Linting

```bash
# Check formatting (does not modify files)
bash scripts/format.sh --check

# Auto-format all contracts
bash scripts/format.sh

# Run linter
bash scripts/lint.sh
```

---

## Environment Variables

No secrets are required for local development. When working with testnet, copy the example config:

```bash
cp configs/testnet.example.toml configs/testnet.toml
# Edit configs/testnet.toml — never commit this file
```

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `stellar: command not found` | See [docs/soroban-cli-setup.md](./soroban-cli-setup.md) for install instructions |
| `wasm32-unknown-unknown` target missing | Run `rustup target add wasm32-unknown-unknown` |
| `error[E0463]: can't find crate for std` | The wasm32 target is missing — see above |
| Cargo version mismatch | The `rust-toolchain.toml` at repo root pins the correct version automatically |
| Contract test fails with `HostError` | Check the test setup in `src/lib.rs`; ensure `soroban-sdk` version matches |

---

## References

- [Stellar Developer Docs](https://developers.stellar.org/docs/smart-contracts)
- [Soroban SDK](https://docs.rs/soroban-sdk)
- [Stellar CLI GitHub](https://github.com/stellar/stellar-cli)
- [Testnet setup guide](./soroban-cli-setup.md)
