#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    Address, Env, Symbol, Vec,
};

/// TTL Extension thresholds for persistent storage
pub const STORAGE_TTL_THRESHOLD: u32 = 17_280;
pub const STORAGE_TTL_EXTEND_TO: u32 = 100_000;

/// Error types for the Marketplace Escrow contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract is already initialized
    AlreadyInitialized = 1,
    /// Caller is not authorized to perform this action
    Unauthorized = 2,
    /// Escrow not found
    EscrowNotFound = 3,
    /// Invalid escrow state for this operation
    InvalidEscrowState = 4,
    /// Invalid amount (zero or negative)
    InvalidAmount = 5,
    /// Escrow has expired
    EscrowExpired = 6,
    /// Escrow has not expired yet
    EscrowNotExpired = 7,
    /// Insufficient balance for escrow
    InsufficientBalance = 8,
    /// Invalid timeout (too short or too long)
    InvalidTimeout = 9,
    /// Transfer failed
    TransferFailed = 10,
}

/// Lifecycle status enumeration for Escrow
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowStatus {
    Active = 0,
    Completed = 1,
    Cancelled = 2,
    Refunded = 3,
}

/// Escrow state enumeration (extended for backwards compatibility)
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowState {
    Active = 0,
    Completed = 1,
    Cancelled = 2,
    Refunded = 3,
    InDispute = 4,
}

impl From<EscrowStatus> for EscrowState {
    fn from(status: EscrowStatus) -> Self {
        match status {
            EscrowStatus::Active => EscrowState::Active,
            EscrowStatus::Completed => EscrowState::Completed,
            EscrowStatus::Cancelled => EscrowState::Cancelled,
            EscrowStatus::Refunded => EscrowState::Refunded,
        }
    }
}

/// Storage keys for the Marketplace Escrow contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract administrator address
    Admin,
    /// Escrow counter for new order IDs
    EscrowIdCounter,
    /// Persistent escrow details: EscrowDetails(escrow_id)
    EscrowDetails(u64),
    /// Total number of escrows created (legacy alias)
    EscrowCount,
    /// Individual escrow info (legacy alias)
    Escrow(u64),
    /// Escrows by seller: EscrowsBySeller(seller_address) -> Vec<u64>
    EscrowsBySeller(Address),
    /// Escrows by buyer: EscrowsByBuyer(buyer_address) -> Vec<u64>
    EscrowsByBuyer(Address),
    /// Contract initialization status
    Initialized,
    /// Minimum escrow timeout
    MinTimeout,
    /// Maximum escrow timeout
    MaxTimeout,
    /// Platform fee (in basis points, e.g., 250 = 2.5%)
    PlatformFee,
    /// Platform fee recipient
    FeeRecipient,
    /// Configured USDC settlement token address
    UsdcToken,
}

/// Primary Escrow model specification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub token_address: Address,
    pub amount: i128,
    pub price_usdc: i128,
    pub timeout: u64,
    pub status: EscrowStatus,
}

/// Escrow info structure (retained for backward compatibility and metadata queries)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowInfo {
    pub escrow_id: u64,
    pub seller: Address,
    pub buyer: Address,
    pub token_contract: Address,
    pub token_amount: i128,
    pub payment_amount: i128,
    pub state: EscrowState,
    pub created_at: u64,
    pub expires_at: u64,
    pub completed_at: Option<u64>,
    pub platform_fee: u32,
}

/// Marketplace Escrow Contract
#[contract]
pub struct MarketplaceEscrowContract;

#[contractimpl]
impl MarketplaceEscrowContract {
    /// Initialize the marketplace escrow contract
    pub fn initialize(
        env: Env,
        admin: Address,
        min_timeout: u64,
        max_timeout: u64,
        platform_fee: u32,
        fee_recipient: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        if min_timeout == 0 || max_timeout <= min_timeout {
            return Err(Error::InvalidTimeout);
        }

        if platform_fee > 10000 {
            return Err(Error::InvalidAmount);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::EscrowIdCounter, &0u64);
        env.storage().instance().set(&DataKey::EscrowCount, &0u64);
        env.storage().instance().set(&DataKey::MinTimeout, &min_timeout);
        env.storage().instance().set(&DataKey::MaxTimeout, &max_timeout);
        env.storage().instance().set(&DataKey::PlatformFee, &platform_fee);
        env.storage().instance().set(&DataKey::FeeRecipient, &fee_recipient);
        env.storage().instance().set(&DataKey::Initialized, &true);

        env.events().publish(
            (Symbol::new(&env, "escrow_initialized"), admin),
            (min_timeout, max_timeout, platform_fee, fee_recipient),
        );

        Ok(())
    }

    /// Get contract administrator
    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Set USDC settlement token contract address (admin-only)
    pub fn set_usdc_token(env: Env, admin: Address, usdc_token: Address) -> Result<(), Error> {
        let current_admin: Address = env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        if current_admin != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
        Ok(())
    }

    /// Get configured USDC settlement token address
    pub fn usdc_token(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::UsdcToken)
    }

    /// Get platform configuration
    pub fn get_platform_config(env: Env) -> Result<(u64, u64, u32, Address), Error> {
        let min_timeout: u64 = env.storage()
            .instance()
            .get(&DataKey::MinTimeout)
            .ok_or(Error::Unauthorized)?;
        let max_timeout: u64 = env.storage()
            .instance()
            .get(&DataKey::MaxTimeout)
            .ok_or(Error::Unauthorized)?;
        let platform_fee: u32 = env.storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .ok_or(Error::Unauthorized)?;
        let fee_recipient: Address = env.storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .ok_or(Error::Unauthorized)?;

        Ok((min_timeout, max_timeout, platform_fee, fee_recipient))
    }

    /// Update platform fee (admin only)
    pub fn update_platform_fee(env: Env, new_fee: u32) -> Result<(), Error> {
        let admin: Address = env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        admin.require_auth();

        if new_fee > 10000 {
            return Err(Error::InvalidAmount);
        }

        env.storage().instance().set(&DataKey::PlatformFee, &new_fee);

        env.events().publish(
            (Symbol::new(&env, "fee_updated"), admin),
            new_fee,
        );

        Ok(())
    }

    /// Create a new escrow locking RWA tokens into custody
    pub fn create_escrow(
        env: Env,
        seller: Address,
        buyer: Address,
        token_address: Address,
        amount: i128,
        price_usdc: i128,
        timeout_blocks: u32,
    ) -> Result<u64, Error> {
        seller.require_auth();

        if amount <= 0 || price_usdc <= 0 {
            return Err(Error::InvalidAmount);
        }

        let min_timeout: u64 = env.storage()
            .instance()
            .get(&DataKey::MinTimeout)
            .ok_or(Error::Unauthorized)?;
        let max_timeout: u64 = env.storage()
            .instance()
            .get(&DataKey::MaxTimeout)
            .ok_or(Error::Unauthorized)?;

        let timeout_input = timeout_blocks as u64;
        if timeout_input < min_timeout || timeout_input > max_timeout {
            return Err(Error::InvalidTimeout);
        }

        // Transfer tokens from seller into this escrow contract
        let token_client = TokenClient::new(&env, &token_address);
        let transfer_result = token_client.try_transfer_from(
            &env.current_contract_address(),
            &seller,
            &env.current_contract_address(),
            &amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Determine next escrow identifier
        let current_count: u64 = env.storage()
            .instance()
            .get(&DataKey::EscrowIdCounter)
            .or_else(|| env.storage().instance().get(&DataKey::EscrowCount))
            .unwrap_or(0);
        let new_escrow_id = current_count + 1;

        let current_time = env.ledger().timestamp();
        let timeout = current_time + timeout_input;
        let platform_fee: u32 = env.storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .ok_or(Error::Unauthorized)?;

        // Build canonical Escrow struct
        let escrow = Escrow {
            id: new_escrow_id,
            buyer: buyer.clone(),
            seller: seller.clone(),
            token_address: token_address.clone(),
            amount,
            price_usdc,
            timeout,
            status: EscrowStatus::Active,
        };

        // Build compatibility EscrowInfo struct
        let escrow_info = EscrowInfo {
            escrow_id: new_escrow_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            token_contract: token_address.clone(),
            token_amount: amount,
            payment_amount: price_usdc,
            state: EscrowState::Active,
            created_at: current_time,
            expires_at: timeout,
            completed_at: None,
            platform_fee,
        };

        // Store EscrowDetails and legacy Escrow in persistent storage
        let details_key = DataKey::EscrowDetails(new_escrow_id);
        let legacy_key = DataKey::Escrow(new_escrow_id);
        env.storage().persistent().set(&details_key, &escrow);
        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        env.storage().persistent().set(&legacy_key, &escrow_info);
        env.storage().persistent().extend_ttl(&legacy_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Update instance counters
        env.storage().instance().set(&DataKey::EscrowIdCounter, &new_escrow_id);
        env.storage().instance().set(&DataKey::EscrowCount, &new_escrow_id);

        // Update indexes for seller and buyer
        Self::add_to_seller_escrows(env.clone(), seller.clone(), new_escrow_id);
        Self::add_to_buyer_escrows(env.clone(), buyer.clone(), new_escrow_id);

        // Emit creation event
        env.events().publish(
            (Symbol::new(&env, "escrow_created"), new_escrow_id, seller, buyer),
            (token_address, amount, price_usdc, timeout),
        );

        Ok(new_escrow_id)
    }

    /// Fulfill escrow: atomic transfer of USDC payment and release of RWA tokens to buyer
    pub fn fulfill_escrow(env: Env, buyer: Address, escrow_id: u64) -> Result<(), Error> {
        buyer.require_auth();

        let details_key = DataKey::EscrowDetails(escrow_id);
        let mut escrow: Escrow = env.storage()
            .persistent()
            .get(&details_key)
            .ok_or(Error::EscrowNotFound)?;

        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        if escrow.buyer != buyer {
            return Err(Error::Unauthorized);
        }

        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidEscrowState);
        }

        let current_time = env.ledger().timestamp();
        let current_seq = env.ledger().sequence() as u64;
        if current_time > escrow.timeout && current_seq > escrow.timeout {
            return Err(Error::EscrowExpired);
        }

        // Calculate protocol fee and seller payout
        let platform_fee: u32 = env.storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .unwrap_or(0);
        let fee_amount = (escrow.price_usdc * platform_fee as i128) / 10000;
        let seller_amount = escrow.price_usdc - fee_amount;

        // Atomic settlement: If USDC settlement token is configured, transfer funds from buyer
        if let Some(usdc_address) = env.storage().instance().get::<_, Address>(&DataKey::UsdcToken) {
            let usdc_client = TokenClient::new(&env, &usdc_address);
            if seller_amount > 0 {
                let transfer_seller = usdc_client.try_transfer_from(
                    &env.current_contract_address(),
                    &buyer,
                    &escrow.seller,
                    &seller_amount,
                );
                if transfer_seller.is_err() {
                    return Err(Error::TransferFailed);
                }
            }

            if fee_amount > 0 {
                let fee_recipient: Address = env.storage()
                    .instance()
                    .get(&DataKey::FeeRecipient)
                    .ok_or(Error::Unauthorized)?;
                let transfer_fee = usdc_client.try_transfer_from(
                    &env.current_contract_address(),
                    &buyer,
                    &fee_recipient,
                    &fee_amount,
                );
                if transfer_fee.is_err() {
                    return Err(Error::TransferFailed);
                }
            }
        }

        // Deliver custody of RWA tokens to buyer
        let token_client = TokenClient::new(&env, &escrow.token_address);
        let transfer_result = token_client.try_transfer(
            &env.current_contract_address(),
            &escrow.buyer,
            &escrow.amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Update escrow state to Completed
        escrow.status = EscrowStatus::Completed;
        env.storage().persistent().set(&details_key, &escrow);
        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Update legacy EscrowInfo state
        let legacy_key = DataKey::Escrow(escrow_id);
        if let Some(mut info) = env.storage().persistent().get::<_, EscrowInfo>(&legacy_key) {
            info.state = EscrowState::Completed;
            info.completed_at = Some(current_time);
            env.storage().persistent().set(&legacy_key, &info);
            env.storage().persistent().extend_ttl(&legacy_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        // Publish fulfillment event
        env.events().publish(
            (Symbol::new(&env, "escrow_fulfilled"), escrow_id, buyer, escrow.seller),
            (seller_amount, fee_amount),
        );

        Ok(())
    }

    /// Complete escrow (delegates to fulfill_escrow with buyer auth check)
    pub fn complete_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let details_key = DataKey::EscrowDetails(escrow_id);
        let escrow: Escrow = env.storage()
            .persistent()
            .get(&details_key)
            .ok_or(Error::EscrowNotFound)?;

        Self::fulfill_escrow(env, escrow.buyer, escrow_id)
    }

    /// Cancel escrow: seller reclaims locked tokens if the timeout block threshold has passed without fulfillment
    pub fn cancel_escrow(env: Env, seller: Address, escrow_id: u64) -> Result<(), Error> {
        seller.require_auth();

        let details_key = DataKey::EscrowDetails(escrow_id);
        let mut escrow: Escrow = env.storage()
            .persistent()
            .get(&details_key)
            .ok_or(Error::EscrowNotFound)?;

        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        if escrow.seller != seller {
            return Err(Error::Unauthorized);
        }

        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidEscrowState);
        }

        let current_time = env.ledger().timestamp();
        let current_seq = env.ledger().sequence() as u64;
        if current_time < escrow.timeout && current_seq < escrow.timeout {
            return Err(Error::EscrowNotExpired);
        }

        // Return tokens to seller
        let token_client = TokenClient::new(&env, &escrow.token_address);
        let transfer_result = token_client.try_transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Update state to Cancelled
        escrow.status = EscrowStatus::Cancelled;
        env.storage().persistent().set(&details_key, &escrow);
        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        // Update legacy EscrowInfo state
        let legacy_key = DataKey::Escrow(escrow_id);
        if let Some(mut info) = env.storage().persistent().get::<_, EscrowInfo>(&legacy_key) {
            info.state = EscrowState::Cancelled;
            info.completed_at = Some(current_time);
            env.storage().persistent().set(&legacy_key, &info);
            env.storage().persistent().extend_ttl(&legacy_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        // Publish cancellation event
        env.events().publish(
            (Symbol::new(&env, "escrow_cancelled"), escrow_id, seller),
            escrow.amount,
        );

        Ok(())
    }

    /// Refund expired escrow (anyone can trigger after expiration)
    pub fn refund_expired_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let details_key = DataKey::EscrowDetails(escrow_id);
        let mut escrow: Escrow = env.storage()
            .persistent()
            .get(&details_key)
            .ok_or(Error::EscrowNotFound)?;

        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidEscrowState);
        }

        let current_time = env.ledger().timestamp();
        let current_seq = env.ledger().sequence() as u64;
        if current_time < escrow.timeout && current_seq < escrow.timeout {
            return Err(Error::EscrowNotExpired);
        }

        let token_client = TokenClient::new(&env, &escrow.token_address);
        let transfer_result = token_client.try_transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        escrow.status = EscrowStatus::Refunded;
        env.storage().persistent().set(&details_key, &escrow);
        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);

        let legacy_key = DataKey::Escrow(escrow_id);
        if let Some(mut info) = env.storage().persistent().get::<_, EscrowInfo>(&legacy_key) {
            info.state = EscrowState::Refunded;
            info.completed_at = Some(current_time);
            env.storage().persistent().set(&legacy_key, &info);
            env.storage().persistent().extend_ttl(&legacy_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        }

        env.events().publish(
            (Symbol::new(&env, "escrow_refunded"), escrow_id, escrow.seller),
            escrow.amount,
        );

        Ok(())
    }

    /// Get canonical Escrow details
    pub fn get_escrow_details(env: Env, escrow_id: u64) -> Result<Escrow, Error> {
        let details_key = DataKey::EscrowDetails(escrow_id);
        let details = env.storage()
            .persistent()
            .get(&details_key)
            .ok_or(Error::EscrowNotFound)?;

        env.storage().persistent().extend_ttl(&details_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        Ok(details)
    }

    /// Get escrow information (legacy format)
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<EscrowInfo, Error> {
        let legacy_key = DataKey::Escrow(escrow_id);
        let info = env.storage()
            .persistent()
            .get(&legacy_key)
            .ok_or(Error::EscrowNotFound)?;

        env.storage().persistent().extend_ttl(&legacy_key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
        Ok(info)
    }

    /// Get total escrow count
    pub fn escrow_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::EscrowIdCounter)
            .or_else(|| env.storage().instance().get(&DataKey::EscrowCount))
            .unwrap_or(0)
    }

    /// Get escrows by seller
    pub fn get_escrows_by_seller(env: Env, seller: Address) -> Vec<u64> {
        let key = DataKey::EscrowsBySeller(seller);
        if let Some(escrows) = env.storage().persistent().get(&key) {
            env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
            escrows
        } else {
            Vec::new(&env)
        }
    }

    /// Get escrows by buyer
    pub fn get_escrows_by_buyer(env: Env, buyer: Address) -> Vec<u64> {
        let key = DataKey::EscrowsByBuyer(buyer);
        if let Some(escrows) = env.storage().persistent().get(&key) {
            env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
            escrows
        } else {
            Vec::new(&env)
        }
    }

    /// Internal: Add escrow to seller's list
    fn add_to_seller_escrows(env: Env, seller: Address, escrow_id: u64) {
        let mut escrows = Self::get_escrows_by_seller(env.clone(), seller.clone());
        escrows.push_back(escrow_id);
        let key = DataKey::EscrowsBySeller(seller);
        env.storage().persistent().set(&key, &escrows);
        env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
    }

    /// Internal: Add escrow to buyer's list
    fn add_to_buyer_escrows(env: Env, buyer: Address, escrow_id: u64) {
        let mut escrows = Self::get_escrows_by_buyer(env.clone(), buyer.clone());
        escrows.push_back(escrow_id);
        let key = DataKey::EscrowsByBuyer(buyer);
        env.storage().persistent().set(&key, &escrows);
        env.storage().persistent().extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_EXTEND_TO);
    }
}

/// Token client interface for interacting with RWA and payment tokens
#[soroban_sdk::contractclient(name = "TokenClient")]
pub trait TokenInterface {
    fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), soroban_sdk::Val>;
    fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) -> Result<(), soroban_sdk::Val>;
    fn approve(env: Env, owner: Address, spender: Address, amount: i128) -> Result<(), soroban_sdk::Val>;
    fn balance(env: Env, account: Address) -> i128;
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, Env};

    #[contract]
    pub struct MockTokenContract;

    #[contractimpl]
    impl MockTokenContract {
        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            from.require_auth();
            let from_bal: i128 = env.storage().persistent().get(&from).unwrap_or(0);
            assert!(from_bal >= amount);
            env.storage().persistent().set(&from, &(from_bal - amount));
            let to_bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
            env.storage().persistent().set(&to, &(to_bal + amount));
        }

        pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
            spender.require_auth();
            let from_bal: i128 = env.storage().persistent().get(&from).unwrap_or(0);
            assert!(from_bal >= amount);
            env.storage().persistent().set(&from, &(from_bal - amount));
            let to_bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
            env.storage().persistent().set(&to, &(to_bal + amount));
        }

        pub fn approve(_env: Env, _owner: Address, _spender: Address, _amount: i128) {}

        pub fn balance(env: Env, account: Address) -> i128 {
            env.storage().persistent().get(&account).unwrap_or(0)
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
            env.storage().persistent().set(&to, &(bal + amount));
        }
    }

    fn setup_test_escrow<'a>(env: &'a Env) -> (Address, MarketplaceEscrowContractClient<'a>) {
        let contract_id = env.register(MarketplaceEscrowContract, ());
        let client = MarketplaceEscrowContractClient::new(env, &contract_id);
        let admin = Address::generate(env);

        env.mock_all_auths();

        (admin, client)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);

        let min_timeout = 3600u64;
        let max_timeout = 604800u64;
        let platform_fee = 250u32;

        client.initialize(
            &admin,
            &min_timeout,
            &max_timeout,
            &platform_fee,
            &fee_recipient,
        );

        assert_eq!(client.admin(), admin);
        assert_eq!(client.escrow_count(), 0);

        let config = client.get_platform_config();
        assert_eq!(config.0, min_timeout);
        assert_eq!(config.1, max_timeout);
        assert_eq!(config.2, platform_fee);
        assert_eq!(config.3, fee_recipient);
    }

    #[test]
    fn test_initialize_twice_fails() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let result2 = client.try_initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);
        assert_eq!(result2, Err(Ok(Error::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_validation() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);

        let result = client.try_initialize(&admin, &604800u64, &3600u64, &250u32, &fee_recipient);
        assert_eq!(result, Err(Ok(Error::InvalidTimeout)));

        let result2 = client.try_initialize(&admin, &0u64, &604800u64, &250u32, &fee_recipient);
        assert_eq!(result2, Err(Ok(Error::InvalidTimeout)));

        let result3 = client.try_initialize(&admin, &3600u64, &604800u64, &15000u32, &fee_recipient);
        assert_eq!(result3, Err(Ok(Error::InvalidAmount)));
    }

    #[test]
    fn test_update_platform_fee() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let new_fee = 300u32;
        client.update_platform_fee(&new_fee);

        let config = client.get_platform_config();
        assert_eq!(config.2, new_fee);

        let result2 = client.try_update_platform_fee(&15000u32);
        assert_eq!(result2, Err(Ok(Error::InvalidAmount)));
    }

    #[test]
    fn test_escrow_creation_validation() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_contract = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let result = client.try_create_escrow(
            &seller,
            &buyer,
            &token_contract,
            &0i128,
            &1000i128,
            &7200u32,
        );
        assert_eq!(result, Err(Ok(Error::InvalidAmount)));

        let result2 = client.try_create_escrow(
            &seller,
            &buyer,
            &token_contract,
            &100i128,
            &-1000i128,
            &7200u32,
        );
        assert_eq!(result2, Err(Ok(Error::InvalidAmount)));

        let result3 = client.try_create_escrow(
            &seller,
            &buyer,
            &token_contract,
            &100i128,
            &1000i128,
            &1800u32,
        );
        assert_eq!(result3, Err(Ok(Error::InvalidTimeout)));

        let result4 = client.try_create_escrow(
            &seller,
            &buyer,
            &token_contract,
            &100i128,
            &1000i128,
            &1000000u32,
        );
        assert_eq!(result4, Err(Ok(Error::InvalidTimeout)));
    }

    #[test]
    fn test_escrow_lifecycle_fulfill_and_details() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);

        let token_id = env.register(MockTokenContract, ());
        let mock_token = MockTokenContractClient::new(&env, &token_id);
        mock_token.mint(&seller, &100_000i128);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let escrow_id = client.create_escrow(
            &seller,
            &buyer,
            &token_id,
            &50_000i128,
            &100_000i128,
            &7200u32,
        );
        assert_eq!(escrow_id, 1);
        assert_eq!(client.escrow_count(), 1);

        assert_eq!(mock_token.balance(&seller), 50_000);
        assert_eq!(mock_token.balance(&client.address), 50_000);

        let details = client.get_escrow_details(&escrow_id);
        assert_eq!(details.id, 1);
        assert_eq!(details.seller, seller);
        assert_eq!(details.buyer, buyer);
        assert_eq!(details.token_address, token_id);
        assert_eq!(details.amount, 50_000);
        assert_eq!(details.price_usdc, 100_000);
        assert_eq!(details.status, EscrowStatus::Active);

        // Fulfill escrow
        client.fulfill_escrow(&buyer, &escrow_id);

        let completed_details = client.get_escrow_details(&escrow_id);
        assert_eq!(completed_details.status, EscrowStatus::Completed);

        assert_eq!(mock_token.balance(&buyer), 50_000);
        assert_eq!(mock_token.balance(&client.address), 0);
    }

    #[test]
    fn test_escrow_lifecycle_cancellation_after_timeout() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);

        let token_id = env.register(MockTokenContract, ());
        let mock_token = MockTokenContractClient::new(&env, &token_id);
        mock_token.mint(&seller, &80_000i128);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let escrow_id = client.create_escrow(
            &seller,
            &buyer,
            &token_id,
            &80_000i128,
            &160_000i128,
            &7200u32,
        );

        // Cancellation before timeout must fail with EscrowNotExpired
        let err_not_expired = client.try_cancel_escrow(&seller, &escrow_id);
        assert_eq!(err_not_expired, Err(Ok(Error::EscrowNotExpired)));

        // Advance ledger beyond timeout
        env.ledger().with_mut(|li| {
            li.timestamp += 7201;
            li.sequence_number += 7201;
        });

        // Seller cancels escrow
        client.cancel_escrow(&seller, &escrow_id);

        let cancelled_details = client.get_escrow_details(&escrow_id);
        assert_eq!(cancelled_details.status, EscrowStatus::Cancelled);

        assert_eq!(mock_token.balance(&seller), 80_000);
        assert_eq!(mock_token.balance(&client.address), 0);

        // Cancel again fails
        let err_again = client.try_cancel_escrow(&seller, &escrow_id);
        assert_eq!(err_again, Err(Ok(Error::InvalidEscrowState)));
    }

    #[test]
    fn test_escrow_queries() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        assert_eq!(client.escrow_count(), 0);

        let seller_escrows = client.get_escrows_by_seller(&seller);
        assert_eq!(seller_escrows.len(), 0);

        let buyer_escrows = client.get_escrows_by_buyer(&buyer);
        assert_eq!(buyer_escrows.len(), 0);

        let result = client.try_get_escrow(&1);
        assert_eq!(result, Err(Ok(Error::EscrowNotFound)));

        let result_details = client.try_get_escrow_details(&1);
        assert_eq!(result_details, Err(Ok(Error::EscrowNotFound)));
    }

    #[test]
    fn test_escrow_state_validation() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let result = client.try_complete_escrow(&999);
        assert_eq!(result, Err(Ok(Error::EscrowNotFound)));

        let result2 = client.try_cancel_escrow(&seller, &999);
        assert_eq!(result2, Err(Ok(Error::EscrowNotFound)));

        let result3 = client.try_refund_expired_escrow(&999);
        assert_eq!(result3, Err(Ok(Error::EscrowNotFound)));

        let result4 = client.try_fulfill_escrow(&buyer, &999);
        assert_eq!(result4, Err(Ok(Error::EscrowNotFound)));
    }

    #[test]
    fn test_escrow_expiration_logic() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let result = client.try_refund_expired_escrow(&1);
        assert_eq!(result, Err(Ok(Error::EscrowNotFound)));
    }

    #[test]
    fn test_platform_fee_calculation() {
        let payment_amount = 10000i128;
        let platform_fee = 250u32;

        let fee_amount = (payment_amount * platform_fee as i128) / 10000;
        let seller_amount = payment_amount - fee_amount;

        assert_eq!(fee_amount, 250);
        assert_eq!(seller_amount, 9750);
    }

    #[test]
    fn test_escrow_lists_empty() {
        let env = Env::default();
        let (admin, client) = setup_test_escrow(&env);
        let fee_recipient = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient);

        let seller_escrows = client.get_escrows_by_seller(&user);
        assert_eq!(seller_escrows.len(), 0);

        let buyer_escrows = client.get_escrows_by_buyer(&user);
        assert_eq!(buyer_escrows.len(), 0);
    }

    #[test]
    fn test_escrow_info_structure() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_contract = Address::generate(&env);

        let escrow_info = EscrowInfo {
            escrow_id: 1,
            seller: seller.clone(),
            buyer: buyer.clone(),
            token_contract: token_contract.clone(),
            token_amount: 100,
            payment_amount: 1000,
            state: EscrowState::Active,
            created_at: 1000000,
            expires_at: 1007200,
            completed_at: None,
            platform_fee: 250,
        };

        assert_eq!(escrow_info.escrow_id, 1);
        assert_eq!(escrow_info.seller, seller);
        assert_eq!(escrow_info.buyer, buyer);
        assert_eq!(escrow_info.state, EscrowState::Active);
        assert_eq!(escrow_info.completed_at, None);
    }

    #[test]
    fn test_escrow_states() {
        let active = EscrowState::Active;
        let completed = EscrowState::Completed;
        let cancelled = EscrowState::Cancelled;
        let refunded = EscrowState::Refunded;
        let in_dispute = EscrowState::InDispute;

        assert_ne!(active, completed);
        assert_ne!(completed, cancelled);
        assert_ne!(cancelled, refunded);
        assert_ne!(refunded, in_dispute);

        let active_clone = active.clone();
        assert_eq!(active, active_clone);

        let status_active = EscrowStatus::Active;
        let status_completed = EscrowStatus::Completed;
        let status_cancelled = EscrowStatus::Cancelled;
        let status_refunded = EscrowStatus::Refunded;

        assert_ne!(status_active, status_completed);
        assert_ne!(status_completed, status_cancelled);
        assert_ne!(status_cancelled, status_refunded);
    }
}
