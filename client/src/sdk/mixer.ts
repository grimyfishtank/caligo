/**
 * Caligo Mixer SDK — high-level deposit and withdrawal interface.
 *
 * Orchestrates the full mixer flow:
 * - Deposit: generate note → submit commitment on-chain → store note
 * - Withdraw: fetch merkle path → generate proof → submit withdrawal
 *
 * This module ties together crypto, proof, and wallet modules.
 */

import type { DepositNote } from "../crypto/secrets.js";
import type { MerkleProof } from "../crypto/merkle.js";
import type { ProofResult } from "../proof/prover.js";
import { generateDepositNote, bigIntToBytes, addressToField } from "../crypto/secrets.js";
import { generateWithdrawalProof, encodeProofForContract } from "../proof/prover.js";
import { NoteStore } from "../wallet/notestore.js";

/** Configuration for a mixer pool instance. */
export interface MixerConfig {
  /** Soroban contract ID for the mixer pool */
  contractId: string;
  /** Pool denomination in stroops */
  denomination: bigint;
  /** Path to compiled circuit WASM */
  wasmPath: string;
  /** Path to ceremony zkey file */
  zkeyPath: string;
  /** Stellar network passphrase */
  networkPassphrase: string;
  /** Soroban RPC URL */
  rpcUrl: string;
}

/** Merkle path data returned by the indexer. */
export interface MerklePathResponse {
  root: string;
  pathElements: string[];
  pathIndices: number[];
  leafIndex: number;
}

/** Result of a deposit operation. */
export interface DepositResult {
  note: DepositNote;
  commitmentHex: string;
  leafIndex?: number;
}

/** Result of a withdrawal operation. */
export interface WithdrawResult {
  proof: ProofResult;
  proofBytes: Uint8Array;
  nullifierHashHex: string;
  publicInputs: Uint8Array[];
}

/**
 * Caligo Mixer SDK.
 *
 * Provides high-level methods for interacting with a mixer pool.
 * Transaction submission is left to the caller (using @stellar/stellar-sdk)
 * to allow flexibility in signing and fee strategies.
 */
export class MixerSDK {
  readonly config: MixerConfig;
  readonly noteStore: NoteStore;

  constructor(config: MixerConfig) {
    this.config = config;
    this.noteStore = new NoteStore();
  }

  /**
   * Prepare a deposit: generate a fresh note and commitment.
   *
   * The caller must:
   * 1. Prompt the user to back up the note (export encrypted backup)
   * 2. Submit the deposit transaction with the commitment bytes
   * 3. Call finalizeDeposit() with the leaf index after on-chain confirmation
   *
   * @returns The deposit note and commitment bytes for on-chain submission
   */
  prepareDeposit(): DepositResult {
    const note = generateDepositNote();
    const commitmentHex = "0x" + note.commitment.toString(16).padStart(64, "0");

    // Store note immediately (even before on-chain confirmation)
    // If the tx fails, the note is harmless — it just won't be in the tree
    this.noteStore.add(note, this.config.contractId);

    return {
      note,
      commitmentHex,
    };
  }

  /**
   * Get the commitment as 32-byte big-endian for on-chain submission.
   */
  getCommitmentBytes(note: DepositNote): Uint8Array {
    return bigIntToBytes(note.commitment);
  }

  /**
   * Update the stored note with the confirmed leaf index.
   * Call this after the deposit transaction is confirmed on-chain.
   */
  finalizeDeposit(commitmentHex: string, leafIndex: number): void {
    const stored = this.noteStore.get(commitmentHex);
    if (stored) {
      stored.leafIndex = leafIndex;
    }
  }

  /**
   * Prepare a withdrawal: generate the Groth16 proof and encode it
   * for on-chain submission.
   *
   * The caller must:
   * 1. Fetch the Merkle path (from indexer or local tree)
   * 2. Submit the withdrawal transaction with the proof bytes
   *
   * @param note - The deposit note to withdraw
   * @param merkleProof - Merkle inclusion proof from the indexer
   * @param recipient - Stellar address to receive funds
   * @param relayer - Stellar relayer address (use a zero-like address for direct withdrawal)
   * @param fee - Relayer fee in stroops (0 for direct withdrawal)
   */
  async prepareWithdrawal(
    note: DepositNote,
    merkleProof: MerkleProof,
    recipient: string,
    relayer: string,
    fee: bigint
  ): Promise<WithdrawResult> {
    const proofResult = await generateWithdrawalProof(
      { note, merkleProof, recipient, relayer, fee },
      this.config.wasmPath,
      this.config.zkeyPath
    );

    const proofBytes = encodeProofForContract(proofResult.proof);

    const nullifierHashHex =
      "0x" + note.nullifierHash.toString(16).padStart(64, "0");

    // Encode public inputs as 32-byte big-endian arrays
    const publicInputs = [
      bigIntToBytes(merkleProof.root),
      bigIntToBytes(note.nullifierHash),
      bigIntToBytes(addressToField(recipient)),
      bigIntToBytes(addressToField(relayer)),
      bigIntToBytes(fee),
    ];

    return {
      proof: proofResult,
      proofBytes,
      nullifierHashHex,
      publicInputs,
    };
  }

  /**
   * Mark a note as spent after successful on-chain withdrawal.
   */
  finalizeWithdrawal(commitmentHex: string): void {
    this.noteStore.markSpent(commitmentHex);
  }

  /**
   * Convert an indexer Merkle path response to the internal format.
   */
  parseMerklePathResponse(response: MerklePathResponse): MerkleProof {
    return {
      root: BigInt(response.root),
      pathElements: response.pathElements.map((e) => BigInt(e)),
      pathIndices: response.pathIndices,
      leafIndex: response.leafIndex,
    };
  }
}
