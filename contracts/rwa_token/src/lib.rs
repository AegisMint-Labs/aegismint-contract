#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, 
    Address, Env, String, Symbol
};

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

/// Storage keys for the RWA token contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract administrator address
    Admin,
    /// Token metadata: name
    Name,
    /// Token metadata: symbol
    Symbol,
    /// Token metadata: decimals
    Decimals,
    /// Total supply of tokens
    TotalSupply,
    /// Individual account balance: Balance(account_address)
    Balance(Address),
    /// Whitelist status: Whitelisted(account_address) -> bool
    Whitelisted(Address),
    /// Transfer allowance: Allowance(owner, spender)
    Allowance(Address, Address),
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
    /// Can only be called once and sets up the token metadata
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

        // Store token metadata
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage().instance().set(&DataKey::Decimals, &decimals);
        env.storage().instance().set(&DataKey::TotalSupply, &total_supply);
        env.storage().instance().set(&DataKey::Initialized, &true);

        // Set initial balance to admin
        env.storage().persistent().set(&DataKey::Balance(admin.clone()), &total_supply);
        
        // Extend TTL for admin balance
        env.storage().persistent().extend_ttl(&DataKey::Balance(admin.clone()), 172800, 172800);

        // Automatically whitelist the admin
        env.storage().persistent().set(&DataKey::Whitelisted(admin.clone()), &true);
        env.storage().persistent().extend_ttl(&DataKey::Whitelisted(admin.clone()), 172800, 172800);

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

    /// Get token information
    pub fn token_info(env: Env) -> Result<TokenInfo, Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
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

    /// Get the contract administrator
    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Get account balance
    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage().persistent()
            .get(&DataKey::Balance(account))
            .unwrap_or(0)
    }

    /// Check if account is whitelisted
    pub fn is_whitelisted(env: Env, account: Address) -> bool {
        env.storage().persistent()
            .get(&DataKey::Whitelisted(account))
            .unwrap_or(false)
    }

    /// Add account to whitelist (admin only)
    pub fn add_to_whitelist(env: Env, account: Address) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        
        admin.require_auth();

        // Check if already whitelisted
        if Self::is_whitelisted(env.clone(), account.clone()) {
            return Err(Error::AlreadyWhitelisted);
        }

        // Add to whitelist
        env.storage().persistent().set(&DataKey::Whitelisted(account.clone()), &true);
        env.storage().persistent().extend_ttl(&DataKey::Whitelisted(account.clone()), 172800, 172800);

        // Publish whitelist event
        env.events().publish(
            (Symbol::new(&env, "whitelisted"), account.clone()),
            true,
        );

        Ok(())
    }

    /// Remove account from whitelist (admin only)
    pub fn remove_from_whitelist(env: Env, account: Address) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        
        admin.require_auth();

        // Check if currently whitelisted
        if !Self::is_whitelisted(env.clone(), account.clone()) {
            return Err(Error::NotCurrentlyWhitelisted);
        }

        // Remove from whitelist
        env.storage().persistent().set(&DataKey::Whitelisted(account.clone()), &false);
        env.storage().persistent().extend_ttl(&DataKey::Whitelisted(account.clone()), 172800, 172800);

        // Publish whitelist removal event
        env.events().publish(
            (Symbol::new(&env, "whitelisted"), account.clone()),
            false,
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

        // Extend TTL for both accounts
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), 172800, 172800);
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), 172800, 172800);

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

        // Set allowance
        env.storage().persistent().set(&DataKey::Allowance(owner.clone(), spender.clone()), &amount);
        env.storage().persistent().extend_ttl(&DataKey::Allowance(owner.clone(), spender.clone()), 172800, 172800);

        // Publish approval event
        env.events().publish(
            (Symbol::new(&env, "approval"), owner.clone(), spender.clone()),
            amount,
        );

        Ok(())
    }

    /// Get allowance amount
    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        env.storage().persistent()
            .get(&DataKey::Allowance(owner, spender))
            .unwrap_or(0)
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
        env.storage().persistent().set(&DataKey::Allowance(from.clone(), spender.clone()), &(current_allowance - amount));

        // Extend TTL for all storage keys
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), 172800, 172800);
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), 172800, 172800);
        env.storage().persistent().extend_ttl(&DataKey::Allowance(from.clone(), spender.clone()), 172800, 172800);

        // Publish transfer event
        env.events().publish(
            (Symbol::new(&env, "transfer_from"), spender.clone(), from.clone(), to.clone()),
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
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), 172800, 172800);

        // Publish mint event
        env.events().publish(
            (Symbol::new(&env, "mint"), to.clone()),
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
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), 172800, 172800);

        // Publish burn event
        env.events().publish(
            (Symbol::new(&env, "burn"), from.clone()),
            amount,
        );

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup_test_contract() -> (Env, Address, RwaTokenContractClient) {
        let env = Env::default();
        let contract_id = env.register(RwaTokenContract, ());
        let client = RwaTokenContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        
        env.mock_all_auths();
        
        (env, admin, client)
    }

    #[test]
    fn test_initialize() {
        let (env, admin, client) = setup_test_contract();
        
        let name = String::from_str(&env, "Real Estate Token");
        let symbol = String::from_str(&env, "RET");
        let decimals = 8u32;
        let total_supply = 1_000_000i128;

        let result = client.initialize(&admin, &name, &symbol, &decimals, &total_supply);
        assert!(result.is_ok());

        // Verify token info
        let token_info = client.token_info().unwrap();
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
        let (env, admin, client) = setup_test_contract();
        
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");

        // First initialization should succeed
        let result = client.initialize(&admin, &name, &symbol, &8u32, &1000i128);
        assert!(result.is_ok());

        // Second initialization should fail
        let result2 = client.initialize(&admin, &name, &symbol, &8u32, &1000i128);
        assert_eq!(result2, Err(Ok(Error::AlreadyInitialized)));
    }

    #[test]
    fn test_whitelist_management() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // User should not be whitelisted initially
        assert!(!client.is_whitelisted(&user));

        // Add to whitelist
        let result = client.add_to_whitelist(&user);
        assert!(result.is_ok());
        assert!(client.is_whitelisted(&user));

        // Adding again should fail
        let result2 = client.add_to_whitelist(&user);
        assert_eq!(result2, Err(Ok(Error::AlreadyWhitelisted)));

        // Remove from whitelist
        let result3 = client.remove_from_whitelist(&user);
        assert!(result3.is_ok());
        assert!(!client.is_whitelisted(&user));

        // Removing again should fail
        let result4 = client.remove_from_whitelist(&user);
        assert_eq!(result4, Err(Ok(Error::NotCurrentlyWhitelisted)));
    }

    #[test]
    fn test_transfer_between_whitelisted_accounts() {
        let (env, admin, client) = setup_test_contract();
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Whitelist users
        client.add_to_whitelist(&user1).unwrap();
        client.add_to_whitelist(&user2).unwrap();

        // Transfer some tokens to user1
        let transfer_amount = 100i128;
        let result = client.transfer(&admin, &user1, &transfer_amount);
        assert!(result.is_ok());

        assert_eq!(client.balance(&admin), 900);
        assert_eq!(client.balance(&user1), 100);

        // Transfer from user1 to user2
        let result2 = client.transfer(&user1, &user2, &50i128);
        assert!(result2.is_ok());

        assert_eq!(client.balance(&user1), 50);
        assert_eq!(client.balance(&user2), 50);
    }

    #[test]
    fn test_transfer_fails_for_non_whitelisted() {
        let (env, admin, client) = setup_test_contract();
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Only whitelist user1
        client.add_to_whitelist(&user1).unwrap();

        // Transfer from admin to non-whitelisted user2 should fail
        let result = client.transfer(&admin, &user2, &100i128);
        assert_eq!(result, Err(Ok(Error::NotWhitelisted)));

        // Transfer from non-whitelisted user2 should fail
        let result2 = client.transfer(&user2, &user1, &100i128);
        assert_eq!(result2, Err(Ok(Error::NotWhitelisted)));
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Whitelist user
        client.add_to_whitelist(&user).unwrap();

        // Try to transfer more than balance
        let result = client.transfer(&user, &admin, &100i128);
        assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let (env, admin, client) = setup_test_contract();
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Whitelist all accounts
        client.add_to_whitelist(&owner).unwrap();
        client.add_to_whitelist(&spender).unwrap();
        client.add_to_whitelist(&recipient).unwrap();

        // Transfer tokens to owner
        client.transfer(&admin, &owner, &500i128).unwrap();

        // Approve spender
        let approval_amount = 200i128;
        let result = client.approve(&owner, &spender, &approval_amount);
        assert!(result.is_ok());
        assert_eq!(client.allowance(&owner, &spender), approval_amount);

        // Transfer from owner to recipient using allowance
        let transfer_amount = 150i128;
        let result2 = client.transfer_from(&spender, &owner, &recipient, &transfer_amount);
        assert!(result2.is_ok());

        // Check balances and remaining allowance
        assert_eq!(client.balance(&owner), 350); // 500 - 150
        assert_eq!(client.balance(&recipient), 150);
        assert_eq!(client.allowance(&owner, &spender), 50); // 200 - 150
    }

    #[test]
    fn test_mint_tokens() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        let initial_supply = 1000i128;
        client.initialize(&admin, &name, &symbol, &8u32, &initial_supply).unwrap();

        // Whitelist user
        client.add_to_whitelist(&user).unwrap();

        // Mint tokens to user
        let mint_amount = 500i128;
        let result = client.mint(&user, &mint_amount);
        assert!(result.is_ok());

        // Check balance and total supply
        assert_eq!(client.balance(&user), mint_amount);
        let token_info = client.token_info().unwrap();
        assert_eq!(token_info.total_supply, initial_supply + mint_amount);
    }

    #[test]
    fn test_mint_fails_for_non_whitelisted() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Try to mint to non-whitelisted user
        let result = client.mint(&user, &500i128);
        assert_eq!(result, Err(Ok(Error::NotWhitelisted)));
    }

    #[test]
    fn test_burn_tokens() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        let initial_supply = 1000i128;
        client.initialize(&admin, &name, &symbol, &8u32, &initial_supply).unwrap();

        // Whitelist user and transfer tokens
        client.add_to_whitelist(&user).unwrap();
        client.transfer(&admin, &user, &300i128).unwrap();

        // Burn tokens from user account
        let burn_amount = 100i128;
        let result = client.burn(&user, &burn_amount);
        assert!(result.is_ok());

        // Check balance and total supply
        assert_eq!(client.balance(&user), 200); // 300 - 100
        let token_info = client.token_info().unwrap();
        assert_eq!(token_info.total_supply, initial_supply - burn_amount);
    }

    #[test]
    fn test_burn_insufficient_balance() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Whitelist user
        client.add_to_whitelist(&user).unwrap();

        // Try to burn more than balance
        let result = client.burn(&user, &100i128);
        assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
    }

    #[test]
    fn test_invalid_amounts() {
        let (env, admin, client) = setup_test_contract();
        let user = Address::generate(&env);
        
        // Initialize contract
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");
        client.initialize(&admin, &name, &symbol, &8u32, &1000i128).unwrap();

        // Whitelist user
        client.add_to_whitelist(&user).unwrap();

        // Test invalid transfer amount
        let result = client.transfer(&admin, &user, &0i128);
        assert_eq!(result, Err(Ok(Error::InvalidAmount)));

        let result2 = client.transfer(&admin, &user, &-100i128);
        assert_eq!(result2, Err(Ok(Error::InvalidAmount)));

        // Test invalid mint amount
        let result3 = client.mint(&user, &0i128);
        assert_eq!(result3, Err(Ok(Error::InvalidAmount)));

        // Test invalid burn amount
        let result4 = client.burn(&admin, &-50i128);
        assert_eq!(result4, Err(Ok(Error::InvalidAmount)));
    }
}