//! # Achievement Certificate Contract
//!
//! Issues verifiable on-chain certificates for student academic achievements.
//!
//! ## Responsibilities
//! - Initialize the contract with an admin address (one-time only)
//! - Issue achievement certificates as on-chain records
//! - Allow public verification of certificates by student address
//! - Restrict issuance to the admin set at initialization
//!
//! ## Storage Keys
//! - `DataKey::Initialized`              — `bool`  — guards one-time init
//! - `DataKey::Admin`                    — `Address` — contract administrator
//! - `DataKey::Achievement(u64)`         — `AchievementRecord` — per-achievement record
//! - `DataKey::AchievementIndex(u64)`    — `u64` — unique achievement ID counter
//! - `DataKey::StudentAchievements(Address)` — `Vec<u64>` — list of achievement IDs per student

#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

// ---------------------------------------------------------------------------
// Storage key enum
// ---------------------------------------------------------------------------

/// All persistent storage keys used by this contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Whether `initialize` has already been called.
    Initialized,
    /// The admin address set during initialization.
    Admin,
    /// Achievement ID counter (auto-incrementing).
    AchievementIdCounter,
    /// Per-achievement record, keyed by the unique achievement ID.
    Achievement(u64),
    /// List of achievement IDs for a given student wallet.
    StudentAchievements(Address),
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Represents an issued achievement certificate on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AchievementRecord {
    /// Unique auto-incrementing achievement ID.
    pub achievement_id: u64,
    /// The student's Stellar wallet address.
    pub student: Address,
    /// Title of the achievement (e.g. "Best in Mathematics").
    pub title: Symbol,
    /// Category of the achievement (e.g. "academic", "extracurricular").
    pub category: Symbol,
    /// SHA-256 hash of off-chain metadata (e.g. certificate PDF).
    pub metadata_hash: BytesN<32>,
    /// Timestamp (ledger close time) when the achievement was issued.
    pub issued_at: u64,
    /// Whether this achievement has been revoked by an admin.
    pub revoked: bool,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct AchievementCertificateContract;

#[contractimpl]
impl AchievementCertificateContract {
    // -----------------------------------------------------------------------
    // Issue #36 — Initialization (one-time only)
    // -----------------------------------------------------------------------

    /// Initialize the contract with an admin address.
    ///
    /// Can only be called once. Subsequent calls will panic with `"AlreadyInit"`.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("AlreadyInit");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AchievementIdCounter, &0u64);
    }

    /// Return the admin address stored during initialization.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"))
    }

    // -----------------------------------------------------------------------
    // Issue #37 — Achievement issuance (admin only)
    // -----------------------------------------------------------------------

    /// Issue a new achievement certificate to a student.
    ///
    /// Only the contract admin may issue achievements. Each issued achievement
    /// is assigned a unique auto-incrementing ID. An event is emitted with the
    /// student wallet and achievement ID.
    ///
    /// Panics:
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"EmptyTitle"` if the title is empty.
    pub fn issue_achievement(
        env: Env,
        student: Address,
        title: Symbol,
        category: Symbol,
        metadata_hash: BytesN<32>,
    ) -> u64 {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();

        if title == symbol_short!("") {
            panic!("EmptyTitle");
        }

        // Increment the achievement ID counter
        let achievement_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AchievementIdCounter)
            .unwrap_or(0u64);
        let next_id = achievement_id + 1;
        env.storage()
            .instance()
            .set(&DataKey::AchievementIdCounter, &next_id);

        // Record the achievement
        let issued_at = env.ledger().sequence() as u64;
        let record = AchievementRecord {
            achievement_id: next_id,
            student: student.clone(),
            title: title.clone(),
            category: category.clone(),
            metadata_hash: metadata_hash.clone(),
            issued_at,
            revoked: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Achievement(next_id), &record);

        // Update the student's achievement list
        let mut student_achievements: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::StudentAchievements(student.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        student_achievements.push_back(next_id);
        env.storage().persistent().set(
            &DataKey::StudentAchievements(student.clone()),
            &student_achievements,
        );

        // Emit event
        env.events().publish(
            (symbol_short!("achieve"),),
            (next_id, student, title, category, metadata_hash, issued_at),
        );

        next_id
    }

    // -----------------------------------------------------------------------
    // Read functions
    // -----------------------------------------------------------------------

    /// Retrieve an achievement record by its ID.
    ///
    /// Returns `Some(AchievementRecord)` if found, or `None` if no achievement
    /// with that ID exists.
    pub fn get_achievement(env: Env, achievement_id: u64) -> Option<AchievementRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Achievement(achievement_id))
    }

    /// Retrieve all achievement IDs for a given student wallet.
    pub fn get_student_achievements(env: Env, student: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::StudentAchievements(student))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // -----------------------------------------------------------------------
    // Issue #40 — Achievement revocation (admin only)
    // -----------------------------------------------------------------------

    /// Revoke an issued achievement certificate.
    ///
    /// Only the contract admin may revoke achievements. The achievement record
    /// remains readable after revocation but its `revoked` flag is set to
    /// `true`. An event is emitted with the revoked achievement ID.
    ///
    /// Panics:
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"AchievementNotFound"` if the achievement ID does not exist.
    /// - `"AlreadyRevoked"` if the achievement has already been revoked.
    pub fn revoke_achievement(env: Env, achievement_id: u64) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();

        let mut record: AchievementRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(achievement_id))
            .unwrap_or_else(|| panic!("AchievementNotFound"));

        if record.revoked {
            panic!("AlreadyRevoked");
        }

        record.revoked = true;
        env.storage()
            .persistent()
            .set(&DataKey::Achievement(achievement_id), &record);

        // Emit revocation event
        env.events().publish(
            (symbol_short!("revoke"),),
            (achievement_id, record.student, record.title),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        Address, BytesN, Env, IntoVal,
    };

    /// Helper — build a BytesN<32> filled with a single byte value.
    fn make_hash(env: &Env, val: u8) -> BytesN<32> {
        BytesN::from_array(env, &[val; 32])
    }

    // -----------------------------------------------------------------------
    // Issue #36 — Initialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_initialize_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    #[should_panic(expected = "AlreadyInit")]
    fn test_initialize_rejects_double_init() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.initialize(&admin);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_get_admin_rejects_uninitialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        client.get_admin();
    }

    // -----------------------------------------------------------------------
    // Issue #37 — issue_achievement tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_issue_achievement_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("BestMath");
        let category = symbol_short!("academic");
        let metadata_hash = make_hash(&env, 0xAA);

        let id = client.issue_achievement(&student, &title, &category, &metadata_hash);
        assert_eq!(id, 1);

        let record = client.get_achievement(&id).unwrap();
        assert_eq!(record.achievement_id, 1);
        assert_eq!(record.student, student);
        assert_eq!(record.title, title);
        assert_eq!(record.category, category);
        assert_eq!(record.metadata_hash, metadata_hash);
        assert_eq!(record.issued_at, env.ledger().sequence() as u64);
    }

    #[test]
    fn test_issue_multiple_achievements_increment_ids() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id1 = client.issue_achievement(&student, &title, &category, &hash);
        let id2 = client.issue_achievement(&student, &title, &category, &hash);
        let id3 = client.issue_achievement(&student, &title, &category, &hash);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_issue_achievement_stored_in_student_list() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id1 = client.issue_achievement(&student, &title, &category, &hash);
        let id2 = client.issue_achievement(&student, &title, &category, &hash);

        let achievements = client.get_student_achievements(&student);
        assert_eq!(achievements.len(), 2);
        assert_eq!(achievements.get_unchecked(0), id1);
        assert_eq!(achievements.get_unchecked(1), id2);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_issue_achievement_rejects_uninitialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        client.issue_achievement(&student, &title, &category, &hash);
    }

    #[test]
    #[should_panic(expected = "HostError")]
    fn test_issue_achievement_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
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

        // Attempt to issue as a non-admin — must fail
        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "issue_achievement",
                    args: (
                        student.clone(),
                        title.clone(),
                        category.clone(),
                        hash.clone(),
                    )
                        .into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .issue_achievement(&student, &title, &category, &hash);
    }

    #[test]
    #[should_panic(expected = "EmptyTitle")]
    fn test_issue_achievement_rejects_empty_title() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let empty_title = Symbol::new(&env, "");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        client.issue_achievement(&student, &empty_title, &category, &hash);
    }

    // -----------------------------------------------------------------------
    // Issue #38 — get_achievement tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_achievement_returns_none_for_unknown() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        assert!(client.get_achievement(&999).is_none());
    }

    #[test]
    fn test_get_achievement_returns_existing_record() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("TopSci");
        let category = symbol_short!("academic");
        let metadata_hash = make_hash(&env, 0xBB);

        let id = client.issue_achievement(&student, &title, &category, &metadata_hash);

        let record = client.get_achievement(&id).unwrap();
        assert_eq!(record.achievement_id, id);
        assert_eq!(record.student, student);
        assert_eq!(record.title, title);
        assert_eq!(record.category, category);
        assert_eq!(record.metadata_hash, metadata_hash);
        assert_eq!(record.issued_at, env.ledger().sequence() as u64);
    }

    #[test]
    fn test_get_achievement_returns_correct_record_for_each_id() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_a = Address::generate(&env);
        let student_b = Address::generate(&env);
        let title1 = symbol_short!("Math");
        let title2 = symbol_short!("Science");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id1 = client.issue_achievement(&student_a, &title1, &category, &hash);
        let id2 = client.issue_achievement(&student_b, &title2, &category, &hash);

        let record1 = client.get_achievement(&id1).unwrap();
        let record2 = client.get_achievement(&id2).unwrap();

        assert_eq!(record1.student, student_a);
        assert_eq!(record1.title, title1);
        assert_eq!(record2.student, student_b);
        assert_eq!(record2.title, title2);
    }

    #[test]
    fn test_get_achievement_does_not_mutate_state() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Physics");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0xCC);

        let id = client.issue_achievement(&student, &title, &category, &hash);

        // Lookup before — snapshot the result
        let before = client.get_achievement(&id).unwrap();
        let student_achievements_before = client.get_student_achievements(&student);

        // Call get_achievement multiple times
        let _ = client.get_achievement(&id);
        let _ = client.get_achievement(&id);
        let _ = client.get_achievement(&0);

        // Verify state is unchanged
        let after = client.get_achievement(&id).unwrap();
        let student_achievements_after = client.get_student_achievements(&student);

        assert_eq!(before, after);
        assert_eq!(
            student_achievements_before.len(),
            student_achievements_after.len()
        );
        assert_eq!(
            student_achievements_before.get_unchecked(0),
            student_achievements_after.get_unchecked(0)
        );
    }

    #[test]
    fn test_get_achievement_returns_none_for_id_zero() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        assert!(client.get_achievement(&0).is_none());
    }

    // -----------------------------------------------------------------------
    // Issue #39 — get_student_achievements tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_student_achievements_returns_empty_for_unknown() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let student = Address::generate(&env);
        let achievements = client.get_student_achievements(&student);
        assert_eq!(achievements.len(), 0);
    }

    #[test]
    fn test_get_student_achievements_returns_ids_in_order() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id1 = client.issue_achievement(&student, &title, &category, &hash);
        let id2 = client.issue_achievement(&student, &title, &category, &hash);
        let id3 = client.issue_achievement(&student, &title, &category, &hash);

        let achievements = client.get_student_achievements(&student);
        assert_eq!(achievements.len(), 3);
        assert_eq!(achievements.get_unchecked(0), id1);
        assert_eq!(achievements.get_unchecked(1), id2);
        assert_eq!(achievements.get_unchecked(2), id3);
    }

    #[test]
    fn test_issue_achievement_updates_student_index() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        // Initially empty
        assert_eq!(client.get_student_achievements(&student).len(), 0);

        // Issue first achievement — index should grow to 1
        let id1 = client.issue_achievement(&student, &title, &category, &hash);
        let achievements = client.get_student_achievements(&student);
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements.get_unchecked(0), id1);

        // Issue second achievement — index should grow to 2
        let id2 = client.issue_achievement(&student, &title, &category, &hash);
        let achievements = client.get_student_achievements(&student);
        assert_eq!(achievements.len(), 2);
        assert_eq!(achievements.get_unchecked(1), id2);
    }

    #[test]
    fn test_multiple_students_separate_achievement_lists() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_a = Address::generate(&env);
        let student_b = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        client.issue_achievement(&student_a, &title, &category, &hash);
        client.issue_achievement(&student_a, &title, &category, &hash);
        client.issue_achievement(&student_b, &title, &category, &hash);

        let a_achievements = client.get_student_achievements(&student_a);
        let b_achievements = client.get_student_achievements(&student_b);

        assert_eq!(a_achievements.len(), 2);
        assert_eq!(b_achievements.len(), 1);
    }

    #[test]
    fn test_get_student_achievements_consistent_across_calls() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        client.issue_achievement(&student, &title, &category, &hash);
        client.issue_achievement(&student, &title, &category, &hash);

        let first_call = client.get_student_achievements(&student);
        let second_call = client.get_student_achievements(&student);

        assert_eq!(first_call.len(), second_call.len());
        assert_eq!(first_call.get_unchecked(0), second_call.get_unchecked(0));
        assert_eq!(first_call.get_unchecked(1), second_call.get_unchecked(1));
    }

    // -----------------------------------------------------------------------
    // Issue #40 — revoke_achievement tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_revoke_achievement_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("BestMath");
        let category = symbol_short!("academic");
        let metadata_hash = make_hash(&env, 0xAA);

        let id = client.issue_achievement(&student, &title, &category, &metadata_hash);
        let record_before = client.get_achievement(&id).unwrap();
        assert!(!record_before.revoked);

        client.revoke_achievement(&id);

        let record_after = client.get_achievement(&id).unwrap();
        assert!(record_after.revoked);
        assert_eq!(record_after.achievement_id, id);
        assert_eq!(record_after.student, student);
        assert_eq!(record_after.title, title);
        assert_eq!(record_after.category, category);
        assert_eq!(record_after.metadata_hash, metadata_hash);
    }

    #[test]
    #[should_panic(expected = "HostError")]
    fn test_revoke_achievement_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
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

        // Issue an achievement as admin
        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id = client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "issue_achievement",
                    args: (
                        student.clone(),
                        title.clone(),
                        category.clone(),
                        hash.clone(),
                    )
                        .into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .issue_achievement(&student, &title, &category, &hash);

        // Attempt to revoke as a non-admin — must fail
        client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "revoke_achievement",
                    args: (id,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .revoke_achievement(&id);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_revoke_achievement_rejects_uninitialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        client.revoke_achievement(&1);
    }

    #[test]
    #[should_panic(expected = "AchievementNotFound")]
    fn test_revoke_achievement_rejects_unknown_id() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        client.revoke_achievement(&999);
    }

    #[test]
    #[should_panic(expected = "AlreadyRevoked")]
    fn test_revoke_achievement_rejects_double_revoke() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id = client.issue_achievement(&student, &title, &category, &hash);
        client.revoke_achievement(&id);
        client.revoke_achievement(&id);
    }

    #[test]
    fn test_revoked_achievement_remains_readable() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Physics");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0xBB);

        let id = client.issue_achievement(&student, &title, &category, &hash);
        client.revoke_achievement(&id);

        // Record is still accessible and contains all original data
        let record = client.get_achievement(&id).unwrap();
        assert_eq!(record.achievement_id, id);
        assert_eq!(record.student, student);
        assert_eq!(record.title, title);
        assert_eq!(record.category, category);
        assert_eq!(record.metadata_hash, hash);
        assert_eq!(record.issued_at, env.ledger().sequence() as u64);
        assert!(record.revoked);

        // Student's achievement list is unchanged
        let student_achievements = client.get_student_achievements(&student);
        assert_eq!(student_achievements.len(), 1);
        assert_eq!(student_achievements.get_unchecked(0), id);
    }

    #[test]
    fn test_revocation_does_not_affect_other_achievements() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCertificateContract);
        let client = AchievementCertificateContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student = Address::generate(&env);
        let title = symbol_short!("Award");
        let category = symbol_short!("academic");
        let hash = make_hash(&env, 0x01);

        let id1 = client.issue_achievement(&student, &title, &category, &hash);
        let id2 = client.issue_achievement(&student, &title, &category, &hash);
        let id3 = client.issue_achievement(&student, &title, &category, &hash);

        client.revoke_achievement(&id2);

        let record1 = client.get_achievement(&id1).unwrap();
        let record2 = client.get_achievement(&id2).unwrap();
        let record3 = client.get_achievement(&id3).unwrap();

        assert!(!record1.revoked);
        assert!(record2.revoked);
        assert!(!record3.revoked);
    }
}
