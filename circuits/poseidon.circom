// Private Stellar Protocol — Poseidon Hash Component
// Re-exports circomlib's Poseidon for use in our circuits.
//
// Poseidon costs ~240 R1CS constraints per 2-input hash.
// SHA-256 would cost ~25,000+ constraints — making proofs impractical.
// See plan.md Section 4.1.

pragma circom 2.0.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
