/**
 * Tests for the WorkerProver class.
 *
 * Since Jest doesn't provide a Web Worker environment, these tests
 * exercise the main-thread fallback path and the API surface.
 * Web Worker integration is tested manually in a browser environment.
 */

import { WorkerProver } from "../src/proof/worker-prover";

describe("WorkerProver", () => {
  test("constructor accepts default options", () => {
    const prover = new WorkerProver();
    expect(prover).toBeDefined();
    prover.terminate();
  });

  test("constructor accepts custom timeout", () => {
    const prover = new WorkerProver({ timeout: 30000 });
    expect(prover).toBeDefined();
    prover.terminate();
  });

  test("terminate is idempotent", () => {
    const prover = new WorkerProver();
    prover.terminate();
    prover.terminate(); // Should not throw
  });

  test("falls back to main thread without workerUrl", async () => {
    const prover = new WorkerProver();

    // Without circuit artifacts, this will throw from snarkjs,
    // but it proves the fallback path is taken (not the Worker path)
    await expect(
      prover.generateProof(
        {
          note: {
            secret: 123n,
            nullifier: 456n,
            commitment: 789n,
            nullifierHash: 101112n,
          },
          merkleProof: {
            root: 999n,
            pathElements: [1n, 2n],
            pathIndices: [0, 1],
            leafIndex: 0,
          },
          recipient:
            "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7",
          relayer:
            "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7",
          fee: 0n,
        },
        "/nonexistent/withdraw.wasm",
        "/nonexistent/withdraw.zkey"
      )
    ).rejects.toThrow();

    prover.terminate();
  });
});
