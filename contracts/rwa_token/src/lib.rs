#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    Address, Env, String, Symbol
};

/// Persistent storage TTL threshold in ledgers (172,800 ledgers ~ 10 days at 5s/ledger)
pub const STORAGE_TTL_THRESHOLD: u32 = 172_800;
/// Persistent storage TTL extension amount in ledgers (172,800 ledgers ~ 10 days at 5s/ledger)
pub const STORAGE_TTL_EXTEND_TO: u32 = 172_800;

/// Error types for the RWA token contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract is already initialized
    AlreadyInitialized = 1,
    /// Caller is not authorized to perform this action
    Unauthorized = 2,
    /// Account is not whitelisted for transfers
    NotWhitelisted = 3,
    /// Insufficient balance for the operation
    InsufficientBalance = 4,
    /// Transfer amount is invalid (zero or negative)
    InvalidAmount = 5,
    /// Account is already whitelisted
    AlreadyWhitelisted = 6,
    /// Account is not currently whitelisted
    NotCurrentlyWhitelisted = 7,
}

/// Allowance identifier key linking token owner and approved spender
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceKey {
    pub owner: Address,
    pub spender: Address,
}

/// Token state metadata structure (name, symbol, decimals)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
}

/// Storage keys for the RWA token contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract administrator address
    Admin,
    /// Token state metadata (name, symbol, decimals)
    State,
    /// Individual account balance: Balance(account_address)
    Balance(Address),
    /// Transfer allowance: Allowance(AllowanceKey)
    Allowance(AllowanceKey),
    /// Whitelist compliance status: Whitelist(account_address) -> bool
    Whitelist(Address),
    /// Legacy allowance pair for compatibility
    AllowancePair(Address, Address),
    /// Legacy whitelisted key for compatibility
    Whitelisted(Address),
    /// Legacy token metadata: name
    Name,
    /// Legacy token metadata: symbol
    Symbol,
    /// Legacy token metadata: decimals
    Decimals,
    /// Total supply of tokens
    TotalSupply,
    /// Contract initialization status
    Initialized,
}

/// Token metadata structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub total_supply: i128,
    pub admin: Address,
}

/// RWA Token Contract
#[contract]
pub struct RwaTokenContract;

#[contractimpl]
impl RwaTokenContract {
    /// Initialize the RWA token contract
    /// Can only be called once and sets up the token metadata and initial supply
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        total_supply: i128,
    ) -> Result<(), Error> {
        // Check if already initialized
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        // Require admin authorization
        admin.require_auth();

        // Store token state metadata
        let token_state = TokenState {
            name: name.clone(),
            symbol: symbol.clone(),
            decimals,
        };
        env.storage().instance().set(&DataKey::State, &token_state);

        // Store individual configuration keys in instance storage
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage().instance().set(&DataKey::Decimals, &decimals);
        env.storage().instance().set(&DataKey::TotalSupply, &total_supply);
        env.storage().instance().set(&DataKey::Initialized, &true);

        // Set initial balance to admin in persistent storage and extend TTL
        let admin_bal_key = DataKey::Balance(admin.clone());
        env.storage().persistent().set(&admin_bal_key, &total_supply);
        env.storage().persistent().extend_ttl(&admin_bal_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Automatically whitelist the admin
        let admin_wl_key = DataKey::Whitelist(admin.clone());
        env.storage().persistent().set(&admin_wl_key, &true);
        env.storage().persistent().extend_ttl(&admin_wl_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let legacy_wl_key = DataKey::Whitelisted(admin.clone());
        env.storage().persistent().set(&legacy_wl_key, &true);
        env.storage().persistent().extend_ttl(&legacy_wl_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Publish initialization event
        env.events().publish(
            (Symbol::new(&env, "initialized"), admin.clone()),
            TokenInfo {
                name,
                symbol,
                decimals,
                total_supply,
                admin,
            },
        );

        Ok(())
    }

    /// Get token metadata information
    pub fn token_info(env: Env) -> Result<TokenInfo, Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        if let Some(state) = env.storage().instance().get::<_, TokenState>(&DataKey::State) {
            let total_supply: i128 = env.storage().instance().get(&DataKey::TotalSupply)
                .ok_or(Error::Unauthorized)?;
            return Ok(TokenInfo {
                name: state.name,
                symbol: state.symbol,
                decimals: state.decimals,
                total_supply,
                admin,
            });
        }

        let name: String = env.storage().instance().get(&DataKey::Name)
            .ok_or(Error::Unauthorized)?;
        let symbol: String = env.storage().instance().get(&DataKey::Symbol)
            .ok_or(Error::Unauthorized)?;
        let decimals: u32 = env.storage().instance().get(&DataKey::Decimals)
            .ok_or(Error::Unauthorized)?;
        let total_supply: i128 = env.storage().instance().get(&DataKey::TotalSupply)
            .ok_or(Error::Unauthorized)?;

        Ok(TokenInfo {
            name,
            symbol,
            decimals,
            total_supply,
            admin,
        })
    }

    /// Get token state metadata (name, symbol, decimals)
    pub fn state(env: Env) -> Result<TokenState, Error> {
        env.storage().instance().get(&DataKey::State)
            .ok_or(Error::Unauthorized)
    }

    /// Get the contract administrator
    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Get account balance and automatically extend storage TTL if entry exists
    pub fn balance(env: Env, account: Address) -> i128 {
        let key = DataKey::Balance(account);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        env.storage().persistent()
            .get(&key)
            .unwrap_or(0)
    }

    /// Check if account is whitelisted and automatically extend storage TTL if entry exists
    pub fn is_whitelisted(env: Env, account: Address) -> bool {
        let key_wl = DataKey::Whitelist(account.clone());
        let val_wl = if env.storage().persistent().has(&key_wl) {
            env.storage().persistent().extend_ttl(&key_wl, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
            env.storage().persistent().get(&key_wl).unwrap_or(false)
        } else {
            false
        };

        let key_wld = DataKey::Whitelisted(account);
        let val_wld = if env.storage().persistent().has(&key_wld) {
            env.storage().persistent().extend_ttl(&key_wld, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
            env.storage().persistent().get(&key_wld).unwrap_or(false)
        } else {
            false
        };

        val_wl || val_wld
    }

    /// Add account to whitelist (admin only, specification standard)
    pub fn add_to_whitelist(env: Env, admin: Address, account: Address) -> Result<(), Error> {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();

        // Check if already whitelisted
        if Self::is_whitelisted(env.clone(), account.clone()) {
            return Err(Error::AlreadyWhitelisted);
        }

        // Add to whitelist in persistent storage
        let key_wl = DataKey::Whitelist(account.clone());
        env.storage().persistent().set(&key_wl, &true);
        env.storage().persistent().extend_ttl(&key_wl, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let key_wld = DataKey::Whitelisted(account.clone());
        env.storage().persistent().set(&key_wld, &true);
        env.storage().persistent().extend_ttl(&key_wld, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Publish whitelist event
        env.events().publish(
            (Symbol::new(&env, "whitelisted"), account),
            true,
        );

        Ok(())
    }

    /// Remove account from whitelist (admin only, specification standard)
    pub fn remove_from_whitelist(env: Env, admin: Address, account: Address) -> Result<(), Error> {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();

        // Check if currently whitelisted
        if !Self::is_whitelisted(env.clone(), account.clone()) {
            return Err(Error::NotCurrentlyWhitelisted);
        }

        // Remove from whitelist
        let key_wl = DataKey::Whitelist(account.clone());
        env.storage().persistent().set(&key_wl, &false);
        env.storage().persistent().extend_ttl(&key_wl, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let key_wld = DataKey::Whitelisted(account.clone());
        env.storage().persistent().set(&key_wld, &false);
        env.storage().persistent().extend_ttl(&key_wld, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Publish whitelist removal event
        env.events().publish(
            (Symbol::new(&env, "whitelisted"), account),
            false,
        );

        Ok(())
    }

    /// Set account whitelist status (admin only, backward-compatible helper)
    pub fn set_whitelist(env: Env, account: Address, status: bool) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        admin.require_auth();

        let key_wl = DataKey::Whitelist(account.clone());
        env.storage().persistent().set(&key_wl, &status);
        env.storage().persistent().extend_ttl(&key_wl, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let key_wld = DataKey::Whitelisted(account.clone());
        env.storage().persistent().set(&key_wld, &status);
        env.storage().persistent().extend_ttl(&key_wld, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Publish whitelist event
        env.events().publish(
            (Symbol::new(&env, "whitelisted"), account),
            status,
        );

        Ok(())
    }

    /// Transfer tokens between accounts (requires both accounts to be whitelisted)
    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        from.require_auth();

        // Validate amount
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Check whitelist compliance for both sender and recipient
        if !Self::is_whitelisted(env.clone(), from.clone()) {
            return Err(Error::NotWhitelisted);
        }
        if !Self::is_whitelisted(env.clone(), to.clone()) {
            return Err(Error::NotWhitelisted);
        }

        // Get current balances
        let from_balance = Self::balance(env.clone(), from.clone());
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }

        let to_balance = Self::balance(env.clone(), to.clone());

        // Perform transfer
        env.storage().persistent().set(&DataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage().persistent().set(&DataKey::Balance(to.clone()), &(to_balance + amount));

        // Extend TTL for both accounts (balances and whitelist states)
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let key_wl_from = DataKey::Whitelist(from.clone());
        if env.storage().persistent().has(&key_wl_from) {
            env.storage().persistent().extend_ttl(&key_wl_from, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        let key_wld_from = DataKey::Whitelisted(from.clone());
        if env.storage().persistent().has(&key_wld_from) {
            env.storage().persistent().extend_ttl(&key_wld_from, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        let key_wl_to = DataKey::Whitelist(to.clone());
        if env.storage().persistent().has(&key_wl_to) {
            env.storage().persistent().extend_ttl(&key_wl_to, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        let key_wld_to = DataKey::Whitelisted(to.clone());
        if env.storage().persistent().has(&key_wld_to) {
            env.storage().persistent().extend_ttl(&key_wld_to, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        // Publish transfer event
        env.events().publish(
            (Symbol::new(&env, "transfer"), from.clone(), to.clone()),
            amount,
        );

        Ok(())
    }

    /// Approve spender to transfer tokens on behalf of owner
    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        amount: i128,
    ) -> Result<(), Error> {
        owner.require_auth();

        // Validate amount (can be 0 to revoke approval)
        if amount < 0 {
            return Err(Error::InvalidAmount);
        }

        // Set allowance with AllowanceKey
        let key = DataKey::Allowance(AllowanceKey {
            owner: owner.clone(),
            spender: spender.clone(),
        });
        env.storage().persistent().set(&key, &amount);
        env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let pair_key = DataKey::AllowancePair(owner.clone(), spender.clone());
        env.storage().persistent().set(&pair_key, &amount);
        env.storage().persistent().extend_ttl(&pair_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Publish approval event
        env.events().publish(
            (Symbol::new(&env, "approval"), owner, spender),
            amount,
        );

        Ok(())
    }

    /// Get allowance amount and extend storage TTL if entry exists
    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        let key = DataKey::Allowance(AllowanceKey {
            owner: owner.clone(),
            spender: spender.clone(),
        });
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
            return env.storage().persistent().get(&key).unwrap_or(0);
        }

        let pair_key = DataKey::AllowancePair(owner, spender);
        if env.storage().persistent().has(&pair_key) {
            env.storage().persistent().extend_ttl(&pair_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
            return env.storage().persistent().get(&pair_key).unwrap_or(0);
        }

        0
    }

    /// Transfer tokens using allowance (requires both accounts to be whitelisted)
    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        spender.require_auth();

        // Validate amount
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Check allowance
        let current_allowance = Self::allowance(env.clone(), from.clone(), spender.clone());
        if current_allowance < amount {
            return Err(Error::InsufficientBalance);
        }

        // Check whitelist status for both accounts
        if !Self::is_whitelisted(env.clone(), from.clone()) {
            return Err(Error::NotWhitelisted);
        }
        if !Self::is_whitelisted(env.clone(), to.clone()) {
            return Err(Error::NotWhitelisted);
        }

        // Get current balances
        let from_balance = Self::balance(env.clone(), from.clone());
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }

        let to_balance = Self::balance(env.clone(), to.clone());

        // Perform transfer
        env.storage().persistent().set(&DataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage().persistent().set(&DataKey::Balance(to.clone()), &(to_balance + amount));

        // Update allowance
        let new_allowance = current_allowance - amount;
        let key = DataKey::Allowance(AllowanceKey {
            owner: from.clone(),
            spender: spender.clone(),
        });
        env.storage().persistent().set(&key, &new_allowance);
        env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let pair_key = DataKey::AllowancePair(from.clone(), spender.clone());
        env.storage().persistent().set(&pair_key, &new_allowance);
        env.storage().persistent().extend_ttl(&pair_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Extend TTL for all storage keys (balances, allowance, and whitelist states)
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let key_wl_from = DataKey::Whitelist(from.clone());
        if env.storage().persistent().has(&key_wl_from) {
            env.storage().persistent().extend_ttl(&key_wl_from, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        let key_wld_from = DataKey::Whitelisted(from.clone());
        if env.storage().persistent().has(&key_wld_from) {
            env.storage().persistent().extend_ttl(&key_wld_from, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        let key_wl_to = DataKey::Whitelist(to.clone());
        if env.storage().persistent().has(&key_wl_to) {
            env.storage().persistent().extend_ttl(&key_wl_to, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        let key_wld_to = DataKey::Whitelisted(to.clone());
        if env.storage().persistent().has(&key_wld_to) {
            env.storage().persistent().extend_ttl(&key_wld_to, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        // Publish transfer event
        env.events().publish(
            (Symbol::new(&env, "transfer_from"), spender, from, to),
            amount,
        );

        Ok(())
    }

    /// Mint new tokens (admin only)
    pub fn mint(env: Env, to: Address, amount: i128) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        admin.require_auth();

        // Validate amount
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Check if recipient is whitelisted
        if !Self::is_whitelisted(env.clone(), to.clone()) {
            return Err(Error::NotWhitelisted);
        }

        // Update balances and total supply
        let current_balance = Self::balance(env.clone(), to.clone());
        let total_supply: i128 = env.storage().instance().get(&DataKey::TotalSupply)
            .ok_or(Error::Unauthorized)?;

        env.storage().persistent().set(&DataKey::Balance(to.clone()), &(current_balance + amount));
        env.storage().instance().set(&DataKey::TotalSupply, &(total_supply + amount));

        // Extend TTL
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        let key_wl_to = DataKey::Whitelist(to.clone());
        if env.storage().persistent().has(&key_wl_to) {
            env.storage().persistent().extend_ttl(&key_wl_to, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        let key_wld_to = DataKey::Whitelisted(to.clone());
        if env.storage().persistent().has(&key_wld_to) {
            env.storage().persistent().extend_ttl(&key_wld_to, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        // Publish mint event
        env.events().publish(
            (Symbol::new(&env, "mint"), to),
            amount,
        );

        Ok(())
    }

    /// Burn tokens (admin only or self-burn)
    pub fn burn(env: Env, from: Address, amount: i128) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        // Allow admin to burn any account's tokens, or users to burn their own
        if admin != from {
            admin.require_auth();
        } else {
            from.require_auth();
        }

        // Validate amount
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Check balance
        let current_balance = Self::balance(env.clone(), from.clone());
        if current_balance < amount {
            return Err(Error::InsufficientBalance);
        }

        // Update balances and total supply
        let total_supply: i128 = env.storage().instance().get(&DataKey::TotalSupply)
            .ok_or(Error::Unauthorized)?;

        env.storage().persistent().set(&DataKey::Balance(from.clone()), &(current_balance - amount));
        env.storage().instance().set(&DataKey::TotalSupply, &(total_supply - amount));

        // Extend TTL
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        let key_wl_from = DataKey::Whitelist(from.clone());
        if env.storage().persistent().has(&key_wl_from) {
            env.storage().persistent().extend_ttl(&key_wl_from, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }
        let key_wld_from = DataKey::Whitelisted(from.clone());
        if env.storage().persistent().has(&key_wld_from) {
            env.storage().persistent().extend_ttl(&key_wld_from, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        // Publish burn event
        env.events().publish(
            (Symbol::new(&env, "burn"), from),
            amount,
        );

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup_test_contract<'a>(env: &'a Env) -> (Address, RwaTokenContractClient<'a>) {
        let contract_id = env.register(RwaTokenContract, ());
        let client = RwaTokenContractClient::new(env, &contract_id);
        let admin = Address::generate(env);

        env.mock_all_auths();

        (admin, client)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);

        let name = String::from_str(&env, "Real Estate Token");
        let symbol = String::from_str(&env, "RET");
        let decimals = 8u32;
        let total_supply = 1_000_000i128;

        client.initialize(&admin, &name, &symbol, &decimals, &total_supply);

        // Verify token info
        let token_info = client.token_info();
        assert_eq!(token_info.name, name);
        assert_eq!(token_info.symbol, symbol);
        assert_eq!(token_info.decimals, decimals);
        assert_eq!(token_info.total_supply, total_supply);
        assert_eq!(token_info.admin, admin);

        // Admin should have all tokens and be whitelisted
        assert_eq!(client.balance(&admin), total_supply);
        assert!(client.is_whitelisted(&admin));
    }

    #[test]
    fn test_initialize_twice_fails() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);

        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");

        // First initialization should succeed
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Second initialization should fail
        let result2 = client.try_initialize(&admin, &name, &symbol, &8u32, &1000i128);
        assert_eq!(result2, Err(Ok(Error::AlreadyInitialized)));
    }

    #[test]
    fn test_whitelist_management() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);
        let non_admin = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // User should not be whitelisted initially
        assert!(!client.is_whitelisted(&user));

        // Non-admin cannot whitelist
        let fail_res = client.try_add_to_whitelist(&non_admin, &user);
        assert_eq!(fail_res, Err(Ok(Error::Unauthorized)));

        // Admin adds to whitelist
        client.add_to_whitelist(&admin, &user);
        assert!(client.is_whitelisted(&user));

        // Adding again should fail
        let result2 = client.try_add_to_whitelist(&admin, &user);
        assert_eq!(result2, Err(Ok(Error::AlreadyWhitelisted)));

        // Non-admin cannot remove from whitelist
        let fail_remove = client.try_remove_from_whitelist(&non_admin, &user);
        assert_eq!(fail_remove, Err(Ok(Error::Unauthorized)));

        // Admin removes from whitelist
        client.remove_from_whitelist(&admin, &user);
        assert!(!client.is_whitelisted(&user));

        // Removing again should fail
        let result4 = client.try_remove_from_whitelist(&admin, &user);
        assert_eq!(result4, Err(Ok(Error::NotCurrentlyWhitelisted)));
    }

    #[test]
    fn test_transfer_between_whitelisted_accounts() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist users
        client.add_to_whitelist(&admin, &user1);
        client.add_to_whitelist(&admin, &user2);

        // Transfer some tokens to user1
        let transfer_amount = 100i128;
        client.transfer(&admin, &user1, &transfer_amount);

        assert_eq!(client.balance(&admin), 900);
        assert_eq!(client.balance(&user1), 100);

        // Transfer from user1 to user2
        client.transfer(&user1, &user2, &50i128);

        assert_eq!(client.balance(&user1), 50);
        assert_eq!(client.balance(&user2), 50);
    }

    #[test]
    fn test_transfer_fails_for_non_whitelisted() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Only whitelist user1
        client.add_to_whitelist(&admin, &user1);

        // Transfer from admin to non-whitelisted user2 should fail
        let result = client.try_transfer(&admin, &user2, &100i128);
        assert_eq!(result, Err(Ok(Error::NotWhitelisted)));

        // Transfer from non-whitelisted user2 should fail
        let result2 = client.try_transfer(&user2, &user1, &100i128);
        assert_eq!(result2, Err(Ok(Error::NotWhitelisted)));
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist user
        client.add_to_whitelist(&admin, &user);

        // Try to transfer more than balance
        let result = client.try_transfer(&user, &admin, &100i128);
        assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist all accounts
        client.add_to_whitelist(&admin, &owner);
        client.add_to_whitelist(&admin, &spender);
        client.add_to_whitelist(&admin, &recipient);

        // Transfer tokens to owner
        client.transfer(&admin, &owner, &500i128);

        // Approve spender
        let approval_amount = 200i128;
        client.approve(&owner, &spender, &approval_amount);
        assert_eq!(client.allowance(&owner, &spender), approval_amount);

        // Transfer from owner to recipient using allowance
        let transfer_amount = 150i128;
        client.transfer_from(&spender, &owner, &recipient, &transfer_amount);

        // Check balances and remaining allowance
        assert_eq!(client.balance(&owner), 350); // 500 - 150
        assert_eq!(client.balance(&recipient), 150);
        assert_eq!(client.allowance(&owner, &spender), 50); // 200 - 150
    }

    #[test]
    fn test_mint_tokens() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        let initial_supply = 1000i128;
        client.initialize(&admin, &name, &symbol, &8u32, &initial_supply);

        // Whitelist user
        client.add_to_whitelist(&admin, &user);

        // Mint tokens to user
        let mint_amount = 500i128;
        client.mint(&user, &mint_amount);

        // Check balance and total supply
        assert_eq!(client.balance(&user), mint_amount);
        let token_info = client.token_info();
        assert_eq!(token_info.total_supply, initial_supply + mint_amount);
    }

    #[test]
    fn test_mint_fails_for_non_whitelisted() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Try to mint to non-whitelisted user
        let result = client.try_mint(&user, &500i128);
        assert_eq!(result, Err(Ok(Error::NotWhitelisted)));
    }

    #[test]
    fn test_burn_tokens() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        let initial_supply = 1000i128;
        client.initialize(&admin, &name, &symbol, &8u32, &initial_supply);

        // Whitelist user and transfer tokens
        client.add_to_whitelist(&admin, &user);
        client.transfer(&admin, &user, &300i128);

        // Burn tokens from user account
        let burn_amount = 100i128;
        client.burn(&user, &burn_amount);

        // Check balance and total supply
        assert_eq!(client.balance(&user), 200); // 300 - 100
        let token_info = client.token_info();
        assert_eq!(token_info.total_supply, initial_supply - burn_amount);
    }

    #[test]
    fn test_burn_insufficient_balance() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist user
        client.add_to_whitelist(&admin, &user);

        // Try to burn more than balance
        let result = client.try_burn(&user, &100i128);
        assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
    }

    #[test]
    fn test_invalid_amounts() {
        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist user
        client.add_to_whitelist(&admin, &user);

        // Test invalid transfer amount
        let result = client.try_transfer(&admin, &user, &0i128);
        assert_eq!(result, Err(Ok(Error::InvalidAmount)));

        let result2 = client.try_transfer(&admin, &user, &-100i128);
        assert_eq!(result2, Err(Ok(Error::InvalidAmount)));

        // Test invalid mint amount
        let result3 = client.try_mint(&user, &0i128);
        assert_eq!(result3, Err(Ok(Error::InvalidAmount)));

        // Test invalid burn amount
        let result4 = client.try_burn(&admin, &-50i128);
        assert_eq!(result4, Err(Ok(Error::InvalidAmount)));
    }

    #[test]
    fn test_ttl_extension_on_balance_lookup() {
        use soroban_sdk::testutils::storage::Persistent as _;
        use soroban_sdk::testutils::Ledger as _;

        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        let initial_supply = 1_000_000i128;
        client.initialize(&admin, &name, &symbol, &8u32, &initial_supply);

        // Verify initial TTL is set to standard 172,800 ledgers
        env.as_contract(&client.address, || {
            let key = DataKey::Balance(admin.clone());
            let ttl = env.storage().persistent().get_ttl(&key);
            assert_eq!(ttl, STORAGE_TTL_EXTEND_TO);
        });

        // Advance ledger by 50,000 ledgers to simulate time passing
        env.ledger().with_mut(|li| {
            li.sequence_number += 50_000;
        });

        // Verify TTL decremented
        env.as_contract(&client.address, || {
            let key = DataKey::Balance(admin.clone());
            let ttl = env.storage().persistent().get_ttl(&key);
            assert_eq!(ttl, STORAGE_TTL_EXTEND_TO - 50_000);
        });

        // Querying balance should trigger automatic TTL extension
        let bal = client.balance(&admin);
        assert_eq!(bal, initial_supply);

        // Verify TTL is restored back to STORAGE_TTL_EXTEND_TO
        env.as_contract(&client.address, || {
            let key = DataKey::Balance(admin.clone());
            let ttl = env.storage().persistent().get_ttl(&key);
            assert_eq!(ttl, STORAGE_TTL_EXTEND_TO);
        });
    }

    #[test]
    fn test_ttl_extension_on_transfer() {
        use soroban_sdk::testutils::storage::Persistent as _;
        use soroban_sdk::testutils::Ledger as _;

        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);

        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist users using set_whitelist
        client.set_whitelist(&user1, &true);
        client.set_whitelist(&user2, &true);

        // Transfer initial balance to user1
        client.transfer(&admin, &user1, &500i128);

        // Advance ledger sequence by 60,000 ledgers
        env.ledger().with_mut(|li| {
            li.sequence_number += 60_000;
        });

        // Verify TTLs are degraded before transfer
        env.as_contract(&client.address, || {
            let key_bal1 = DataKey::Balance(user1.clone());
            let key_wl1 = DataKey::Whitelisted(user1.clone());
            let key_wl2 = DataKey::Whitelisted(user2.clone());

            assert_eq!(env.storage().persistent().get_ttl(&key_bal1), STORAGE_TTL_EXTEND_TO - 60_000);
            assert_eq!(env.storage().persistent().get_ttl(&key_wl1), STORAGE_TTL_EXTEND_TO - 60_000);
            assert_eq!(env.storage().persistent().get_ttl(&key_wl2), STORAGE_TTL_EXTEND_TO - 60_000);
        });

        // Perform transfer from user1 to user2
        client.transfer(&user1, &user2, &200i128);

        // Verify balances and whitelist TTLs are all extended to STORAGE_TTL_EXTEND_TO
        env.as_contract(&client.address, || {
            let key_bal1 = DataKey::Balance(user1.clone());
            let key_bal2 = DataKey::Balance(user2.clone());
            let key_wl1 = DataKey::Whitelisted(user1.clone());
            let key_wl2 = DataKey::Whitelisted(user2.clone());

            assert_eq!(env.storage().persistent().get_ttl(&key_bal1), STORAGE_TTL_EXTEND_TO);
            assert_eq!(env.storage().persistent().get_ttl(&key_bal2), STORAGE_TTL_EXTEND_TO);
            assert_eq!(env.storage().persistent().get_ttl(&key_wl1), STORAGE_TTL_EXTEND_TO);
            assert_eq!(env.storage().persistent().get_ttl(&key_wl2), STORAGE_TTL_EXTEND_TO);
        });

        assert_eq!(client.balance(&user1), 300);
        assert_eq!(client.balance(&user2), 200);
    }

    #[test]
    fn test_set_whitelist_and_ttl_extension() {
        use soroban_sdk::testutils::storage::Persistent as _;
        use soroban_sdk::testutils::Ledger as _;

        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let user = Address::generate(&env);

        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        // Whitelist user using set_whitelist
        client.set_whitelist(&user, &true);
        assert!(client.is_whitelisted(&user));

        // Advance ledger
        env.ledger().with_mut(|li| {
            li.sequence_number += 40_000;
        });

        // is_whitelisted lookup triggers TTL extension
        assert!(client.is_whitelisted(&user));
        env.as_contract(&client.address, || {
            let key = DataKey::Whitelisted(user.clone());
            assert_eq!(env.storage().persistent().get_ttl(&key), STORAGE_TTL_EXTEND_TO);
        });

        // Advance ledger again
        env.ledger().with_mut(|li| {
            li.sequence_number += 30_000;
        });

        // Admin updates whitelist status to false
        client.set_whitelist(&user, &false);
        assert!(!client.is_whitelisted(&user));

        env.as_contract(&client.address, || {
            let key = DataKey::Whitelisted(user.clone());
            assert_eq!(env.storage().persistent().get_ttl(&key), STORAGE_TTL_EXTEND_TO);
        });
    }

    #[test]
    fn test_transfer_from_extends_storage_ttl() {
        use soroban_sdk::testutils::storage::Persistent as _;
        use soroban_sdk::testutils::Ledger as _;

        let env = Env::default();
        let (admin, client) = setup_test_contract(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128);

        client.set_whitelist(&owner, &true);
        client.set_whitelist(&spender, &true);
        client.set_whitelist(&recipient, &true);

        client.transfer(&admin, &owner, &500i128);
        client.approve(&owner, &spender, &200i128);

        // Advance ledger
        env.ledger().with_mut(|li| {
            li.sequence_number += 45_000;
        });

        client.transfer_from(&spender, &owner, &recipient, &150i128);

        // Verify storage TTLs extended
        env.as_contract(&client.address, || {
            assert_eq!(env.storage().persistent().get_ttl(&DataKey::Balance(owner.clone())), STORAGE_TTL_EXTEND_TO);
            assert_eq!(env.storage().persistent().get_ttl(&DataKey::Balance(recipient.clone())), STORAGE_TTL_EXTEND_TO);
            let allow_key = DataKey::Allowance(AllowanceKey {
                owner: owner.clone(),
                spender: spender.clone(),
            });
            assert_eq!(env.storage().persistent().get_ttl(&allow_key), STORAGE_TTL_EXTEND_TO);
            let pair_key = DataKey::AllowancePair(owner.clone(), spender.clone());
            assert_eq!(env.storage().persistent().get_ttl(&pair_key), STORAGE_TTL_EXTEND_TO);
            assert_eq!(env.storage().persistent().get_ttl(&DataKey::Whitelisted(owner.clone())), STORAGE_TTL_EXTEND_TO);
            assert_eq!(env.storage().persistent().get_ttl(&DataKey::Whitelisted(recipient.clone())), STORAGE_TTL_EXTEND_TO);
        });
    }
}
