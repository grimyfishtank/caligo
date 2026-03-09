use soroban_sdk::{contracttype, BytesN, Env};

/// Storage key definitions for the MixerPool contract.
///
/// We use three storage tiers:
/// - Instance: small, frequently accessed config (denomination, tree depth, counters)
/// - Persistent: long-lived state (nullifiers, merkle nodes, root history)
/// - Temporary: not used in V1
#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    /// Pool denomination in stroops (1 XLM = 10_000_000 stroops)
    Denomination,
    /// Admin address (deployer, immutable after init)
    Admin,
    /// Token contract address (native XLM wrapper)
    Token,
    /// Merkle tree depth
    TreeDepth,
    /// Next available leaf index in the Merkle tree
    NextLeafIndex,
    /// Maximum number of roots to retain in history
    RootHistorySize,
    /// Current root history write index (circular buffer pointer)
    RootHistoryIndex,
    /// Merkle tree node at (level, index)
    MerkleNode(u32, u32),
    /// Root history entry at position i in the circular buffer
    RootHistoryEntry(u32),
    /// Whether a nullifier hash has been spent
    NullifierSpent(BytesN<32>),
    /// Whether a commitment has already been inserted
    CommitmentInserted(BytesN<32>),
    /// Maximum relayer fee in stroops
    MaxFee,
    /// Whether the contract has been initialized
    Initialized,
}

/// Pool configuration defaults (used by deployment scripts)
#[allow(dead_code)]
pub const DEFAULT_TREE_DEPTH: u32 = 20;
#[allow(dead_code)]
pub const DEFAULT_ROOT_HISTORY_SIZE: u32 = 500;

// ── Instance storage helpers ──

pub fn set_denomination(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::Denomination, &amount);
}

pub fn get_denomination(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::Denomination).unwrap()
}

pub fn set_admin(env: &Env, admin: &soroban_sdk::Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

#[allow(dead_code)]
pub fn get_admin(env: &Env) -> soroban_sdk::Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_tree_depth(env: &Env, depth: u32) {
    env.storage().instance().set(&DataKey::TreeDepth, &depth);
}

pub fn get_tree_depth(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::TreeDepth).unwrap()
}

pub fn set_next_leaf_index(env: &Env, index: u32) {
    env.storage().instance().set(&DataKey::NextLeafIndex, &index);
}

pub fn get_next_leaf_index(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::NextLeafIndex)
        .unwrap_or(0)
}

pub fn set_root_history_size(env: &Env, size: u32) {
    env.storage()
        .instance()
        .set(&DataKey::RootHistorySize, &size);
}

pub fn get_root_history_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::RootHistorySize)
        .unwrap()
}

pub fn set_root_history_index(env: &Env, index: u32) {
    env.storage()
        .instance()
        .set(&DataKey::RootHistoryIndex, &index);
}

pub fn get_root_history_index(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::RootHistoryIndex)
        .unwrap_or(0)
}

pub fn set_max_fee(env: &Env, fee: i128) {
    env.storage().instance().set(&DataKey::MaxFee, &fee);
}

pub fn get_max_fee(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::MaxFee).unwrap()
}

pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&DataKey::Initialized, &true);
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Initialized)
        .unwrap_or(false)
}

// ── Persistent storage helpers ──

pub fn set_merkle_node(env: &Env, level: u32, index: u32, hash: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::MerkleNode(level, index), hash);
}

pub fn get_merkle_node(env: &Env, level: u32, index: u32) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::MerkleNode(level, index))
}

pub fn set_root_history_entry(env: &Env, position: u32, root: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::RootHistoryEntry(position), root);
}

pub fn get_root_history_entry(env: &Env, position: u32) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::RootHistoryEntry(position))
}

pub fn set_nullifier_spent(env: &Env, nullifier_hash: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::NullifierSpent(nullifier_hash.clone()), &true);
}

pub fn is_nullifier_spent(env: &Env, nullifier_hash: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::NullifierSpent(nullifier_hash.clone()))
        .unwrap_or(false)
}

pub fn set_commitment_inserted(env: &Env, commitment: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::CommitmentInserted(commitment.clone()), &true);
}

pub fn is_commitment_inserted(env: &Env, commitment: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::CommitmentInserted(commitment.clone()))
        .unwrap_or(false)
}

// ── Token storage ──

pub fn set_token(env: &Env, token: &soroban_sdk::Address) {
    env.storage().instance().set(&DataKey::Token, token);
}

pub fn get_token(env: &Env) -> soroban_sdk::Address {
    env.storage().instance().get(&DataKey::Token).unwrap()
}
