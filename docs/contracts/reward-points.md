# Reward Points Contract — API Reference

The reward points contract manages the point ledger students earn through attendance, academic performance, and sponsored events. Admins mint points into student wallets; students can redeem (burn) points for rewards.

---

## Overview

| | |
|---|---|
| Contract crate | `contracts/rewards` |
| Source | `contracts/rewards/src/lib.rs` |
| Storage | instance: `Initialized`, `Admin`; persistent: `Balance(Address)` |
| State-changing functions | `initialize`, `mint_reward`, `batch_mint_rewards`, `redeem_rewards` |
| Read-only functions | `get_admin`, `get_balance` |

All state-changing functions require the **admin address** set during initialization. Read operations are public.

---

## Initialization

### `initialize(admin: Address)`

Sets the contract administrator. **One-time only** — subsequent calls panic with `AlreadyInit`.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN
```

Panics: `AlreadyInit` if already initialized.

---

## Write Functions (admin only)

### `mint_reward(recipient: Address, amount: i128, reason: RewardReason)`

Credits `amount` reward points to a student's `recipient` wallet. `amount` must be greater than zero.

`RewardReason` values:
- `Attendance` — student met attendance requirements
- `Academic` — student achieved grade/performance threshold
- `Event` — student participated in a sponsored event

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- mint_reward \
  --recipient GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN \
  --amount 100 \
  --reason Attendance
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `ZeroAmount` — `amount` is zero or negative

### `batch_mint_rewards(recipients: Vec<Address>, amounts: Vec<i128>, reason: RewardReason)`

Credits reward points to multiple students in one transaction. `recipients` and `amounts` must be the same length and non-empty; each amount must be greater than zero. Emits one reward event per recipient.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- batch_mint_rewards \
  --recipients '["GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN","GBCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN"]' \
  --amounts '[100,50]' \
  --reason Attendance
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `EmptyBatch` — `recipients` is empty
- `LengthMismatch` — `recipients` and `amounts` differ in length
- `ZeroAmount` — any amount is zero or negative

### `redeem_rewards(wallet: Address, amount: i128)`

Burns `amount` reward points from a student's balance as part of a redemption flow. `amount` must be greater than zero and cannot exceed the student's current balance.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- redeem_rewards \
  --wallet GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN \
  --amount 40
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `ZeroAmount` — `amount` is zero or negative
- `InsufficientBalance` — `amount` exceeds the wallet's balance

---

## Read Functions (public)

### `get_admin() -> Address`

Returns the admin address stored during initialization. Panics with `NotInit` if the contract has not been initialized.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_admin
```

### `get_balance(wallet: Address) -> i128`

Returns the current reward point balance for `wallet`, or `0` if the wallet has never received rewards. Read-only — does not mutate state.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_balance \
  --wallet GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN
```

---

## Events

| Topic | Data | Description |
|---|---|---|
| `reward` | `(recipient: Address, amount: i128, reason: RewardReason)` | Emitted when points are minted to a single recipient |
| `redeem` | `(wallet: Address, amount: i128)` | Emitted when points are burned from a wallet during redemption |

Notes:
- `batch_mint_rewards` emits one `reward` event **per recipient** (no aggregate event).
- The reason is emitted as a `u32` value matching the numeric mapping in `RewardReason::as_u32` (`1` = Attendance, `2` = Academic, `3` = Event).

---

## Security Notes

- Every state-changing function is guarded by `admin.require_auth()`, so only the administrator can mint, batch mint, and redeem.
- Balances are stored per wallet address under a persistent `Balance` key and can never go negative: redemption is rejected with `InsufficientBalance` when the burn would exceed the current balance.
- All addresses used in the examples above are placeholder data and must be replaced with real Stellar addresses.
