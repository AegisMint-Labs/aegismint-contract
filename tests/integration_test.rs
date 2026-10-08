#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger as _, storage::Persistent as _},
    Address, BytesN, Env, String,
};

#[path = "../contracts/asset_factory/src/lib.rs"]
mod asset_factory;

#[path = "../contracts/rwa_token/src/lib.rs"]
mod rwa_token;

#[path = "../contracts/marketplace_escrow/src/lib.rs"]
mod marketplace_escrow;

use asset_factory::{AssetFactoryContract, AssetFactoryContractClient, Error as FactoryError};
use rwa_token::{RwaTokenContract, RwaTokenContractClient, Error as TokenError, DataKey as TokenKey, STORAGE_TTL_EXTEND_TO};
use marketplace_escrow::{MarketplaceEscrowContract, MarketplaceEscrowContractClient, EscrowState, Error as EscrowError};

/// Integration Test 1: RWA Token Whitelisting and Storage TTL Lifecycle
#[test]
fn test_rwa_token_whitelisting_and_ttl_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);
    let non_whitelisted = Address::generate(&env);

    // Deploy RWA Token
    let token_id = env.register(RwaTokenContract, ());
    let token_client = RwaTokenContractClient::new(&env, &token_id);

    let name = String::from_str(&env, "Aegis Real Estate Token");
    let symbol = String::from_str(&env, "ARET");
    let initial_supply = 1_000_000i128;

    token_client.initialize(&admin, &name, &symbol, &8u32, &initial_supply);

    // Verify token info & admin initial state
    let info = token_client.token_info();
    assert_eq!(info.name, name);
    assert_eq!(info.symbol, symbol);
    assert_eq!(info.total_supply, initial_supply);
    assert_eq!(token_client.balance(&admin), initial_supply);
    assert!(token_client.is_whitelisted(&admin));

    // Whitelist investor1 via set_whitelist
    token_client.set_whitelist(&investor1, &true);
    assert!(token_client.is_whitelisted(&investor1));

    // Whitelist investor2 via add_to_whitelist
    token_client.add_to_whitelist(&investor2);
    assert!(token_client.is_whitelisted(&investor2));

    // Transfer to non-whitelisted account must fail
    let err_transfer = token_client.try_transfer(&admin, &non_whitelisted, &50_000i128);
    assert_eq!(err_transfer, Err(Ok(TokenError::NotWhitelisted)));

    // Transfer from admin to investor1 succeeds
    token_client.transfer(&admin, &investor1, &200_000i128);
    assert_eq!(token_client.balance(&investor1), 200_000);
    assert_eq!(token_client.balance(&admin), 800_000);

    // Advance ledger to simulate time elapsed and verify TTL degradation
    env.ledger().with_mut(|li| {
        li.sequence_number += 50_000;
    });

    env.as_contract(&token_id, || {
        let key_bal = TokenKey::Balance(investor1.clone());
        let ttl = env.storage().persistent().get_ttl(&key_bal);
        assert_eq!(ttl, STORAGE_TTL_EXTEND_TO - 50_000);
    });

    // Querying balance restores TTL
    assert_eq!(token_client.balance(&investor1), 200_000);
    env.as_contract(&token_id, || {
        let key_bal = TokenKey::Balance(investor1.clone());
        assert_eq!(env.storage().persistent().get_ttl(&key_bal), STORAGE_TTL_EXTEND_TO);
    });

    // Transfer between investor1 and investor2 extends TTL for both balances and whitelists
    token_client.transfer(&investor1, &investor2, &50_000i128);
    assert_eq!(token_client.balance(&investor1), 150_000);
    assert_eq!(token_client.balance(&investor2), 50_000);

    env.as_contract(&token_id, || {
        assert_eq!(env.storage().persistent().get_ttl(&TokenKey::Balance(investor1.clone())), STORAGE_TTL_EXTEND_TO);
        assert_eq!(env.storage().persistent().get_ttl(&TokenKey::Balance(investor2.clone())), STORAGE_TTL_EXTEND_TO);
        assert_eq!(env.storage().persistent().get_ttl(&TokenKey::Whitelisted(investor1.clone())), STORAGE_TTL_EXTEND_TO);
        assert_eq!(env.storage().persistent().get_ttl(&TokenKey::Whitelisted(investor2.clone())), STORAGE_TTL_EXTEND_TO);
    });

    // Revoke whitelist for investor2
    token_client.set_whitelist(&investor2, &false);
    assert!(!token_client.is_whitelisted(&investor2));

    // Subsequent transfer to investor2 should now fail
    let err_revoked = token_client.try_transfer(&investor1, &investor2, &10_000i128);
    assert_eq!(err_revoked, Err(Ok(TokenError::NotWhitelisted)));
}

/// Integration Test 2: Marketplace Escrow Complete Fulfillment Workflow
#[test]
fn test_marketplace_escrow_complete_fulfillment() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Deploy RWA Token
    let token_id = env.register(RwaTokenContract, ());
    let token_client = RwaTokenContractClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Treasury Bond Token"),
        &String::from_str(&env, "TBT"),
        &6u32,
        &5_000_000i128,
    );

    // Deploy Marketplace Escrow
    let escrow_id = env.register(MarketplaceEscrowContract, ());
    let escrow_client = MarketplaceEscrowContractClient::new(&env, &escrow_id);

    let min_timeout = 3600u64;
    let max_timeout = 604800u64;
    let platform_fee = 250u32; // 2.5%

    escrow_client.initialize(&admin, &min_timeout, &max_timeout, &platform_fee, &fee_recipient);

    // Whitelist participants and escrow contract on token
    token_client.set_whitelist(&seller, &true);
    token_client.set_whitelist(&buyer, &true);
    token_client.set_whitelist(&escrow_id, &true);

    // Fund seller
    let seller_initial_balance = 500_000i128;
    token_client.transfer(&admin, &seller, &seller_initial_balance);
    assert_eq!(token_client.balance(&seller), seller_initial_balance);

    // Seller approves escrow contract to deposit tokens
    let escrow_token_amount = 100_000i128;
    let escrow_payment_amount = 250_000i128;
    let timeout_seconds = 7200u64;

    token_client.approve(&seller, &escrow_id, &escrow_token_amount);
    assert_eq!(token_client.allowance(&seller, &escrow_id), escrow_token_amount);

    // Create escrow order
    let order_id = escrow_client.create_escrow(
        &seller,
        &buyer,
        &token_id,
        &escrow_token_amount,
        &escrow_payment_amount,
        &timeout_seconds,
    );
    assert_eq!(order_id, 1);

    // Tokens transferred into escrow contract
    assert_eq!(token_client.balance(&seller), seller_initial_balance - escrow_token_amount);
    assert_eq!(token_client.balance(&escrow_id), escrow_token_amount);

    // Verify escrow state
    let escrow_data = escrow_client.get_escrow(&order_id);
    assert_eq!(escrow_data.seller, seller);
    assert_eq!(escrow_data.buyer, buyer);
    assert_eq!(escrow_data.token_contract, token_id);
    assert_eq!(escrow_data.token_amount, escrow_token_amount);
    assert_eq!(escrow_data.payment_amount, escrow_payment_amount);
    assert_eq!(escrow_data.state, EscrowState::Active);

    // Complete escrow (buyer fulfills payment)
    escrow_client.complete_escrow(&order_id);

    // Verify final state and token delivery
    let completed_data = escrow_client.get_escrow(&order_id);
    assert_eq!(completed_data.state, EscrowState::Completed);
    assert!(completed_data.completed_at.is_some());

    assert_eq!(token_client.balance(&buyer), escrow_token_amount);
    assert_eq!(token_client.balance(&escrow_id), 0);

    // Query seller and buyer lists
    let seller_orders = escrow_client.get_escrows_by_seller(&seller);
    assert_eq!(seller_orders.len(), 1);
    assert_eq!(seller_orders.get(0).unwrap(), order_id);

    let buyer_orders = escrow_client.get_escrows_by_buyer(&buyer);
    assert_eq!(buyer_orders.len(), 1);
    assert_eq!(buyer_orders.get(0).unwrap(), order_id);
}

/// Integration Test 3: Marketplace Escrow Seller Cancellation Mechanism
#[test]
fn test_marketplace_escrow_seller_cancellation() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let token_id = env.register(RwaTokenContract, ());
    let token_client = RwaTokenContractClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Solar Farm Debt Token"),
        &String::from_str(&env, "SFDT"),
        &8u32,
        &2_000_000i128,
    );

    let escrow_id = env.register(MarketplaceEscrowContract, ());
    let escrow_client = MarketplaceEscrowContractClient::new(&env, &escrow_id);
    escrow_client.initialize(&admin, &3600u64, &604800u64, &200u32, &fee_recipient);

    token_client.set_whitelist(&seller, &true);
    token_client.set_whitelist(&buyer, &true);
    token_client.set_whitelist(&escrow_id, &true);

    let seller_deposit = 75_000i128;
    token_client.transfer(&admin, &seller, &seller_deposit);

    token_client.approve(&seller, &escrow_id, &seller_deposit);
    let order_id = escrow_client.create_escrow(
        &seller,
        &buyer,
        &token_id,
        &seller_deposit,
        &150_000i128,
        &7200u64,
    );

    assert_eq!(token_client.balance(&seller), 0);
    assert_eq!(token_client.balance(&escrow_id), seller_deposit);

    // Seller cancels escrow
    escrow_client.cancel_escrow(&order_id);

    // Verify cancellation state and refunded token balance
    let cancelled_data = escrow_client.get_escrow(&order_id);
    assert_eq!(cancelled_data.state, EscrowState::Cancelled);
    assert!(cancelled_data.completed_at.is_some());

    assert_eq!(token_client.balance(&seller), seller_deposit);
    assert_eq!(token_client.balance(&escrow_id), 0);

    // Attempting to cancel again should fail
    let err_double_cancel = escrow_client.try_cancel_escrow(&order_id);
    assert_eq!(err_double_cancel, Err(Ok(EscrowError::InvalidEscrowState)));

    // Attempting to complete cancelled escrow should fail
    let err_complete = escrow_client.try_complete_escrow(&order_id);
    assert_eq!(err_complete, Err(Ok(EscrowError::InvalidEscrowState)));
}

/// Integration Test 4: Asset Factory Governance and Deterministic Parameter Validation
#[test]
fn test_asset_factory_management_and_validation() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let deployer = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let factory_id = env.register(AssetFactoryContract, ());
    let factory_client = AssetFactoryContractClient::new(&env, &factory_id);

    let initial_wasm = BytesN::from_array(&env, &[10u8; 32]);
    let updated_wasm = BytesN::from_array(&env, &[20u8; 32]);

    factory_client.initialize(&admin, &initial_wasm);

    assert_eq!(factory_client.admin(), admin);
    assert_eq!(factory_client.approved_wasm_hash(), initial_wasm);
    assert_eq!(factory_client.asset_count(), 0);

    // Update approved WASM hash
    factory_client.update_approved_wasm_hash(&updated_wasm);
    assert_eq!(factory_client.approved_wasm_hash(), updated_wasm);

    // Parameter validation on deployment
    let salt = BytesN::from_array(&env, &[1u8; 32]);
    let name = String::from_str(&env, "Invalid Token");
    let symbol = String::from_str(&env, "INV");

    let err_zero_supply = factory_client.try_deploy_rwa_token(
        &deployer,
        &salt,
        &token_admin,
        &name,
        &symbol,
        &8u32,
        &0i128,
    );
    assert_eq!(err_zero_supply, Err(Ok(FactoryError::InvalidParameters)));

    let err_neg_supply = factory_client.try_deploy_rwa_token(
        &deployer,
        &salt,
        &token_admin,
        &name,
        &symbol,
        &8u32,
        &-1000i128,
    );
    assert_eq!(err_neg_supply, Err(Ok(FactoryError::InvalidParameters)));

    // Factory asset queries for unpopulated state
    let empty_assets = factory_client.get_assets(&0, &10);
    assert_eq!(empty_assets.len(), 0);

    let deployer_assets = factory_client.get_assets_by_deployer(&deployer);
    assert_eq!(deployer_assets.len(), 0);

    let random_addr = Address::generate(&env);
    assert!(!factory_client.is_factory_deployed(&random_addr));
}

/// Integration Test 5: End-to-End Multi-Contract Platform Interaction
#[test]
fn test_end_to_end_aegismint_platform_integration() {
    let env = Env::default();
    env.mock_all_auths();

    let platform_admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let issuer = Address::generate(&env);
    let primary_buyer = Address::generate(&env);
    let secondary_buyer = Address::generate(&env);

    // Deploy contracts
    let token_id = env.register(RwaTokenContract, ());
    let token_client = RwaTokenContractClient::new(&env, &token_id);

    let escrow_id = env.register(MarketplaceEscrowContract, ());
    let escrow_client = MarketplaceEscrowContractClient::new(&env, &escrow_id);

    let factory_id = env.register(AssetFactoryContract, ());
    let factory_client = AssetFactoryContractClient::new(&env, &factory_id);

    // Initialize all contracts
    let wasm_hash = BytesN::from_array(&env, &[99u8; 32]);
    factory_client.initialize(&platform_admin, &wasm_hash);

    escrow_client.initialize(
        &platform_admin,
        &1800u64,
        &864000u64,
        &150u32, // 1.5%
        &fee_recipient,
    );

    token_client.initialize(
        &issuer,
        &String::from_str(&env, "Commercial Property Yield"),
        &String::from_str(&env, "CPY"),
        &7u32,
        &10_000_000i128,
    );

    // Configure whitelists
    token_client.set_whitelist(&primary_buyer, &true);
    token_client.set_whitelist(&secondary_buyer, &true);
    token_client.set_whitelist(&escrow_id, &true);

    // Issue initial allocation to primary buyer
    token_client.transfer(&issuer, &primary_buyer, &1_000_000i128);
    assert_eq!(token_client.balance(&primary_buyer), 1_000_000);

    // Primary buyer lists 400,000 tokens for secondary buyer
    let sale_amount = 400_000i128;
    let payment_amount = 1_200_000i128;
    let timeout = 14400u64;

    token_client.approve(&primary_buyer, &escrow_id, &sale_amount);
    let order_id = escrow_client.create_escrow(
        &primary_buyer,
        &secondary_buyer,
        &token_id,
        &sale_amount,
        &payment_amount,
        &timeout,
    );

    // Verify token lock
    assert_eq!(token_client.balance(&primary_buyer), 600_000);
    assert_eq!(token_client.balance(&escrow_id), sale_amount);

    // Secondary buyer completes purchase
    escrow_client.complete_escrow(&order_id);

    // Secondary buyer now holds tokens
    assert_eq!(token_client.balance(&secondary_buyer), sale_amount);
    assert_eq!(token_client.balance(&escrow_id), 0);

    // Update platform fee by admin
    escrow_client.update_platform_fee(&200u32);
    let config = escrow_client.get_platform_config();
    assert_eq!(config.2, 200u32);
}
