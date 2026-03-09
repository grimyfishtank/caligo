#![no_std]

mod errors;
mod storage;

use errors::RegistryError;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

/// Relayer information stored on-chain.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RelayerInfo {
    /// Relayer's Stellar address (receives fee payments).
    pub address: Address,
    /// Relayer's API endpoint URL for proof relay requests.
    pub endpoint: String,
    /// Fee rate in basis points (e.g., 100 = 1%).
    pub fee_bps: u32,
    /// Whether the relayer is currently active and accepting requests.
    pub active: bool,
}

#[contract]
pub struct RelayerRegistry;

#[contractimpl]
impl RelayerRegistry {
    /// Initialize the registry with an admin address.
    ///
    /// The admin can remove relayers and update the max fee cap.
    /// Relayer registration is permissionless.
    pub fn initialize(env: Env, admin: Address, max_fee_bps: u32) -> Result<(), RegistryError> {
        if storage::is_initialized(&env) {
            return Err(RegistryError::AlreadyInitialized);
        }

        admin.require_auth();

        storage::set_admin(&env, &admin);
        storage::set_max_fee_bps(&env, max_fee_bps);
        storage::set_relayer_count(&env, 0);
        storage::set_initialized(&env);

        Ok(())
    }

    /// Register a new relayer or update an existing registration.
    ///
    /// Any address can register as a relayer. The fee rate must not
    /// exceed the registry's max_fee_bps cap.
    pub fn register(
        env: Env,
        relayer: Address,
        endpoint: String,
        fee_bps: u32,
    ) -> Result<(), RegistryError> {
        relayer.require_auth();

        let max_fee = storage::get_max_fee_bps(&env);
        if fee_bps > max_fee {
            return Err(RegistryError::FeeTooHigh);
        }

        if endpoint.len() == 0 {
            return Err(RegistryError::InvalidEndpoint);
        }

        let info = RelayerInfo {
            address: relayer.clone(),
            endpoint,
            fee_bps,
            active: true,
        };

        let is_new = !storage::has_relayer(&env, &relayer);
        storage::set_relayer(&env, &relayer, &info);

        if is_new {
            let count = storage::get_relayer_count(&env);
            storage::set_relayer_index(&env, count, &relayer);
            storage::set_relayer_count(&env, count + 1);
        }

        env.events().publish(("register",), relayer);

        Ok(())
    }

    /// Deactivate a relayer. Can be called by the relayer themselves or the admin.
    pub fn deactivate(env: Env, caller: Address, relayer: Address) -> Result<(), RegistryError> {
        caller.require_auth();

        let admin = storage::get_admin(&env);
        if caller != relayer && caller != admin {
            return Err(RegistryError::Unauthorized);
        }

        let mut info = storage::get_relayer(&env, &relayer)
            .ok_or(RegistryError::RelayerNotFound)?;

        info.active = false;
        storage::set_relayer(&env, &relayer, &info);

        env.events().publish(("deactivate",), relayer);

        Ok(())
    }

    /// Reactivate a previously deactivated relayer. Only the relayer can do this.
    pub fn reactivate(env: Env, relayer: Address) -> Result<(), RegistryError> {
        relayer.require_auth();

        let mut info = storage::get_relayer(&env, &relayer)
            .ok_or(RegistryError::RelayerNotFound)?;

        info.active = true;
        storage::set_relayer(&env, &relayer, &info);

        env.events().publish(("reactivate",), relayer);

        Ok(())
    }

    /// Update the maximum fee cap. Admin only.
    pub fn set_max_fee(env: Env, admin: Address, max_fee_bps: u32) -> Result<(), RegistryError> {
        admin.require_auth();

        if admin != storage::get_admin(&env) {
            return Err(RegistryError::Unauthorized);
        }

        storage::set_max_fee_bps(&env, max_fee_bps);

        Ok(())
    }

    // ── Read-only queries ──

    /// Get relayer info by address.
    pub fn get_relayer(env: Env, relayer: Address) -> Option<RelayerInfo> {
        storage::get_relayer(&env, &relayer)
    }

    /// Get all active relayers.
    pub fn get_active_relayers(env: Env) -> Vec<RelayerInfo> {
        let count = storage::get_relayer_count(&env);
        let mut result = Vec::new(&env);

        for i in 0..count {
            if let Some(addr) = storage::get_relayer_index(&env, i) {
                if let Some(info) = storage::get_relayer(&env, &addr) {
                    if info.active {
                        result.push_back(info);
                    }
                }
            }
        }

        result
    }

    /// Get the maximum fee cap in basis points.
    pub fn get_max_fee(env: Env) -> u32 {
        storage::get_max_fee_bps(&env)
    }

    /// Get the total number of registered relayers (active + inactive).
    pub fn get_relayer_count(env: Env) -> u32 {
        storage::get_relayer_count(&env)
    }
}

#[cfg(test)]
mod tests;
