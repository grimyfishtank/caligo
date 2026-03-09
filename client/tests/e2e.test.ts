/**
 * End-to-end integration test: deposit → Merkle proof → ZK proof → verification.
 *
 * This test exercises the full client-side flow using real circuit artifacts:
 *   1. Generate deposit note (secret, nullifier, commitment)
 *   2. Insert commitment into client-side Merkle tree
 *   3. Generate Groth16 withdrawal proof using snarkjs
 *   4. Verify the proof locally
 *   5. Encode proof for on-chain submission
 *
 * Requires circuit build artifacts in circuits/build/
 * (run `npm run build:circuit && npm run setup` first).
 */

import * as path from "path";
import * as fs from "fs";
import {
  initPoseidon,
  generateDepositNote,
  addressToField,
  bigIntToBytes,
} from "../src/crypto";
import { MerkleTree } from "../src/crypto/merkle";
import {
  generateWithdrawalProof,
  verifyProof,
  encodeProofForContract,
} from "../src/proof";
import { NoteStore } from "../src/wallet";
import { estimateRelayerFee } from "../src/relayer";

const CIRCUITS_DIR = path.join(__dirname, "..", "..", "circuits", "build");
const WASM_PATH = path.join(CIRCUITS_DIR, "withdraw_js", "withdraw.wasm");
const ZKEY_PATH = path.join(CIRCUITS_DIR, "withdraw_0001.zkey");
const VK_PATH = path.join(CIRCUITS_DIR, "verification_key.json");
const TREE_DEPTH = 20;

// Skip if circuit artifacts aren't built
const artifactsExist =
  fs.existsSync(WASM_PATH) &&
  fs.existsSync(ZKEY_PATH) &&
  fs.existsSync(VK_PATH);

const describeOrSkip = artifactsExist ? describe : describe.skip;

describeOrSkip("E2E: Full deposit → withdraw cycle", () => {
  beforeAll(async () => {
    await initPoseidon();
  });

  test("complete withdrawal with valid proof", async () => {
    // ── Step 1: Generate deposit note ──
    const note = generateDepositNote();
    expect(note.secret).toBeDefined();
    expect(note.commitment).toBeDefined();

    // ── Step 2: Build Merkle tree and insert commitments ──
    const tree = new MerkleTree(TREE_DEPTH);

    // Insert some dummy commitments first (simulates other deposits)
    for (let i = 0; i < 3; i++) {
      const dummy = generateDepositNote();
      tree.insert(dummy.commitment);
    }

    // Insert our commitment
    const leafIndex = tree.insert(note.commitment);
    expect(leafIndex).toBe(3);

    // ── Step 3: Get Merkle proof ──
    const merkleProof = tree.getProof(leafIndex);
    expect(merkleProof.root).toBe(tree.getRoot());
    expect(merkleProof.pathElements.length).toBe(TREE_DEPTH);

    // ── Step 4: Generate Groth16 proof ──
    const recipient = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const relayer = "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A";
    const fee = 0n; // Direct withdrawal, no relayer fee

    const proofResult = await generateWithdrawalProof(
      { note, merkleProof, recipient, relayer, fee },
      WASM_PATH,
      ZKEY_PATH
    );

    expect(proofResult.proof).toBeDefined();
    expect(proofResult.publicSignals).toHaveLength(5);

    // ── Step 5: Verify proof locally ──
    const isValid = await verifyProof(
      proofResult.proof,
      proofResult.publicSignals,
      VK_PATH
    );
    expect(isValid).toBe(true);

    // ── Step 6: Encode proof for on-chain submission ──
    const proofBytes = encodeProofForContract(proofResult.proof);
    expect(proofBytes.length).toBe(256);

    // Verify the public signals match our inputs
    const [sigRoot, sigNullifierHash, sigRecipient, sigRelayer, sigFee] =
      proofResult.publicSignals;

    expect(BigInt(sigRoot)).toBe(merkleProof.root);
    expect(BigInt(sigNullifierHash)).toBe(note.nullifierHash);
    expect(BigInt(sigRecipient)).toBe(addressToField(recipient));
    expect(BigInt(sigRelayer)).toBe(addressToField(relayer));
    expect(BigInt(sigFee)).toBe(fee);
  }, 30000); // Allow 30s for proof generation

  test("invalid secret produces invalid proof", async () => {
    const note = generateDepositNote();
    const tree = new MerkleTree(TREE_DEPTH);
    const leafIndex = tree.insert(note.commitment);
    const merkleProof = tree.getProof(leafIndex);

    // Tamper with the secret
    const tamperedNote = { ...note, secret: note.secret + 1n };

    const recipient = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const relayer = "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A";

    // Proof generation should fail at the constraint level
    await expect(
      generateWithdrawalProof(
        { note: tamperedNote, merkleProof, recipient, relayer, fee: 0n },
        WASM_PATH,
        ZKEY_PATH
      )
    ).rejects.toThrow();
  }, 30000);

  test("wrong nullifier hash rejected", async () => {
    const note = generateDepositNote();
    const tree = new MerkleTree(TREE_DEPTH);
    const leafIndex = tree.insert(note.commitment);
    const merkleProof = tree.getProof(leafIndex);

    // Tamper with nullifierHash
    const tamperedNote = { ...note, nullifierHash: note.nullifierHash + 1n };

    const recipient = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const relayer = "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A";

    await expect(
      generateWithdrawalProof(
        { note: tamperedNote, merkleProof, recipient, relayer, fee: 0n },
        WASM_PATH,
        ZKEY_PATH
      )
    ).rejects.toThrow();
  }, 30000);

  test("note store integration with full flow", async () => {
    const store = new NoteStore();
    const poolId = "test-pool-100xlm";
    const password = "secure-backup-pass-123";

    // Generate and store note
    const note = generateDepositNote();
    const commitmentHex =
      "0x" + note.commitment.toString(16).padStart(64, "0");
    store.add(note, poolId, 5);

    // Export encrypted backup
    const backup = store.exportNote(commitmentHex, password);
    expect(backup.version).toBe(1);

    // Simulate note loss — import into fresh store
    const freshStore = new NoteStore();
    const restored = freshStore.importNote(backup, password);

    // Verify restored note works for proof generation
    const tree = new MerkleTree(TREE_DEPTH);
    tree.insert(restored.commitment);
    const proof = tree.getProof(0);

    const recipient = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const relayer = "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A";

    const result = await generateWithdrawalProof(
      { note: restored, merkleProof: proof, recipient, relayer, fee: 0n },
      WASM_PATH,
      ZKEY_PATH
    );

    const isValid = await verifyProof(
      result.proof,
      result.publicSignals,
      VK_PATH
    );
    expect(isValid).toBe(true);
  }, 30000);

  test("relayer fee estimation", () => {
    // 100 XLM pool, 1% relayer fee
    const denomination = 1_000_000_000n; // 100 XLM in stroops
    const feeBps = 100; // 1%
    const fee = estimateRelayerFee(denomination, feeBps);
    expect(fee).toBe(10_000_000n); // 1 XLM
  });

  test("proof encoding produces valid 256-byte output", async () => {
    const note = generateDepositNote();
    const tree = new MerkleTree(TREE_DEPTH);
    tree.insert(note.commitment);
    const merkleProof = tree.getProof(0);

    const recipient = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const relayer = "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A";

    const result = await generateWithdrawalProof(
      { note, merkleProof, recipient, relayer, fee: 0n },
      WASM_PATH,
      ZKEY_PATH
    );

    const encoded = encodeProofForContract(result.proof);

    // Verify layout: A(64) + B(128) + C(64) = 256
    expect(encoded.length).toBe(256);

    // A and C are G1 points (should be non-zero)
    const aBytes = encoded.slice(0, 64);
    const cBytes = encoded.slice(192, 256);
    expect(aBytes.some((b) => b !== 0)).toBe(true);
    expect(cBytes.some((b) => b !== 0)).toBe(true);

    // B is a G2 point (should be non-zero)
    const bBytes = encoded.slice(64, 192);
    expect(bBytes.some((b) => b !== 0)).toBe(true);
  }, 30000);
});
