// Private Stellar Protocol — Withdrawal Circuit
// Proves knowledge of (secret, nullifier) corresponding to a commitment
// in the pool's Merkle tree, without revealing which deposit it is.
// See plan.md Section 5.1.
//
// Public inputs are bound into the proof to prevent tampering:
//   - root: the Merkle root the proof is verified against
//   - nullifierHash: prevents double-spend
//   - recipient: withdrawal destination (must be fresh address)
//   - relayer: relayer address (0 if direct withdrawal)
//   - fee: relayer fee in stroops (0 if direct)

pragma circom 2.0.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "merkle.circom";

template Withdraw(depth) {
    // ── Public inputs ──
    signal input root;
    signal input nullifierHash;
    signal input recipient;
    signal input relayer;
    signal input fee;

    // ── Private inputs ──
    signal input secret;
    signal input nullifier;
    signal input pathElements[depth];
    signal input pathIndices[depth];

    // 1. Compute commitment = Poseidon(secret, nullifier)
    component commitmentHasher = Poseidon(2);
    commitmentHasher.inputs[0] <== secret;
    commitmentHasher.inputs[1] <== nullifier;

    // 2. Compute nullifier hash = Poseidon(nullifier)
    component nullifierHasher = Poseidon(1);
    nullifierHasher.inputs[0] <== nullifier;

    // 3. Verify nullifier hash matches the public input
    nullifierHash === nullifierHasher.out;

    // 4. Verify Merkle inclusion proof
    component merkleProof = MerkleProof(depth);
    merkleProof.leaf <== commitmentHasher.out;
    for (var i = 0; i < depth; i++) {
        merkleProof.pathElements[i] <== pathElements[i];
        merkleProof.pathIndices[i] <== pathIndices[i];
    }

    // 5. Verify the computed root matches the public input
    root === merkleProof.root;

    // 6. Bind recipient, relayer, fee into the proof
    //    These dummy constraints force the values into the proof's
    //    public inputs, preventing a malicious relayer from
    //    substituting their own address or inflating the fee.
    signal recipientSquare;
    recipientSquare <== recipient * recipient;
    signal relayerSquare;
    relayerSquare <== relayer * relayer;
    signal feeSquare;
    feeSquare <== fee * fee;
}

// Instantiate with depth 20 (capacity = 1,048,576 deposits)
component main {public [root, nullifierHash, recipient, relayer, fee]} = Withdraw(20);
