/**
 * Client-side Merkle tree for computing proofs locally.
 *
 * This mirrors the on-chain Poseidon Merkle tree structure exactly.
 * The tree is used to compute path_elements and path_indices for the
 * withdrawal circuit's Merkle inclusion proof.
 */

import { poseidonHash1, poseidonHash2 } from "./poseidon.js";

export interface MerkleProof {
  /** Sibling hashes along the path from leaf to root */
  pathElements: bigint[];
  /** Direction bits: 0 = leaf is left child, 1 = leaf is right child */
  pathIndices: number[];
  /** Merkle root */
  root: bigint;
  /** Leaf index in the tree */
  leafIndex: number;
}

/**
 * Incremental Poseidon Merkle tree matching the on-chain structure.
 *
 * Zero values at each level are computed as:
 * - Level 0: Poseidon(0)
 * - Level n: Poseidon(zero_{n-1}, zero_{n-1})
 */
export class MerkleTree {
  readonly depth: number;
  private leaves: bigint[] = [];
  private zeroValues: bigint[];
  private layers: bigint[][];

  constructor(depth: number) {
    this.depth = depth;
    this.zeroValues = new Array(depth + 1);
    this.layers = new Array(depth + 1);

    // Compute zero hashes for each level (matches on-chain zero_hash())
    this.zeroValues[0] = poseidonHash1(0n);
    for (let i = 1; i <= depth; i++) {
      this.zeroValues[i] = poseidonHash2(
        this.zeroValues[i - 1],
        this.zeroValues[i - 1]
      );
    }

    for (let i = 0; i <= depth; i++) {
      this.layers[i] = [];
    }
  }

  /** Insert a commitment as a new leaf. Returns the leaf index. */
  insert(commitment: bigint): number {
    const index = this.leaves.length;
    this.leaves.push(commitment);
    this.layers[0] = [...this.leaves];
    this.rebuild();
    return index;
  }

  /** Get the current Merkle root. */
  getRoot(): bigint {
    if (this.layers[this.depth].length === 0) {
      return this.zeroValues[this.depth];
    }
    return this.layers[this.depth][0];
  }

  /** Compute the Merkle proof for a leaf at the given index. */
  getProof(index: number): MerkleProof {
    if (index >= this.leaves.length) {
      throw new Error(`Leaf index ${index} out of bounds (${this.leaves.length} leaves)`);
    }

    const pathElements: bigint[] = [];
    const pathIndices: number[] = [];
    let currentIndex = index;

    for (let level = 0; level < this.depth; level++) {
      const siblingIndex =
        currentIndex % 2 === 0 ? currentIndex + 1 : currentIndex - 1;
      const sibling =
        this.layers[level][siblingIndex] ?? this.zeroValues[level];

      pathElements.push(sibling);
      pathIndices.push(currentIndex % 2);
      currentIndex = Math.floor(currentIndex / 2);
    }

    return {
      pathElements,
      pathIndices,
      root: this.getRoot(),
      leafIndex: index,
    };
  }

  /** Number of leaves inserted. */
  get leafCount(): number {
    return this.leaves.length;
  }

  private rebuild(): void {
    for (let level = 0; level < this.depth; level++) {
      const layerSize = Math.ceil(this.layers[level].length / 2);
      this.layers[level + 1] = new Array(layerSize);

      for (let i = 0; i < layerSize; i++) {
        const left = this.layers[level][i * 2] ?? this.zeroValues[level];
        const right = this.layers[level][i * 2 + 1] ?? this.zeroValues[level];
        this.layers[level + 1][i] = poseidonHash2(left, right);
      }
    }
  }
}
