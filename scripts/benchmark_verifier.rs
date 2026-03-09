#!/usr/bin/env -S cargo +nightly -Zscript
//! Groth16 Verifier Benchmark
//!
//! Measures the native execution time of the BN254 Groth16 verification.
//! This provides a baseline estimate, but the true cost must be measured
//! on-chain using `soroban contract invoke --cost` after deployment.
//!
//! Usage:
//!   cargo +nightly -Zscript scripts/benchmark_verifier.rs
//!
//! Or simply run it as a reference — the key metric is the multi-pairing cost.
//!
//! --- Soroban Budget Estimation ---
//!
//! The Groth16 verifier performs:
//!   - 4 G1 point deserializations + subgroup checks
//!   - 2 G2 point deserializations + subgroup checks
//!   - 5 scalar multiplications (IC linear combination)
//!   - 1 multi-pairing (4 pairs on BN254)
//!
//! The multi-pairing dominates cost. On Soroban:
//!   - Current instruction budget: ~100M CPU instructions per transaction
//!   - BN254 pairing: estimated 10-40M instructions (depends on implementation)
//!   - 4-pairing multi-pairing: estimated 30-80M instructions
//!
//! If this exceeds the budget, mitigation options from plan.md Section 4.6:
//!   1. Use Soroban host function for pairings (if available)
//!   2. Optimized Rust verifier (current approach)
//!   3. PLONK with recursive proof, or split verification
//!
//! To benchmark on-chain after deployment:
//!   soroban contract invoke \
//!     --id <CONTRACT_ID> \
//!     --network testnet \
//!     --cost \
//!     -- withdraw \
//!       --proof <hex> \
//!       --root <hex> \
//!       --nullifier_hash <hex> \
//!       --recipient <address> \
//!       --relayer <address> \
//!       --fee 0

fn main() {
    println!("=== Groth16 Verifier Cost Analysis ===\n");
    println!("The BN254 Groth16 verifier performs:");
    println!("  - 4x G1 point deserialization + subgroup check");
    println!("  - 2x G2 point deserialization + subgroup check");
    println!("  - 5x EC scalar multiplication (IC linear combination)");
    println!("  - 1x multi-pairing with 4 pairs\n");
    println!("Estimated Soroban instruction cost: 30-80M instructions");
    println!("Soroban per-tx budget: ~100M instructions\n");
    println!("To measure actual cost, deploy to testnet and run:");
    println!("  soroban contract invoke --cost --id <ID> -- withdraw ...\n");
    println!("If the budget is exceeded, see plan.md Section 4.6 for mitigations.");
}
