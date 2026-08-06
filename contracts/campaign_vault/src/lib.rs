//! # Campaign Vault Contract
//!
//! Holds sponsor-funded reward pools for specific educational campaigns.
//!
//! ## Responsibilities
//! - Accept sponsor deposits tied to a campaign ID
//! - Release funds to the Rewards contract when criteria are met
//! - Return unspent funds to sponsors after campaign expiry
//! - Admin initialization and campaign storage types (issue #27)
//! - Campaign lookup functions and ID index (issue #29)
//! - Campaign lifecycle status management (issue #31)
//! - Campaign reward allocation tracking (issue #32)
//! - Campaign criteria reference handling (issue #33)
//! - Campaign closeout summary lookup (issue #34)
//!
//! ## Storage Keys
//! - `DataKey::Initialized`     — `bool`    — guards one-time init
//! - `DataKey::Admin`           — `Address` — contract administrator
//! - `DataKey::CampaignCount`   — `u32`     — monotonic campaign ID counter
//! - `DataKey::Campaign(id)`    — `Campaign` — campaign record keyed by ID
//! - `DataKey::CampaignIds`     — `Vec<u32>` — index of all campaign IDs
//! - `DataKey::Pool(id)`        — `i128`    — funded reward pool per campaign
//! - `DataKey::Funding(id, addr)` — `i128`  — cumulative sponsor contribution
//! - `DataKey::Allocated(id)`   — `i128`    — distributed reward total per campaign
//! - `DataKey::RewardAllocation(id, addr)` — `i128` — per-student allocation

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
pub enum DataKey {
    /// Whether `initialize` has already been called.
    Initialized,
    /// Stores the admin [`Address`] set during `initialize`.
    Admin,
    /// Monotonic counter used to assign unique campaign IDs.
    CampaignCount,
    /// Stores a [`Campaign`] record keyed by its numeric ID.
    Campaign(u32),
    /// Tracks the total funded reward pool for a campaign ID.
    Pool(u32),
    /// Tracks the total allocation contributed by a sponsor for a campaign.
    Funding((u32, Address)),
    /// Index of all campaign IDs ever created.
    CampaignIds,
    /// Tracks the total reward amount distributed against a campaign.
    Allocated(u32),
    /// Tracks the reward amount allocated to a student for a campaign.
    RewardAllocation((u32, Address)),
}

// ---------------------------------------------------------------------------
// Campaign types
// ---------------------------------------------------------------------------

/// Lifecycle state of a sponsor campaign (issue #31).
#[contracttype]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CampaignStatus {
    /// Campaign has been created but is not yet accepting funding.
    Draft,
    /// Campaign is open for funding and reward distribution.
    Active,
    /// Campaign is temporarily on hold; no funding or rewards are processed.
    Paused,
    /// Campaign has finished; the lifecycle is terminal.
    Completed,
    /// Campaign was cancelled; unspent funds return to the sponsor.
    Cancelled,
}

/// A sponsor-funded educational reward campaign.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Campaign {
    /// Unique numeric campaign identifier assigned by the contract.
    pub id: u32,
    /// Display title of the campaign.
    pub title: Symbol,
    /// Sponsor [`Address`] funding the campaign.
    pub sponsor: Address,
    /// Reward pool amount allocated by the sponsor.
    pub reward_pool: i128,
    /// Reference to the criteria document or criteria ID.
    pub criteria_ref: Symbol,
    /// Campaign start date (Unix seconds).
    pub start_date: u64,
    /// Campaign end date (Unix seconds).
    pub end_date: u64,
    /// Current lifecycle state.
    pub status: CampaignStatus,
}

/// Closeout summary of a campaign's reward pool (issue #34).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignSummary {
    /// Unique numeric campaign identifier.
    pub campaign_id: u32,
    /// Total reward amount funded into the campaign pool.
    pub funded: i128,
    /// Total reward amount distributed to students.
    pub distributed: i128,
    /// Remaining pool available for allocation (`funded - distributed`).
    pub remaining: i128,
    /// Current lifecycle state of the campaign.
    pub status: CampaignStatus,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct CampaignVaultContract;

#[contractimpl]
impl CampaignVaultContract {
    // -----------------------------------------------------------------------
    // Issue #27 — Initialization
    // -----------------------------------------------------------------------

    /// Initialize the contract with an admin address.
    ///
    /// Can only be called once. Subsequent calls panic with `"AlreadyInit"`.
    ///
    /// # Arguments
    /// * `admin` — Address that will administer this contract.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("AlreadyInit");
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Return the admin address stored during initialization.
    ///
    /// Panics with `"NotInit"` if the contract has not been initialized.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"))
    }

    // -----------------------------------------------------------------------
    // Issue #28 — Campaign creation
    // -----------------------------------------------------------------------

    /// Create a new sponsor-funded reward campaign.
    ///
    /// Only the contract admin may create campaigns. The campaign is assigned
    /// a unique numeric ID via an internal counter and starts in the `Active`
    /// state.
    ///
    /// # Arguments
    /// * `title`        — Display title (must not be empty).
    /// * `sponsor`      — Sponsor [`Address`] funding the campaign.
    /// * `reward_pool`  — Reward pool amount allocated by the sponsor (> 0).
    /// * `criteria_ref` — Reference to the criteria document or criteria ID.
    /// * `start_date`   — Campaign start date (Unix seconds).
    /// * `end_date`     — Campaign end date (Unix seconds, must be after start).
    ///
    /// # Returns
    /// The unique numeric campaign ID.
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"EmptyTitle"` if `title` is empty.
    /// - `"ZeroPool"` if `reward_pool` is zero or negative.
    /// - `"InvalidDateRange"` if `end_date` is not after `start_date`.
    pub fn create_campaign(
        env: Env,
        title: Symbol,
        sponsor: Address,
        reward_pool: i128,
        criteria_ref: Symbol,
        start_date: u64,
        end_date: u64,
    ) -> u32 {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        if title == symbol_short!("") {
            panic!("EmptyTitle");
        }
        if criteria_ref == symbol_short!("") {
            panic!("EmptyCriteria");
        }
        if reward_pool <= 0 {
            panic!("ZeroPool");
        }
        if end_date <= start_date {
            panic!("InvalidDateRange");
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CampaignCount)
            .unwrap_or(0);
        let id = count + 1;
        env.storage().instance().set(&DataKey::CampaignCount, &id);

        // Append the new ID to the campaign index (issue #29)
        let mut ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CampaignIds)
            .unwrap_or(Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(&DataKey::CampaignIds, &ids);

        let campaign = Campaign {
            id,
            title,
            sponsor,
            reward_pool,
            criteria_ref,
            start_date,
            end_date,
            status: CampaignStatus::Active,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(id), &campaign);

        id
    }

    /// Return the [`Campaign`] record for `campaign_id`, or `None` if unknown.
    ///
    /// Lookups never mutate contract state.
    pub fn get_campaign(env: Env, campaign_id: u32) -> Option<Campaign> {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
    }

    /// Return the list of all campaign IDs created so far (issue #29).
    ///
    /// Returns an empty [`Vec`] if no campaigns have been created yet.
    pub fn get_campaign_ids(env: Env) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::CampaignIds)
            .unwrap_or(Vec::new(&env))
    }

    // -----------------------------------------------------------------------
    // Issue #30 — Sponsor funding
    // -----------------------------------------------------------------------

    /// Record a sponsor funding contribution toward a campaign.
    ///
    /// Only the contract admin may record funding. The funding amount must be
    /// greater than zero and the campaign must exist and be `Active`.
    ///
    /// # Arguments
    /// * `campaign_id` — Target campaign ID.
    /// * `sponsor`     — Sponsor [`Address`] making the contribution.
    /// * `amount`      — Funding amount (must be > 0).
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"UnknownCampaign"` if the campaign does not exist.
    /// - `"CampaignClosed"` if the campaign is not `Active`.
    /// - `"ZeroAmount"` if `amount` is zero or negative.
    pub fn fund_campaign(env: Env, campaign_id: u32, sponsor: Address, amount: i128) {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        if amount <= 0 {
            panic!("ZeroAmount");
        }

        let campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap_or_else(|| panic!("UnknownCampaign"));

        if campaign.status != CampaignStatus::Active {
            panic!("CampaignClosed");
        }

        // Increase the campaign's funded reward pool
        let pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Pool(campaign_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Pool(campaign_id), &(pool + amount));

        // Track this sponsor's cumulative allocation
        let contributed: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Funding((campaign_id, sponsor.clone())))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Funding((campaign_id, sponsor.clone())),
            &(contributed + amount),
        );

        // Emit funding event for off-chain indexing
        env.events()
            .publish((symbol_short!("fund"),), (sponsor, campaign_id, amount));
    }

    /// Return the total funded reward pool for `campaign_id`.
    ///
    /// Returns `0` if the campaign has never been funded.
    pub fn get_pool(env: Env, campaign_id: u32) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Pool(campaign_id))
            .unwrap_or(0)
    }

    /// Return the cumulative allocation contributed by `sponsor` for `campaign_id`.
    ///
    /// Returns `0` if the sponsor has never contributed.
    pub fn get_funding(env: Env, campaign_id: u32, sponsor: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Funding((campaign_id, sponsor)))
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Issue #31 — Campaign status management
    // -----------------------------------------------------------------------

    /// Update the lifecycle status of an existing campaign.
    ///
    /// Only the contract admin may update status. The requested transition
    /// must be valid for the current state, otherwise the call panics.
    ///
    /// # Allowed transitions
    /// - `Draft`     → `Active`, `Cancelled`
    /// - `Active`    → `Paused`, `Completed`, `Cancelled`
    /// - `Paused`    → `Active`, `Completed`, `Cancelled`
    /// - `Completed` → (terminal)
    /// - `Cancelled` → (terminal)
    ///
    /// # Arguments
    /// * `campaign_id` — Target campaign ID.
    /// * `new_status`  — Desired lifecycle state.
    ///
    /// # Returns
    /// The updated [`CampaignStatus`].
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"UnknownCampaign"` if the campaign does not exist.
    /// - `"InvalidTransition"` if `new_status` is not reachable from the current status.
    pub fn update_campaign_status(
        env: Env,
        campaign_id: u32,
        new_status: CampaignStatus,
    ) -> CampaignStatus {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        let mut campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap_or_else(|| panic!("UnknownCampaign"));

        if !Self::can_transition(campaign.status, new_status) {
            panic!("InvalidTransition");
        }

        campaign.status = new_status;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        // Emit status event for off-chain indexing
        env.events()
            .publish((symbol_short!("status"),), (campaign_id, new_status));

        new_status
    }

    // -----------------------------------------------------------------------
    // Issue #32 — Reward allocation tracking
    // -----------------------------------------------------------------------

    /// Record a reward allocation against a campaign's funded pool.
    ///
    /// Only the contract admin (the authorized reward engine) may record
    /// allocations. An allocation increases the campaign's distributed total
    /// and can never exceed the available pool.
    ///
    /// # Arguments
    /// * `campaign_id` — Target campaign ID.
    /// * `student`     — Student [`Address`] receiving the reward.
    /// * `amount`      — Allocation amount (must be > 0).
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"ZeroAmount"` if `amount` is zero or negative.
    /// - `"UnknownCampaign"` if the campaign does not exist.
    /// - `"CampaignClosed"` if the campaign is not `Active`.
    /// - `"InsufficientPool"` if `amount` exceeds the remaining available pool.
    pub fn record_campaign_reward(env: Env, campaign_id: u32, student: Address, amount: i128) {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        if amount <= 0 {
            panic!("ZeroAmount");
        }

        let campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap_or_else(|| panic!("UnknownCampaign"));

        if campaign.status != CampaignStatus::Active {
            panic!("CampaignClosed");
        }

        // Prevent over-allocation against the remaining funded pool
        let pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Pool(campaign_id))
            .unwrap_or(0);
        let allocated: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Allocated(campaign_id))
            .unwrap_or(0);
        if amount > pool - allocated {
            panic!("InsufficientPool");
        }

        // Increase the campaign's distributed total
        env.storage()
            .persistent()
            .set(&DataKey::Allocated(campaign_id), &(allocated + amount));

        // Track per-student allocations
        let student_allocated: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RewardAllocation((campaign_id, student.clone())))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::RewardAllocation((campaign_id, student.clone())),
            &(student_allocated + amount),
        );

        // Emit allocation event for off-chain indexing
        env.events()
            .publish((symbol_short!("reward"),), (campaign_id, student, amount));
    }

    /// Return the total reward amount distributed against `campaign_id`.
    ///
    /// Returns `0` if nothing has been allocated yet.
    pub fn get_allocated(env: Env, campaign_id: u32) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Allocated(campaign_id))
            .unwrap_or(0)
    }

    /// Return the reward amount allocated to `student` for `campaign_id`.
    ///
    /// Returns `0` if the student has no allocation.
    pub fn get_student_allocation(env: Env, campaign_id: u32, student: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::RewardAllocation((campaign_id, student)))
            .unwrap_or(0)
    }

    /// Return the remaining pool available for allocation against `campaign_id`.
    ///
    /// Returns `0` if the campaign has never been funded.
    pub fn get_available_pool(env: Env, campaign_id: u32) -> i128 {
        let pool = Self::get_pool(env.clone(), campaign_id);
        let allocated = Self::get_allocated(env, campaign_id);
        pool - allocated
    }

    // -----------------------------------------------------------------------
    // Issue #34 — Campaign closeout summary
    // -----------------------------------------------------------------------

    /// Return a closeout [`CampaignSummary`] for `campaign_id`.
    ///
    /// Read-only lookup that reports the total funded amount, the total
    /// distributed amount, the remaining pool, and the campaign's current
    /// lifecycle status. Works for campaigns in any state, including
    /// finalized (`Completed` or `Cancelled`) campaigns.
    ///
    /// # Arguments
    /// * `campaign_id` — Target campaign ID.
    ///
    /// # Returns
    /// A [`CampaignSummary`] with `funded = pool`, `distributed = allocated`,
    /// and `remaining = funded - distributed`.
    ///
    /// # Panics
    /// - `"UnknownCampaign"` if the campaign does not exist.
    pub fn get_campaign_summary(env: Env, campaign_id: u32) -> CampaignSummary {
        let campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap_or_else(|| panic!("UnknownCampaign"));

        let funded = Self::get_pool(env.clone(), campaign_id);
        let distributed = Self::get_allocated(env.clone(), campaign_id);
        let remaining = funded - distributed;

        CampaignSummary {
            campaign_id,
            funded,
            distributed,
            remaining,
            status: campaign.status,
        }
    }

    // -----------------------------------------------------------------------
    // Issue #33 — Criteria reference handling
    // -----------------------------------------------------------------------

    /// Update the criteria reference of an existing campaign.
    ///
    /// Only the contract admin may update the reference, and only while the
    /// campaign has not been finalized (`Completed` or `Cancelled`).
    ///
    /// # Arguments
    /// * `campaign_id`  — Target campaign ID.
    /// * `criteria_ref` — New criteria document reference or criteria ID.
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"EmptyCriteria"` if `criteria_ref` is empty.
    /// - `"UnknownCampaign"` if the campaign does not exist.
    /// - `"CampaignFinalized"` if the campaign is `Completed` or `Cancelled`.
    pub fn update_criteria_ref(env: Env, campaign_id: u32, criteria_ref: Symbol) {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        if criteria_ref == symbol_short!("") {
            panic!("EmptyCriteria");
        }

        let mut campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap_or_else(|| panic!("UnknownCampaign"));

        if Self::is_finalized(campaign.status) {
            panic!("CampaignFinalized");
        }

        campaign.criteria_ref = criteria_ref;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn assert_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }
    }

    fn assert_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();
    }

    /// Whether `from` can legally transition to `to` under the campaign
    /// lifecycle rules. A transition to the current state is invalid.
    fn can_transition(from: CampaignStatus, to: CampaignStatus) -> bool {
        if from == to {
            return false;
        }
        match from {
            CampaignStatus::Draft => {
                matches!(to, CampaignStatus::Active | CampaignStatus::Cancelled)
            }
            CampaignStatus::Active => matches!(
                to,
                CampaignStatus::Paused | CampaignStatus::Completed | CampaignStatus::Cancelled
            ),
            CampaignStatus::Paused => matches!(
                to,
                CampaignStatus::Active | CampaignStatus::Completed | CampaignStatus::Cancelled
            ),
            CampaignStatus::Completed | CampaignStatus::Cancelled => false,
        }
    }

    /// Whether a campaign is in a terminal state.
    fn is_finalized(status: CampaignStatus) -> bool {
        matches!(
            status,
            CampaignStatus::Completed | CampaignStatus::Cancelled
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events as _, MockAuth, MockAuthInvoke},
        Address, Env, IntoVal, Symbol, TryIntoVal, Val, Vec,
    };

    /// Set up a fresh initialized contract and return (env, client, admin).
    fn setup() -> (Env, CampaignVaultContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        (env, client, admin)
    }

    #[test]
    fn test_initialize_sets_admin() {
        let (env, client, admin) = setup();
        assert_eq!(client.get_admin(), admin);
        let _ = env;
    }

    #[test]
    #[should_panic(expected = "AlreadyInit")]
    fn test_initialize_rejects_double_init() {
        let (env, client, admin) = setup();
        let _ = env;
        client.initialize(&admin);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_get_admin_rejects_when_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        client.get_admin();
    }

    // -----------------------------------------------------------------------
    // Issue #28 — create_campaign tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_campaign_success() {
        let (env, client, admin) = setup();

        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        // Unique ID is assigned
        assert_eq!(id, 1);
        let second = client.create_campaign(
            &symbol_short!("chem"),
            &sponsor,
            &2_000_i128,
            &symbol_short!("gpa25"),
            &100u64,
            &200u64,
        );
        assert_eq!(second, 2);

        // Record is stored with all fields and Active status
        let campaign = client.get_campaign(&1).unwrap();
        assert_eq!(campaign.id, 1);
        assert_eq!(campaign.title, symbol_short!("math"));
        assert_eq!(campaign.sponsor, sponsor);
        assert_eq!(campaign.reward_pool, 1_000);
        assert_eq!(campaign.criteria_ref, symbol_short!("gpa30"));
        assert_eq!(campaign.start_date, 100);
        assert_eq!(campaign.end_date, 200);
        assert_eq!(campaign.status, CampaignStatus::Active);
        let _ = admin;
    }

    #[test]
    fn test_create_campaign_returns_unknown_campaign_none() {
        let (env, client, _admin) = setup();
        assert!(client.get_campaign(&99).is_none());
        let _ = env;
    }

    #[test]
    #[should_panic(expected = "EmptyTitle")]
    fn test_create_campaign_rejects_empty_title() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        client.create_campaign(
            &Symbol::new(&env, ""),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
    }

    #[test]
    #[should_panic(expected = "ZeroPool")]
    fn test_create_campaign_rejects_zero_pool() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &0_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
    }

    #[test]
    #[should_panic(expected = "InvalidDateRange")]
    fn test_create_campaign_rejects_invalid_date_range() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        // end_date before start_date
        client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &200u64,
            &100u64,
        );
    }

    #[test]
    #[should_panic(expected = "InvalidDateRange")]
    fn test_create_campaign_rejects_equal_date_range() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &100u64,
        );
    }

    #[test]
    fn test_create_campaign_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        // Authorize only the admin for `initialize`
        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "initialize",
                    args: (&admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .initialize(&admin);

        // Only the attacker is authorized for `create_campaign` — the admin's
        // authorization is missing, so the call must fail.
        let sponsor = Address::generate(&env);
        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "create_campaign",
                    args: (
                        symbol_short!("math"),
                        sponsor.clone(),
                        1_000_i128,
                        symbol_short!("gpa30"),
                        100u64,
                        200u64,
                    )
                        .into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_create_campaign(
                &symbol_short!("math"),
                &sponsor,
                &1_000_i128,
                &symbol_short!("gpa30"),
                &100u64,
                &200u64,
            );
        assert!(result.is_err());
    }

    #[test]
    fn test_create_campaign_succeeds_as_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "initialize",
                    args: (&admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .initialize(&admin);

        let sponsor = Address::generate(&env);
        let result = client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "create_campaign",
                    args: (
                        symbol_short!("math"),
                        sponsor.clone(),
                        1_000_i128,
                        symbol_short!("gpa30"),
                        100u64,
                        200u64,
                    )
                        .into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_create_campaign(
                &symbol_short!("math"),
                &sponsor,
                &1_000_i128,
                &symbol_short!("gpa30"),
                &100u64,
                &200u64,
            );
        assert_eq!(result.unwrap().unwrap(), 1);
    }

    // -----------------------------------------------------------------------
    // Issue #30 — fund_campaign tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_fund_campaign_increases_pool_and_records_allocation() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        client.fund_campaign(&id, &sponsor, &500);
        assert_eq!(client.get_pool(&id), 500);
        assert_eq!(client.get_funding(&id, &sponsor), 500);

        // Subsequent contributions accumulate
        client.fund_campaign(&id, &sponsor, &250);
        assert_eq!(client.get_pool(&id), 750);
        assert_eq!(client.get_funding(&id, &sponsor), 750);

        let _ = env;
    }

    #[test]
    fn test_fund_campaign_tracks_allocations_per_sponsor() {
        let (env, client, _admin) = setup();
        let sponsor_a = Address::generate(&env);
        let sponsor_b = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor_a,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        client.fund_campaign(&id, &sponsor_a, &300);
        client.fund_campaign(&id, &sponsor_b, &700);

        assert_eq!(client.get_pool(&id), 1_000);
        assert_eq!(client.get_funding(&id, &sponsor_a), 300);
        assert_eq!(client.get_funding(&id, &sponsor_b), 700);
    }

    #[test]
    fn test_fund_campaign_emits_event() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        client.fund_campaign(&id, &sponsor, &500);

        let events = env.events().all();
        assert_eq!(events.len(), 1);

        let (_contract_id, topics, data) = events.get(0).unwrap();
        let topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        assert_eq!(topic, symbol_short!("fund"));

        let vals: Vec<Val> = data.try_into_val(&env).unwrap();
        assert_eq!(vals.len(), 3);
        let emitted_sponsor: Address = vals.get(0).unwrap().try_into_val(&env).unwrap();
        let emitted_campaign: u32 = vals.get(1).unwrap().try_into_val(&env).unwrap();
        let emitted_amount: i128 = vals.get(2).unwrap().try_into_val(&env).unwrap();
        assert_eq!(emitted_sponsor, sponsor);
        assert_eq!(emitted_campaign, id);
        assert_eq!(emitted_amount, 500);
    }

    #[test]
    #[should_panic(expected = "ZeroAmount")]
    fn test_fund_campaign_rejects_zero_amount() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &0);
    }

    #[test]
    #[should_panic(expected = "UnknownCampaign")]
    fn test_fund_campaign_rejects_unknown_campaign() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        client.fund_campaign(&99, &sponsor, &100);
    }

    #[test]
    #[should_panic(expected = "CampaignClosed")]
    fn test_fund_campaign_rejects_closed_campaign() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        // Force the campaign into the Completed state via direct storage access
        env.as_contract(&contract_id, || {
            let mut campaign: Campaign = env
                .storage()
                .persistent()
                .get(&DataKey::Campaign(id))
                .unwrap();
            campaign.status = CampaignStatus::Completed;
            env.storage()
                .persistent()
                .set(&DataKey::Campaign(id), &campaign);
        });

        client.fund_campaign(&id, &sponsor, &100);
    }

    #[test]
    fn test_fund_campaign_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "initialize",
                    args: (&admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "create_campaign",
                    args: (
                        symbol_short!("math"),
                        sponsor.clone(),
                        1_000_i128,
                        symbol_short!("gpa30"),
                        100u64,
                        200u64,
                    )
                        .into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_create_campaign(
                &symbol_short!("math"),
                &sponsor,
                &1_000_i128,
                &symbol_short!("gpa30"),
                &100u64,
                &200u64,
            )
            .unwrap()
            .unwrap();

        // Authorize only a non-admin attacker for `fund_campaign` — the admin's
        // authorization is missing, so the call must fail.
        let attacker = Address::generate(&env);
        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "fund_campaign",
                    args: (id, sponsor.clone(), 100_i128).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_fund_campaign(&id, &sponsor, &100_i128);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Issue #29 — Campaign lookup tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_campaign_ids_returns_created_ids() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.create_campaign(
            &symbol_short!("chem"),
            &sponsor,
            &2_000_i128,
            &symbol_short!("gpa25"),
            &100u64,
            &200u64,
        );

        let ids = client.get_campaign_ids();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), 1);
        assert_eq!(ids.get(1).unwrap(), 2);
        let _ = env;
    }

    #[test]
    fn test_get_campaign_ids_returns_empty_before_creation() {
        let (env, client, _admin) = setup();
        assert_eq!(client.get_campaign_ids().len(), 0);
        let _ = env;
    }

    #[test]
    fn test_lookup_does_not_mutate_state() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        let before = client.get_campaign(&id).unwrap();
        let _ = client.get_campaign(&id);
        let after = client.get_campaign(&id).unwrap();
        assert_eq!(before, after);
        let _ = env;
    }

    // -----------------------------------------------------------------------
    // Issue #31 — Campaign status management tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_campaign_status_active_to_paused() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        let status = client.update_campaign_status(&id, &CampaignStatus::Paused);
        assert_eq!(status, CampaignStatus::Paused);
        assert_eq!(
            client.get_campaign(&id).unwrap().status,
            CampaignStatus::Paused
        );
        let _ = env;
    }

    #[test]
    fn test_update_campaign_status_paused_to_active() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        client.update_campaign_status(&id, &CampaignStatus::Paused);
        let status = client.update_campaign_status(&id, &CampaignStatus::Active);
        assert_eq!(status, CampaignStatus::Active);
        let _ = env;
    }

    #[test]
    fn test_update_campaign_status_active_to_completed() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        let status = client.update_campaign_status(&id, &CampaignStatus::Completed);
        assert_eq!(status, CampaignStatus::Completed);
        let _ = env;
    }

    #[test]
    fn test_update_campaign_status_draft_to_active() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        // Force the campaign into the Draft state via direct storage access
        env.as_contract(&contract_id, || {
            let mut campaign: Campaign = env
                .storage()
                .persistent()
                .get(&DataKey::Campaign(id))
                .unwrap();
            campaign.status = CampaignStatus::Draft;
            env.storage()
                .persistent()
                .set(&DataKey::Campaign(id), &campaign);
        });

        let status = client.update_campaign_status(&id, &CampaignStatus::Active);
        assert_eq!(status, CampaignStatus::Active);
    }

    #[test]
    fn test_update_campaign_status_emits_event() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        client.update_campaign_status(&id, &CampaignStatus::Completed);

        let events = env.events().all();
        assert_eq!(events.len(), 1);

        let (_contract_id, topics, _data) = events.get(0).unwrap();
        let topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        assert_eq!(topic, symbol_short!("status"));
    }

    #[test]
    #[should_panic(expected = "InvalidTransition")]
    fn test_update_campaign_status_rejects_active_to_draft() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.update_campaign_status(&id, &CampaignStatus::Draft);
    }

    #[test]
    #[should_panic(expected = "InvalidTransition")]
    fn test_update_campaign_status_rejects_same_status() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.update_campaign_status(&id, &CampaignStatus::Active);
    }

    #[test]
    #[should_panic(expected = "InvalidTransition")]
    fn test_update_campaign_status_rejects_transition_from_completed() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.update_campaign_status(&id, &CampaignStatus::Completed);
        client.update_campaign_status(&id, &CampaignStatus::Active);
    }

    #[test]
    #[should_panic(expected = "UnknownCampaign")]
    fn test_update_campaign_status_rejects_unknown_campaign() {
        let (_env, client, _admin) = setup();
        client.update_campaign_status(&99, &CampaignStatus::Completed);
    }

    #[test]
    fn test_update_campaign_status_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "initialize",
                    args: (&admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = create_campaign_with_auth(&env, &contract_id, &client, &admin, &sponsor);

        // Authorize only a non-admin attacker for `update_campaign_status`
        let attacker = Address::generate(&env);
        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "update_campaign_status",
                    args: (id, CampaignStatus::Completed).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_update_campaign_status(&id, &CampaignStatus::Completed);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Issue #32 — Reward allocation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_campaign_reward_increases_allocated() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &800);

        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &300);

        assert_eq!(client.get_allocated(&id), 300);
        assert_eq!(client.get_available_pool(&id), 500);
        assert_eq!(client.get_student_allocation(&id, &student), 300);
        let _ = env;
    }

    #[test]
    fn test_record_campaign_reward_tracks_per_student() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &1_000);

        let student_a = Address::generate(&env);
        let student_b = Address::generate(&env);
        client.record_campaign_reward(&id, &student_a, &400);
        client.record_campaign_reward(&id, &student_b, &600);

        assert_eq!(client.get_allocated(&id), 1_000);
        assert_eq!(client.get_student_allocation(&id, &student_a), 400);
        assert_eq!(client.get_student_allocation(&id, &student_b), 600);
        let _ = env;
    }

    #[test]
    fn test_record_campaign_reward_emits_event() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &500);

        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &300);

        let events = env.events().all();
        assert_eq!(events.len(), 2);

        let (_contract_id, topics, data) = events.get(1).unwrap();
        let topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        assert_eq!(topic, symbol_short!("reward"));

        let vals: Vec<Val> = data.try_into_val(&env).unwrap();
        assert_eq!(vals.len(), 3);
        let emitted_campaign: u32 = vals.get(0).unwrap().try_into_val(&env).unwrap();
        let emitted_student: Address = vals.get(1).unwrap().try_into_val(&env).unwrap();
        let emitted_amount: i128 = vals.get(2).unwrap().try_into_val(&env).unwrap();
        assert_eq!(emitted_campaign, id);
        assert_eq!(emitted_student, student);
        assert_eq!(emitted_amount, 300);
    }

    #[test]
    #[should_panic(expected = "ZeroAmount")]
    fn test_record_campaign_reward_rejects_zero_amount() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &500);
        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &0);
    }

    #[test]
    #[should_panic(expected = "InsufficientPool")]
    fn test_record_campaign_reward_rejects_over_allocation() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &500);

        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &300);
        client.record_campaign_reward(&id, &student, &300);
    }

    #[test]
    #[should_panic(expected = "InsufficientPool")]
    fn test_record_campaign_reward_rejects_allocation_without_funding() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &100);
    }

    #[test]
    #[should_panic(expected = "UnknownCampaign")]
    fn test_record_campaign_reward_rejects_unknown_campaign() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);
        client.record_campaign_reward(&99, &student, &100);
    }

    #[test]
    #[should_panic(expected = "CampaignClosed")]
    fn test_record_campaign_reward_rejects_completed_campaign() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &500);

        // Force the campaign into the Completed state via direct storage access
        env.as_contract(&contract_id, || {
            let mut campaign: Campaign = env
                .storage()
                .persistent()
                .get(&DataKey::Campaign(id))
                .unwrap();
            campaign.status = CampaignStatus::Completed;
            env.storage()
                .persistent()
                .set(&DataKey::Campaign(id), &campaign);
        });

        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &100);
    }

    #[test]
    fn test_record_campaign_reward_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "initialize",
                    args: (&admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = create_campaign_with_auth(&env, &contract_id, &client, &admin, &sponsor);
        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "fund_campaign",
                    args: (id, sponsor.clone(), 500_i128).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .fund_campaign(&id, &sponsor, &500);

        // Authorize only a non-admin attacker for `record_campaign_reward`
        let attacker = Address::generate(&env);
        let student = Address::generate(&env);
        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "record_campaign_reward",
                    args: (id, student.clone(), 100_i128).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_record_campaign_reward(&id, &student, &100_i128);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Issue #33 — Criteria reference tests
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "EmptyCriteria")]
    fn test_create_campaign_rejects_empty_criteria_ref() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &Symbol::new(&env, ""),
            &100u64,
            &200u64,
        );
    }

    #[test]
    fn test_update_criteria_ref_success() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        client.update_criteria_ref(&id, &symbol_short!("gpa35"));
        assert_eq!(
            client.get_campaign(&id).unwrap().criteria_ref,
            symbol_short!("gpa35")
        );
        let _ = env;
    }

    #[test]
    #[should_panic(expected = "EmptyCriteria")]
    fn test_update_criteria_ref_rejects_empty() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.update_criteria_ref(&id, &Symbol::new(&env, ""));
    }

    #[test]
    #[should_panic(expected = "CampaignFinalized")]
    fn test_update_criteria_ref_rejects_finalized_campaign() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.update_campaign_status(&id, &CampaignStatus::Completed);
        client.update_criteria_ref(&id, &symbol_short!("gpa35"));
    }

    #[test]
    #[should_panic(expected = "UnknownCampaign")]
    fn test_update_criteria_ref_rejects_unknown_campaign() {
        let (_env, client, _admin) = setup();
        client.update_criteria_ref(&99, &symbol_short!("gpa35"));
    }

    #[test]
    fn test_update_criteria_ref_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CampaignVaultContract);
        let client = CampaignVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "initialize",
                    args: (&admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .initialize(&admin);

        let sponsor = Address::generate(&env);
        let id = create_campaign_with_auth(&env, &contract_id, &client, &admin, &sponsor);

        // Authorize only a non-admin attacker for `update_criteria_ref`
        let attacker = Address::generate(&env);
        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "update_criteria_ref",
                    args: (id, symbol_short!("gpa35")).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_update_criteria_ref(&id, &symbol_short!("gpa35"));
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Issue #34 — Campaign closeout summary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_campaign_summary_reports_funded_distributed_remaining() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &800);

        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &300);

        let summary = client.get_campaign_summary(&id);
        assert_eq!(summary.campaign_id, id);
        assert_eq!(summary.funded, 800);
        assert_eq!(summary.distributed, 300);
        assert_eq!(summary.remaining, 500);
        assert_eq!(summary.status, CampaignStatus::Active);
        let _ = env;
    }

    #[test]
    fn test_get_campaign_summary_reports_zero_amounts_for_unfunded() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );

        let summary = client.get_campaign_summary(&id);
        assert_eq!(summary.funded, 0);
        assert_eq!(summary.distributed, 0);
        assert_eq!(summary.remaining, 0);
        assert_eq!(summary.status, CampaignStatus::Active);
        let _ = env;
    }

    #[test]
    fn test_get_campaign_summary_reports_finalized_status() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &1_000);
        client.update_campaign_status(&id, &CampaignStatus::Completed);

        let summary = client.get_campaign_summary(&id);
        assert_eq!(summary.funded, 1_000);
        assert_eq!(summary.status, CampaignStatus::Completed);
        let _ = env;
    }

    #[test]
    fn test_get_campaign_summary_matches_individual_getters() {
        let (env, client, _admin) = setup();
        let sponsor = Address::generate(&env);
        let id = client.create_campaign(
            &symbol_short!("math"),
            &sponsor,
            &1_000_i128,
            &symbol_short!("gpa30"),
            &100u64,
            &200u64,
        );
        client.fund_campaign(&id, &sponsor, &750);

        let student = Address::generate(&env);
        client.record_campaign_reward(&id, &student, &250);

        let summary = client.get_campaign_summary(&id);
        assert_eq!(summary.funded, client.get_pool(&id));
        assert_eq!(summary.distributed, client.get_allocated(&id));
        assert_eq!(summary.remaining, client.get_available_pool(&id));
        let _ = env;
    }

    #[test]
    #[should_panic(expected = "UnknownCampaign")]
    fn test_get_campaign_summary_rejects_unknown_campaign() {
        let (_env, client, _admin) = setup();
        client.get_campaign_summary(&99);
    }

    /// Create a campaign as the admin using explicit `MockAuth`, returning its ID.
    fn create_campaign_with_auth(
        env: &Env,
        contract_id: &Address,
        client: &CampaignVaultContractClient<'static>,
        admin: &Address,
        sponsor: &Address,
    ) -> u32 {
        client
            .mock_auths(&[MockAuth {
                address: admin,
                invoke: &MockAuthInvoke {
                    contract: contract_id,
                    fn_name: "create_campaign",
                    args: (
                        symbol_short!("math"),
                        sponsor.clone(),
                        1_000_i128,
                        symbol_short!("gpa30"),
                        100u64,
                        200u64,
                    )
                        .into_val(env),
                    sub_invokes: &[],
                },
            }])
            .try_create_campaign(
                &symbol_short!("math"),
                sponsor,
                &1_000_i128,
                &symbol_short!("gpa30"),
                &100u64,
                &200u64,
            )
            .unwrap()
            .unwrap()
    }
}
