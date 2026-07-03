# Soroban CLI Setup Guide

This guide walks contributors through installing the Stellar Soroban CLI and configuring it for local development against the Stellar testnet.

---

## Prerequisites

- **Rust 1.81.0** — managed automatically via `rust-toolchain.toml` at the repo root.
- **`wasm32-unknown-unknown` target** — included in the toolchain file.
- An internet connection to reach Stellar testnet RPC endpoints.

---

## 1. Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify:

```bash
rustc --version
# Expected: rustc 1.81.0 (eeb90cda1 2024-09-04) or later
```

---

## 2. Install the Soroban CLI

```bash
cargo install --locked stellar-cli --features opt
```

> **Note:** The `--features opt` flag enables the optimizer (requires `wasm-opt`). You can omit it on a first install and add it later.

Verify:

```bash
stellar --version
# Expected output: stellar 21.x.x
```

### macOS (Homebrew alternative)

```bash
brew install stellar-cli
```

### Windows / WSL

Use the Rust/Cargo method above inside your WSL2 terminal. The Homebrew path is not supported on Windows natively.

---

## 3. Add the WebAssembly target

The toolchain file handles this automatically, but you can also add it manually:

```bash
rustup target add wasm32-unknown-unknown
```

---

## 4. Configure the Stellar testnet network

```bash
stellar network add \
  --global testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

Verify the network was saved:

```bash
stellar network ls
```

---

## 5. Create a test identity (keypair)

```bash
stellar keys generate --global my-key --network testnet
```

> ⚠️ **Never commit your secret key.** The key is stored in `~/.config/stellar/identity/` — it never touches this repository.

Show the public key:

```bash
stellar keys address my-key
```

Fund the account via Friendbot (testnet only):

```bash
stellar keys fund my-key --network testnet
```

---

## 6. Build and test contracts locally

From the `contracts/` directory:

```bash
cd contracts/
cargo build --target wasm32-unknown-unknown --release
cargo test
```

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `command not found: stellar` | Make sure `~/.cargo/bin` is on your `$PATH`. Re-run `source "$HOME/.cargo/env"`. |
| `wasm32-unknown-unknown` target missing | Run `rustup target add wasm32-unknown-unknown`. |
| Friendbot rate-limited | Wait a minute and retry, or use the [Stellar Lab](https://laboratory.stellar.org/#account-creator?network=test). |
| RPC errors on testnet | Check [status.stellar.org](https://status.stellar.org) for network incidents. |
| `error[E0463]: can't find crate for std` | You're missing the `wasm32` target — see step 3. |

---

## References

- [Stellar Developer Docs](https://developers.stellar.org/docs/smart-contracts)
- [Soroban SDK](https://docs.rs/soroban-sdk)
- [Stellar CLI GitHub](https://github.com/stellar/stellar-cli)
- [Stellar Testnet Friendbot](https://friendbot.stellar.org)
