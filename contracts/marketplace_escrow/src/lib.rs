#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    Address, Env, Symbol, Vec
};

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

/// Escrow state enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowState {
    /// Escrow is active and awaiting fulfillment
    Active,
    /// Escrow has been completed successfully
    Completed,
    /// Escrow has been cancelled by seller
    Cancelled,
    /// Escrow has been refunded due to expiration
    Refunded,
    /// Escrow is in dispute resolution
    InDispute,
}

/// Storage keys for the Marketplace Escrow contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract administrator address
    Admin,
    /// Total number of escrows created
    EscrowCount,
    /// Individual escrow: Escrow(escrow_id)
    Escrow(u64),
    /// Escrows by seller: EscrowsBySeller(seller_address) -> Vec<u64>
    EscrowsBySeller(Address),
    /// Escrows by buyer: EscrowsByBuyer(buyer_address) -> Vec<u64>
    EscrowsByBuyer(Address),
    /// Contract initialization status
    Initialized,
    /// Minimum escrow timeout (in seconds)
    MinTimeout,
    /// Maximum escrow timeout (in seconds) 
    MaxTimeout,
    /// Platform fee (in basis points, e.g., 250 = 2.5%)
    PlatformFee,
    /// Platform fee recipient
    FeeRecipient,
}

/// Escrow details structure
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
        // Check if already initialized
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        // Require admin authorization
        admin.require_auth();

        // Validate parameters
        if min_timeout == 0 || max_timeout <= min_timeout {
            return Err(Error::InvalidTimeout);
        }

        if platform_fee > 10000 { // Max 100% fee
            return Err(Error::InvalidAmount);
        }

        // Store configuration
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::EscrowCount, &0u64);
        env.storage().instance().set(&DataKey::MinTimeout, &min_timeout);
        env.storage().instance().set(&DataKey::MaxTimeout, &max_timeout);
        env.storage().instance().set(&DataKey::PlatformFee, &platform_fee);
        env.storage().instance().set(&DataKey::FeeRecipient, &fee_recipient);
        env.storage().instance().set(&DataKey::Initialized, &true);

        // Publish initialization event
        env.events().publish(
            (Symbol::new(&env, "escrow_initialized"), admin.clone()),
            (min_timeout, max_timeout, platform_fee, fee_recipient),
        );

        Ok(())
    }

    /// Get contract administrator
    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Get platform configuration
    pub fn get_platform_config(env: Env) -> Result<(u64, u64, u32, Address), Error> {
        let min_timeout: u64 = env.storage().instance().get(&DataKey::MinTimeout)
            .ok_or(Error::Unauthorized)?;
        let max_timeout: u64 = env.storage().instance().get(&DataKey::MaxTimeout)
            .ok_or(Error::Unauthorized)?;
        let platform_fee: u32 = env.storage().instance().get(&DataKey::PlatformFee)
            .ok_or(Error::Unauthorized)?;
        let fee_recipient: Address = env.storage().instance().get(&DataKey::FeeRecipient)
            .ok_or(Error::Unauthorized)?;

        Ok((min_timeout, max_timeout, platform_fee, fee_recipient))
    }

    /// Update platform fee (admin only)
    pub fn update_platform_fee(env: Env, new_fee: u32) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
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

    /// Create a new escrow
    pub fn create_escrow(
        env: Env,
        seller: Address,
        buyer: Address,
        token_contract: Address,
        token_amount: i128,
        payment_amount: i128,
        timeout_seconds: u64,
    ) -> Result<u64, Error> {
        seller.require_auth();

        // Validate parameters
        if token_amount <= 0 || payment_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let min_timeout: u64 = env.storage().instance().get(&DataKey::MinTimeout)
            .ok_or(Error::Unauthorized)?;
        let max_timeout: u64 = env.storage().instance().get(&DataKey::MaxTimeout)
            .ok_or(Error::Unauthorized)?;

        if timeout_seconds < min_timeout || timeout_seconds > max_timeout {
            return Err(Error::InvalidTimeout);
        }

        // Transfer tokens from seller to this contract
        let token_client = TokenClient::new(&env, &token_contract);
        let transfer_result = token_client.try_transfer_from(
            &env.current_contract_address(),
            &seller,
            &env.current_contract_address(),
            &token_amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Create escrow
        let escrow_count: u64 = env.storage().instance().get(&DataKey::EscrowCount).unwrap_or(0);
        let new_escrow_id = escrow_count + 1;
        
        let current_time = env.ledger().timestamp();
        let platform_fee: u32 = env.storage().instance().get(&DataKey::PlatformFee)
            .ok_or(Error::Unauthorized)?;

        let escrow_info = EscrowInfo {
            escrow_id: new_escrow_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            token_contract: token_contract.clone(),
            token_amount,
            payment_amount,
            state: EscrowState::Active,
            created_at: current_time,
            expires_at: current_time + timeout_seconds,
            completed_at: None,
            platform_fee,
        };

        // Store escrow
        env.storage().persistent().set(&DataKey::Escrow(new_escrow_id), &escrow_info);
        env.storage().instance().set(&DataKey::EscrowCount, &new_escrow_id);

        // Extend TTL
        env.storage().persistent().extend_ttl(&DataKey::Escrow(new_escrow_id), 172800, 172800);

        // Update seller and buyer escrow lists
        Self::add_to_seller_escrows(env.clone(), seller.clone(), new_escrow_id);
        Self::add_to_buyer_escrows(env.clone(), buyer.clone(), new_escrow_id);

        // Publish creation event
        env.events().publish(
            (Symbol::new(&env, "escrow_created"), new_escrow_id, seller, buyer),
            escrow_info,
        );

        Ok(new_escrow_id)
    }

    /// Complete escrow (buyer fulfills payment)
    pub fn complete_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let mut escrow_info: EscrowInfo = env.storage().persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(Error::EscrowNotFound)?;

        escrow_info.buyer.require_auth();

        // Check escrow state
        if escrow_info.state != EscrowState::Active {
            return Err(Error::InvalidEscrowState);
        }

        // Check expiration
        if env.ledger().timestamp() > escrow_info.expires_at {
            return Err(Error::EscrowExpired);
        }

        // Calculate platform fee
        let fee_amount = (escrow_info.payment_amount * escrow_info.platform_fee as i128) / 10000;
        let seller_amount = escrow_info.payment_amount - fee_amount;

        // Transfer payment from buyer to seller (minus fee)
        if seller_amount > 0 {
            // Note: In production, this would need to handle native token transfers
            // For now, assuming payment is handled off-chain or via separate token contract
        }

        // Transfer platform fee to fee recipient
        if fee_amount > 0 {
            let fee_recipient: Address = env.storage().instance()
                .get(&DataKey::FeeRecipient)
                .ok_or(Error::Unauthorized)?;
            // Fee transfer logic would go here
        }

        // Transfer escrowed tokens to buyer
        let token_client = TokenClient::new(&env, &escrow_info.token_contract);
        let transfer_result = token_client.try_transfer(
            &env.current_contract_address(),
            &escrow_info.buyer,
            &escrow_info.token_amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Update escrow state
        escrow_info.state = EscrowState::Completed;
        escrow_info.completed_at = Some(env.ledger().timestamp());

        env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow_info);
        env.storage().persistent().extend_ttl(&DataKey::Escrow(escrow_id), 172800, 172800);

        // Publish completion event
        env.events().publish(
            (Symbol::new(&env, "escrow_completed"), escrow_id),
            (seller_amount, fee_amount),
        );

        Ok(())
    }

    /// Cancel escrow (seller cancels before expiration)
    pub fn cancel_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let mut escrow_info: EscrowInfo = env.storage().persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(Error::EscrowNotFound)?;

        escrow_info.seller.require_auth();

        // Check escrow state
        if escrow_info.state != EscrowState::Active {
            return Err(Error::InvalidEscrowState);
        }

        // Return tokens to seller
        let token_client = TokenClient::new(&env, &escrow_info.token_contract);
        let transfer_result = token_client.try_transfer(
            &env.current_contract_address(),
            &escrow_info.seller,
            &escrow_info.token_amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Update escrow state
        escrow_info.state = EscrowState::Cancelled;
        escrow_info.completed_at = Some(env.ledger().timestamp());

        env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow_info);
        env.storage().persistent().extend_ttl(&DataKey::Escrow(escrow_id), 172800, 172800);

        // Publish cancellation event
        env.events().publish(
            (Symbol::new(&env, "escrow_cancelled"), escrow_id),
            escrow_info.seller,
        );

        Ok(())
    }

    /// Refund expired escrow (anyone can call after expiration)
    pub fn refund_expired_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let mut escrow_info: EscrowInfo = env.storage().persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(Error::EscrowNotFound)?;

        // Check escrow state
        if escrow_info.state != EscrowState::Active {
            return Err(Error::InvalidEscrowState);
        }

        // Check expiration
        if env.ledger().timestamp() <= escrow_info.expires_at {
            return Err(Error::EscrowNotExpired);
        }

        // Return tokens to seller
        let token_client = TokenClient::new(&env, &escrow_info.token_contract);
        let transfer_result = token_client.try_transfer(
            &env.current_contract_address(),
            &escrow_info.seller,
            &escrow_info.token_amount,
        );

        if transfer_result.is_err() {
            return Err(Error::TransferFailed);
        }

        // Update escrow state
        escrow_info.state = EscrowState::Refunded;
        escrow_info.completed_at = Some(env.ledger().timestamp());

        env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow_info);
        env.storage().persistent().extend_ttl(&DataKey::Escrow(escrow_id), 172800, 172800);

        // Publish refund event
        env.events().publish(
            (Symbol::new(&env, "escrow_refunded"), escrow_id),
            escrow_info.seller,
        );

        Ok(())
    }

    /// Get escrow information
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<EscrowInfo, Error> {
        env.storage().persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(Error::EscrowNotFound)
    }

    /// Get total escrow count
    pub fn escrow_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::EscrowCount).unwrap_or(0)
    }

    /// Get escrows by seller
    pub fn get_escrows_by_seller(env: Env, seller: Address) -> Vec<u64> {
        env.storage().persistent()
            .get(&DataKey::EscrowsBySeller(seller))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get escrows by buyer  
    pub fn get_escrows_by_buyer(env: Env, buyer: Address) -> Vec<u64> {
        env.storage().persistent()
            .get(&DataKey::EscrowsByBuyer(buyer))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Internal: Add escrow to seller's list
    fn add_to_seller_escrows(env: Env, seller: Address, escrow_id: u64) {
        let mut escrows = Self::get_escrows_by_seller(env.clone(), seller.clone());
        escrows.push_back(escrow_id);
        env.storage().persistent().set(&DataKey::EscrowsBySeller(seller.clone()), &escrows);
        env.storage().persistent().extend_ttl(&DataKey::EscrowsBySeller(seller), 172800, 172800);
    }

    /// Internal: Add escrow to buyer's list
    fn add_to_buyer_escrows(env: Env, buyer: Address, escrow_id: u64) {
        let mut escrows = Self::get_escrows_by_buyer(env.clone(), buyer.clone());
        escrows.push_back(escrow_id);
        env.storage().persistent().set(&DataKey::EscrowsByBuyer(buyer.clone()), &escrows);
        env.storage().persistent().extend_ttl(&DataKey::EscrowsByBuyer(buyer), 172800, 172800);
    }
}

/// Token client interface for interacting with RWA tokens
#[soroban_sdk::contractclient(name = "TokenClient")]
pub trait TokenInterface {
    fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), soroban_sdk::Val>;
    fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) -> Result<(), soroban_sdk::Val>;
    fn approve(env: Env, owner: Address, spender: Address, amount: i128) -> Result<(), soroban_sdk::Val>;
    fn balance(env: Env, account: Address) -> i128;
}