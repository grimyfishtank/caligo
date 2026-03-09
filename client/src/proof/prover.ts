/**
 * Groth16 proof generation wrapper around snarkjs.
 *
 * Generates withdrawal proofs using the compiled circuit WASM and zkey.
 * In a browser environment, proof generation should run in a Web Worker
 * to avoid blocking the UI thread (1–10 seconds depending on device).
 */

import type { MerkleProof } from "../crypto/merkle.js";
import type { DepositNote } from "../crypto/secrets.js";
import { addressToField } from "../crypto/secrets.js";

/** Groth16 proof data as returned by snarkjs. */
export interface Groth16Proof {
  pi_a: string[];
  pi_b: string[][];
  pi_c: string[];
  protocol: string;
  curve: string;
}

/** Full proof result including public signals. */
export interface ProofResult {
  proof: Groth16Proof;
  publicSignals: string[];
}

/** Inputs required to generate a withdrawal proof. */
export interface WithdrawalProofInput {
  note: DepositNote;
  merkleProof: MerkleProof;
  recipient: string; // Stellar address
  relayer: string; // Stellar address (or zero address for direct withdrawal)
  fee: bigint;
}

/**
 * Generate a Groth16 withdrawal proof.
 *
 * @param input - Withdrawal proof inputs (note, merkle proof, addresses, fee)
 * @param wasmPath - Path to the compiled circuit WASM file
 * @param zkeyPath - Path to the ceremony zkey file
 * @returns The proof and public signals
 */
export async function generateWithdrawalProof(
  input: WithdrawalProofInput,
  wasmPath: string,
  zkeyPath: string
): Promise<ProofResult> {
  // Dynamic import to support both Node.js and browser environments
  const snarkjs = await import("snarkjs");

  const recipientField = addressToField(input.recipient);
  const relayerField = addressToField(input.relayer);

  const circuitInput = {
    // Public inputs
    root: input.merkleProof.root.toString(),
    nullifierHash: input.note.nullifierHash.toString(),
    recipient: recipientField.toString(),
    relayer: relayerField.toString(),
    fee: input.fee.toString(),
    // Private inputs
    secret: input.note.secret.toString(),
    nullifier: input.note.nullifier.toString(),
    pathElements: input.merkleProof.pathElements.map((e) => e.toString()),
    pathIndices: input.merkleProof.pathIndices.map((e) => e.toString()),
  };

  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    circuitInput,
    wasmPath,
    zkeyPath
  );

  return { proof, publicSignals };
}

/**
 * Verify a Groth16 proof locally (for testing/debugging).
 *
 * In production, verification happens on-chain in the Soroban contract.
 */
export async function verifyProof(
  proof: Groth16Proof,
  publicSignals: string[],
  vkPath: string
): Promise<boolean> {
  const snarkjs = await import("snarkjs");
  const fs = await import("fs");
  const vk = JSON.parse(fs.readFileSync(vkPath, "utf8"));
  return snarkjs.groth16.verify(vk, publicSignals, proof);
}

/**
 * Encode a Groth16 proof into the 256-byte format expected by the on-chain verifier.
 *
 * Layout (big-endian, uncompressed affine points):
 *   [0..64)    — A (G1): x[32] || y[32]
 *   [64..192)  — B (G2): x_c0[32] || x_c1[32] || y_c0[32] || y_c1[32]
 *   [192..256) — C (G1): x[32] || y[32]
 *
 * snarkjs proof format:
 *   pi_a: [x, y, "1"]
 *   pi_b: [[x_c1, x_c0], [y_c1, y_c0], ["1", "0"]]  (note: snarkjs uses c1,c0 order)
 *   pi_c: [x, y, "1"]
 */
export function encodeProofForContract(proof: Groth16Proof): Uint8Array {
  const result = new Uint8Array(256);

  // A (G1) — pi_a[0] = x, pi_a[1] = y
  writeFieldElement(result, 0, BigInt(proof.pi_a[0]));
  writeFieldElement(result, 32, BigInt(proof.pi_a[1]));

  // B (G2) — snarkjs stores as [[c1, c0], [c1, c0], ...]
  // On-chain verifier expects [c0, c1] order
  writeFieldElement(result, 64, BigInt(proof.pi_b[0][1])); // x_c0
  writeFieldElement(result, 96, BigInt(proof.pi_b[0][0])); // x_c1
  writeFieldElement(result, 128, BigInt(proof.pi_b[1][1])); // y_c0
  writeFieldElement(result, 160, BigInt(proof.pi_b[1][0])); // y_c1

  // C (G1) — pi_c[0] = x, pi_c[1] = y
  writeFieldElement(result, 192, BigInt(proof.pi_c[0]));
  writeFieldElement(result, 224, BigInt(proof.pi_c[1]));

  return result;
}

/** Write a field element as 32-byte big-endian into a buffer at the given offset. */
function writeFieldElement(buf: Uint8Array, offset: number, value: bigint): void {
  let val = value;
  for (let i = 31; i >= 0; i--) {
    buf[offset + i] = Number(val & 0xffn);
    val >>= 8n;
  }
}
