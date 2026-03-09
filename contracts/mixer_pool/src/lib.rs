#![no_std]

mod errors;
mod merkle;
mod poseidon;
mod storage;
pub mod verifier;
#[cfg(feature = "vk")]
mod vk_constants;

use errors::MixerError;
use soroban_sdk::{contract, contractimpl, token, Address, Bytes, BytesN, Env, Vec};

#[contract]
pub struct MixerPool;

#[contractimpl]
impl MixerPool {
    /// Initialize the mixer pool contract.
    ///
    /// Must be called exactly once after deployment. Sets the pool denomination,
    /// tree parameters, and admin address. The contract is immutable after init —
    /// there is no upgrade mechanism (see plan.md Section 17).
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        denomination: i128,
        max_fee: i128,
        tree_depth: u32,
        root_history_size: u32,
    ) -> Result<(), MixerError> {
        if storage::is_initialized(&env) {
            return Err(MixerError::AlreadyInitialized);
        }
        if denomination <= 0 {
            return Err(MixerError::InvalidDenomination);
        }
        if tree_depth == 0 || tree_depth > 32 {
            return Err(MixerError::InvalidTreeDepth);
        }
        if root_history_size == 0 {
            return Err(MixerError::InvalidRootHistorySize);
        }

        admin.require_auth();

        storage::set_admin(&env, &admin);
        storage::set_token(&env, &token);
        storage::set_denomination(&env, denomination);
        storage::set_max_fee(&env, max_fee);
        storage::set_tree_depth(&env, tree_depth);
        storage::set_root_history_size(&env, root_history_size);
        storage::set_next_leaf_index(&env, 0);
        storage::set_root_history_index(&env, 0);

        // Store the initial empty tree root in history
        let initial_root = merkle::get_current_root(&env);
        push_root_history(&env, &initial_root);

        storage::set_initialized(&env);

        Ok(())
    }

    /// Deposit a fixed-denomination amount into the mixer pool.
    ///
    /// The caller must have approved the token transfer for exactly the pool
    /// denomination. The commitment is appended to the Poseidon Merkle tree
    /// and the root history is updated.
    ///
    /// # Security
    /// - Exact denomination enforcement prevents partial deposits
    /// - Duplicate commitment check prevents commitment replay
    /// - Tree capacity check prevents index overflow
    /// - Poseidon Merkle tree matches the ZK circuit's hash function
    pub fn deposit(
        env: Env,
        depositor: Address,
        commitment: BytesN<32>,
    ) -> Result<(), MixerError> {
        depositor.require_auth();

        if storage::is_commitment_inserted(&env, &commitment) {
            return Err(MixerError::DuplicateCommitment);
        }

        let next_index = storage::get_next_leaf_index(&env);
        let depth = storage::get_tree_depth(&env);
        let max_leaves: u32 = 1u32.checked_shl(depth).unwrap_or(u32::MAX);
        if next_index >= max_leaves {
            return Err(MixerError::TreeFull);
        }

        // Transfer exact denomination from depositor to this contract
        let denomination = storage::get_denomination(&env);
        let token_client = token::Client::new(&env, &storage::get_token(&env));
        token_client.transfer(&depositor, &env.current_contract_address(), &denomination);

        // Mark commitment as inserted (before tree mutation — checks-effects pattern)
        storage::set_commitment_inserted(&env, &commitment);

        // Insert into Poseidon Merkle tree and push new root to history
        let new_root = merkle::insert_leaf(&env, &commitment);
        push_root_history(&env, &new_root);

        env.events()
            .publish(("deposit",), (commitment, next_index));

        Ok(())
    }

    /// Withdraw funds from the mixer pool using a Groth16 zero-knowledge proof.
    ///
    /// The proof demonstrates knowledge of (secret, nullifier) such that
    /// Poseidon(secret, nullifier) is a leaf in the Merkle tree with the given root,
    /// without revealing which leaf.
    ///
    /// # Security
    /// - Root must exist in history window
    /// - Nullifier must not be spent (double-spend prevention)
    /// - Fee must not exceed contract maximum
    /// - Groth16 proof must be valid (verified on-chain via BN254 pairing)
    /// - Nullifier is marked spent BEFORE transfers (checks-effects-interactions)
    /// - Recipient, relayer, and fee are bound into the proof as public inputs
    pub fn withdraw(
        env: Env,
        proof: BytesN<256>,
        root: BytesN<32>,
        nullifier_hash: BytesN<32>,
        recipient: Address,
        relayer: Address,
        fee: i128,
    ) -> Result<(), MixerError> {
        // 1. Validate root exists in history
        if !is_known_root(&env, &root) {
            return Err(MixerError::InvalidRoot);
        }

        // 2. Check nullifier hasn't been spent
        if storage::is_nullifier_spent(&env, &nullifier_hash) {
            return Err(MixerError::NullifierSpent);
        }

        // 3. Validate fee bounds
        if fee < 0 || fee > storage::get_max_fee(&env) {
            return Err(MixerError::FeeTooHigh);
        }

        // 4. Verify Groth16 proof
        //    The public inputs bind the proof to this specific withdrawal:
        //    [root, nullifier_hash, recipient, relayer, fee]
        //
        //    The recipient and relayer addresses are converted to field elements
        //    by interpreting their raw bytes as big-endian integers mod p.
        let public_inputs = build_public_inputs(&env, &root, &nullifier_hash, &recipient, &relayer, fee);

        let proof_valid = verifier::verify_proof(
            &proof.to_array(),
            &public_inputs,
        );

        if !proof_valid {
            return Err(MixerError::InvalidProof);
        }

        // 5. Execute withdrawal
        execute_withdrawal(&env, &nullifier_hash, &recipient, &relayer, fee)
    }

    // ── Read-only queries ──

    /// Returns the current (most recent) Merkle root.
    pub fn get_root(env: Env) -> BytesN<32> {
        merkle::get_current_root(&env)
    }

    /// Returns all roots currently stored in the history window.
    pub fn get_root_history(env: Env) -> Vec<BytesN<32>> {
        let history_size = storage::get_root_history_size(&env);
        let mut roots = Vec::new(&env);
        for i in 0..history_size {
            if let Some(root) = storage::get_root_history_entry(&env, i) {
                roots.push_back(root);
            }
        }
        roots
    }

    /// Check if a nullifier hash has already been spent.
    pub fn is_nullifier_spent(env: Env, nullifier_hash: BytesN<32>) -> bool {
        storage::is_nullifier_spent(&env, &nullifier_hash)
    }

    /// Returns the pool denomination in stroops.
    pub fn get_denomination(env: Env) -> i128 {
        storage::get_denomination(&env)
    }

    /// Returns the current number of deposits in the pool.
    pub fn get_deposit_count(env: Env) -> u32 {
        storage::get_next_leaf_index(&env)
    }
}

// ── Internal helpers ──

/// Build the public inputs array for proof verification.
///
/// Converts contract types (BytesN, Address, i128) to the 32-byte big-endian
/// field element encoding expected by the Groth16 verifier.
fn build_public_inputs(
    env: &Env,
    root: &BytesN<32>,
    nullifier_hash: &BytesN<32>,
    recipient: &Address,
    relayer: &Address,
    fee: i128,
) -> [[u8; 32]; verifier::NUM_PUBLIC_INPUTS] {
    let root_bytes = root.to_array();
    let nullifier_bytes = nullifier_hash.to_array();

    // Addresses are converted to field elements by hashing or direct encoding.
    // For the ZK circuit, recipient and relayer are treated as field elements.
    // The client SDK must use the same encoding when generating the proof.
    //
    // We use a simple approach: take the raw address bytes (up to 32 bytes)
    // and pad/truncate to 32 bytes big-endian.
    let recipient_bytes = address_to_field_bytes(env, recipient);
    let relayer_bytes = address_to_field_bytes(env, relayer);

    // Fee is encoded as a 32-byte big-endian unsigned integer
    let mut fee_bytes = [0u8; 32];
    let fee_u128 = fee as u128;
    let fee_be = fee_u128.to_be_bytes();
    fee_bytes[16..32].copy_from_slice(&fee_be);

    [root_bytes, nullifier_bytes, recipient_bytes, relayer_bytes, fee_bytes]
}

/// Convert a Stellar address to a 32-byte field element representation.
///
/// Uses SHA-256(strkey) for a deterministic, collision-resistant mapping.
/// The strkey is the standard Stellar address encoding (e.g., G... or C...).
///
/// **Critical**: The client SDK must use the identical encoding:
///   SHA-256(address_strkey_bytes) → 32-byte big-endian field element.
pub(crate) fn address_to_field_bytes(env: &Env, addr: &Address) -> [u8; 32] {
    let addr_str = addr.to_string();
    let len = addr_str.len() as usize;
    let mut raw = Bytes::new(env);
    // Copy the strkey characters into a Bytes buffer
    let mut buf = [0u8; 56]; // Stellar strkeys are 56 characters
    let copy_len = if len > 56 { 56 } else { len };
    addr_str.copy_into_slice(&mut buf[..copy_len]);
    for i in 0..copy_len {
        raw.push_back(buf[i]);
    }
    // SHA-256 produces a deterministic 32-byte hash
    let hash = env.crypto().sha256(&raw);
    hash.to_array()
}

/// Execute the withdrawal transfer logic.
fn execute_withdrawal(
    env: &Env,
    nullifier_hash: &BytesN<32>,
    recipient: &Address,
    relayer: &Address,
    fee: i128,
) -> Result<(), MixerError> {
    // Mark nullifier as spent BEFORE transfers (checks-effects-interactions)
    storage::set_nullifier_spent(env, nullifier_hash);

    let denomination = storage::get_denomination(env);
    let token_client = token::Client::new(env, &storage::get_token(env));

    let withdrawal_amount = denomination - fee;
    token_client.transfer(&env.current_contract_address(), recipient, &withdrawal_amount);

    if fee > 0 {
        token_client.transfer(&env.current_contract_address(), relayer, &fee);
    }

    env.events()
        .publish(("withdrawal",), nullifier_hash.clone());

    Ok(())
}

/// Push a new root into the circular root history buffer.
fn push_root_history(env: &Env, root: &BytesN<32>) {
    let history_size = storage::get_root_history_size(env);
    let current_index = storage::get_root_history_index(env);

    storage::set_root_history_entry(env, current_index, root);

    let next_index = (current_index + 1) % history_size;
    storage::set_root_history_index(env, next_index);
}

/// Check if a root exists in the root history window.
fn is_known_root(env: &Env, root: &BytesN<32>) -> bool {
    let history_size = storage::get_root_history_size(env);
    for i in 0..history_size {
        if let Some(stored_root) = storage::get_root_history_entry(env, i) {
            if stored_root == *root {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
