//! Poseidon hash function for the BN254 scalar field.
//!
//! Uses the `light-poseidon` crate which implements the same Poseidon
//! parameters as circomlib (the standard ZK circuit library). This ensures
//! on-chain and off-chain hashes are identical.
//!
//! The hash operates over the BN254 scalar field (Fr), which is the
//! native field for Groth16 proofs on the alt_bn128 curve.

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use light_poseidon::{Poseidon, PoseidonHasher};
use soroban_sdk::{BytesN, Env};

/// Convert a 32-byte big-endian array to a BN254 scalar field element.
///
/// The input is interpreted as an unsigned integer in big-endian format
/// and reduced modulo the field order. This matches the convention used
/// by snarkjs and circomlib.
pub fn bytes_to_fr(bytes: &[u8; 32]) -> Fr {
    // arkworks uses little-endian internally
    let mut le_bytes = *bytes;
    le_bytes.reverse();
    Fr::from_le_bytes_mod_order(&le_bytes)
}

/// Convert a BN254 scalar field element to a 32-byte big-endian array.
///
/// This is the canonical encoding used by snarkjs and on-chain storage.
pub fn fr_to_bytes(fr: &Fr) -> [u8; 32] {
    let big_int = fr.into_bigint();
    let le_bytes = big_int.to_bytes_le();
    let mut be_bytes = [0u8; 32];
    let len = le_bytes.len().min(32);
    for i in 0..len {
        be_bytes[31 - i] = le_bytes[i];
    }
    be_bytes
}

/// Poseidon hash of two field elements: H(left, right).
///
/// Used for Merkle tree internal node hashing.
/// Matches circomlib's Poseidon(2) template.
pub fn hash_pair(left: &Fr, right: &Fr) -> Fr {
    let mut hasher = Poseidon::<Fr>::new_circom(2).expect("poseidon init");
    hasher.hash(&[*left, *right]).expect("poseidon hash")
}

/// Poseidon hash of a single field element: H(input).
///
/// Used for nullifier hashing: nullifier_hash = Poseidon(nullifier).
/// Matches circomlib's Poseidon(1) template.
pub fn hash_single(input: &Fr) -> Fr {
    let mut hasher = Poseidon::<Fr>::new_circom(1).expect("poseidon init");
    hasher.hash(&[*input]).expect("poseidon hash")
}

/// Hash a commitment from raw bytes: Poseidon(left_bytes, right_bytes).
///
/// Convenience wrapper for on-chain Merkle tree operations.
/// Takes Soroban BytesN<32> values, converts to field elements,
/// hashes with Poseidon, and returns the result as BytesN<32>.
pub fn hash_pair_bytes(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
    let left_fr = bytes_to_fr(&left.to_array());
    let right_fr = bytes_to_fr(&right.to_array());
    let result = hash_pair(&left_fr, &right_fr);
    BytesN::from_array(env, &fr_to_bytes(&result))
}

/// Hash a single value from raw bytes: Poseidon(input_bytes).
///
/// Convenience wrapper for nullifier hashing.
pub fn hash_single_bytes(env: &Env, input: &BytesN<32>) -> BytesN<32> {
    let input_fr = bytes_to_fr(&input.to_array());
    let result = hash_single(&input_fr);
    BytesN::from_array(env, &fr_to_bytes(&result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_poseidon_hash_pair_deterministic() {
        let a = bytes_to_fr(&[1u8; 32]);
        let b = bytes_to_fr(&[2u8; 32]);

        let h1 = hash_pair(&a, &b);
        let h2 = hash_pair(&a, &b);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_poseidon_hash_pair_order_matters() {
        let a = bytes_to_fr(&[1u8; 32]);
        let b = bytes_to_fr(&[2u8; 32]);

        let h1 = hash_pair(&a, &b);
        let h2 = hash_pair(&b, &a);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_poseidon_hash_single() {
        let a = bytes_to_fr(&[42u8; 32]);
        let h = hash_single(&a);
        // Should produce a non-zero result
        assert_ne!(h, Fr::from(0u64));
    }

    #[test]
    fn test_bytes_roundtrip() {
        let original = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ];
        let fr = bytes_to_fr(&original);
        let recovered = fr_to_bytes(&fr);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_hash_pair_bytes_matches_fr() {
        let env = Env::default();
        let left_bytes = [3u8; 32];
        let right_bytes = [7u8; 32];

        let left_soroban = BytesN::from_array(&env, &left_bytes);
        let right_soroban = BytesN::from_array(&env, &right_bytes);

        // Hash via bytes wrapper
        let result_bytes = hash_pair_bytes(&env, &left_soroban, &right_soroban);

        // Hash via Fr directly
        let left_fr = bytes_to_fr(&left_bytes);
        let right_fr = bytes_to_fr(&right_bytes);
        let result_fr = hash_pair(&left_fr, &right_fr);

        assert_eq!(result_bytes.to_array(), fr_to_bytes(&result_fr));
    }
}
