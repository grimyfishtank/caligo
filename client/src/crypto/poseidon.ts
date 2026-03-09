/**
 * Poseidon hash wrapper using circomlibjs.
 *
 * This must produce identical outputs to:
 * - The circom Poseidon circuits (withdraw.circom)
 * - The on-chain light-poseidon Rust crate (contracts/mixer_pool/src/poseidon.rs)
 *
 * All three use the same BN254 scalar field and Poseidon constants.
 */

// circomlibjs types are not published, so we declare what we need
type PoseidonHasher = {
  (inputs: bigint[]): Uint8Array;
  F: {
    toObject(val: Uint8Array): bigint;
  };
};

let poseidonHasher: PoseidonHasher | null = null;

/**
 * Initialize the Poseidon hasher. Must be called once before any hashing.
 * This loads the circomlibjs WASM implementation.
 */
export async function initPoseidon(): Promise<void> {
  if (poseidonHasher) return;
  const { buildPoseidon } = await import("circomlibjs");
  poseidonHasher = await buildPoseidon();
}

function getHasher(): PoseidonHasher {
  if (!poseidonHasher) {
    throw new Error("Poseidon not initialized. Call initPoseidon() first.");
  }
  return poseidonHasher;
}

/**
 * Poseidon hash of two field elements.
 * Used for: commitment = Poseidon(secret, nullifier), Merkle nodes.
 */
export function poseidonHash2(a: bigint, b: bigint): bigint {
  const hasher = getHasher();
  const hash = hasher([a, b]);
  return hasher.F.toObject(hash);
}

/**
 * Poseidon hash of a single field element.
 * Used for: nullifierHash = Poseidon(nullifier), zero leaf hash.
 */
export function poseidonHash1(a: bigint): bigint {
  const hasher = getHasher();
  const hash = hasher([a]);
  return hasher.F.toObject(hash);
}

/**
 * Convert a 32-byte big-endian Uint8Array to a bigint.
 */
export function bytesToBigInt(bytes: Uint8Array): bigint {
  let n = 0n;
  for (const b of bytes) {
    n = (n << 8n) | BigInt(b);
  }
  return n;
}

/**
 * Convert a bigint to a 32-byte big-endian Uint8Array.
 */
export function bigIntToBytes(n: bigint): Uint8Array {
  const bytes = new Uint8Array(32);
  let val = n;
  for (let i = 31; i >= 0; i--) {
    bytes[i] = Number(val & 0xffn);
    val >>= 8n;
  }
  return bytes;
}
