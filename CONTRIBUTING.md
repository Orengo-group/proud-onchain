# Contributing to PROUD On-Chain

Thank you for your interest in contributing to PROUD! This guide will help you get your local environment set up, understand our workflow, and open your first pull request.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Local Setup](#local-setup)
3. [Environment Variables](#environment-variables)
4. [Running Tests](#running-tests)
5. [Branch Naming](#branch-naming)
6. [Commit Style](#commit-style)
7. [Submitting a Pull Request](#submitting-a-pull-request)
8. [PR Checklist](#pr-checklist)
9. [Security Reminders](#security-reminders)

---

## Prerequisites

You will need the following tools installed before working on the on-chain layer:

| Tool | Version | Install |
|------|---------|---------|
| Rust | stable + `wasm32-unknown-unknown` target | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Stellar CLI | ≥ 21.x | [docs.stellar.org](https://developers.stellar.org/docs/tools/developer-tools/stellar-cli) |
| soroban-sdk | 21.0.0 | declared in `Cargo.toml` — pulled automatically |

Install the required Rust targets after `rustup` is set up:

```bash
rustup target add wasm32-unknown-unknown
```

Check the pinned toolchain version used by the project:

```bash
cat rust-toolchain.toml
```

---

## Local Setup

```bash
# 1. Fork this repository on GitHub, then clone your fork
git clone https://github.com/<your-username>/proud-onchain.git
cd proud-onchain

# 2. Add the upstream remote so you can pull future changes
git remote add upstream https://github.com/Orengo-group/proud-onchain.git

# 3. Install Rust dependencies (just building will fetch them)
cd contracts
cargo build --release --target wasm32-unknown-unknown
```

You can also use the helper scripts in `scripts/`:

```bash
# Build all contracts
./scripts/build-contracts.sh

# Start a local Stellar sandbox (requires Docker)
./scripts/start-localnet.sh

# Configure testnet identities
./scripts/configure-testnet.sh
```

---

## Environment Variables

The project uses `.env` files for local configuration. A documented template is provided:

```bash
cp .env.example .env
# Edit .env with your testnet RPC URL, key alias, and contract IDs
```

> ⚠️ **Never commit your `.env` file.** It is listed in `.gitignore`.  
> ⚠️ **Never put a raw secret/private key in any file.** Use a key alias managed by `stellar keys generate`.

See [`.env.example`](./.env.example) for a full explanation of every variable.

---

## Running Tests

All contract tests are standard Rust unit tests and run with `cargo test`:

```bash
# Run all tests in the workspace
cd contracts
cargo test

# Run tests for a specific contract
cargo test -p identity_registry

# Run a single test by name
cargo test -p identity_registry test_register_student
```

Tests must pass before opening a pull request. The CI pipeline (`contracts-ci.yml`) will also run them automatically on every PR.

---

## Branch Naming

Use the following format for your branch names:

```
<type>/<short-description>
```

Examples:

| Type | Example |
|------|---------|
| `feat` | `feat/register-student-function` |
| `fix` | `fix/duplicate-nfc-check` |
| `docs` | `docs/update-contributing` |
| `refactor` | `refactor/storage-keys` |
| `chore` | `chore/update-sdk-version` |

Create your branch from an up-to-date `main`:

```bash
git fetch upstream
git checkout -b feat/your-feature upstream/main
```

---

## Commit Style

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<optional scope>): <short summary>

[optional body]

[optional footer — e.g. Closes #12]
```

Examples:

```
feat(identity_registry): add register_student function

Implement student registration with duplicate checks for student ID,
NFC hash, and wallet address.

Closes #12
```

```
fix(identity_registry): reject duplicate NFC hash on re-registration
```

Keep the summary line under **72 characters**. Use the body to explain *why*, not *what*.

---

## Submitting a Pull Request

1. Push your branch to your fork:

   ```bash
   git push -u origin feat/your-feature
   ```

2. Open a PR against `Orengo-group/proud-onchain` → `main`.

3. Fill in the PR description:
   - **What** changed and **why**
   - How you tested it
   - Reference any issues it closes: `Closes #<number>`

4. Ensure all CI checks pass (Rust format, lint, tests).

5. Request a review from a maintainer.

---

## PR Checklist

Before marking your PR as ready for review, confirm the following:

- [ ] `cargo test` passes locally for all affected contracts
- [ ] `cargo fmt --check` reports no formatting issues
- [ ] `cargo clippy -- -D warnings` reports no warnings
- [ ] No secrets, private keys, or `.env` files are included in the commit
- [ ] New functions have doc-comments (`///`) explaining their purpose
- [ ] Storage keys are documented with their type and purpose
- [ ] PR description references the issue it closes (`Closes #N`)
- [ ] Branch is up-to-date with `main`

---

## Security Reminders

- **Never commit secret keys, seed phrases, or private keys.** Use `stellar keys generate` and reference keys only by alias.
- **Never commit `.env` files.** Copy from `.env.example`, fill in locally, and keep the file out of git.
- If you accidentally commit a secret, **rotate the key immediately** and open an issue so maintainers can review the history.
- Do not log or expose contract admin addresses in test output unnecessarily.
- Validate all inputs in contract functions — reject empty strings, malformed addresses, and duplicate values explicitly.

---

Questions? Open a GitHub Discussion or leave a comment on the relevant issue. We're happy to help!
