//! Uniqueness, remove/reinsert semantics, and deterministic error regression tests.
//!
//! Covers issue #255: enforce registry uniqueness, define remove/reinsert
//! semantics, and expose deterministic errors for duplicates.

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CredenceRegistry, ());
    let admin = Address::generate(&env);
    CredenceRegistryClient::new(&env, &contract_id).initialize(&admin);
    (env, contract_id, admin)
}

fn client(env: &Env, contract_id: &Address) -> CredenceRegistryClient<'_> {
    CredenceRegistryClient::new(env, contract_id)
}

// ── uniqueness: duplicate identity ───────────────────────────────────────────

/// Registering the same identity twice yields error #400.
#[test]
#[should_panic(expected = "Error(Contract, #400)")]
fn duplicate_identity_yields_error_400() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    c.register(&id, &Address::generate(&env), &true);
    c.register(&id, &Address::generate(&env), &true);
}

/// try_register returns Err for duplicate identity (non-panicking path).
#[test]
fn duplicate_identity_try_register_returns_err() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    c.register(&id, &Address::generate(&env), &true);
    let result = c.try_register(&id, &Address::generate(&env), &true);
    assert!(result.is_err(), "duplicate identity must return Err");
}

/// Deactivated identity still blocks re-registration (storage key still present).
#[test]
#[should_panic(expected = "Error(Contract, #400)")]
fn deactivated_identity_blocks_reregister() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    let bond = Address::generate(&env);
    c.register(&id, &bond, &true);
    c.deactivate(&id);
    // Must still fail — deactivate is a soft delete, not a remove
    c.register(&id, &Address::generate(&env), &true);
}

// ── uniqueness: duplicate bond contract ──────────────────────────────────────

/// Registering the same bond contract for two identities yields error #401.
#[test]
#[should_panic(expected = "Error(Contract, #401)")]
fn duplicate_bond_contract_yields_error_401() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let bond = Address::generate(&env);
    c.register(&Address::generate(&env), &bond, &true);
    c.register(&Address::generate(&env), &bond, &true);
}

/// try_register returns Err for duplicate bond contract.
#[test]
fn duplicate_bond_contract_try_register_returns_err() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let bond = Address::generate(&env);
    c.register(&Address::generate(&env), &bond, &true);
    let result = c.try_register(&Address::generate(&env), &bond, &true);
    assert!(result.is_err(), "duplicate bond contract must return Err");
}

// ── remove semantics ──────────────────────────────────────────────────────────

/// remove clears the forward mapping — get_bond_contract panics after removal.
#[test]
#[should_panic(expected = "Error(Contract, #402)")]
fn remove_clears_forward_mapping() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    c.register(&id, &Address::generate(&env), &true);
    c.remove(&id);
    c.get_bond_contract(&id); // must panic #402
}

/// remove clears the reverse mapping — get_identity panics after removal.
#[test]
#[should_panic(expected = "Error(Contract, #403)")]
fn remove_clears_reverse_mapping() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    let bond = Address::generate(&env);
    c.register(&id, &bond, &true);
    c.remove(&id);
    c.get_identity(&bond); // must panic #403
}

/// is_registered returns false after remove.
#[test]
fn remove_makes_is_registered_false() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    c.register(&id, &Address::generate(&env), &true);
    assert!(c.is_registered(&id));
    c.remove(&id);
    assert!(!c.is_registered(&id));
}

/// remove shrinks the identities list by one.
#[test]
fn remove_shrinks_identities_list() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id1 = Address::generate(&env);
    let id2 = Address::generate(&env);
    c.register(&id1, &Address::generate(&env), &true);
    c.register(&id2, &Address::generate(&env), &true);
    assert_eq!(c.get_all_identities().len(), 2);
    c.remove(&id1);
    assert_eq!(c.get_all_identities().len(), 1);
    assert!(!c.get_all_identities().iter().any(|a| a == id1));
}

/// remove on a non-existent identity yields error #402.
#[test]
#[should_panic(expected = "Error(Contract, #402)")]
fn remove_nonexistent_yields_error_402() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    c.remove(&Address::generate(&env));
}

// ── reinsert semantics ────────────────────────────────────────────────────────

/// After remove, the same identity can be re-registered (reinsert).
#[test]
fn reinsert_after_remove_succeeds() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    let bond1 = Address::generate(&env);
    let bond2 = Address::generate(&env);

    c.register(&id, &bond1, &true);
    c.remove(&id);
    // Re-register with a different bond contract
    let entry = c.register(&id, &bond2, &true);
    assert_eq!(entry.identity, id);
    assert_eq!(entry.bond_contract, bond2);
    assert!(entry.active);
}

/// After remove, the same bond contract can be re-registered with a new identity.
#[test]
fn reinsert_same_bond_contract_after_remove_succeeds() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id1 = Address::generate(&env);
    let id2 = Address::generate(&env);
    let bond = Address::generate(&env);

    c.register(&id1, &bond, &true);
    c.remove(&id1);
    let entry = c.register(&id2, &bond, &true);
    assert_eq!(entry.bond_contract, bond);
    assert_eq!(entry.identity, id2);
}

/// Reinserted entry appears in get_all_identities exactly once.
#[test]
fn reinsert_appears_once_in_identities_list() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);

    c.register(&id, &Address::generate(&env), &true);
    c.remove(&id);
    c.register(&id, &Address::generate(&env), &true);

    let all = c.get_all_identities();
    let count = all.iter().filter(|a| *a == id).count();
    assert_eq!(count, 1, "reinserted identity must appear exactly once");
}

/// Reinserted entry has a fresh registered_at timestamp.
#[test]
fn reinsert_has_fresh_timestamp() {
    let (env, cid, _) = setup();
    env.ledger().with_mut(|li| li.timestamp = 1_000);
    let c = client(&env, &cid);
    let id = Address::generate(&env);

    c.register(&id, &Address::generate(&env), &true);
    let first_ts = c.get_bond_contract(&id).registered_at;

    c.remove(&id);
    env.ledger().with_mut(|li| li.timestamp = 9_000);
    c.register(&id, &Address::generate(&env), &true);
    let second_ts = c.get_bond_contract(&id).registered_at;

    assert!(
        second_ts > first_ts,
        "reinserted entry must have a newer timestamp"
    );
}

// ── regression: deactivate vs remove distinction ─────────────────────────────

/// deactivate keeps the entry; remove deletes it entirely.
#[test]
fn deactivate_keeps_entry_remove_deletes_it() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);

    let id_soft = Address::generate(&env);
    let id_hard = Address::generate(&env);
    c.register(&id_soft, &Address::generate(&env), &true);
    c.register(&id_hard, &Address::generate(&env), &true);

    c.deactivate(&id_soft);
    c.remove(&id_hard);

    // Soft-deleted entry still retrievable
    let entry = c.get_bond_contract(&id_soft);
    assert!(
        !entry.active,
        "deactivated entry must still exist but be inactive"
    );

    // Hard-deleted entry is gone
    let result = c.try_get_bond_contract(&id_hard);
    assert!(result.is_err(), "removed entry must not be retrievable");
}

/// remove on a deactivated identity succeeds (hard-delete of soft-deleted entry).
#[test]
fn remove_deactivated_identity_succeeds() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);
    let id = Address::generate(&env);
    c.register(&id, &Address::generate(&env), &true);
    c.deactivate(&id);
    c.remove(&id); // must not panic
    assert!(!c.is_registered(&id));
    assert!(c.try_get_bond_contract(&id).is_err());
}

// ── regression: error codes are deterministic ────────────────────────────────

/// Error codes are stable wire values — table-driven check.
#[test]
fn error_codes_are_deterministic() {
    let (env, cid, _) = setup();
    let c = client(&env, &cid);

    // #400 IdentityAlreadyRegistered
    let id = Address::generate(&env);
    c.register(&id, &Address::generate(&env), &true);
    let err = c
        .try_register(&id, &Address::generate(&env), &true)
        .unwrap_err();
    assert!(
        format!("{err:?}").contains("400"),
        "IdentityAlreadyRegistered must be #400"
    );

    // #401 BondContractAlreadyRegistered
    let bond = Address::generate(&env);
    c.register(&Address::generate(&env), &bond, &true);
    let err = c
        .try_register(&Address::generate(&env), &bond, &true)
        .unwrap_err();
    assert!(
        format!("{err:?}").contains("401"),
        "BondContractAlreadyRegistered must be #401"
    );

    // #402 IdentityNotRegistered
    let err = c
        .try_get_bond_contract(&Address::generate(&env))
        .unwrap_err();
    assert!(
        format!("{err:?}").contains("402"),
        "IdentityNotRegistered must be #402"
    );

    // #403 BondContractNotRegistered
    let err = c.try_get_identity(&Address::generate(&env)).unwrap_err();
    assert!(
        format!("{err:?}").contains("403"),
        "BondContractNotRegistered must be #403"
    );

    // #404 AlreadyDeactivated
    let id2 = Address::generate(&env);
    c.register(&id2, &Address::generate(&env), &true);
    c.deactivate(&id2);
    let err = c.try_deactivate(&id2).unwrap_err();
    assert!(
        format!("{err:?}").contains("404"),
        "AlreadyDeactivated must be #404"
    );

    // #405 AlreadyActive
    let id3 = Address::generate(&env);
    c.register(&id3, &Address::generate(&env), &true);
    let err = c.try_reactivate(&id3).unwrap_err();
    assert!(
        format!("{err:?}").contains("405"),
        "AlreadyActive must be #405"
    );
}
