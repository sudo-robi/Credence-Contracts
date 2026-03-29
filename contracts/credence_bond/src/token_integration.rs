//! USDC token integration helpers for Credence Bond.
//! Centralizes token configuration, allowance checks, and transfer operations.
//! Now uses safe token operations for consistent error handling.

use crate::DataKey;
use crate::safe_token;
use soroban_sdk::{Address, Env, String, Symbol};

/// Stellar network passphrase label used for USDC mainnet references.
pub const STELLAR_MAINNET: &str = "mainnet";

/// Stellar network passphrase label used for USDC testnet references.
pub const STELLAR_TESTNET: &str = "testnet";

fn network_key(e: &Env) -> Symbol {
    Symbol::new(e, "usdc_net")
}


/// @notice Sets the token contract used by bond operations.
/// @dev Requires admin auth and stores token in instance storage.
pub fn set_token(e: &Env, admin: &Address, token: &Address) {
    let stored_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic!("not initialized"));
    admin.require_auth();
    if *admin != stored_admin {
        panic!("not admin");
    }
    e.storage().instance().set(&DataKey::BondToken, token);
}

/// @notice Sets the USDC token contract and associated network label.
/// @dev Network label is informational for auditing and can be "mainnet" or "testnet".
pub fn set_usdc_token(e: &Env, admin: &Address, token: &Address, network: &String) {
    if *network != String::from_str(e, STELLAR_MAINNET)
        && *network != String::from_str(e, STELLAR_TESTNET)
    {
        panic!("unsupported stellar network");
    }
    set_token(e, admin, token);
    e.storage().instance().set(&network_key(e), network);
    e.events().publish(
        (Symbol::new(e, "usdc_token_set"),),
        (token.clone(), network.clone()),
    );
}

/// @notice Returns the configured token address.
/// @dev Panics if token has not been configured.
pub fn get_token(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::BondToken)
        .unwrap_or_else(|| panic!("token not set"))
}

/// @notice Returns the configured USDC network label if set.
pub fn get_usdc_network(e: &Env) -> Option<String> {
    e.storage().instance().get(&network_key(e))
}

/// @notice Checks if owner has enough allowance for the contract to spend amount.
/// @dev Uses safe allowance checking with proper error handling.
pub fn require_allowance(e: &Env, owner: &Address, amount: i128) {
    safe_token::safe_require_allowance(e, owner, amount);
}

/// @notice Transfers tokens from owner into the bond contract.
/// @dev Uses safe transfer with proper validation and error handling.
pub fn transfer_into_contract(e: &Env, owner: &Address, amount: i128) {
    safe_token::safe_transfer_from(e, owner, amount);
}

/// @notice Transfers tokens from the bond contract to recipient.
/// @dev Uses safe transfer for standard withdrawals and penalty/treasury transfers.
pub fn transfer_from_contract(e: &Env, recipient: &Address, amount: i128) {
    safe_token::safe_transfer(e, recipient, amount);
}
