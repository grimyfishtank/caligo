//! Off-chain incremental Poseidon Merkle tree.
//!
//! Mirrors the on-chain tree exactly so that Merkle proofs generated
//! here are valid for the ZK withdrawal circuit.
//!
//! The tree is append-only and stores all leaves in memory.
//! For production, leaves and nodes would be persisted to PostgreSQL.

use crate::poseidon;

/// Merkle inclusion proof for a leaf.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MerkleProof {
    /// The Merkle root at the time of proof generation.
    pub root: [u8; 32],
    /// Sibling hashes along the path from leaf to root.
    pub path_elements: Vec<[u8; 32]>,
    /// Direction bits: 0 = leaf is left child, 1 = leaf is right child.
    pub path_indices: Vec<u32>,
    /// Index of the leaf in the tree.
    pub leaf_index: u32,
}

/// Incremental Poseidon Merkle tree.
pub struct MerkleTree {
    depth: u32,
    leaves: Vec<[u8; 32]>,
    /// Layers[0] = leaves, Layers[depth] = [root]
    layers: Vec<Vec<[u8; 32]>>,
    /// Precomputed zero hashes for each level.
    zero_values: Vec<[u8; 32]>,
}

impl MerkleTree {
    /// Create a new empty Merkle tree with the given depth.
    pub fn new(depth: u32) -> Self {
        let mut zero_values = vec![[0u8; 32]; (depth + 1) as usize];

        // Level 0: Poseidon(0) — matches on-chain zero_hash(env, 0)
        zero_values[0] = poseidon::hash_single(&[0u8; 32]);
        for i in 1..=depth as usize {
            zero_values[i] = poseidon::hash_pair(&zero_values[i - 1], &zero_values[i - 1]);
        }

        let layers = (0..=depth as usize).map(|_| Vec::new()).collect();

        MerkleTree {
            depth,
            leaves: Vec::new(),
            layers,
            zero_values,
        }
    }

    /// Insert a commitment into the tree. Returns the leaf index.
    pub fn insert(&mut self, commitment: [u8; 32]) -> u32 {
        let index = self.leaves.len() as u32;
        self.leaves.push(commitment);
        self.layers[0] = self.leaves.clone();
        self.rebuild();
        index
    }

    /// Get the current Merkle root.
    pub fn root(&self) -> [u8; 32] {
        if self.layers[self.depth as usize].is_empty() {
            self.zero_values[self.depth as usize]
        } else {
            self.layers[self.depth as usize][0]
        }
    }

    /// Generate a Merkle proof for the leaf at the given index.
    pub fn proof(&self, index: u32) -> Option<MerkleProof> {
        if index >= self.leaves.len() as u32 {
            return None;
        }

        let mut path_elements = Vec::with_capacity(self.depth as usize);
        let mut path_indices = Vec::with_capacity(self.depth as usize);
        let mut current_index = index;

        for level in 0..self.depth as usize {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            let sibling = self.layers[level]
                .get(sibling_index as usize)
                .copied()
                .unwrap_or(self.zero_values[level]);

            path_elements.push(sibling);
            path_indices.push(current_index % 2);
            current_index /= 2;
        }

        Some(MerkleProof {
            root: self.root(),
            path_elements,
            path_indices,
            leaf_index: index,
        })
    }

    /// Number of leaves in the tree.
    pub fn leaf_count(&self) -> u32 {
        self.leaves.len() as u32
    }

    /// Get the tree depth.
    #[allow(dead_code)]
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Get a leaf by index.
    #[allow(dead_code)]
    pub fn leaf(&self, index: u32) -> Option<[u8; 32]> {
        self.leaves.get(index as usize).copied()
    }

    /// Find the index of a commitment in the tree.
    pub fn find_commitment(&self, commitment: &[u8; 32]) -> Option<u32> {
        self.leaves.iter().position(|l| l == commitment).map(|i| i as u32)
    }

    fn rebuild(&mut self) {
        for level in 0..self.depth as usize {
            let layer_size = (self.layers[level].len() + 1) / 2;
            self.layers[level + 1] = Vec::with_capacity(layer_size);

            for i in 0..layer_size {
                let left = self.layers[level]
                    .get(i * 2)
                    .copied()
                    .unwrap_or(self.zero_values[level]);
                let right = self.layers[level]
                    .get(i * 2 + 1)
                    .copied()
                    .unwrap_or(self.zero_values[level]);
                self.layers[level + 1].push(poseidon::hash_pair(&left, &right));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree_deterministic() {
        let t1 = MerkleTree::new(4);
        let t2 = MerkleTree::new(4);
        assert_eq!(t1.root(), t2.root());
    }

    #[test]
    fn test_insert_changes_root() {
        let mut tree = MerkleTree::new(4);
        let empty_root = tree.root();
        let commitment = poseidon::hash_pair(&[1u8; 32], &[2u8; 32]);
        tree.insert(commitment);
        assert_ne!(tree.root(), empty_root);
    }

    #[test]
    fn test_proof_reconstructs_root() {
        let mut tree = MerkleTree::new(4);
        let c = poseidon::hash_pair(&[10u8; 32], &[20u8; 32]);
        let idx = tree.insert(c);
        let proof = tree.proof(idx).unwrap();

        // Reconstruct root from proof
        let mut hash = c;
        for i in 0..proof.path_elements.len() {
            if proof.path_indices[i] == 0 {
                hash = poseidon::hash_pair(&hash, &proof.path_elements[i]);
            } else {
                hash = poseidon::hash_pair(&proof.path_elements[i], &hash);
            }
        }
        assert_eq!(hash, proof.root);
    }

    #[test]
    fn test_multiple_inserts() {
        let mut tree = MerkleTree::new(4);
        let c1 = poseidon::hash_pair(&[1u8; 32], &[2u8; 32]);
        let c2 = poseidon::hash_pair(&[3u8; 32], &[4u8; 32]);
        let c3 = poseidon::hash_pair(&[5u8; 32], &[6u8; 32]);

        tree.insert(c1);
        tree.insert(c2);
        let idx = tree.insert(c3);

        let proof = tree.proof(idx).unwrap();
        assert_eq!(proof.leaf_index, 2);
        assert_eq!(proof.path_elements.len(), 4);

        // Verify proof
        let mut hash = c3;
        for i in 0..proof.path_elements.len() {
            if proof.path_indices[i] == 0 {
                hash = poseidon::hash_pair(&hash, &proof.path_elements[i]);
            } else {
                hash = poseidon::hash_pair(&proof.path_elements[i], &hash);
            }
        }
        assert_eq!(hash, tree.root());
    }

    #[test]
    fn test_out_of_bounds_proof() {
        let tree = MerkleTree::new(4);
        assert!(tree.proof(0).is_none());
    }

    #[test]
    fn test_insertion_order_matters() {
        let c1 = poseidon::hash_pair(&[1u8; 32], &[2u8; 32]);
        let c2 = poseidon::hash_pair(&[3u8; 32], &[4u8; 32]);

        let mut t1 = MerkleTree::new(4);
        t1.insert(c1);
        t1.insert(c2);

        let mut t2 = MerkleTree::new(4);
        t2.insert(c2);
        t2.insert(c1);

        assert_ne!(t1.root(), t2.root());
    }
}
