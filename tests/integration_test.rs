#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Env, String, BytesN};

// Import contract types and clients
use asset_factory::{AssetFactoryContract, AssetFactoryContractClient};
use rwa_token::{RwaTokenContract, RwaTokenContractClient};
use marketplace_escrow::{MarketplaceEscrowContract, MarketplaceEscrowContractClient, EscrowState};

/// Integration test demonstrating the complete AegisMint platform workflow
#[test]
fn test_complete_rwa_marketplace_workflow() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup accounts
    let platform_admin = Address::generate(&env);
    let asset_deployer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);
    let fee_recipient = Address::generate(&env);

    // Deploy Asset Factory Contract
    let factory_id = env.register(AssetFactoryContract, ());
    let factory_client = AssetFactoryContractClient::new(&env, &factory_id);

    // Deploy RWA Token Contract (this will be used as the approved WASM)
    let rwa_token_id = env.register(RwaTokenContract, ());
    let rwa_wasm_hash = env.deployer().upload_contract_wasm(rwa_token_id);

    // Deploy Marketplace Escrow Contract
    let escrow_id = env.register(MarketplaceEscrowContract, ());
    let escrow_client = MarketplaceEscrowContractClient::new(&env, &escrow_id);

    // Step 1: Initialize Asset Factory
    factory_client.initialize(&platform_admin, &rwa_wasm_hash).unwrap();

    // Step 2: Initialize Marketplace Escrow
    let min_timeout = 3600u64; // 1 hour
    let max_timeout = 604800u64; // 1 week
    let platform_fee = 250u32; // 2.5%
    
    escrow_client.initialize(
        &platform_admin,
        &min_timeout,
        &max_timeout,
        &platform_fee,
        &fee_recipient,
    ).unwrap();

    // Step 3: Deploy RWA Token via Factory
    let salt = BytesN::from_array(&env, &[1u8; 32]);
    let token_name = String::from_str(&env, "Real Estate Portfolio Token");
    let token_symbol = String::from_str(&env, "REPT");
    let token_decimals = 8u32;
    let token_supply = 1_000_000i128;

    let deployed_token_address = factory_client.deploy_rwa_token(
        &asset_deployer,
        &salt,
        &token_admin,
        &token_name,
        &token_symbol,
        &token_decimals,
        &token_supply,
    ).unwrap();

    // Create client for the deployed token
    let token_client = RwaTokenContractClient::new(&env, &deployed_token_address);

    // Verify token deployment and initialization
    let token_info = token_client.token_info().unwrap();
    assert_eq!(token_info.name, token_name);
    assert_eq!(token_info.symbol, token_symbol);
    assert_eq!(token_info.total_supply, token_supply);
    assert_eq!(token_client.balance(&token_admin), token_supply);
    assert!(token_client.is_whitelisted(&token_admin));

    // Verify factory records
    assert_eq!(factory_client.asset_count(), 1);
    let asset_info = factory_client.get_asset(&1).unwrap();
    assert_eq!(asset_info.contract_address, deployed_token_address);
    assert_eq!(asset_info.deployer, asset_deployer);

    // Step 4: Whitelist investors for token transfers
    token_client.add_to_whitelist(&investor1).unwrap();
    token_client.add_to_whitelist(&investor2).unwrap();
    token_client.add_to_whitelist(&escrow_id).unwrap(); // Whitelist escrow contract

    // Step 5: Distribute tokens to investors
    let investor1_amount = 100_000i128;
    let investor2_amount = 150_000i128;
    
    token_client.transfer(&token_admin, &investor1, &investor1_amount).unwrap();
    token_client.transfer(&token_admin, &investor2, &investor2_amount).unwrap();

    // Verify balances
    assert_eq!(token_client.balance(&investor1), investor1_amount);
    assert_eq!(token_client.balance(&investor2), investor2_amount);

    // Step 6: Create escrow for P2P trading
    // investor1 wants to sell 50,000 tokens to investor2 for 500,000 payment units
    let escrow_token_amount = 50_000i128;
    let escrow_payment_amount = 500_000i128;
    let escrow_timeout = 7200u64; // 2 hours

    // First, investor1 needs to approve the escrow contract to transfer their tokens
    token_client.approve(&investor1, &escrow_id, &escrow_token_amount).unwrap();

    // Create the escrow
    let escrow_order_id = escrow_client.create_escrow(
        &investor1, // seller
        &investor2, // buyer
        &deployed_token_address,
        &escrow_token_amount,
        &escrow_payment_amount,
        &escrow_timeout,
    ).unwrap();

    // Verify escrow creation
    let escrow_info = escrow_client.get_escrow(&escrow_order_id).unwrap();
    assert_eq!(escrow_info.seller, investor1);
    assert_eq!(escrow_info.buyer, investor2);
    assert_eq!(escrow_info.token_amount, escrow_token_amount);
    assert_eq!(escrow_info.state, EscrowState::Active);

    // Verify tokens were transferred to escrow
    assert_eq!(token_client.balance(&investor1), investor1_amount - escrow_token_amount);
    assert_eq!(token_client.balance(&escrow_id), escrow_token_amount);

    // Step 7: Complete the escrow (buyer fulfills payment)
    escrow_client.complete_escrow(&escrow_order_id).unwrap();

    // Verify escrow completion
    let completed_escrow = escrow_client.get_escrow(&escrow_order_id).unwrap();
    assert_eq!(completed_escrow.state, EscrowState::Completed);
    assert!(completed_escrow.completed_at.is_some());

    // Verify final token balances
    assert_eq!(token_client.balance(&investor2), investor2_amount + escrow_token_amount);
    assert_eq!(token_client.balance(&escrow_id), 0); // Escrow should be empty

    // Verify escrow tracking
    let investor1_escrows = escrow_client.get_escrows_by_seller(&investor1);
    assert_eq!(investor1_escrows.len(), 1);
    assert_eq!(investor1_escrows.get(0).unwrap(), escrow_order_id);

    let investor2_escrows = escrow_client.get_escrows_by_buyer(&investor2);
    assert_eq!(investor2_escrows.len(), 1);
    assert_eq!(investor2_escrows.get(0).unwrap(), escrow_order_id);

    // Step 8: Test additional factory functionality
    // Verify factory deployment tracking
    assert!(factory_client.is_factory_deployed(&deployed_token_address));
    
    let factory_asset = factory_client.get_asset_by_address(&deployed_token_address).unwrap();
    assert_eq!(factory_asset.asset_id, 1);

    let deployer_assets = factory_client.get_assets_by_deployer(&asset_deployer);
    assert_eq!(deployer_assets.len(), 1);
    assert_eq!(deployer_assets.get(0).unwrap().asset_id, 1);
}

/// Test error scenarios and edge cases
#[test]
fn test_integration_error_scenarios() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let fee_recipient = Address::generate(&env);

    // Deploy contracts
    let factory_id = env.register(AssetFactoryContract, ());
    let factory_client = AssetFactoryContractClient::new(&env, &factory_id);

    let rwa_token_id = env.register(RwaTokenContract, ());
    let rwa_wasm_hash = env.deployer().upload_contract_wasm(rwa_token_id);

    let escrow_id = env.register(MarketplaceEscrowContract, ());
    let escrow_client = MarketplaceEscrowContractClient::new(&env, &escrow_id);

    // Initialize contracts
    factory_client.initialize(&admin, &rwa_wasm_hash).unwrap();
    escrow_client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient).unwrap();

    // Test: Creating escrow with non-whitelisted participants
    // This would fail in the token transfer, but we test the escrow validation
    let result = escrow_client.create_escrow(
        &user1,
        &user2,
        &rwa_token_id, // Using the template contract (not deployed token)
        &1000i128,
        &10000i128,
        &7200u64,
    );
    // This should fail because the token contract doesn't exist or transfer fails
    assert!(result.is_err());

    // Test: Invalid escrow parameters
    let result = escrow_client.create_escrow(
        &user1,
        &user2,
        &rwa_token_id,
        &0i128, // Invalid: zero amount
        &10000i128,
        &7200u64,
    );
    assert!(result.is_err());
}

/// Test cross-contract authorization patterns
#[test]
fn test_authorization_patterns() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    let fee_recipient = Address::generate(&env);

    // Deploy and initialize RWA token
    let token_id = env.register(RwaTokenContract, ());
    let token_client = RwaTokenContractClient::new(&env, &token_id);

    let token_name = String::from_str(&env, "Test Token");
    let token_symbol = String::from_str(&env, "TEST");
    token_client.initialize(&admin, &token_name, &token_symbol, &8u32, &1000000i128).unwrap();

    // Deploy and initialize escrow
    let escrow_id = env.register(MarketplaceEscrowContract, ());
    let escrow_client = MarketplaceEscrowContractClient::new(&env, &escrow_id);
    escrow_client.initialize(&admin, &3600u64, &604800u64, &250u32, &fee_recipient).unwrap();

    // Test unauthorized operations
    // Note: In mock environment, auths are mocked, so we test the structure
    // In real deployment, these would fail due to authorization checks

    // Verify admin functions exist and have proper structure
    assert!(token_client.admin().is_ok());
    assert!(escrow_client.admin().is_ok());

    // Test configuration updates (admin only)
    let result = escrow_client.update_platform_fee(&300u32);
    assert!(result.is_ok()); // Passes because auths are mocked

    // Verify platform configuration
    let config = escrow_client.get_platform_config().unwrap();
    assert_eq!(config.2, 300u32); // Platform fee updated
}

/// Test contract deployment and factory integration
#[test]
fn test_factory_deployment_integration() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let deployer = Address::generate(&env);
    let token_admin = Address::generate(&env);

    // Deploy factory
    let factory_id = env.register(AssetFactoryContract, ());
    let factory_client = AssetFactoryContractClient::new(&env, &factory_id);

    let rwa_token_id = env.register(RwaTokenContract, ());
    let rwa_wasm_hash = env.deployer().upload_contract_wasm(rwa_token_id);

    factory_client.initialize(&admin, &rwa_wasm_hash).unwrap();

    // Test multiple deployments with different salts
    let salt1 = BytesN::from_array(&env, &[1u8; 32]);
    let salt2 = BytesN::from_array(&env, &[2u8; 32]);

    let token1_addr = factory_client.deploy_rwa_token(
        &deployer,
        &salt1,
        &token_admin,
        &String::from_str(&env, "Token 1"),
        &String::from_str(&env, "TKN1"),
        &8u32,
        &1000000i128,
    ).unwrap();

    let token2_addr = factory_client.deploy_rwa_token(
        &deployer,
        &salt2,
        &token_admin,
        &String::from_str(&env, "Token 2"),
        &String::from_str(&env, "TKN2"),
        &6u32,
        &2000000i128,
    ).unwrap();

    // Verify deployments are tracked correctly
    assert_ne!(token1_addr, token2_addr);
    assert_eq!(factory_client.asset_count(), 2);

    let assets = factory_client.get_assets(&0, &10);
    assert_eq!(assets.len(), 2);

    // Verify both contracts are properly initialized
    let token1_client = RwaTokenContractClient::new(&env, &token1_addr);
    let token2_client = RwaTokenContractClient::new(&env, &token2_addr);

    let info1 = token1_client.token_info().unwrap();
    let info2 = token2_client.token_info().unwrap();

    assert_eq!(info1.decimals, 8);
    assert_eq!(info1.total_supply, 1000000);
    assert_eq!(info2.decimals, 6);
    assert_eq!(info2.total_supply, 2000000);
}