use soroban_sdk::{contracttype, Address, Env};

use crate::RelayerInfo;

#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    MaxFeeBps,
    RelayerCount,
    /// Relayer info keyed by address.
    Relayer(Address),
    /// Index → address mapping for enumeration.
    RelayerIndex(u32),
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage()
        .instance()
        .has(&DataKey::Initialized)
}

pub fn set_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&DataKey::Initialized, &true);
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap()
}

pub fn set_max_fee_bps(env: &Env, max_fee_bps: u32) {
    env.storage()
        .instance()
        .set(&DataKey::MaxFeeBps, &max_fee_bps);
}

pub fn get_max_fee_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MaxFeeBps)
        .unwrap()
}

pub fn set_relayer_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&DataKey::RelayerCount, &count);
}

pub fn get_relayer_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::RelayerCount)
        .unwrap_or(0)
}

pub fn has_relayer(env: &Env, relayer: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Relayer(relayer.clone()))
}

pub fn set_relayer(env: &Env, relayer: &Address, info: &RelayerInfo) {
    env.storage()
        .persistent()
        .set(&DataKey::Relayer(relayer.clone()), info);
}

pub fn get_relayer(env: &Env, relayer: &Address) -> Option<RelayerInfo> {
    env.storage()
        .persistent()
        .get(&DataKey::Relayer(relayer.clone()))
}

pub fn set_relayer_index(env: &Env, index: u32, relayer: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::RelayerIndex(index), relayer);
}

pub fn get_relayer_index(env: &Env, index: u32) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::RelayerIndex(index))
}
