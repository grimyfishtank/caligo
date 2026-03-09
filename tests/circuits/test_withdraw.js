/**
 * Private Stellar Protocol — Withdrawal Circuit E2E Test
 *
 * Tests the full cycle: generate commitment, build Merkle tree,
 * create proof, verify proof — all off-chain using snarkjs.
 *
 * This validates that:
 * 1. The circuit compiles and accepts valid inputs
 * 2. Poseidon hashing produces consistent results
 * 3. Merkle proof verification works at depth 20
 * 4. Invalid inputs are rejected
 */

const snarkjs = require("snarkjs");
const path = require("path");
const fs = require("fs");

const BUILD_DIR = path.join(__dirname, "..", "..", "circuits", "build");
const WASM_PATH = path.join(BUILD_DIR, "withdraw_js", "withdraw.wasm");
const ZKEY_PATH = path.join(BUILD_DIR, "withdraw_0001.zkey");
const VK_PATH = path.join(BUILD_DIR, "verification_key.json");

const TREE_DEPTH = 20;

// ── Poseidon hash (using snarkjs/circomlib compatible implementation) ──

let poseidonHasher;

async function loadPoseidon() {
  // Use the circomlib poseidon implementation via snarkjs's bundled version
  // or load from node_modules
  try {
    const { buildPoseidon } = require("circomlibjs");
    poseidonHasher = await buildPoseidon();
  } catch {
    // If circomlibjs is not available, try the built-in approach
    console.error(
      "circomlibjs not found. Install it: npm install circomlibjs"
    );
    process.exit(1);
  }
}

function poseidon(inputs) {
  const hash = poseidonHasher(inputs.map(BigInt));
  return poseidonHasher.F.toObject(hash);
}

// ── Merkle Tree ──

class MerkleTree {
  constructor(depth) {
    this.depth = depth;
    this.leaves = [];
    this.zeroValues = new Array(depth + 1);
    this.layers = new Array(depth + 1);

    // Compute zero hashes for each level
    this.zeroValues[0] = poseidon([0n]);
    for (let i = 1; i <= depth; i++) {
      this.zeroValues[i] = poseidon([
        this.zeroValues[i - 1],
        this.zeroValues[i - 1],
      ]);
    }

    // Initialize layers
    for (let i = 0; i <= depth; i++) {
      this.layers[i] = [];
    }
  }

  insert(commitment) {
    const index = this.leaves.length;
    this.leaves.push(commitment);
    this.layers[0] = [...this.leaves];
    this._rebuild();
    return index;
  }

  _rebuild() {
    for (let level = 0; level < this.depth; level++) {
      const layerSize = Math.ceil(this.layers[level].length / 2);
      this.layers[level + 1] = new Array(layerSize);

      for (let i = 0; i < layerSize; i++) {
        const left =
          this.layers[level][i * 2] || this.zeroValues[level];
        const right =
          this.layers[level][i * 2 + 1] || this.zeroValues[level];
        this.layers[level + 1][i] = poseidon([left, right]);
      }
    }
  }

  getRoot() {
    if (this.layers[this.depth].length === 0) {
      return this.zeroValues[this.depth];
    }
    return this.layers[this.depth][0];
  }

  getProof(index) {
    const pathElements = [];
    const pathIndices = [];

    let currentIndex = index;
    for (let level = 0; level < this.depth; level++) {
      const siblingIndex =
        currentIndex % 2 === 0 ? currentIndex + 1 : currentIndex - 1;
      const sibling =
        this.layers[level][siblingIndex] || this.zeroValues[level];

      pathElements.push(sibling);
      pathIndices.push(currentIndex % 2);

      currentIndex = Math.floor(currentIndex / 2);
    }

    return { pathElements, pathIndices };
  }
}

// ── Test Helpers ──

function randomBigInt() {
  // Generate a random field element (< BN128 scalar field order)
  const bytes = new Uint8Array(31); // 31 bytes to stay under field order
  require("crypto").getRandomValues(bytes);
  let n = 0n;
  for (const b of bytes) {
    n = (n << 8n) | BigInt(b);
  }
  return n;
}

// ── Tests ──

async function main() {
  console.log("=== Private Stellar Protocol — Circuit Tests ===\n");

  // Check that build artifacts exist
  if (!fs.existsSync(WASM_PATH)) {
    console.error(`WASM not found at ${WASM_PATH}. Run: npm run build:circuit`);
    process.exit(1);
  }
  if (!fs.existsSync(ZKEY_PATH)) {
    console.error(`zkey not found at ${ZKEY_PATH}. Run: npm run setup`);
    process.exit(1);
  }

  await loadPoseidon();

  let passed = 0;
  let failed = 0;

  // ── Test 1: Valid proof generation and verification ──
  try {
    console.log("Test 1: Valid proof generation and verification...");

    const secret = randomBigInt();
    const nullifier = randomBigInt();
    const commitment = poseidon([secret, nullifier]);
    const nullifierHash = poseidon([nullifier]);

    const tree = new MerkleTree(TREE_DEPTH);

    // Insert some dummy commitments first (to test non-zero-index deposits)
    for (let i = 0; i < 3; i++) {
      tree.insert(poseidon([randomBigInt(), randomBigInt()]));
    }

    // Insert our commitment
    const leafIndex = tree.insert(commitment);
    const root = tree.getRoot();
    const { pathElements, pathIndices } = tree.getProof(leafIndex);

    // Use dummy values for recipient, relayer, fee (they're just field elements)
    const recipient = 12345n;
    const relayer = 0n;
    const fee = 0n;

    const input = {
      root: root.toString(),
      nullifierHash: nullifierHash.toString(),
      recipient: recipient.toString(),
      relayer: relayer.toString(),
      fee: fee.toString(),
      secret: secret.toString(),
      nullifier: nullifier.toString(),
      pathElements: pathElements.map((e) => e.toString()),
      pathIndices: pathIndices.map((e) => e.toString()),
    };

    // Generate proof
    const startTime = Date.now();
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(
      input,
      WASM_PATH,
      ZKEY_PATH
    );
    const proofTime = Date.now() - startTime;
    console.log(`  Proof generated in ${proofTime}ms`);

    // Verify proof
    const vk = JSON.parse(fs.readFileSync(VK_PATH, "utf8"));
    const valid = await snarkjs.groth16.verify(vk, publicSignals, proof);

    if (valid) {
      console.log("  PASSED: Valid proof verified successfully\n");
      passed++;
    } else {
      console.log("  FAILED: Valid proof was rejected\n");
      failed++;
    }
  } catch (e) {
    console.log(`  FAILED: ${e.message}\n`);
    failed++;
  }

  // ── Test 2: Invalid nullifier hash rejected ──
  try {
    console.log("Test 2: Invalid nullifier hash is rejected...");

    const secret = randomBigInt();
    const nullifier = randomBigInt();
    const commitment = poseidon([secret, nullifier]);
    const wrongNullifierHash = poseidon([randomBigInt()]); // wrong!

    const tree = new MerkleTree(TREE_DEPTH);
    const leafIndex = tree.insert(commitment);
    const root = tree.getRoot();
    const { pathElements, pathIndices } = tree.getProof(leafIndex);

    const input = {
      root: root.toString(),
      nullifierHash: wrongNullifierHash.toString(), // wrong
      recipient: "1",
      relayer: "0",
      fee: "0",
      secret: secret.toString(),
      nullifier: nullifier.toString(),
      pathElements: pathElements.map((e) => e.toString()),
      pathIndices: pathIndices.map((e) => e.toString()),
    };

    try {
      await snarkjs.groth16.fullProve(input, WASM_PATH, ZKEY_PATH);
      console.log("  FAILED: Should have thrown (invalid nullifier hash)\n");
      failed++;
    } catch {
      console.log(
        "  PASSED: Proof generation correctly rejected invalid nullifier hash\n"
      );
      passed++;
    }
  } catch (e) {
    console.log(`  FAILED: ${e.message}\n`);
    failed++;
  }

  // ── Test 3: Wrong secret rejected ──
  try {
    console.log("Test 3: Wrong secret is rejected...");

    const secret = randomBigInt();
    const nullifier = randomBigInt();
    const commitment = poseidon([secret, nullifier]);
    const nullifierHash = poseidon([nullifier]);

    const tree = new MerkleTree(TREE_DEPTH);
    const leafIndex = tree.insert(commitment);
    const root = tree.getRoot();
    const { pathElements, pathIndices } = tree.getProof(leafIndex);

    const input = {
      root: root.toString(),
      nullifierHash: nullifierHash.toString(),
      recipient: "1",
      relayer: "0",
      fee: "0",
      secret: randomBigInt().toString(), // wrong secret!
      nullifier: nullifier.toString(),
      pathElements: pathElements.map((e) => e.toString()),
      pathIndices: pathIndices.map((e) => e.toString()),
    };

    try {
      await snarkjs.groth16.fullProve(input, WASM_PATH, ZKEY_PATH);
      console.log("  FAILED: Should have thrown (wrong secret)\n");
      failed++;
    } catch {
      console.log(
        "  PASSED: Proof generation correctly rejected wrong secret\n"
      );
      passed++;
    }
  } catch (e) {
    console.log(`  FAILED: ${e.message}\n`);
    failed++;
  }

  // ── Test 4: Forged proof rejected by verifier ──
  try {
    console.log("Test 4: Forged proof rejected by verifier...");

    const vk = JSON.parse(fs.readFileSync(VK_PATH, "utf8"));

    // Create a fake proof with random values
    const fakeProof = {
      pi_a: ["1", "2", "1"],
      pi_b: [
        ["1", "2"],
        ["3", "4"],
        ["1", "0"],
      ],
      pi_c: ["5", "6", "1"],
      protocol: "groth16",
      curve: "bn128",
    };

    const fakeSignals = ["1", "2", "3", "4", "5"];

    const valid = await snarkjs.groth16.verify(
      vk,
      fakeSignals,
      fakeProof
    );

    if (!valid) {
      console.log("  PASSED: Forged proof correctly rejected\n");
      passed++;
    } else {
      console.log("  FAILED: Forged proof was accepted!\n");
      failed++;
    }
  } catch (e) {
    // Some implementations throw on invalid proofs
    console.log("  PASSED: Forged proof threw error (correctly rejected)\n");
    passed++;
  }

  // ── Summary ──
  console.log("=== Results ===");
  console.log(`Passed: ${passed}/${passed + failed}`);
  console.log(`Failed: ${failed}/${passed + failed}`);

  process.exit(failed > 0 ? 1 : 0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
