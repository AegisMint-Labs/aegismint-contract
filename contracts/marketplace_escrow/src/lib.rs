#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

/// Storage key types for the Marketplace Escrow contract
#[contracttype]
pub enum StorageKey {
    Admin,
    EscrowCount,
    Escrow(u64),
}

/// Escrow status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    Active,
    Completed,
    Cancelled,
    Disputed,
}

/// Escrow details
#[contracttype]
#[derive(Clone)]
pub struct Escrow {
    pub id: u64,
    pub seller: Address,
    pub buyer: Address,
    pub token_address: Address,
    pub amount: i128,
    pub status: EscrowStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

#[contract]
pub struct MarketplaceEscrow;

#[contractimpl]
impl MarketplaceEscrow {
    /// Initialize the escrow contract
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&StorageKey::Admin) {
            panic!("Contract already initialized");
        }

        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::EscrowCount, &0u64);
    }

    /// Create a new escrow
    pub fn create_escrow(
        env: Env,
        seller: Address,
        buyer: Address,
        token_address: Address,
        amount: i128,
    ) -> u64 {
        buyer.require_auth();

        let escrow_count: u64 = env
            .storage()
            .instance()
            .get(&StorageKey::EscrowCount)
            .unwrap_or(0);

        let escrow_id = escrow_count + 1;

        let escrow = Escrow {
            id: escrow_id,
            seller,
            buyer,
            token_address,
            amount,
            status: EscrowStatus::Active,
            created_at: env.ledger().timestamp(),
            completed_at: None,
        };

        env.storage()
            .instance()
            .set(&StorageKey::Escrow(escrow_id), &escrow);
        env.storage()
            .instance()
            .set(&StorageKey::EscrowCount, &escrow_id);

        escrow_id
    }

    /// Complete an escrow (release funds to seller)
    pub fn complete_escrow(env: Env, escrow_id: u64) {
        let mut escrow: Escrow = env
            .storage()
            .instance()
            .get(&StorageKey::Escrow(escrow_id))
            .expect("Escrow not found");

        // Only buyer or admin can complete
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Not initialized");

        let caller = env.current_contract_address();
        if caller != escrow.buyer && caller != admin {
            escrow.buyer.require_auth();
        }

        if escrow.status != EscrowStatus::Active {
            panic!("Escrow is not active");
        }

        escrow.status = EscrowStatus::Completed;
        escrow.completed_at = Some(env.ledger().timestamp());

        env.storage()
            .instance()
            .set(&StorageKey::Escrow(escrow_id), &escrow);

        // In production, this would trigger token transfer to seller
    }

    /// Cancel an escrow (refund to buyer)
    pub fn cancel_escrow(env: Env, escrow_id: u64) {
        let mut escrow: Escrow = env
            .storage()
            .instance()
            .get(&StorageKey::Escrow(escrow_id))
            .expect("Escrow not found");

        // Only seller or admin can cancel
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Not initialized");

        let caller = env.current_contract_address();
        if caller != escrow.seller && caller != admin {
            escrow.seller.require_auth();
        }

        if escrow.status != EscrowStatus::Active {
            panic!("Escrow is not active");
        }

        escrow.status = EscrowStatus::Cancelled;
        escrow.completed_at = Some(env.ledger().timestamp());

        env.storage()
            .instance()
            .set(&StorageKey::Escrow(escrow_id), &escrow);

        // In production, this would trigger token refund to buyer
    }

    /// Raise a dispute on an escrow
    pub fn dispute_escrow(env: Env, escrow_id: u64) {
        let mut escrow: Escrow = env
            .storage()
            .instance()
            .get(&StorageKey::Escrow(escrow_id))
            .expect("Escrow not found");

        // Either party can dispute
        let caller = env.current_contract_address();
        if caller != escrow.buyer && caller != escrow.seller {
            escrow.buyer.require_auth();
        }

        if escrow.status != EscrowStatus::Active {
            panic!("Escrow is not active");
        }

        escrow.status = EscrowStatus::Disputed;

        env.storage()
            .instance()
            .set(&StorageKey::Escrow(escrow_id), &escrow);
    }

    /// Resolve a disputed escrow (admin only)
    pub fn resolve_dispute(env: Env, escrow_id: u64, release_to_seller: bool) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .instance()
            .get(&StorageKey::Escrow(escrow_id))
            .expect("Escrow not found");

        if escrow.status != EscrowStatus::Disputed {
            panic!("Escrow is not disputed");
        }

        if release_to_seller {
            escrow.status = EscrowStatus::Completed;
        } else {
            escrow.status = EscrowStatus::Cancelled;
        }

        escrow.completed_at = Some(env.ledger().timestamp());

        env.storage()
            .instance()
            .set(&StorageKey::Escrow(escrow_id), &escrow);

        // In production, this would trigger token transfer
    }

    /// Get escrow details
    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage().instance().get(&StorageKey::Escrow(escrow_id))
    }

    /// Get total escrow count
    pub fn get_escrow_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&StorageKey::EscrowCount)
            .unwrap_or(0)
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
        let contract_id = env.register(MarketplaceEscrow, ());
        let client = MarketplaceEscrowClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_escrow_count(), 0);
    }

    #[test]
    fn test_create_and_complete_escrow() {
        let env = Env::default();
        let contract_id = env.register(MarketplaceEscrow, ());
        let client = MarketplaceEscrowClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        let escrow_id = client.create_escrow(&seller, &buyer, &token, &1000);

        assert_eq!(escrow_id, 1);
        assert_eq!(client.get_escrow_count(), 1);

        let escrow = client.get_escrow(&escrow_id).unwrap();
        assert_eq!(escrow.status, EscrowStatus::Active);

        client.complete_escrow(&escrow_id);

        let completed_escrow = client.get_escrow(&escrow_id).unwrap();
        assert_eq!(completed_escrow.status, EscrowStatus::Completed);
    }

    #[test]
    fn test_dispute_resolution() {
        let env = Env::default();
        let contract_id = env.register(MarketplaceEscrow, ());
        let client = MarketplaceEscrowClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        let escrow_id = client.create_escrow(&seller, &buyer, &token, &1000);
        client.dispute_escrow(&escrow_id);

        let disputed_escrow = client.get_escrow(&escrow_id).unwrap();
        assert_eq!(disputed_escrow.status, EscrowStatus::Disputed);

        client.resolve_dispute(&escrow_id, &true);

        let resolved_escrow = client.get_escrow(&escrow_id).unwrap();
        assert_eq!(resolved_escrow.status, EscrowStatus::Completed);
    }
}
