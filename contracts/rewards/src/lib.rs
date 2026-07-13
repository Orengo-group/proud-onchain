//! # Rewards Contract
//!
//! Handles rule-based reward distribution to students based on attendance
//! and academic performance thresholds.
//!
//! ## Responsibilities
//! - Admin-only minting of reward points to student wallets (issue #21)
//! - Batch minting to multiple students in one operation (issue #22)
//! - Reason codes for why a reward was issued (issue #23)
//! - Emit reward events for off-chain indexing
//! - Evaluate eligibility criteria (attendance %, grade thresholds)

#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracterror, contracttype, panic_with_error,
    symbol_short, Address, Env, Symbol, Vec,
};

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// Contract error codes returned when validation or authorization fails.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RewardError {
    /// Caller is not the stored admin.
    Unauthorized = 1,
    /// Mint amount must be greater than zero.
    ZeroAmount = 2,
    /// Batch list must not be empty.
    EmptyBatch = 3,
    /// Provided reason code is not recognised.
    UnknownReason = 4,
}

// ---------------------------------------------------------------------------
// Reward reason codes (issue #23)
// ---------------------------------------------------------------------------

/// Supported reasons for issuing a reward.
///
/// - `Attendance` – student met attendance requirements.
/// - `Academic`   – student achieved grade/performance threshold.
/// - `Event`      – student participated in a sponsored event.
///
/// Any value outside these three is rejected with [`RewardError::UnknownReason`].
#[contracttype]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RewardReason {
    Attendance,
    Academic,
    Event,
}

impl RewardReason {
    /// Parse a raw `u32` value into a [`RewardReason`].
    ///
    /// Uses the same numeric mapping as the public API:
    /// - `1` → `Attendance`
    /// - `2` → `Academic`
    /// - `3` → `Event`
    /// - anything else → `None`
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::Attendance),
            2 => Some(Self::Academic),
            3 => Some(Self::Event),
            _ => None,
        }
    }

    /// Numeric identifier used in events and external APIs.
    pub fn as_u32(self) -> u32 {
        match self {
            Self::Attendance => 1,
            Self::Academic => 2,
            Self::Event => 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
pub enum DataKey {
    /// Stores the admin [`Address`] set during `initialize`.
    Admin,
    /// Stores the reward point balance for a given student [`Address`].
    Balance(Address),
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct RewardsContract;

#[contractimpl]
impl RewardsContract {
    // -----------------------------------------------------------------------
    // Initialization
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
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"))
    }

    // -----------------------------------------------------------------------
    // Issue #21 — Admin-only reward minting
    // -----------------------------------------------------------------------

    /// Mint `amount` reward points to `recipient`.
    ///
    /// Only the contract admin may call this function. The mint amount must be
    /// greater than zero.
    ///
    /// # Arguments
    /// * `recipient` — Student wallet to credit.
    /// * `amount`    — Number of reward points to mint (must be > 0).
    /// * `reason`    — Reason the reward is being issued.
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"ZeroAmount"` if `amount` is zero or negative.
    pub fn mint_reward(env: Env, recipient: Address, amount: i128, reason: RewardReason) {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        if amount <= 0 {
            panic!("ZeroAmount");
        }

        let current: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(recipient.clone()))
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&DataKey::Balance(recipient.clone()), &(current + amount));

        // Emit reward event for off-chain indexing
        env.events().publish(
            (soroban_sdk::symbol_short!("reward"),),
            (recipient, amount, reason),
        );
    }

    // -----------------------------------------------------------------------
    // Issue #22 — Batch reward minting
    // -----------------------------------------------------------------------

    /// Mint reward points to multiple students in one transaction.
    ///
    /// `recipients` and `amounts` must be the same length. Only the admin may
    /// call this function. The batch must not be empty and no amount may be zero.
    ///
    /// # Arguments
    /// * `recipients` — Student wallets to credit.
    /// * `amounts`    — Corresponding reward amounts (each must be > 0).
    /// * `reason`     — Reason applied to the entire batch.
    ///
    /// # Panics
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"EmptyBatch"` if `recipients` is empty.
    /// - `"LengthMismatch"` if `recipients` and `amounts` differ in length.
    /// - `"ZeroAmount"` if any amount is zero or negative.
    pub fn batch_mint_rewards(
        env: Env,
        recipients: Vec<Address>,
        amounts: Vec<i128>,
        reason: RewardReason,
    ) {
        Self::assert_initialized(&env);
        Self::assert_admin(&env);

        if recipients.is_empty() {
            panic!("EmptyBatch");
        }

        if recipients.len() != amounts.len() {
            panic!("LengthMismatch");
        }

        for i in 0..recipients.len() {
            let recipient = recipients.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            if amount <= 0 {
                panic!("ZeroAmount");
            }

            let current: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(recipient.clone()))
                .unwrap_or(0);

            env.storage()
                .persistent()
                .set(&DataKey::Balance(recipient.clone()), &(current + amount));

            // Emit one event per recipient
            env.events().publish(
                (soroban_sdk::symbol_short!("reward"),),
                (recipient, amount, reason.clone()),
            );
        }
    }

    // -----------------------------------------------------------------------
    // Balance query
    // -----------------------------------------------------------------------

    /// Return the current reward point balance for `wallet`.
    ///
    /// Returns `0` if the wallet has never received any rewards.
    pub fn get_balance(env: Env, wallet: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(wallet))
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

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Panic with [`RewardError::Unauthorized`] if `caller` is not the stored admin.
    fn assert_admin(env: &Env, caller: &Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, RewardError::Unauthorized));
        if admin != *caller {
            panic_with_error!(env, RewardError::Unauthorized);
        }
    }

    /// Add `amount` to `student`'s balance and return the new total.
    fn add_balance(env: &Env, student: &Address, amount: i128) -> i128 {
        let current: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance(student.clone()))
            .unwrap_or(0);
        let new_balance = current + amount;
        env.storage()
            .instance()
            .set(&DataKey::Balance(student.clone()), &new_balance);
        new_balance
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    /// Set up a fresh initialized contract and return (env, client, admin).
    fn setup() -> (Env, RewardsContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, RewardsContract);
        let client = RewardsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        (env, client, admin)
    }

    // -----------------------------------------------------------------------
    // Initialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_initialize_success() {
        let (env, client, admin) = setup();
        assert_eq!(client.get_admin(), admin);
        let _ = env;
    }

    #[test]
    #[should_panic(expected = "AlreadyInit")]
    fn test_initialize_rejects_double_init() {
        let (env, client, admin) = setup();
        let _ = env;
        // Second call must panic
        client.initialize(&admin);
    }

    // -----------------------------------------------------------------------
    // Issue #21 — mint_reward tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mint_reward_increases_balance() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);

        assert_eq!(client.get_balance(&student), 0);
        client.mint_reward(&student, &100, &RewardReason::Attendance);
        assert_eq!(client.get_balance(&student), 100);
    }

    #[test]
    fn test_mint_reward_accumulates_balance() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);

        client.mint_reward(&student, &50, &RewardReason::Attendance);
        client.mint_reward(&student, &75, &RewardReason::Academic);
        client.mint_reward(&student, &25, &RewardReason::Event);
        assert_eq!(client.get_balance(&student), 150);
    }

    #[test]
    #[should_panic(expected = "ZeroAmount")]
    fn test_mint_reward_rejects_zero_amount() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);
        client.mint_reward(&student, &0, &RewardReason::Attendance);
    }

    #[test]
    #[should_panic(expected = "ZeroAmount")]
    fn test_mint_reward_rejects_negative_amount() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);
        client.mint_reward(&student, &-10, &RewardReason::Attendance);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_mint_reward_rejects_when_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RewardsContract);
        let client = RewardsContractClient::new(&env, &contract_id);
        let student = Address::generate(&env);
        client.mint_reward(&student, &10, &RewardReason::Attendance);
    }

    // -----------------------------------------------------------------------
    // Issue #22 — batch_mint_rewards tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_mint_rewards_success() {
        let (env, client, _admin) = setup();

        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);

        let recipients = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);
        let amounts = Vec::from_array(&env, [100_i128, 200_i128, 50_i128]);

        client.batch_mint_rewards(&recipients, &amounts, &RewardReason::Attendance);

        assert_eq!(client.get_balance(&s1), 100);
        assert_eq!(client.get_balance(&s2), 200);
        assert_eq!(client.get_balance(&s3), 50);
    }

    #[test]
    fn test_batch_mint_rewards_accumulates_existing_balance() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);

        // Pre-existing balance
        client.mint_reward(&student, &10, &RewardReason::Academic);

        let recipients = Vec::from_array(&env, [student.clone()]);
        let amounts = Vec::from_array(&env, [90_i128]);
        client.batch_mint_rewards(&recipients, &amounts, &RewardReason::Event);

        assert_eq!(client.get_balance(&student), 100);
    }

    #[test]
    #[should_panic(expected = "EmptyBatch")]
    fn test_batch_mint_rewards_rejects_empty_batch() {
        let (env, client, _admin) = setup();
        let recipients: Vec<Address> = Vec::new(&env);
        let amounts: Vec<i128> = Vec::new(&env);
        client.batch_mint_rewards(&recipients, &amounts, &RewardReason::Attendance);
    }

    #[test]
    #[should_panic(expected = "LengthMismatch")]
    fn test_batch_mint_rewards_rejects_mismatched_lengths() {
        let (env, client, _admin) = setup();
        let s1 = Address::generate(&env);
        let recipients = Vec::from_array(&env, [s1.clone()]);
        let amounts = Vec::from_array(&env, [10_i128, 20_i128]);
        client.batch_mint_rewards(&recipients, &amounts, &RewardReason::Attendance);
    }

    #[test]
    #[should_panic(expected = "ZeroAmount")]
    fn test_batch_mint_rewards_rejects_zero_amount() {
        let (env, client, _admin) = setup();
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let recipients = Vec::from_array(&env, [s1.clone(), s2.clone()]);
        let amounts = Vec::from_array(&env, [50_i128, 0_i128]);
        client.batch_mint_rewards(&recipients, &amounts, &RewardReason::Attendance);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_batch_mint_rewards_rejects_when_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RewardsContract);
        let client = RewardsContractClient::new(&env, &contract_id);
        let s1 = Address::generate(&env);
        let recipients = Vec::from_array(&env, [s1.clone()]);
        let amounts = Vec::from_array(&env, [10_i128]);
        client.batch_mint_rewards(&recipients, &amounts, &RewardReason::Attendance);
    }

    // -----------------------------------------------------------------------
    // Issue #23 — Reward reason codes tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mint_reward_with_attendance_reason() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);
        client.mint_reward(&student, &10, &RewardReason::Attendance);
        assert_eq!(client.get_balance(&student), 10);
    }

    #[test]
    fn test_mint_reward_with_academic_reason() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);
        client.mint_reward(&student, &20, &RewardReason::Academic);
        assert_eq!(client.get_balance(&student), 20);
    }

    #[test]
    fn test_mint_reward_with_event_reason() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);
        client.mint_reward(&student, &30, &RewardReason::Event);
        assert_eq!(client.get_balance(&student), 30);
    }

    #[test]
    fn test_batch_mint_with_all_reasons() {
        let (env, client, _admin) = setup();

        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);

        // Batch with Attendance
        client.batch_mint_rewards(
            &Vec::from_array(&env, [s1.clone()]),
            &Vec::from_array(&env, [15_i128]),
            &RewardReason::Attendance,
        );
        // Batch with Academic
        client.batch_mint_rewards(
            &Vec::from_array(&env, [s1.clone()]),
            &Vec::from_array(&env, [25_i128]),
            &RewardReason::Academic,
        );
        // Batch with Event
        client.batch_mint_rewards(
            &Vec::from_array(&env, [s2.clone()]),
            &Vec::from_array(&env, [40_i128]),
            &RewardReason::Event,
        );

        assert_eq!(client.get_balance(&s1), 40);
        assert_eq!(client.get_balance(&s2), 40);
    }

    // -----------------------------------------------------------------------
    // get_balance test
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_balance_returns_zero_for_unknown_wallet() {
        let (env, client, _admin) = setup();
        let unknown = Address::generate(&env);
        assert_eq!(client.get_balance(&unknown), 0);
    }
}
