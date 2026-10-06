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