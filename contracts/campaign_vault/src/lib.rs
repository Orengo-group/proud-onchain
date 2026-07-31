//! # Campaign Vault Contract
//!
//! Holds sponsor-funded reward pools for specific educational campaigns.
//!
//! ## Responsibilities
//! - Accept sponsor deposits tied to a campaign ID
//! - Release funds to the Rewards contract when criteria are met
//! - Return unspent funds to sponsors after campaign expiry
//! - Admin initialization and campaign storage types (issue #27)

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

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
}

// ---------------------------------------------------------------------------
// Campaign types
// ---------------------------------------------------------------------------

/// Lifecycle state of a sponsor campaign.
#[contracttype]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CampaignStatus {
    /// Campaign is open for funding.
    Active,
    /// Campaign is closed; no further funding is accepted.
    Closed,
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
    pub fn get_campaign(env: Env, campaign_id: u32) -> Option<Campaign> {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
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

        // Force the campaign into the Closed state via direct storage access
        env.as_contract(&contract_id, || {
            let mut campaign: Campaign = env
                .storage()
                .persistent()
                .get(&DataKey::Campaign(id))
                .unwrap();
            campaign.status = CampaignStatus::Closed;
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
}
