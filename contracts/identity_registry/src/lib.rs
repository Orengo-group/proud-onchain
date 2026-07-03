//! # Identity Registry Contract
//!
//! Manages the mapping between NFC UIDs and student on-chain identities.
//!
//! ## Responsibilities
//! - Initialize the contract with an admin address (one-time only)
//! - Register a student identity linked to a hashed NFC UID and Stellar wallet
//! - Reject duplicate student IDs, NFC hashes, and wallet addresses
//!
//! ## Storage Keys
//! - `DataKey::Initialized`            — `bool`  — guards one-time init
//! - `DataKey::Admin`                  — `Address` — contract administrator
//! - `DataKey::Student(student_id)`    — `StudentRecord` — per-student record
//! - `DataKey::NfcIndex(nfc_hash)`     — `BytesN<32>` (student_id) — reverse index
//! - `DataKey::WalletIndex(wallet)`    — `BytesN<32>` (student_id) — reverse index

#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, BytesN, Env,
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
    /// Per-student record, keyed by the unique student ID bytes.
    Student(BytesN<32>),
    /// Reverse index: NFC hash → student_id (for duplicate detection).
    NfcIndex(BytesN<32>),
    /// Reverse index: wallet address bytes → student_id (for duplicate detection).
    WalletIndex(Address),
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Represents a registered student identity on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StudentRecord {
    /// Unique student identifier (e.g. school-issued ID, hashed).
    pub student_id: BytesN<32>,
    /// SHA-256 (or equivalent) hash of the student's NFC UID.
    pub nfc_hash: BytesN<32>,
    /// The student's Stellar wallet address for reward distribution.
    pub wallet: Address,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct IdentityRegistryContract;

#[contractimpl]
impl IdentityRegistryContract {
    // -----------------------------------------------------------------------
    // Issue #11 — Initialization (one-time only)
    // -----------------------------------------------------------------------

    /// Initialize the contract with an admin address.
    ///
    /// Can only be called once. Subsequent calls will panic with `"AlreadyInit"`.
    ///
    /// # Arguments
    /// * `admin` — The address that will administrate this contract.
    pub fn initialize(env: Env, admin: Address) {
        // Guard: reject re-initialization
        if env
            .storage()
            .instance()
            .has(&DataKey::Initialized)
        {
            panic!("AlreadyInit");
        }

        // Require the caller to authenticate as admin
        admin.require_auth();

        // Persist state
        env.storage()
            .instance()
            .set(&DataKey::Initialized, &true);
        env.storage()
            .instance()
            .set(&DataKey::Admin, &admin);
    }

    /// Return the admin address stored during initialization.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"))
    }

    // -----------------------------------------------------------------------
    // Issue #12 — Student registration with duplicate checks
    // -----------------------------------------------------------------------

    /// Register a new student identity.
    ///
    /// Validates that the `student_id`, `nfc_hash`, and `wallet` are all unique
    /// before storing the record. Panics with a descriptive message on any
    /// duplicate or if the contract has not been initialized.
    ///
    /// # Arguments
    /// * `student_id` — Unique 32-byte student identifier.
    /// * `nfc_hash`   — 32-byte hash of the student's NFC UID.
    /// * `wallet`     — The student's Stellar wallet address.
    pub fn register_student(
        env: Env,
        student_id: BytesN<32>,
        nfc_hash: BytesN<32>,
        wallet: Address,
    ) {
        // Contract must be initialized first
        if !env
            .storage()
            .instance()
            .has(&DataKey::Initialized)
        {
            panic!("NotInit");
        }

        // Duplicate student_id check
        if env
            .storage()
            .persistent()
            .has(&DataKey::Student(student_id.clone()))
        {
            panic!("DuplicateStudentId");
        }

        // Duplicate NFC hash check
        if env
            .storage()
            .persistent()
            .has(&DataKey::NfcIndex(nfc_hash.clone()))
        {
            panic!("DuplicateNfcHash");
        }

        // Duplicate wallet check
        if env
            .storage()
            .persistent()
            .has(&DataKey::WalletIndex(wallet.clone()))
        {
            panic!("DuplicateWallet");
        }

        // Store the student record
        let record = StudentRecord {
            student_id: student_id.clone(),
            nfc_hash: nfc_hash.clone(),
            wallet: wallet.clone(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Student(student_id.clone()), &record);

        // Store reverse indices for duplicate detection
        env.storage()
            .persistent()
            .set(&DataKey::NfcIndex(nfc_hash), &student_id);
        env.storage()
            .persistent()
            .set(&DataKey::WalletIndex(wallet), &student_id);
    }

    /// Look up a student record by student ID.
    ///
    /// Returns `None` if no student with that ID has been registered.
    pub fn get_student(env: Env, student_id: BytesN<32>) -> Option<StudentRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Student(student_id))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

    /// Helper — build a BytesN<32> filled with a single byte value.
    fn make_id(env: &Env, val: u8) -> BytesN<32> {
        BytesN::from_array(env, &[val; 32])
    }

    // -----------------------------------------------------------------------
    // Issue #11 — Initialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_initialize_success() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Admin should be stored and retrievable
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    #[should_panic(expected = "AlreadyInit")]
    fn test_initialize_rejects_double_init() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        // Second call must panic
        client.initialize(&admin);
    }

    // -----------------------------------------------------------------------
    // Issue #12 — register_student success and failure tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_student_success() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0x01);
        let nfc_hash = make_id(&env, 0x02);
        let wallet = Address::generate(&env);

        client.register_student(&student_id, &nfc_hash, &wallet);

        let record = client.get_student(&student_id).unwrap();
        assert_eq!(record.student_id, student_id);
        assert_eq!(record.nfc_hash, nfc_hash);
        assert_eq!(record.wallet, wallet);
    }

    #[test]
    #[should_panic(expected = "DuplicateStudentId")]
    fn test_register_student_rejects_duplicate_student_id() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0xAA);

        // First registration — succeeds
        client.register_student(&student_id, &make_id(&env, 0x01), &Address::generate(&env));

        // Same student_id — must panic
        client.register_student(&student_id, &make_id(&env, 0x02), &Address::generate(&env));
    }

    #[test]
    #[should_panic(expected = "DuplicateNfcHash")]
    fn test_register_student_rejects_duplicate_nfc_hash() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let nfc_hash = make_id(&env, 0xBB);

        // First registration — succeeds
        client.register_student(&make_id(&env, 0x01), &nfc_hash, &Address::generate(&env));

        // Same NFC hash — must panic
        client.register_student(&make_id(&env, 0x02), &nfc_hash, &Address::generate(&env));
    }

    #[test]
    #[should_panic(expected = "DuplicateWallet")]
    fn test_register_student_rejects_duplicate_wallet() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);

        // First registration — succeeds
        client.register_student(&make_id(&env, 0x01), &make_id(&env, 0x01), &wallet);

        // Same wallet — must panic
        client.register_student(&make_id(&env, 0x02), &make_id(&env, 0x02), &wallet);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_register_student_rejects_when_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        // No initialize() call — must panic
        client.register_student(
            &make_id(&env, 0x01),
            &make_id(&env, 0x01),
            &Address::generate(&env),
        );
    }

    #[test]
    fn test_get_student_returns_none_for_unregistered() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let result = client.get_student(&make_id(&env, 0xFF));
        assert!(result.is_none());
    }

    #[test]
    fn test_multiple_students_can_be_registered() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Register three distinct students
        for i in 1u8..=3 {
            client.register_student(
                &make_id(&env, i),
                &make_id(&env, i + 10),
                &Address::generate(&env),
            );
        }

        // Each record should be retrievable
        for i in 1u8..=3 {
            let record = client.get_student(&make_id(&env, i)).unwrap();
            assert_eq!(record.student_id, make_id(&env, i));
            assert_eq!(record.nfc_hash, make_id(&env, i + 10));
        }
    }
}
