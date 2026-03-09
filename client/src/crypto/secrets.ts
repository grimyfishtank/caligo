/**
 * Secret and commitment generation for the mixer protocol.
 *
 * Generates cryptographically secure random values for deposits
 * and computes the Poseidon commitment and nullifier hash.
 */

import { randomBytes } from "crypto";
import { poseidonHash2, poseidonHash1, bytesToBigInt, bigIntToBytes } from "./poseidon.js";

/** BN254 scalar field order — all values must be reduced mod this */
const BN254_FIELD_ORDER =
  21888242871839275222246405745257275088548364400416034343698204186575808495617n;

/**
 * A deposit note containing all data needed to withdraw.
 * If this is lost, the funds are permanently unrecoverable.
 */
export interface DepositNote {
  /** Random secret (private, never revealed) */
  secret: bigint;
  /** Random nullifier (revealed as hash at withdrawal) */
  nullifier: bigint;
  /** Poseidon(secret, nullifier) — stored on-chain as Merkle leaf */
  commitment: bigint;
  /** Poseidon(nullifier) — public input at withdrawal */
  nullifierHash: bigint;
}

/**
 * Generate a random field element (< BN254 scalar field order).
 * Uses crypto.randomBytes for CSPRNG.
 */
export function randomFieldElement(): bigint {
  // Generate 31 bytes to stay well under the field order (~254 bits)
  const bytes = randomBytes(31);
  let n = 0n;
  for (const b of bytes) {
    n = (n << 8n) | BigInt(b);
  }
  // Reduce mod field order (should already be under, but be safe)
  return n % BN254_FIELD_ORDER;
}

/**
 * Generate a fresh deposit note with random secret and nullifier.
 *
 * The commitment is computed as Poseidon(secret, nullifier) and will be
 * submitted on-chain during deposit. The nullifierHash is computed as
 * Poseidon(nullifier) and will be used as a public input during withdrawal.
 */
export function generateDepositNote(): DepositNote {
  const secret = randomFieldElement();
  const nullifier = randomFieldElement();
  const commitment = poseidonHash2(secret, nullifier);
  const nullifierHash = poseidonHash1(nullifier);
  return { secret, nullifier, commitment, nullifierHash };
}

/**
 * Recompute commitment and nullifierHash from secret and nullifier.
 * Used when restoring a note from backup.
 */
export function recomputeNote(secret: bigint, nullifier: bigint): DepositNote {
  const commitment = poseidonHash2(secret, nullifier);
  const nullifierHash = poseidonHash1(nullifier);
  return { secret, nullifier, commitment, nullifierHash };
}

/**
 * Serialize a deposit note to a JSON-safe object (hex strings).
 */
export function serializeNote(note: DepositNote): Record<string, string> {
  return {
    secret: "0x" + note.secret.toString(16).padStart(64, "0"),
    nullifier: "0x" + note.nullifier.toString(16).padStart(64, "0"),
    commitment: "0x" + note.commitment.toString(16).padStart(64, "0"),
    nullifierHash: "0x" + note.nullifierHash.toString(16).padStart(64, "0"),
  };
}

/**
 * Deserialize a deposit note from hex strings.
 */
export function deserializeNote(data: Record<string, string>): DepositNote {
  const secret = BigInt(data.secret);
  const nullifier = BigInt(data.nullifier);
  // Recompute to verify integrity
  const note = recomputeNote(secret, nullifier);
  const expectedCommitment = BigInt(data.commitment);
  if (note.commitment !== expectedCommitment) {
    throw new Error("Note integrity check failed: commitment mismatch");
  }
  return note;
}

/**
 * Convert a Stellar address to its 32-byte field element representation.
 *
 * MUST match the on-chain encoding in contracts/mixer_pool/src/lib.rs:
 *   SHA-256(strkey_bytes) → 32-byte big-endian field element
 *
 * This is used for the `recipient` and `relayer` public inputs in the proof.
 */
export function addressToFieldBytes(address: string): Uint8Array {
  const { createHash } = require("crypto") as typeof import("crypto");
  const hash = createHash("sha256").update(Buffer.from(address, "utf-8")).digest();
  return new Uint8Array(hash);
}

/**
 * Convert a Stellar address to a bigint field element.
 *
 * The result is reduced modulo the BN254 scalar field order because
 * SHA-256 outputs can exceed the field size. The circuit and on-chain
 * verifier both perform this reduction (via from_le_bytes_mod_order).
 */
export function addressToField(address: string): bigint {
  return bytesToBigInt(addressToFieldBytes(address)) % BN254_FIELD_ORDER;
}
