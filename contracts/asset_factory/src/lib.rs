#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

/// Storage key types for the Asset Factory contract
#[contracttype]
pub enum StorageKey {
    Admin,
    AssetCount,
    Asset(u64),
}

/// Asset metadata structure
#[contracttype]
#[derive(Clone)]
pub struct Asset {
    pub id: u64,
    pub token_address: Address,
    pub owner: Address,
    pub name: String,
    pub symbol: String,
    pub total_supply: i128,
    pub metadata_uri: String,
    pub created_at: u64,
}

#[contract]
pub struct AssetFactory;

#[contractimpl]
impl AssetFactory {
    /// Initialize the asset factory contract
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&StorageKey::Admin) {
            panic!("Contract already initialized");
        }
        
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage().instance().set(&StorageKey::AssetCount, &0u64);
    }

    /// Create a new RWA token
    pub fn create_asset(
        env: Env,
        owner: Address,
        name: String,
        symbol: String,
        total_supply: i128,
        metadata_uri: String,
    ) -> u64 {
        owner.require_auth();

        let asset_count: u64 = env
            .storage()
            .instance()
            .get(&StorageKey::AssetCount)
            .unwrap_or(0);

        let asset_id = asset_count + 1;

        // In production, this would deploy a new RWA token contract
        // For now, we'll create a placeholder token address
        let token_address = env.current_contract_address();

        let asset = Asset {
            id: asset_id,
            token_address,
            owner: owner.clone(),
            name,
            symbol,
            total_supply,
            metadata_uri,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&StorageKey::Asset(asset_id), &asset);
        env.storage()
            .instance()
            .set(&StorageKey::AssetCount, &asset_id);

        asset_id
    }

    /// Get asset details by ID
    pub fn get_asset(env: Env, asset_id: u64) -> Option<Asset> {
        env.storage().instance().get(&StorageKey::Asset(asset_id))
    }

    /// Get total number of assets created
    pub fn get_asset_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&StorageKey::AssetCount)
            .unwrap_or(0)
    }

    /// Get all assets (paginated)
    pub fn get_assets(env: Env, start: u64, limit: u64) -> Vec<Asset> {
        let mut assets = Vec::new(&env);
        let asset_count = Self::get_asset_count(env.clone());

        let end = start.saturating_add(limit).min(asset_count);

        for i in start..end {
            if let Some(asset) = env.storage().instance().get(&StorageKey::Asset(i + 1)) {
                assets.push_back(asset);
            }
        }

        assets
    }

    /// Get contract admin
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Contract not initialized")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AssetFactory);
        let client = AssetFactoryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_asset_count(), 0);
    }

    #[test]
    fn test_create_asset() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AssetFactory);
        let client = AssetFactoryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let owner = Address::generate(&env);

        client.initialize(&admin);

        let asset_id = client.create_asset(
            &owner,
            &String::from_str(&env, "Real Estate Token"),
            &String::from_str(&env, "RET"),
            &1000000,
            &String::from_str(&env, "ipfs://metadata"),
        );

        assert_eq!(asset_id, 1);
        assert_eq!(client.get_asset_count(), 1);

        let asset = client.get_asset(&asset_id).unwrap();
        assert_eq!(asset.owner, owner);
        assert_eq!(asset.total_supply, 1000000);
    }
}
