//! # Identity Registry Contract
//!
//! Manages the mapping between NFC UIDs and student on-chain identities.
//!
//! ## Responsibilities
//! - Initialize the contract with an admin address (one-time only)
//! - Register a student identity linked to a hashed NFC UID and Stellar wallet (#17)
//! - Reject duplicate student IDs, NFC hashes, and wallet addresses
//! - Lookup student by student ID (#13) or by NFC hash (#14)
//! - Update a student's wallet address with admin authorization (#15, #17)
//! - Deactivate and reactivate student records without data loss (#16, #17)
//! - Restrict all write operations to the admin set at initialization (#17)
//!
//! ## Storage Keys
//! - `DataKey::Initialized`            — `bool`  — guards one-time init
//! - `DataKey::Admin`                  — `Address` — contract administrator
//! - `DataKey::Student(student_id)`    — `StudentRecord` — per-student record
//! - `DataKey::NfcIndex(nfc_hash)`     — `BytesN<32>` (student_id) — reverse index
//! - `DataKey::WalletIndex(wallet)`    — `BytesN<32>` (student_id) — reverse index

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

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
    /// Reverse index: NFC hash → student_id (for duplicate detection and lookup).
    NfcIndex(BytesN<32>),
    /// Reverse index: wallet address → student_id (for duplicate detection).
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
    /// Whether this student record is active. Defaults to true on registration.
    pub active: bool,
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
    // Issue #12 — Student registration with duplicate checks (#17: admin only)
    // -----------------------------------------------------------------------

    /// Register a new student identity.
    ///
    /// Only the contract admin may register students. Validates that the
    /// `student_id`, `nfc_hash`, and `wallet` are all unique before storing the
    /// record. The record's `active` field is set to `true`.
    ///
    /// Panics:
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"DuplicateStudentId"` if the student ID is already registered.
    /// - `"DuplicateNfcHash"` if the NFC hash is already registered.
    /// - `"DuplicateWallet"` if the wallet is already registered.
    pub fn register_student(
        env: Env,
        student_id: BytesN<32>,
        nfc_hash: BytesN<32>,
        wallet: Address,
    ) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }

        // Only admin may register students (#17)
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();

        if env
            .storage()
            .persistent()
            .has(&DataKey::Student(student_id.clone()))
        {
            panic!("DuplicateStudentId");
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::NfcIndex(nfc_hash.clone()))
        {
            panic!("DuplicateNfcHash");
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::WalletIndex(wallet.clone()))
        {
            panic!("DuplicateWallet");
        }

        let record = StudentRecord {
            student_id: student_id.clone(),
            nfc_hash: nfc_hash.clone(),
            wallet: wallet.clone(),
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Student(student_id.clone()), &record);
        env.storage()
            .persistent()
            .set(&DataKey::NfcIndex(nfc_hash), &student_id);
        env.storage()
            .persistent()
            .set(&DataKey::WalletIndex(wallet), &student_id);
    }

    // -----------------------------------------------------------------------
    // Issue #13 — Student lookup by student ID
    // -----------------------------------------------------------------------

    /// Look up a student record by student ID.
    ///
    /// Returns `Some(StudentRecord)` if found, or `None` if no student with
    /// that ID has been registered. Does not mutate state.
    pub fn get_student(env: Env, student_id: BytesN<32>) -> Option<StudentRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Student(student_id))
    }

    // -----------------------------------------------------------------------
    // Issue #14 — Student lookup by NFC hash
    // -----------------------------------------------------------------------

    /// Look up a student record by their NFC hash.
    ///
    /// Uses the `NfcIndex` reverse mapping to resolve the NFC hash to a
    /// student ID, then returns the full `StudentRecord`. Raw NFC UIDs are
    /// never stored; only the pre-hashed value is accepted.
    ///
    /// Returns `Some(StudentRecord)` if the hash resolves to a student,
    /// or `None` if the hash is unknown.
    pub fn get_student_by_nfc_hash(env: Env, nfc_hash: BytesN<32>) -> Option<StudentRecord> {
        let student_id: Option<BytesN<32>> =
            env.storage().persistent().get(&DataKey::NfcIndex(nfc_hash));

        match student_id {
            None => None,
            Some(sid) => env.storage().persistent().get(&DataKey::Student(sid)),
        }
    }

    // -----------------------------------------------------------------------
    // Issue #15 — Update student wallet address (admin only)
    // -----------------------------------------------------------------------

    /// Update the wallet address for an existing student.
    ///
    /// Only the contract admin can call this function. The new wallet must not
    /// already be registered to another student. The old wallet index is
    /// removed and a new one is created.
    ///
    /// Panics:
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"StudentNotFound"` if no student with that ID exists.
    /// - `"DuplicateWallet"` if the new wallet is already in use.
    pub fn update_student_wallet(env: Env, student_id: BytesN<32>, new_wallet: Address) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }

        // Only admin may update wallets
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();

        // Load existing record
        let mut record: StudentRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Student(student_id.clone()))
            .unwrap_or_else(|| panic!("StudentNotFound"));

        // New wallet must not belong to another student
        if env
            .storage()
            .persistent()
            .has(&DataKey::WalletIndex(new_wallet.clone()))
        {
            panic!("DuplicateWallet");
        }

        // Remove old wallet index
        env.storage()
            .persistent()
            .remove(&DataKey::WalletIndex(record.wallet.clone()));

        // Update record and write new wallet index
        record.wallet = new_wallet.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Student(student_id), &record);
        env.storage()
            .persistent()
            .set(&DataKey::WalletIndex(new_wallet), &record.student_id);
    }

    // -----------------------------------------------------------------------
    // Issue #16 — Deactivate / reactivate student (admin only)
    // -----------------------------------------------------------------------

    /// Deactivate a student record without deleting historical data.
    ///
    /// Only the contract admin may deactivate a student. The record is
    /// preserved with `active` set to `false`. All lookup functions continue
    /// to return the record, including the `active` flag.
    ///
    /// Panics:
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"StudentNotFound"` if no student with that ID exists.
    pub fn deactivate_student(env: Env, student_id: BytesN<32>) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();

        let mut record: StudentRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Student(student_id.clone()))
            .unwrap_or_else(|| panic!("StudentNotFound"));

        record.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Student(student_id), &record);
    }

    /// Reactivate a previously deactivated student record.
    ///
    /// Only the contract admin may reactivate a student. Sets `active` back
    /// to `true` on the existing record.
    ///
    /// Panics:
    /// - `"NotInit"` if the contract has not been initialized.
    /// - `"Unauthorized"` if the caller is not the admin.
    /// - `"StudentNotFound"` if no student with that ID exists.
    pub fn reactivate_student(env: Env, student_id: BytesN<32>) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("NotInit");
        }

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("NotInit"));
        admin.require_auth();

        let mut record: StudentRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Student(student_id.clone()))
            .unwrap_or_else(|| panic!("StudentNotFound"));

        record.active = true;
        env.storage()
            .persistent()
            .set(&DataKey::Student(student_id), &record);
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
        client.initialize(&admin);
    }

    // -----------------------------------------------------------------------
    // Issue #12 — register_student tests
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
        assert!(record.active);
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
        client.register_student(&student_id, &make_id(&env, 0x01), &Address::generate(&env));
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
        client.register_student(&make_id(&env, 0x01), &nfc_hash, &Address::generate(&env));
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
        client.register_student(&make_id(&env, 0x01), &make_id(&env, 0x01), &wallet);
        client.register_student(&make_id(&env, 0x02), &make_id(&env, 0x02), &wallet);
    }

    #[test]
    #[should_panic(expected = "NotInit")]
    fn test_register_student_rejects_when_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        client.register_student(
            &make_id(&env, 0x01),
            &make_id(&env, 0x01),
            &Address::generate(&env),
        );
    }

    // -----------------------------------------------------------------------
    // Issue #17 — Admin authorization tests
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "HostError")]
    fn test_register_student_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
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

        // Authorize only a non-admin for `register_student` — the admin's
        // authorization is missing, so the call must fail.
        let student_id = make_id(&env, 0x77);
        let nfc_hash = make_id(&env, 0x78);
        let wallet = Address::generate(&env);
        client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "register_student",
                    args: (student_id.clone(), nfc_hash.clone(), wallet.clone()).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .register_student(&student_id, &nfc_hash, &wallet);
    }

    #[test]
    fn test_register_student_succeeds_as_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let student_id = make_id(&env, 0x66);
        let nfc_hash = make_id(&env, 0x67);
        let wallet = Address::generate(&env);

        // Authorize the admin for `initialize`, then for `register_student`
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

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "register_student",
                    args: (student_id.clone(), nfc_hash.clone(), wallet.clone()).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .register_student(&student_id, &nfc_hash, &wallet);

        let record = client.get_student(&student_id).unwrap();
        assert_eq!(record.wallet, wallet);
    }

    #[test]
    #[should_panic(expected = "HostError")]
    fn test_update_wallet_rejects_non_admin() {
        let env = Env::default();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        // Authorize the admin for `initialize` and `register_student`
        let student_id = make_id(&env, 0x68);
        let nfc_hash = make_id(&env, 0x69);
        let wallet = Address::generate(&env);
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

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "register_student",
                    args: (student_id.clone(), nfc_hash.clone(), wallet.clone()).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .register_student(&student_id, &nfc_hash, &wallet);

        // Attempt a wallet update authorized by a non-admin — must fail
        let new_wallet = Address::generate(&env);
        client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "update_student_wallet",
                    args: (student_id.clone(), new_wallet.clone()).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .update_student_wallet(&student_id, &new_wallet);
    }

    #[test]
    fn test_multiple_students_can_be_registered() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        for i in 1u8..=3 {
            client.register_student(
                &make_id(&env, i),
                &make_id(&env, i + 10),
                &Address::generate(&env),
            );
        }
        for i in 1u8..=3 {
            let record = client.get_student(&make_id(&env, i)).unwrap();
            assert_eq!(record.student_id, make_id(&env, i));
            assert_eq!(record.nfc_hash, make_id(&env, i + 10));
        }
    }

    // -----------------------------------------------------------------------
    // Issue #13 — get_student tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_student_returns_none_for_unregistered() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        assert!(client.get_student(&make_id(&env, 0xFF)).is_none());
    }

    #[test]
    fn test_get_student_returns_correct_record() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0x10);
        let nfc_hash = make_id(&env, 0x20);
        let wallet = Address::generate(&env);
        client.register_student(&student_id, &nfc_hash, &wallet);

        let record = client.get_student(&student_id).unwrap();
        assert_eq!(record.student_id, student_id);
        assert_eq!(record.nfc_hash, nfc_hash);
        assert_eq!(record.wallet, wallet);
        assert!(record.active);
    }

    // -----------------------------------------------------------------------
    // Issue #14 — get_student_by_nfc_hash tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_student_by_nfc_hash_returns_correct_record() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0x30);
        let nfc_hash = make_id(&env, 0x40);
        let wallet = Address::generate(&env);
        client.register_student(&student_id, &nfc_hash, &wallet);

        let record = client.get_student_by_nfc_hash(&nfc_hash).unwrap();
        assert_eq!(record.student_id, student_id);
        assert_eq!(record.nfc_hash, nfc_hash);
        assert_eq!(record.wallet, wallet);
    }

    #[test]
    fn test_get_student_by_nfc_hash_returns_none_for_unknown_hash() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert!(client
            .get_student_by_nfc_hash(&make_id(&env, 0x99))
            .is_none());
    }

    // -----------------------------------------------------------------------
    // Issue #15 — update_student_wallet tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_student_wallet_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0x50);
        let nfc_hash = make_id(&env, 0x51);
        let old_wallet = Address::generate(&env);
        client.register_student(&student_id, &nfc_hash, &old_wallet);

        let new_wallet = Address::generate(&env);
        client.update_student_wallet(&student_id, &new_wallet);

        let record = client.get_student(&student_id).unwrap();
        assert_eq!(record.wallet, new_wallet);
    }

    #[test]
    fn test_update_wallet_frees_old_wallet_for_reuse() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let s1 = make_id(&env, 0x60);
        let old_wallet = Address::generate(&env);
        client.register_student(&s1, &make_id(&env, 0x61), &old_wallet);

        let new_wallet = Address::generate(&env);
        client.update_student_wallet(&s1, &new_wallet);

        // old_wallet is now free — another student can register with it
        let s2 = make_id(&env, 0x62);
        client.register_student(&s2, &make_id(&env, 0x63), &old_wallet);
        let record2 = client.get_student(&s2).unwrap();
        assert_eq!(record2.wallet, old_wallet);
    }

    #[test]
    #[should_panic(expected = "DuplicateWallet")]
    fn test_update_wallet_rejects_wallet_already_in_use() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet_a = Address::generate(&env);
        let wallet_b = Address::generate(&env);
        client.register_student(&make_id(&env, 0x70), &make_id(&env, 0x71), &wallet_a);
        client.register_student(&make_id(&env, 0x72), &make_id(&env, 0x73), &wallet_b);

        // Try to update student 1 to use wallet_b (already taken)
        client.update_student_wallet(&make_id(&env, 0x70), &wallet_b);
    }

    #[test]
    #[should_panic(expected = "StudentNotFound")]
    fn test_update_wallet_rejects_unknown_student() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.update_student_wallet(&make_id(&env, 0xDE), &Address::generate(&env));
    }

    // -----------------------------------------------------------------------
    // Issue #16 — deactivate_student / reactivate_student tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_deactivate_student_sets_active_false() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0x80);
        client.register_student(&student_id, &make_id(&env, 0x81), &Address::generate(&env));

        // Initially active
        assert!(client.get_student(&student_id).unwrap().active);

        client.deactivate_student(&student_id);
        assert!(!client.get_student(&student_id).unwrap().active);
    }

    #[test]
    fn test_reactivate_student_sets_active_true() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0x90);
        client.register_student(&student_id, &make_id(&env, 0x91), &Address::generate(&env));

        client.deactivate_student(&student_id);
        assert!(!client.get_student(&student_id).unwrap().active);

        client.reactivate_student(&student_id);
        assert!(client.get_student(&student_id).unwrap().active);
    }

    #[test]
    fn test_deactivate_preserves_student_data() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let student_id = make_id(&env, 0xA0);
        let nfc_hash = make_id(&env, 0xA1);
        let wallet = Address::generate(&env);
        client.register_student(&student_id, &nfc_hash, &wallet);
        client.deactivate_student(&student_id);

        // Record still fully accessible after deactivation
        let record = client.get_student(&student_id).unwrap();
        assert_eq!(record.student_id, student_id);
        assert_eq!(record.nfc_hash, nfc_hash);
        assert_eq!(record.wallet, wallet);
        assert!(!record.active);
    }

    #[test]
    #[should_panic(expected = "StudentNotFound")]
    fn test_deactivate_rejects_unknown_student() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.deactivate_student(&make_id(&env, 0xEE));
    }

    #[test]
    #[should_panic(expected = "StudentNotFound")]
    fn test_reactivate_rejects_unknown_student() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, IdentityRegistryContract);
        let client = IdentityRegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.reactivate_student(&make_id(&env, 0xEF));
    }
}
