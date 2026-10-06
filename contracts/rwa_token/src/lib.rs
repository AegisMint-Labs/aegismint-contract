#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

/// Storage key types for the RWA Token contract
#[contracttype]
pub enum StorageKey {
    Admin,
    Name,
    Symbol,
    Decimals,
    TotalSupply,
    Balance(Address),
    Allowance(Address, Address),
    Metadata,
    Paused,
}

/// Token metadata
#[contracttype]
#[derive(Clone)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub metadata_uri: String,
}

#[contract]
pub struct RWAToken;

#[contractimpl]
impl RWAToken {
    /// Initialize the RWA token
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        total_supply: i128,
        metadata_uri: String,
    ) {
        if env.storage().instance().has(&StorageKey::Admin) {
            panic!("Contract already initialized");
        }

        admin.require_auth();

        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage().instance().set(&StorageKey::Name, &name);
        env.storage().instance().set(&StorageKey::Symbol, &symbol);
        env.storage().instance().set(&StorageKey::Decimals, &decimals);
        env.storage()
            .instance()
            .set(&StorageKey::TotalSupply, &total_supply);
        env.storage()
            .instance()
            .set(&StorageKey::Balance(admin.clone()), &total_supply);
        env.storage().instance().set(&StorageKey::Paused, &false);

        let metadata = TokenMetadata {
            name,
            symbol,
            decimals,
            metadata_uri,
        };
        env.storage().instance().set(&StorageKey::Metadata, &metadata);
    }

    /// Get token name
    pub fn name(env: Env) -> String {
        env.storage()
            .instance()
            .get(&StorageKey::Name)
            .expect("Not initialized")
    }

    /// Get token symbol
    pub fn symbol(env: Env) -> String {
        env.storage()
            .instance()
            .get(&StorageKey::Symbol)
            .expect("Not initialized")
    }

    /// Get token decimals
    pub fn decimals(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&StorageKey::Decimals)
            .expect("Not initialized")
    }

    /// Get total supply
    pub fn total_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&StorageKey::TotalSupply)
            .unwrap_or(0)
    }

    /// Get balance of an address
    pub fn balance(env: Env, address: Address) -> i128 {
        env.storage()
            .instance()
            .get(&StorageKey::Balance(address))
            .unwrap_or(0)
    }

    /// Transfer tokens
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        if Self::is_paused(env.clone()) {
            panic!("Token transfers are paused");
        }

        let from_balance = Self::balance(env.clone(), from.clone());
        if from_balance < amount {
            panic!("Insufficient balance");
        }

        let to_balance = Self::balance(env.clone(), to.clone());

        env.storage()
            .instance()
            .set(&StorageKey::Balance(from.clone()), &(from_balance - amount));
        env.storage()
            .instance()
            .set(&StorageKey::Balance(to.clone()), &(to_balance + amount));
    }

    /// Approve spending allowance
    pub fn approve(env: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();

        env.storage()
            .instance()
            .set(&StorageKey::Allowance(from, spender), &amount);
    }

    /// Get allowance
    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        env.storage()
            .instance()
            .get(&StorageKey::Allowance(from, spender))
            .unwrap_or(0)
    }

    /// Transfer from (using allowance)
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        if Self::is_paused(env.clone()) {
            panic!("Token transfers are paused");
        }

        let allowance = Self::allowance(env.clone(), from.clone(), spender.clone());
        if allowance < amount {
            panic!("Insufficient allowance");
        }

        let from_balance = Self::balance(env.clone(), from.clone());
        if from_balance < amount {
            panic!("Insufficient balance");
        }

        let to_balance = Self::balance(env.clone(), to.clone());

        env.storage()
            .instance()
            .set(&StorageKey::Balance(from.clone()), &(from_balance - amount));
        env.storage()
            .instance()
            .set(&StorageKey::Balance(to.clone()), &(to_balance + amount));
        env.storage().instance().set(
            &StorageKey::Allowance(from, spender),
            &(allowance - amount),
        );
    }

    /// Pause token transfers (admin only)
    pub fn pause(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        env.storage().instance().set(&StorageKey::Paused, &true);
    }

    /// Unpause token transfers (admin only)
    pub fn unpause(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        env.storage().instance().set(&StorageKey::Paused, &false);
    }

    /// Check if token is paused
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&StorageKey::Paused)
            .unwrap_or(false)
    }

    /// Get token metadata
    pub fn get_metadata(env: Env) -> TokenMetadata {
        env.storage()
            .instance()
            .get(&StorageKey::Metadata)
            .expect("Not initialized")
    }

    /// Get admin address
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Not initialized")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, RWAToken);
        let client = RWATokenClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(
            &admin,
            &String::from_str(&env, "Real Estate Token"),
            &String::from_str(&env, "RET"),
            &7,
            &1000000,
            &String::from_str(&env, "ipfs://metadata"),
        );

        assert_eq!(client.name(), String::from_str(&env, "Real Estate Token"));
        assert_eq!(client.symbol(), String::from_str(&env, "RET"));
        assert_eq!(client.decimals(), 7);
        assert_eq!(client.total_supply(), 1000000);
        assert_eq!(client.balance(&admin), 1000000);
    }

    #[test]
    fn test_transfer() {
        let env = Env::default();
        let contract_id = env.register_contract(None, RWAToken);
        let client = RWATokenClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(
            &admin,
            &String::from_str(&env, "Real Estate Token"),
            &String::from_str(&env, "RET"),
            &7,
            &1000000,
            &String::from_str(&env, "ipfs://metadata"),
        );

        client.transfer(&admin, &user, &100000);

        assert_eq!(client.balance(&admin), 900000);
        assert_eq!(client.balance(&user), 100000);
    }
}
