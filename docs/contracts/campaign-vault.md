# Campaign Vault Contract — API Reference

The campaign vault contract holds sponsor-funded reward pools for educational reward campaigns. Sponsors fund campaigns, the backend reward engine allocates rewards against the funded pool, and admins manage the campaign lifecycle.

---

## Overview

| | |
|---|---|
| Contract crate | `contracts/campaign_vault` |
| Source | `contracts/campaign_vault/src/lib.rs` |
| Storage | instance: `Initialized`, `Admin`, `CampaignCount`; persistent: `Campaign(u32)`, `CampaignIds`, `Pool(u32)`, `Funding(u32, Address)`, `Allocated(u32)`, `RewardAllocation(u32, Address)` |
| State-changing functions | `initialize`, `create_campaign`, `fund_campaign`, `update_campaign_status`, `record_campaign_reward`, `update_criteria_ref` |
| Read-only functions | `get_admin`, `get_campaign`, `get_campaign_ids`, `get_pool`, `get_funding`, `get_allocated`, `get_student_allocation`, `get_available_pool`, `get_campaign_summary` |

All state-changing functions require the **admin address** set during initialization. Read operations are public.

---

## Campaign lifecycle

A campaign moves through the following states:

| Status | Meaning | Allowed transitions |
|---|---|---|
| `Draft` | Created but not yet accepting funding | → `Active`, `Cancelled` |
| `Active` | Open for funding and reward distribution | → `Paused`, `Completed`, `Cancelled` |
| `Paused` | Temporarily on hold; no funding or rewards processed | → `Active`, `Completed`, `Cancelled` |
| `Completed` | Finished; terminal state | — |
| `Cancelled` | Terminated; unspent funds return to the sponsor | — |

Campaigns are created in the `Active` state. `Completed` and `Cancelled` are final — no status change, funding, allocation, or criteria update is allowed after a campaign reaches them.

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

### `create_campaign(title: Symbol, sponsor: Address, reward_pool: i128, criteria_ref: Symbol, start_date: u64, end_date: u64) -> u32`

Creates a new campaign in the `Active` state and returns its unique numeric ID. `title` and `criteria_ref` must be non-empty, `reward_pool` must be greater than zero, and `end_date` must be after `start_date`.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- create_campaign \
  --title Math \
  --sponsor GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN \
  --reward_pool 1000 \
  --criteria_ref gpa30 \
  --start_date 1720000000 \
  --end_date 1722600000
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `EmptyTitle` — `title` is empty
- `EmptyCriteria` — `criteria_ref` is empty
- `ZeroPool` — `reward_pool` is zero or negative
- `InvalidDateRange` — `end_date` is not after `start_date`

### `fund_campaign(campaign_id: u32, sponsor: Address, amount: i128)`

Records a sponsor contribution toward a campaign. The campaign must exist and be `Active`, and `amount` must be greater than zero. Increases the campaign pool and tracks the sponsor's cumulative allocation.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- fund_campaign \
  --campaign_id 1 \
  --sponsor GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN \
  --amount 500
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `ZeroAmount` — `amount` is zero or negative
- `UnknownCampaign` — campaign does not exist
- `CampaignClosed` — campaign is not `Active`

### `update_campaign_status(campaign_id: u32, new_status: CampaignStatus) -> CampaignStatus`

Transitions a campaign to `new_status`. Only valid lifecycle transitions are accepted; terminal states (`Completed`, `Cancelled`) cannot change.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- update_campaign_status \
  --campaign_id 1 \
  --new_status Completed
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `UnknownCampaign` — campaign does not exist
- `InvalidTransition` — `new_status` is not reachable from the current status

### `record_campaign_reward(campaign_id: u32, student: Address, amount: i128)`

Allocates `amount` of the campaign pool to a student. Requires the campaign to be `Active` and funded, and `amount` must be greater than zero. Allocations can never exceed the remaining available pool.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- record_campaign_reward \
  --campaign_id 1 \
  --student GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN \
  --amount 100
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `ZeroAmount` — `amount` is zero or negative
- `UnknownCampaign` — campaign does not exist
- `CampaignClosed` — campaign is not `Active`
- `InsufficientPool` — `amount` exceeds the remaining available pool

### `update_criteria_ref(campaign_id: u32, criteria_ref: Symbol)`

Updates the off-chain criteria reference stored on a campaign. Rejected for empty references and for finalized campaigns (`Completed`, `Cancelled`).

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- update_criteria_ref \
  --campaign_id 1 \
  --criteria_ref gpa35
```

Panics:
- `NotInit` — contract not initialized
- `Unauthorized` — caller is not the admin
- `EmptyCriteria` — `criteria_ref` is empty
- `UnknownCampaign` — campaign does not exist
- `CampaignFinalized` — campaign is `Completed` or `Cancelled`

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

### `get_campaign(campaign_id: u32) -> Campaign | None`

Returns the full campaign record, or `None` if the campaign does not exist. Read-only — never mutates state.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_campaign \
  --campaign_id 1
```

### `get_campaign_ids() -> Vec<u32>`

Returns the list of all campaign IDs created so far, in creation order. Returns an empty list if no campaigns exist.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_campaign_ids
```

### `get_pool(campaign_id: u32) -> i128`

Returns the total funded reward pool for a campaign, or `0` if it has never been funded.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_pool \
  --campaign_id 1
```

### `get_funding(campaign_id: u32, sponsor: Address) -> i128`

Returns the cumulative contribution from `sponsor` for a campaign, or `0` if the sponsor has never contributed.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_funding \
  --campaign_id 1 \
  --sponsor GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN
```

### `get_allocated(campaign_id: u32) -> i128`

Returns the total reward amount distributed against a campaign, or `0` if nothing has been allocated.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_allocated \
  --campaign_id 1
```

### `get_student_allocation(campaign_id: u32, student: Address) -> i128`

Returns the reward amount allocated to `student` for a campaign, or `0` if the student has no allocation.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_student_allocation \
  --campaign_id 1 \
  --student GABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN
```

### `get_available_pool(campaign_id: u32) -> i128`

Returns the remaining pool available for allocation (`get_pool - get_allocated`), or `0` if the campaign has never been funded.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_available_pool \
  --campaign_id 1
```

### `get_campaign_summary(campaign_id: u32) -> CampaignSummary`

Returns a closeout summary for a campaign in any state, including finalized campaigns. The summary reports `campaign_id`, `funded`, `distributed`, `remaining` (`funded - distributed`), and `status`.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source any-account \
  --network testnet \
  -- get_campaign_summary \
  --campaign_id 1
```

---

## Events

| Topic | Data | Description |
|---|---|---|
| `fund` | `(sponsor: Address, campaign_id: u32, amount: i128)` | Emitted when a sponsor contribution is recorded |
| `status` | `(campaign_id: u32, new_status: CampaignStatus)` | Emitted when a campaign transitions to a new lifecycle state |
| `reward` | `(campaign_id: u32, student: Address, amount: i128)` | Emitted when a reward allocation is recorded against a campaign |

---

## Authorization rules

- **Admin-only (require `admin` as the source):** `initialize`, `create_campaign`, `fund_campaign`, `update_campaign_status`, `record_campaign_reward`, `update_criteria_ref`.
- **Public (read-only):** `get_admin`, `get_campaign`, `get_campaign_ids`, `get_pool`, `get_funding`, `get_allocated`, `get_student_allocation`, `get_available_pool`, `get_campaign_summary`.
- `record_campaign_reward` is intended for the backend reward engine operating as the contract admin.

---

## Security Notes

- Every state-changing function is guarded by `admin.require_auth()`.
- Allocations cannot exceed the funded pool: `record_campaign_reward` panics with `InsufficientPool` on over-allocation.
- Finalized campaigns (`Completed`, `Cancelled`) reject status changes, further funding, reward allocations, and criteria updates.
- All addresses used in the examples above are placeholder data and must be replaced with real Stellar addresses.
