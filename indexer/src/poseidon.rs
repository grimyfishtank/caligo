//! Poseidon hash functions matching the on-chain implementation.
//!
//! Uses the same `light-poseidon` crate as the Soroban contract
//! to ensure identical Merkle roots.

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use light_poseidon::{Poseidon, PoseidonHasher};

/// Convert a 32-byte big-endian array to an arkworks field element.
fn bytes_to_fr(bytes: &[u8; 32]) -> Fr {
    let mut le = *bytes;
    le.reverse();
    Fr::from_le_bytes_mod_order(&le)
}

/// Convert an arkworks field element to a 32-byte big-endian array.
fn fr_to_bytes(fr: &Fr) -> [u8; 32] {
    let le_bytes = fr.into_bigint().to_bytes_le();
    let mut result = [0u8; 32];
    for (i, &b) in le_bytes.iter().enumerate().take(32) {
        result[31 - i] = b;
    }
    result
}

/// Poseidon hash of two field elements (for Merkle tree nodes).
pub fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Poseidon::<Fr>::new_circom(2).expect("Poseidon(2) init");
    let l = bytes_to_fr(left);
    let r = bytes_to_fr(right);
    let result = hasher.hash(&[l, r]).expect("Poseidon hash");
    fr_to_bytes(&result)
}

/// Poseidon hash of a single field element (for zero leaf, nullifier hash).
pub fn hash_single(input: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Poseidon::<Fr>::new_circom(1).expect("Poseidon(1) init");
    let inp = bytes_to_fr(input);
    let result = hasher.hash(&[inp]).expect("Poseidon hash");
    fr_to_bytes(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_pair_deterministic() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let h1 = hash_pair(&a, &b);
        let h2 = hash_pair(&a, &b);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_pair_order_matters() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let h1 = hash_pair(&a, &b);
        let h2 = hash_pair(&b, &a);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_single_deterministic() {
        let a = [0u8; 32];
        let h1 = hash_single(&a);
        let h2 = hash_single(&a);
        assert_eq!(h1, h2);
    }
}
