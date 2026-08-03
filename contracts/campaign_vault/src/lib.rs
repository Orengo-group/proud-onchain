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
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

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
}
