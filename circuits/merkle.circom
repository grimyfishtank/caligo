// Private Stellar Protocol — Merkle Inclusion Proof Circuit
// Proves that a leaf exists in a Poseidon-based Merkle tree of given depth.
// See plan.md Section 5.2.

pragma circom 2.0.0;

include "../node_modules/circomlib/circuits/poseidon.circom";

/// MerkleProof verifies that `leaf` is contained in a Merkle tree
/// with the given `pathElements` and `pathIndices`, producing `root`.
///
/// pathIndices[i] == 0 means the current hash is the LEFT child
/// pathIndices[i] == 1 means the current hash is the RIGHT child
template MerkleProof(depth) {
    signal input leaf;
    signal input pathElements[depth];
    signal input pathIndices[depth];
    signal output root;

    component hashers[depth];
    signal hashes[depth + 1];
    hashes[0] <== leaf;

    for (var i = 0; i < depth; i++) {
        // Constrain pathIndices to be binary (0 or 1)
        pathIndices[i] * (1 - pathIndices[i]) === 0;

        hashers[i] = Poseidon(2);

        // Select input ordering based on pathIndices:
        //   pathIndices[i] == 0: hash(current, sibling)
        //   pathIndices[i] == 1: hash(sibling, current)
        hashers[i].inputs[0] <== hashes[i] + (pathElements[i] - hashes[i]) * pathIndices[i];
        hashers[i].inputs[1] <== pathElements[i] + (hashes[i] - pathElements[i]) * pathIndices[i];

        hashes[i + 1] <== hashers[i].out;
    }

    root <== hashes[depth];
}
