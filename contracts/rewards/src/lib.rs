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

    /// Store the admin address.  Must be called once before any minting.
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    // -----------------------------------------------------------------------
    // Issue #21 – admin-only mint_reward
    // -----------------------------------------------------------------------

    /// Mint `amount` reward points to `student`.
    ///
    /// ### Authorization
    /// Only the stored admin may call this function.
    ///
    /// ### Validation
    /// - `amount` must be > 0 (returns [`RewardError::ZeroAmount`]).
    /// - `reason` must be 1 (Attendance), 2 (Academic), or 3 (Event)
    ///   (returns [`RewardError::UnknownReason`] otherwise).
    ///
    /// ### Side-effects
    /// - Increases the student's persistent balance.
    /// - Emits a `"reward"` event `(student, amount, reason_code)`.
    ///
    /// Returns the student's new balance after minting.
    pub fn mint_reward(
        env: Env,
        admin: Address,
        student: Address,
        amount: i128,
        reason: u32,
    ) -> i128 {
        // Authorization
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        // Validate amount
        if amount <= 0 {
            panic_with_error!(&env, RewardError::ZeroAmount);
        }

        // Validate reason code
        let reason_code = RewardReason::from_u32(reason)
            .unwrap_or_else(|| panic_with_error!(&env, RewardError::UnknownReason));

        // Update balance
        let new_balance = Self::add_balance(&env, &student, amount);

        // Emit event: topic = ("reward", student), data = (amount, reason_u32)
        env.events().publish(
            (symbol_short!("reward"), student.clone()),
            (amount, reason_code.as_u32()),
        );

        new_balance
    }

    // -----------------------------------------------------------------------
    // Issue #22 – batch_mint_rewards
    // -----------------------------------------------------------------------

    /// Mint rewards to multiple students in one atomic call.
    ///
    /// `recipients` is a [`Vec`] of `(student_address, amount, reason_u32)` tuples,
    /// where `reason_u32` follows the same mapping as [`mint_reward`]:
    /// `1` = Attendance, `2` = Academic, `3` = Event.
    ///
    /// ### Authorization
    /// Only the stored admin may call this function.
    ///
    /// ### Validation
    /// - Batch must not be empty ([`RewardError::EmptyBatch`]).
    /// - Each amount must be > 0 ([`RewardError::ZeroAmount`]).
    /// - Each reason must be a valid code ([`RewardError::UnknownReason`]).
    ///
    /// ### Atomicity (all-or-nothing)
    /// All entries are validated **before** any state is written.
    /// If any entry is invalid the entire call fails and no balances are updated.
    ///
    /// Returns a [`Vec`] of new balances in the same order as `recipients`.
    pub fn batch_mint_rewards(
        env: Env,
        admin: Address,
        recipients: Vec<(Address, i128, u32)>,
    ) -> Vec<i128> {
        // Authorization
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        // Batch must not be empty
        if recipients.is_empty() {
            panic_with_error!(&env, RewardError::EmptyBatch);
        }

        // Validation pass – ensures all-or-nothing atomicity
        for entry in recipients.iter() {
            let (_, amount, reason) = entry;
            if amount <= 0 {
                panic_with_error!(&env, RewardError::ZeroAmount);
            }
            if RewardReason::from_u32(reason).is_none() {
                panic_with_error!(&env, RewardError::UnknownReason);
            }
        }

        // Write pass – only reached when every entry is valid
        let mut new_balances: Vec<i128> = Vec::new(&env);
        for entry in recipients.iter() {
            let (student, amount, reason) = entry;
            let new_balance = Self::add_balance(&env, &student, amount);
            env.events().publish(
                (symbol_short!("reward"), student.clone()),
                (amount, reason),
            );
            new_balances.push_back(new_balance);
        }

        new_balances
    }

    // -----------------------------------------------------------------------
    // Read helpers
    // -----------------------------------------------------------------------

    /// Return the reward point balance for `student` (0 if not yet set).
    pub fn balance(env: Env, student: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Balance(student))
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Legacy placeholder – kept for backwards-compatibility
    // -----------------------------------------------------------------------

    /// Placeholder: kept so existing integrations do not break.
    pub fn distribute(_env: Env) -> Symbol {
        symbol_short!("ok")
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

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Vec};

    /// Helper: deploy a fresh contract and return (env, client, admin, student).
    fn setup() -> (Env, RewardsContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RewardsContract);
        let client = RewardsContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let student = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin, student)
    }

    // -----------------------------------------------------------------------
    // Legacy placeholder
    // -----------------------------------------------------------------------

    #[test]
    fn test_distribute_placeholder() {
        let env = Env::default();
        let contract_id = env.register_contract(None, RewardsContract);
        let client = RewardsContractClient::new(&env, &contract_id);
        assert_eq!(client.distribute(), symbol_short!("ok"));
    }

    // -----------------------------------------------------------------------
    // Issue #21 – mint_reward
    // -----------------------------------------------------------------------

    #[test]
    fn test_mint_reward_success() {
        let (_env, client, admin, student) = setup();
        let bal = client.mint_reward(&admin, &student, &100, &1 /* Attendance */);
        assert_eq!(bal, 100);
        assert_eq!(client.balance(&student), 100);
    }

    #[test]
    fn test_mint_reward_accumulates_balance() {
        let (_env, client, admin, student) = setup();
        client.mint_reward(&admin, &student, &50, &2 /* Academic */);
        let bal = client.mint_reward(&admin, &student, &75, &3 /* Event */);
        assert_eq!(bal, 125);
        assert_eq!(client.balance(&student), 125);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")] // ZeroAmount
    fn test_mint_reward_zero_amount_rejected() {
        let (_env, client, admin, student) = setup();
        client.mint_reward(&admin, &student, &0, &1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")] // ZeroAmount
    fn test_mint_reward_negative_amount_rejected() {
        let (_env, client, admin, student) = setup();
        client.mint_reward(&admin, &student, &-10, &1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")] // Unauthorized
    fn test_mint_reward_non_admin_rejected() {
        let (env, client, _admin, student) = setup();
        let non_admin = Address::generate(&env);
        client.mint_reward(&non_admin, &student, &100, &1);
    }

    // -----------------------------------------------------------------------
    // Issue #22 – batch_mint_rewards
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_mint_multiple_students() {
        let (env, client, admin, student1) = setup();
        let student2 = Address::generate(&env);
        let mut batch = Vec::new(&env);
        batch.push_back((student1.clone(), 200_i128, 1_u32));
        batch.push_back((student2.clone(), 300_i128, 2_u32));
        let balances = client.batch_mint_rewards(&admin, &batch);
        assert_eq!(balances.get(0).unwrap(), 200);
        assert_eq!(balances.get(1).unwrap(), 300);
        assert_eq!(client.balance(&student1), 200);
        assert_eq!(client.balance(&student2), 300);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")] // EmptyBatch
    fn test_batch_mint_empty_batch_rejected() {
        let (env, client, admin, _student) = setup();
        let batch: Vec<(Address, i128, u32)> = Vec::new(&env);
        client.batch_mint_rewards(&admin, &batch);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")] // ZeroAmount
    fn test_batch_mint_zero_amount_rejected() {
        let (env, client, admin, student) = setup();
        let mut batch = Vec::new(&env);
        batch.push_back((student.clone(), 0_i128, 1_u32));
        client.batch_mint_rewards(&admin, &batch);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")] // Unauthorized
    fn test_batch_mint_non_admin_rejected() {
        let (env, client, _admin, student) = setup();
        let non_admin = Address::generate(&env);
        let mut batch = Vec::new(&env);
        batch.push_back((student.clone(), 100_i128, 1_u32));
        client.batch_mint_rewards(&non_admin, &batch);
    }

    // -----------------------------------------------------------------------
    // Issue #23 – reward reason codes
    // -----------------------------------------------------------------------

    #[test]
    fn test_reason_attendance() {
        let (_env, client, admin, student) = setup();
        let bal = client.mint_reward(&admin, &student, &10, &(RewardReason::Attendance.as_u32()));
        assert_eq!(bal, 10);
    }

    #[test]
    fn test_reason_academic() {
        let (_env, client, admin, student) = setup();
        let bal = client.mint_reward(&admin, &student, &20, &(RewardReason::Academic.as_u32()));
        assert_eq!(bal, 20);
    }

    #[test]
    fn test_reason_event() {
        let (_env, client, admin, student) = setup();
        let bal = client.mint_reward(&admin, &student, &30, &(RewardReason::Event.as_u32()));
        assert_eq!(bal, 30);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")] // UnknownReason
    fn test_unknown_reason_rejected() {
        let (_env, client, admin, student) = setup();
        client.mint_reward(&admin, &student, &50, &99); // 99 is not a valid reason
    }

    #[test]
    fn test_batch_mint_all_three_reasons() {
        let (env, client, admin, student1) = setup();
        let student2 = Address::generate(&env);
        let student3 = Address::generate(&env);
        let mut batch = Vec::new(&env);
        batch.push_back((student1.clone(), 10_i128, RewardReason::Attendance.as_u32()));
        batch.push_back((student2.clone(), 20_i128, RewardReason::Academic.as_u32()));
        batch.push_back((student3.clone(), 30_i128, RewardReason::Event.as_u32()));
        let balances = client.batch_mint_rewards(&admin, &batch);
        assert_eq!(balances.get(0).unwrap(), 10);
        assert_eq!(balances.get(1).unwrap(), 20);
        assert_eq!(balances.get(2).unwrap(), 30);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")] // UnknownReason
    fn test_batch_mint_unknown_reason_rejected() {
        let (env, client, admin, student) = setup();
        let mut batch = Vec::new(&env);
        batch.push_back((student.clone(), 50_i128, 99_u32)); // invalid reason
        client.batch_mint_rewards(&admin, &batch);
    }
}
