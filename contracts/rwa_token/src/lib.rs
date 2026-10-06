#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    NotWhitelisted = 2,
    InsufficientBalance = 3,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Whitelisted(Address),
    Balance(Address),
    TotalSupply,
}

#[contract]
pub struct RwaTokenContract;

#[contractimpl]
impl RwaTokenContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_whitelist(env: Env, account: Address, status: bool) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Whitelisted(account.clone()), &status);
        
        env.events().publish(
            (symbol_short!("whitelist"), account.clone()),
            status,
        );
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(id))
            .unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();

        if amount <= 0 {
            return Err(Error::InsufficientBalance);
        }

        let from_whitelisted: bool = env.storage().persistent().get(&DataKey::Whitelisted(from.clone())).unwrap_or(false);
        let to_whitelisted: bool = env.storage().persistent().get(&DataKey::Whitelisted(to.clone())).unwrap_or(false);

        if !from_whitelisted || !to_whitelisted {
            return Err(Error::NotWhitelisted);
        }

        let from_balance: i128 = env.storage().persistent().get(&DataKey::Balance(from.clone())).unwrap_or(0);
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }

        let to_balance: i128 = env.storage().persistent().get(&DataKey::Balance(to.clone())).unwrap_or(0);

        env.storage().persistent().set(&DataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage().persistent().set(&DataKey::Balance(to.clone()), &(to_balance + amount));

        // Extend TTL on persistent storage
        env.storage().persistent().extend_ttl(&DataKey::Balance(from.clone()), 172800, 172800);
        env.storage().persistent().extend_ttl(&DataKey::Balance(to.clone()), 172800, 172800);

        env.events().publish(
            (symbol_short!("transfer"), from.clone(), to.clone()),
            amount,
        );

        Ok(())
    }
}
