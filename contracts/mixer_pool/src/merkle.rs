//! Incremental Merkle tree using Poseidon hashing.
//!
//! The tree stores commitments as leaves and computes parent nodes
//! using Poseidon(left, right). This matches the hash function used
//! in the ZK withdrawal circuit, enabling trustless verification.
//!
//! The tree is append-only: leaves are added at the next available index
//! and the path from the leaf to the root is recomputed.
//!
//! See plan.md Section 4.3.

use soroban_sdk::{BytesN, Env};

use crate::poseidon;
use crate::storage;

/// Returns the zero hash for a given level of the Merkle tree.
///
/// Level 0 = Poseidon(0) — the hash of a zero-valued leaf
/// Level n = Poseidon(zero_{n-1}, zero_{n-1})
///
/// These values are deterministic and recomputed on each call.
/// The instruction cost is bounded by the tree depth (max 32 levels).
pub fn zero_hash(env: &Env, level: u32) -> BytesN<32> {
    if level == 0 {
        // Zero leaf: the Poseidon hash of a zero field element
        // This represents an empty/unoccupied leaf slot
        let zero = BytesN::from_array(env, &[0u8; 32]);
        poseidon::hash_single_bytes(env, &zero)
    } else {
        let child = zero_hash(env, level - 1);
        poseidon::hash_pair_bytes(env, &child, &child)
    }
}

/// Insert a commitment into the incremental Merkle tree and return the new root.
///
/// The commitment is placed at the next available leaf index. Parent hashes
/// are recomputed up to the root using Poseidon(left, right).
///
/// # Arguments
/// * `env` - Soroban environment
/// * `commitment` - The Poseidon(secret, nullifier) commitment to insert
///
/// # Returns
/// The new Merkle root after insertion.
///
/// # Important
/// The commitment is stored directly as the leaf value (not hashed again).
/// The client SDK computes commitment = Poseidon(secret, nullifier) off-chain,
/// and the same value is used as the leaf in both the on-chain tree and the
/// ZK circuit's Merkle proof.
pub fn insert_leaf(env: &Env, commitment: &BytesN<32>) -> BytesN<32> {
    let depth = storage::get_tree_depth(env);
    let leaf_index = storage::get_next_leaf_index(env);

    // Store the commitment directly as the leaf (no additional hashing)
    storage::set_merkle_node(env, 0, leaf_index, commitment);

    // Recompute parent hashes from leaf to root
    let mut current_hash = commitment.clone();
    let mut current_index = leaf_index;

    for level in 0..depth {
        let (left, right) = if current_index % 2 == 0 {
            // Current node is a left child — sibling is right (may be zero)
            let right = storage::get_merkle_node(env, level, current_index + 1)
                .unwrap_or_else(|| zero_hash(env, level));
            (current_hash.clone(), right)
        } else {
            // Current node is a right child — sibling is left (must exist)
            let left = storage::get_merkle_node(env, level, current_index - 1)
                .unwrap_or_else(|| zero_hash(env, level));
            (left, current_hash.clone())
        };

        current_hash = poseidon::hash_pair_bytes(env, &left, &right);
        current_index /= 2;

        // Store the parent node
        storage::set_merkle_node(env, level + 1, current_index, &current_hash);
    }

    // Update the next leaf index
    storage::set_next_leaf_index(env, leaf_index + 1);

    current_hash
}

/// Get the current Merkle root from storage.
///
/// Returns the root node at (depth, 0), or the zero hash for the
/// full tree depth if no leaves have been inserted yet.
pub fn get_current_root(env: &Env) -> BytesN<32> {
    let depth = storage::get_tree_depth(env);
    storage::get_merkle_node(env, depth, 0).unwrap_or_else(|| zero_hash(env, depth))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_zero_hash_deterministic() {
        let env = Env::default();
        let z0a = zero_hash(&env, 0);
        let z0b = zero_hash(&env, 0);
        assert_eq!(z0a, z0b);
    }

    #[test]
    fn test_zero_hash_level1_is_hash_of_children() {
        let env = Env::default();
        let z0 = zero_hash(&env, 0);
        let z1 = zero_hash(&env, 1);
        let expected = poseidon::hash_pair_bytes(&env, &z0, &z0);
        assert_eq!(z1, expected);
    }

    #[test]
    fn test_poseidon_hash_order_matters() {
        let env = Env::default();
        let a = BytesN::from_array(&env, &[1u8; 32]);
        let b = BytesN::from_array(&env, &[2u8; 32]);

        let h1 = poseidon::hash_pair_bytes(&env, &a, &b);
        let h2 = poseidon::hash_pair_bytes(&env, &b, &a);
        assert_ne!(h1, h2);
    }
}
