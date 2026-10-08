#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    Address, BytesN, Env, String, Symbol, Vec
};

/// Error types for the Asset Factory contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract is already initialized
    AlreadyInitialized = 1,
    /// Caller is not authorized to perform this action
    Unauthorized = 2,
    /// Invalid WASM hash provided
    InvalidWasmHash = 3,
    /// Deployment failed
    DeploymentFailed = 4,
    /// Asset not found
    AssetNotFound = 5,
    /// Invalid parameters provided
    InvalidParameters = 6,
}

/// Storage keys for the Asset Factory contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract administrator address
    Admin,
    /// Approved WASM hash for RWA token deployments
    ApprovedWasm,
    /// Total number of deployed assets
    AssetCount,
    /// Asset registry: Asset(asset_id) -> AssetInfo
    Asset(u64),
    /// Asset by address mapping: AssetByAddress(contract_address) -> asset_id
    AssetByAddress(Address),
    /// Contract initialization status
    Initialized,
}

/// Deployed asset information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetInfo {
    pub asset_id: u64,
    pub contract_address: Address,
    pub deployer: Address,
    pub admin: Address,
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub total_supply: i128,
    pub deployed_at: u64,
    pub salt: BytesN<32>,
}

/// Asset Factory Contract
#[contract]
pub struct AssetFactoryContract;

#[contractimpl]
impl AssetFactoryContract {
    /// Initialize the asset factory contract
    pub fn initialize(
        env: Env,
        admin: Address,
        approved_wasm_hash: BytesN<32>,
    ) -> Result<(), Error> {
        // Check if already initialized
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        // Require admin authorization
        admin.require_auth();

        // Store configuration
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ApprovedWasm, &approved_wasm_hash);
        env.storage().instance().set(&DataKey::AssetCount, &0u64);
        env.storage().instance().set(&DataKey::Initialized, &true);

        // Publish initialization event
        env.events().publish(
            (Symbol::new(&env, "factory_initialized"), admin.clone()),
            approved_wasm_hash,
        );

        Ok(())
    }

    /// Get the factory administrator
    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Get the approved WASM hash
    pub fn approved_wasm_hash(env: Env) -> Result<BytesN<32>, Error> {
        env.storage().instance().get(&DataKey::ApprovedWasm)
            .ok_or(Error::Unauthorized)
    }

    /// Update the approved WASM hash (admin only)
    pub fn update_approved_wasm_hash(
        env: Env,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        admin.require_auth();

        env.storage().instance().set(&DataKey::ApprovedWasm, &new_wasm_hash);

        // Publish update event
        env.events().publish(
            (Symbol::new(&env, "wasm_updated"), admin),
            new_wasm_hash,
        );

        Ok(())
    }

    /// Deploy a new RWA token instance
    pub fn deploy_rwa_token(
        env: Env,
        deployer: Address,
        salt: BytesN<32>,
        token_admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        total_supply: i128,
    ) -> Result<Address, Error> {
        deployer.require_auth();

        // Validate parameters
        if total_supply <= 0 {
            return Err(Error::InvalidParameters);
        }

        // Get approved WASM hash
        let wasm_hash: BytesN<32> = env.storage().instance().get(&DataKey::ApprovedWasm)
            .ok_or(Error::InvalidWasmHash)?;

        // Deploy the contract using deterministic deployment
        let deployed_address = env
            .deployer()
            .with_address(deployer.clone(), salt.clone())
            .deploy(wasm_hash);

        // Initialize the deployed RWA token contract
        let rwa_client = RwaTokenClient::new(&env, &deployed_address);
        let init_result = rwa_client.try_initialize(
            &token_admin,
            &name,
            &symbol,
            &decimals,
            &total_supply,
        );

        if init_result.is_err() {
            return Err(Error::DeploymentFailed);
        }

        // Increment asset count
        let asset_count: u64 = env.storage().instance().get(&DataKey::AssetCount).unwrap_or(0);
        let new_asset_id = asset_count + 1;
        env.storage().instance().set(&DataKey::AssetCount, &new_asset_id);

        // Store asset information
        let asset_info = AssetInfo {
            asset_id: new_asset_id,
            contract_address: deployed_address.clone(),
            deployer: deployer.clone(),
            admin: token_admin.clone(),
            name: name.clone(),
            symbol: symbol.clone(),
            decimals,
            total_supply,
            deployed_at: env.ledger().timestamp(),
            salt,
        };

        env.storage().instance().set(&DataKey::Asset(new_asset_id), &asset_info);
        env.storage().instance().set(&DataKey::AssetByAddress(deployed_address.clone()), &new_asset_id);

        // Publish deployment event
        env.events().publish(
            (Symbol::new(&env, "asset_deployed"), deployed_address.clone(), deployer),
            asset_info,
        );

        Ok(deployed_address)
    }

    /// Get asset information by ID
    pub fn get_asset(env: Env, asset_id: u64) -> Result<AssetInfo, Error> {
        env.storage().instance().get(&DataKey::Asset(asset_id))
            .ok_or(Error::AssetNotFound)
    }

    /// Get asset information by contract address
    pub fn get_asset_by_address(env: Env, contract_address: Address) -> Result<AssetInfo, Error> {
        let asset_id: u64 = env.storage().instance()
            .get(&DataKey::AssetByAddress(contract_address))
            .ok_or(Error::AssetNotFound)?;

        Self::get_asset(env, asset_id)
    }

    /// Get total number of deployed assets
    pub fn asset_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::AssetCount).unwrap_or(0)
    }

    /// Get multiple assets (paginated)
    pub fn get_assets(env: Env, start: u64, limit: u64) -> Vec<AssetInfo> {
        let mut assets = Vec::new(&env);
        let total_count = Self::asset_count(env.clone());

        if start >= total_count {
            return assets;
        }

        let end = (start + limit).min(total_count);

        for i in start..end {
            if let Ok(asset_info) = Self::get_asset(env.clone(), i + 1) {
                assets.push_back(asset_info);
            }
        }

        assets
    }

    /// Get assets deployed by a specific deployer
    pub fn get_assets_by_deployer(env: Env, deployer: Address) -> Vec<AssetInfo> {
        let mut assets = Vec::new(&env);
        let total_count = Self::asset_count(env.clone());

        for i in 1..=total_count {
            if let Ok(asset_info) = Self::get_asset(env.clone(), i) {
                if asset_info.deployer == deployer {
                    assets.push_back(asset_info);
                }
            }
        }

        assets
    }

    /// Verify if a contract address was deployed by this factory
    pub fn is_factory_deployed(env: Env, contract_address: Address) -> bool {
        env.storage().instance().has(&DataKey::AssetByAddress(contract_address))
    }
}

// Client interface for RWA token contracts
#[soroban_sdk::contractclient(name = "RwaTokenClient")]
pub trait RwaTokenInterface {
    fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        total_supply: i128,
    ) -> Result<(), soroban_sdk::Val>;
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup_test_factory<'a>(env: &'a Env) -> (Address, AssetFactoryContractClient<'a>) {
        let contract_id = env.register(AssetFactoryContract, ());
        let client = AssetFactoryContractClient::new(env, &contract_id);
        let admin = Address::generate(env);

        env.mock_all_auths();

        (admin, client)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);

        client.initialize(&admin, &wasm_hash);

        assert_eq!(client.admin(), admin);
        assert_eq!(client.approved_wasm_hash(), wasm_hash);
        assert_eq!(client.asset_count(), 0);
    }

    #[test]
    fn test_initialize_twice_fails() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);

        // First initialization should succeed
        client.initialize(&admin, &wasm_hash);

        // Second initialization should fail
        let result2 = client.try_initialize(&admin, &wasm_hash);
        assert_eq!(result2, Err(Ok(Error::AlreadyInitialized)));
    }

    #[test]
    fn test_update_approved_wasm_hash() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let initial_wasm = BytesN::from_array(&env, &[1u8; 32]);
        let new_wasm = BytesN::from_array(&env, &[2u8; 32]);

        // Initialize factory
        client.initialize(&admin, &initial_wasm);

        // Update WASM hash
        client.update_approved_wasm_hash(&new_wasm);
        assert_eq!(client.approved_wasm_hash(), new_wasm);
    }

    #[test]
    fn test_deploy_rwa_token_validation() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        let deployer = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[3u8; 32]);

        // Initialize factory
        client.initialize(&admin, &wasm_hash);

        // Test invalid total supply
        let name = String::from_str(&env, "Test Token");
        let symbol = String::from_str(&env, "TEST");

        let result = client.try_deploy_rwa_token(
            &deployer,
            &salt,
            &token_admin,
            &name,
            &symbol,
            &8u32,
            &0i128, // Invalid: zero supply
        );
        assert_eq!(result, Err(Ok(Error::InvalidParameters)));

        let result2 = client.try_deploy_rwa_token(
            &deployer,
            &salt,
            &token_admin,
            &name,
            &symbol,
            &8u32,
            &-100i128, // Invalid: negative supply
        );
        assert_eq!(result2, Err(Ok(Error::InvalidParameters)));
    }

    #[test]
    fn test_asset_queries() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let target_wasm = BytesN::from_array(&env, &[1u8; 32]);

        // Initialize factory
        client.initialize(&admin, &target_wasm);

        // Test empty state
        assert_eq!(client.asset_count(), 0);
        let assets = client.get_assets(&0, &10);
        assert_eq!(assets.len(), 0);
    }

    #[test]
    fn test_pagination() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let target_wasm = BytesN::from_array(&env, &[1u8; 32]);

        // Initialize factory
        client.initialize(&admin, &target_wasm);

        // Test pagination with no assets
        let assets = client.get_assets(&0, &5);
        assert_eq!(assets.len(), 0);

        // Test pagination starting beyond available assets
        let assets = client.get_assets(&10, &5);
        assert_eq!(assets.len(), 0);
    }

    #[test]
    fn test_deployer_queries() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let target_wasm = BytesN::from_array(&env, &[1u8; 32]);
        let deployer = Address::generate(&env);

        // Initialize factory
        client.initialize(&admin, &target_wasm);

        // Test with no deployments
        let assets = client.get_assets_by_deployer(&deployer);
        assert_eq!(assets.len(), 0);
    }

    #[test]
    fn test_is_factory_deployed() {
        let env = Env::default();
        let (admin, client) = setup_test_factory(&env);
        let target_wasm = BytesN::from_array(&env, &[1u8; 32]);
        let random_address = Address::generate(&env);

        // Initialize factory
        client.initialize(&admin, &target_wasm);

        // Test with random address (should return false)
        assert!(!client.is_factory_deployed(&random_address));
    }
}
