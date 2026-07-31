# Identity Registry Contract — API Reference

The identity registry is the source of truth for student on-chain identities. It maps a **hashed NFC UID** to a **Stellar wallet address** and a **student ID**, enabling attendance verification and reward distribution.

---

## Overview

| | |
|---|---|
| Contract crate | `contracts/identity_registry` |
| Source | `contracts/identity_registry/src/lib.rs` |
| Storage | instance: `Initialized`, `Admin`; persistent: `Student`, `NfcIndex`, `WalletIndex` |
| State-changing functions | `initialize`, `register_student`, `update_student_wallet`, `deactivate_student`, `reactivate_student` |
| Read-only functions | `get_admin`, `get_student`, `get_student_by_nfc_hash` |

All write operations require the **admin address** set during initialization. Read operations are public.

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

### `register_student(student_id: BytesN<32>, nfc_hash: BytesN<32>, wallet: Address)`

Registers a new student. The `student_id`, `nfc_hash`, and `wallet` must each be unique. The record is stored with `active = true`.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- register_student \
  --student_id 0x0000000000000000000000000000000000000000000000000000000000000001 \
  --nfc_hash 0x4f1c9b8f8b9c1d8f9a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4 \
  --wallet GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `DuplicateStudentId` — student ID already registered
- `DuplicateNfcHash` — NFC hash already registered
- `DuplicateWallet` — wallet already registered to another student

### `update_student_wallet(student_id: BytesN<32>, new_wallet: Address)`

Changes the wallet for an existing student. The old wallet index is removed and the new wallet is indexed.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- update_student_wallet \
  --student_id 0x0000000000000000000000000000000000000000000000000000000000000001 \
  --new_wallet GNEWWALLET0000000000000000000000000000000000000000000000000
```

Panics: `NotInit`, `Unauthorized`, `StudentNotFound`, `DuplicateWallet`.

### `deactivate_student(student_id: BytesN<32>)`

Sets `active = false` on an existing record. The record and all lookups remain available.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- deactivate_student \
  --student_id 0x0000000000000000000000000000000000000000000000000000000000000001
```

Panics: `NotInit`, `Unauthorized`, `StudentNotFound`.

### `reactivate_student(student_id: BytesN<32>)`

Sets `active = true` on a previously deactivated record.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- reactivate_student \
  --student_id 0x0000000000000000000000000000000000000000000000000000000000000001
```

Panics: `NotInit`, `Unauthorized`, `StudentNotFound`.

---

## Read Functions (public)

### `get_admin() -> Address`

Returns the admin address stored at initialization.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_admin
```

Panics: `NotInit` if the contract has not been initialized.

### `get_student(student_id: BytesN<32>) -> Option<StudentRecord>`

Returns the full `StudentRecord` for a student ID, or `None` if unknown.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_student \
  --student_id 0x0000000000000000000000000000000000000000000000000000000000000001
```

### `get_student_by_nfc_hash(nfc_hash: BytesN<32>) -> Option<StudentRecord>`

Resolves an NFC hash to a student via the reverse index, then returns the record. Returns `None` for unknown hashes.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_student_by_nfc_hash \
  --nfc_hash 0x4f1c9b8f8b9c1d8f9a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4
```

---

## Types

### `StudentRecord`

| Field | Type | Description |
|---|---|---|
| `student_id` | `BytesN<32>` | Unique school-issued identifier |
| `nfc_hash` | `BytesN<32>` | SHA-256 hash of the student's NFC UID |
| `wallet` | `Address` | Stellar wallet used for reward distribution |
| `active` | `bool` | Whether the record is currently active |

---

## Security Notes

1. **Never store raw NFC UIDs on-chain.** NFC UIDs are sensitive identifiers. Only the **SHA-256 hash** of the UID may be submitted to `register_student`. This prevents replay against a compromised ledger and keeps raw hardware identifiers off-chain.

2. **Hashing before submission.** The backend must hash the NFC UID before any contract call:

   ```bash
   # Example: hash a hex NFC UID with SHA-256
   echo -n "0403A2B1C4D5E6F7" | shasum -a 256
   # 4f1c9b8f8b9c1d8f9a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4
   ```

3. **Admin key custody.** The admin key controls all write operations. Store it in a hardware wallet or split via multisig. Rotate via redeploy — the contract does not support admin transfer.

4. **Wallet uniqueness.** Each wallet can belong to only one student. Reuse attempts fail with `DuplicateWallet`.

---

## Backend Integration Flow

```
NFC reader → raw UID → SHA-256 → register_student(nfc_hash, student_id, wallet)
                                          │
        attendance check → get_student_by_nfc_hash(nfc_hash) → student record
```

1. On registration, the backend hashes the NFC UID and calls `register_student`.
2. On attendance scans, the backend hashes the scanned UID and calls `get_student_by_nfc_hash` to resolve the student.
3. Reward distribution reads the resolved wallet address and targets the rewards contract.

---

## References

- [Local development guide](./../local-development.md)
- [Stellar CLI setup](./../soroban-cli-setup.md)
- [Soroban SDK](https://docs.rs/soroban-sdk)
