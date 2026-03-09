/**
 * Web Worker script for off-thread Groth16 proof generation.
 *
 * This file runs inside a Web Worker. It receives proof generation
 * requests via postMessage and returns results back to the main thread.
 *
 * Usage (from main thread):
 *   const worker = new Worker(new URL('./worker-thread.js', import.meta.url));
 *   worker.postMessage({ type: 'generate', payload: { circuitInput, wasmPath, zkeyPath } });
 *   worker.onmessage = (e) => { console.log(e.data); };
 */

// Web Worker global scope
const ctx = self as unknown as Worker;

interface GenerateRequest {
  type: "generate";
  id: string;
  payload: {
    circuitInput: Record<string, string | string[]>;
    wasmPath: string;
    zkeyPath: string;
  };
}

interface WorkerResponse {
  type: "result" | "error";
  id: string;
  proof?: { proof: unknown; publicSignals: string[] };
  error?: string;
}

ctx.onmessage = async (event: MessageEvent<GenerateRequest>) => {
  const { type, id, payload } = event.data;

  if (type !== "generate") {
    const resp: WorkerResponse = {
      type: "error",
      id: id || "unknown",
      error: `Unknown message type: ${type}`,
    };
    ctx.postMessage(resp);
    return;
  }

  try {
    const snarkjs = await import("snarkjs");
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(
      payload.circuitInput,
      payload.wasmPath,
      payload.zkeyPath
    );

    const resp: WorkerResponse = {
      type: "result",
      id,
      proof: { proof, publicSignals },
    };
    ctx.postMessage(resp);
  } catch (err) {
    const resp: WorkerResponse = {
      type: "error",
      id,
      error: err instanceof Error ? err.message : String(err),
    };
    ctx.postMessage(resp);
  }
};
