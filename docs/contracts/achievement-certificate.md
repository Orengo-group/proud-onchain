# Achievement Certificate Contract — API Reference

The achievement certificate contract issues verifiable on-chain certificates for student academic achievements. It stores each certificate as a structured record and supports lookup by achievement ID or student wallet.

---

## Overview

| | |
|---|---|
| Contract crate | `contracts/achievement_certificate` |
| Source | `contracts/achievement_certificate/src/lib.rs` |
| Storage | instance: `Initialized`, `Admin`, `AchievementIdCounter`; persistent: `Achievement(u64)`, `StudentAchievements(Address)` |
| State-changing functions | `initialize`, `issue_achievement`, `revoke_achievement` |
| Read-only functions | `get_admin`, `get_achievement`, `get_student_achievements` |

All write operations require the **admin address** set during initialization. Read operations are public.

---

## Initialization

### `initialize(admin: Address)`

Sets the contract administrator. **One-time only** — subsequent calls panic with `AlreadyInit`. The admin address is stored in instance storage and is required for all state-changing operations.

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

### `issue_achievement(student: Address, title: Symbol, category: Symbol, metadata_hash: BytesN<32>) -> u64`

Issues a new achievement certificate to a student. Each issued certificate is assigned a unique auto-incrementing ID starting at 1. The `issued_at` field is set to the current ledger sequence number.

`title` must be a non-empty `Symbol` (max 9 bytes). `category` is an open-ended symbol (e.g. `"academic"`, `"extracurricular"`). `metadata_hash` is a SHA-256 hash of the off-chain certificate document (e.g. a PDF or image).

Returns the new achievement ID.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- issue_achievement \
  --student GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN \
  --title BestMath \
  --category academic \
  --metadata_hash 0x4f1c9b8f8b9c1d8f9a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `EmptyTitle` — `title` is empty

### `revoke_achievement(achievement_id: u64)`

Revokes an issued achievement certificate. The record remains readable after revocation, but its `revoked` field is set to `true`. Revoked achievements cannot be re-revoked.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- revoke_achievement \
  --achievement_id 1
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `AchievementNotFound` — no achievement with that ID exists
- `AlreadyRevoked` — achievement has already been revoked

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

### `get_achievement(achievement_id: u64) -> Option<AchievementRecord>`

Returns the full `AchievementRecord` for a given achievement ID, or `None` if no achievement with that ID exists. Revoked achievements are still returned — check the `revoked` field to determine validity.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_achievement \
  --achievement_id 1
```

### `get_student_achievements(student: Address) -> Vec<u64>`

Returns a list of achievement IDs issued to the given student wallet, in issuance order. Returns an empty list if the student has no achievements.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_student_achievements \
  --student GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN
```

---

## Types

### `AchievementRecord`

| Field | Type | Description |
|---|---|---|
| `achievement_id` | `u64` | Unique auto-incrementing ID (starts at 1) |
| `student` | `Address` | Stellar wallet that owns the certificate |
| `title` | `Symbol` | Achievement title (e.g. `"BestMath"`, max 9 bytes) |
| `category` | `Symbol` | Achievement category (e.g. `"academic"`) |
| `metadata_hash` | `BytesN<32>` | SHA-256 hash of the off-chain certificate document |
| `issued_at` | `u64` | Ledger sequence number when the certificate was issued |
| `revoked` | `bool` | Whether this certificate has been revoked by the admin |

---

## Events

| Topic | Data | Description |
|---|---|---|
| `achieve` | `(achievement_id: u64, student: Address, title: Symbol, category: Symbol, metadata_hash: BytesN<32>, issued_at: u64)` | Emitted when a certificate is issued |
| `revoke` | `(achievement_id: u64, student: Address, title: Symbol)` | Emitted when a certificate is revoked |

---

## Security Notes

1. **Admin key custody.** The admin key controls issuance and revocation. Store it in a hardware wallet or split via multisig. Rotate via redeploy — the contract does not support admin transfer.

2. **Off-chain metadata.** The `metadata_hash` field stores only a SHA-256 hash of the certificate document, not the document itself. The full certificate (PDF, image, etc.) must be stored off-chain and resolved by the backend using this hash.

3. **Revocation is permanent.** Once revoked, a certificate cannot be un-revoked. The `revoked` flag is irreversible — if a certificate was revoked in error, a new one must be issued.

4. **Achievement IDs are sequential.** IDs start at 1 and increment globally across all students. Gaps in the sequence indicate revoked or non-existent achievements.

---

## Verification Flow

```
Backend → get_achievement(id) → AchievementRecord
                                      │
                    check revoked == false → certificate is valid
                    check metadata_hash → resolve off-chain document
```

1. A verifier submits the achievement ID to `get_achievement`.
2. The returned `AchievementRecord` contains all on-chain data.
3. If `revoked` is `true`, the certificate is invalid.
4. If `revoked` is `false`, the backend resolves `metadata_hash` against the off-chain document store to retrieve the full certificate.

---

## References

- [Local development guide](./../local-development.md)
- [Stellar CLI setup](./../soroban-cli-setup.md)
- [Soroban SDK](https://docs.rs/soroban-sdk)
