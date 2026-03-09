/**
 * Web Worker wrapper for Groth16 proof generation.
 *
 * Offloads the computationally expensive snarkjs proof generation
 * to a Web Worker so the main thread (UI) remains responsive.
 *
 * Falls back to main-thread generation in environments without
 * Web Worker support (e.g., Node.js test runner).
 *
 * Usage:
 *   const prover = new WorkerProver();
 *   const result = await prover.generateProof(input, wasmPath, zkeyPath);
 *   prover.terminate();
 */

import type { WithdrawalProofInput, ProofResult } from "./prover.js";
import { addressToField } from "../crypto/secrets.js";

/** Options for the WorkerProver. */
export interface WorkerProverOptions {
  /**
   * URL or path to the compiled worker-thread.js file.
   * If not provided, falls back to main-thread proof generation.
   */
  workerUrl?: URL | string;

  /**
   * Timeout in milliseconds for proof generation.
   * Default: 60000 (60 seconds).
   */
  timeout?: number;
}

/**
 * Proof generator that runs snarkjs in a Web Worker.
 *
 * Provides the same interface as generateWithdrawalProof() but
 * runs the computation off the main thread.
 */
export class WorkerProver {
  private worker: Worker | null = null;
  private pendingRequests = new Map<
    string,
    { resolve: (r: ProofResult) => void; reject: (e: Error) => void }
  >();
  private requestCounter = 0;
  private readonly timeout: number;
  private readonly workerUrl?: URL | string;

  constructor(options: WorkerProverOptions = {}) {
    this.timeout = options.timeout ?? 60000;
    this.workerUrl = options.workerUrl;
  }

  /**
   * Generate a Groth16 withdrawal proof in a Web Worker.
   *
   * If Web Workers are unavailable (Node.js, no workerUrl),
   * falls back to main-thread generation.
   */
  async generateProof(
    input: WithdrawalProofInput,
    wasmPath: string,
    zkeyPath: string
  ): Promise<ProofResult> {
    const circuitInput = buildCircuitInput(input);

    // Try Web Worker path
    if (this.workerUrl && typeof Worker !== "undefined") {
      return this.generateInWorker(circuitInput, wasmPath, zkeyPath);
    }

    // Fallback: main-thread generation
    return this.generateOnMainThread(circuitInput, wasmPath, zkeyPath);
  }

  /**
   * Terminate the worker. Call this when done to free resources.
   */
  terminate(): void {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }
    // Reject any pending requests
    for (const [, pending] of this.pendingRequests) {
      pending.reject(new Error("Worker terminated"));
    }
    this.pendingRequests.clear();
  }

  private ensureWorker(): Worker {
    if (!this.worker) {
      this.worker = new Worker(this.workerUrl as URL | string, {
        type: "module",
      });

      this.worker.onmessage = (event) => {
        const { type, id, proof, error } = event.data;
        const pending = this.pendingRequests.get(id);
        if (!pending) return;

        this.pendingRequests.delete(id);
        if (type === "result" && proof) {
          pending.resolve(proof);
        } else {
          pending.reject(new Error(error || "Unknown worker error"));
        }
      };

      this.worker.onerror = (event) => {
        // Reject all pending requests on worker error
        for (const [, pending] of this.pendingRequests) {
          pending.reject(new Error(`Worker error: ${event.message}`));
        }
        this.pendingRequests.clear();
      };
    }
    return this.worker;
  }

  private generateInWorker(
    circuitInput: Record<string, string | string[]>,
    wasmPath: string,
    zkeyPath: string
  ): Promise<ProofResult> {
    return new Promise((resolve, reject) => {
      const id = String(++this.requestCounter);
      const worker = this.ensureWorker();

      // Set timeout
      const timer = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Proof generation timed out after ${this.timeout}ms`));
      }, this.timeout);

      this.pendingRequests.set(id, {
        resolve: (result) => {
          clearTimeout(timer);
          resolve(result);
        },
        reject: (err) => {
          clearTimeout(timer);
          reject(err);
        },
      });

      worker.postMessage({
        type: "generate",
        id,
        payload: { circuitInput, wasmPath, zkeyPath },
      });
    });
  }

  private async generateOnMainThread(
    circuitInput: Record<string, string | string[]>,
    wasmPath: string,
    zkeyPath: string
  ): Promise<ProofResult> {
    const snarkjs = await import("snarkjs");
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(
      circuitInput,
      wasmPath,
      zkeyPath
    );
    return { proof, publicSignals };
  }
}

/** Build the circuit input object from withdrawal proof inputs. */
function buildCircuitInput(
  input: WithdrawalProofInput
): Record<string, string | string[]> {
  const recipientField = addressToField(input.recipient);
  const relayerField = addressToField(input.relayer);

  return {
    root: input.merkleProof.root.toString(),
    nullifierHash: input.note.nullifierHash.toString(),
    recipient: recipientField.toString(),
    relayer: relayerField.toString(),
    fee: input.fee.toString(),
    secret: input.note.secret.toString(),
    nullifier: input.note.nullifier.toString(),
    pathElements: input.merkleProof.pathElements.map((e) => e.toString()),
    pathIndices: input.merkleProof.pathIndices.map((e) => e.toString()),
  };
}
