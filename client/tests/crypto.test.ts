/**
 * Tests for the crypto module: Poseidon hashing, secrets, encryption, Merkle tree.
 */

import {
  initPoseidon,
  poseidonHash1,
  poseidonHash2,
  bytesToBigInt,
  bigIntToBytes,
} from "../src/crypto/poseidon";
import {
  generateDepositNote,
  recomputeNote,
  serializeNote,
  deserializeNote,
  randomFieldElement,
  addressToField,
} from "../src/crypto/secrets";
import { encrypt, decrypt } from "../src/crypto/encryption";
import { MerkleTree } from "../src/crypto/merkle";

beforeAll(async () => {
  await initPoseidon();
});

describe("Poseidon hashing", () => {
  test("poseidonHash2 is deterministic", () => {
    const a = 123n;
    const b = 456n;
    const h1 = poseidonHash2(a, b);
    const h2 = poseidonHash2(a, b);
    expect(h1).toBe(h2);
  });

  test("poseidonHash2 is order-dependent", () => {
    const a = 123n;
    const b = 456n;
    const h1 = poseidonHash2(a, b);
    const h2 = poseidonHash2(b, a);
    expect(h1).not.toBe(h2);
  });

  test("poseidonHash1 is deterministic", () => {
    const h1 = poseidonHash1(42n);
    const h2 = poseidonHash1(42n);
    expect(h1).toBe(h2);
  });

  test("poseidonHash1 produces different outputs for different inputs", () => {
    const h1 = poseidonHash1(1n);
    const h2 = poseidonHash1(2n);
    expect(h1).not.toBe(h2);
  });
});

describe("bigint/bytes conversion", () => {
  test("roundtrip conversion", () => {
    const original = 0xdeadbeefcafebabe1234567890abcdefn;
    const bytes = bigIntToBytes(original);
    const result = bytesToBigInt(bytes);
    expect(result).toBe(original);
  });

  test("zero roundtrip", () => {
    const bytes = bigIntToBytes(0n);
    expect(bytes).toEqual(new Uint8Array(32));
    expect(bytesToBigInt(bytes)).toBe(0n);
  });

  test("max 31-byte value roundtrip", () => {
    const val = (1n << 248n) - 1n;
    const bytes = bigIntToBytes(val);
    const result = bytesToBigInt(bytes);
    expect(result).toBe(val);
  });
});

describe("Deposit note generation", () => {
  test("generates valid note with all fields", () => {
    const note = generateDepositNote();
    expect(note.secret).toBeDefined();
    expect(note.nullifier).toBeDefined();
    expect(note.commitment).toBeDefined();
    expect(note.nullifierHash).toBeDefined();
  });

  test("commitment matches Poseidon(secret, nullifier)", () => {
    const note = generateDepositNote();
    const expected = poseidonHash2(note.secret, note.nullifier);
    expect(note.commitment).toBe(expected);
  });

  test("nullifierHash matches Poseidon(nullifier)", () => {
    const note = generateDepositNote();
    const expected = poseidonHash1(note.nullifier);
    expect(note.nullifierHash).toBe(expected);
  });

  test("each note has unique secrets", () => {
    const note1 = generateDepositNote();
    const note2 = generateDepositNote();
    expect(note1.secret).not.toBe(note2.secret);
    expect(note1.nullifier).not.toBe(note2.nullifier);
    expect(note1.commitment).not.toBe(note2.commitment);
  });

  test("recomputeNote matches original", () => {
    const note = generateDepositNote();
    const recomputed = recomputeNote(note.secret, note.nullifier);
    expect(recomputed.commitment).toBe(note.commitment);
    expect(recomputed.nullifierHash).toBe(note.nullifierHash);
  });
});

describe("Note serialization", () => {
  test("serialize/deserialize roundtrip", () => {
    const note = generateDepositNote();
    const serialized = serializeNote(note);
    const deserialized = deserializeNote(serialized);
    expect(deserialized.secret).toBe(note.secret);
    expect(deserialized.nullifier).toBe(note.nullifier);
    expect(deserialized.commitment).toBe(note.commitment);
    expect(deserialized.nullifierHash).toBe(note.nullifierHash);
  });

  test("detects tampered commitment", () => {
    const note = generateDepositNote();
    const serialized = serializeNote(note);
    serialized.commitment = "0x" + "ff".repeat(32);
    expect(() => deserializeNote(serialized)).toThrow("integrity check failed");
  });
});

describe("AES-256-GCM encryption", () => {
  const password = "test-password-12345";
  const plaintext = '{"secret":"0xabc","nullifier":"0xdef"}';

  test("encrypt/decrypt roundtrip", () => {
    const encrypted = encrypt(plaintext, password);
    const decrypted = decrypt(encrypted, password);
    expect(decrypted).toBe(plaintext);
  });

  test("wrong password fails", () => {
    const encrypted = encrypt(plaintext, password);
    expect(() => decrypt(encrypted, "wrong-password-1234")).toThrow();
  });

  test("different encryptions produce different ciphertexts", () => {
    const e1 = encrypt(plaintext, password);
    const e2 = encrypt(plaintext, password);
    expect(e1.ciphertext).not.toBe(e2.ciphertext);
    expect(e1.iv).not.toBe(e2.iv);
    expect(e1.salt).not.toBe(e2.salt);
  });

  test("rejects short password", () => {
    expect(() => encrypt(plaintext, "short")).toThrow("at least 8");
  });

  test("detects tampered ciphertext", () => {
    const encrypted = encrypt(plaintext, password);
    // Flip a byte in the ciphertext
    const tamperedHex = encrypted.ciphertext.split("");
    tamperedHex[0] = tamperedHex[0] === "a" ? "b" : "a";
    encrypted.ciphertext = tamperedHex.join("");
    expect(() => decrypt(encrypted, password)).toThrow();
  });
});

describe("Address to field element", () => {
  test("deterministic conversion", () => {
    const addr = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const f1 = addressToField(addr);
    const f2 = addressToField(addr);
    expect(f1).toBe(f2);
  });

  test("different addresses produce different field elements", () => {
    const addr1 = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const addr2 = "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A";
    expect(addressToField(addr1)).not.toBe(addressToField(addr2));
  });
});

describe("Merkle tree", () => {
  test("empty tree has deterministic root", () => {
    const tree = new MerkleTree(4);
    const root1 = tree.getRoot();
    const tree2 = new MerkleTree(4);
    const root2 = tree2.getRoot();
    expect(root1).toBe(root2);
  });

  test("inserting changes root", () => {
    const tree = new MerkleTree(4);
    const emptyRoot = tree.getRoot();
    tree.insert(poseidonHash2(1n, 2n));
    expect(tree.getRoot()).not.toBe(emptyRoot);
  });

  test("different insertion order produces different roots", () => {
    const c1 = poseidonHash2(1n, 2n);
    const c2 = poseidonHash2(3n, 4n);

    const tree1 = new MerkleTree(4);
    tree1.insert(c1);
    tree1.insert(c2);

    const tree2 = new MerkleTree(4);
    tree2.insert(c2);
    tree2.insert(c1);

    expect(tree1.getRoot()).not.toBe(tree2.getRoot());
  });

  test("merkle proof verifies against root", () => {
    const tree = new MerkleTree(4);
    const c1 = poseidonHash2(10n, 20n);
    const c2 = poseidonHash2(30n, 40n);
    const c3 = poseidonHash2(50n, 60n);

    tree.insert(c1);
    tree.insert(c2);
    const idx = tree.insert(c3);

    const proof = tree.getProof(idx);
    expect(proof.root).toBe(tree.getRoot());
    expect(proof.leafIndex).toBe(idx);
    expect(proof.pathElements.length).toBe(4);
    expect(proof.pathIndices.length).toBe(4);
  });

  test("proof path reconstructs root", () => {
    const tree = new MerkleTree(4);
    const commitment = poseidonHash2(100n, 200n);
    const idx = tree.insert(commitment);
    const proof = tree.getProof(idx);

    // Manually reconstruct root from proof
    let hash = commitment;
    for (let i = 0; i < proof.pathElements.length; i++) {
      if (proof.pathIndices[i] === 0) {
        hash = poseidonHash2(hash, proof.pathElements[i]);
      } else {
        hash = poseidonHash2(proof.pathElements[i], hash);
      }
    }
    expect(hash).toBe(proof.root);
  });

  test("out-of-bounds proof throws", () => {
    const tree = new MerkleTree(4);
    tree.insert(poseidonHash2(1n, 2n));
    expect(() => tree.getProof(5)).toThrow("out of bounds");
  });
});

describe("randomFieldElement", () => {
  test("produces values less than field order", () => {
    const fieldOrder =
      21888242871839275222246405745257275088548364400416034343698204186575808495617n;
    for (let i = 0; i < 10; i++) {
      const val = randomFieldElement();
      expect(val).toBeGreaterThanOrEqual(0n);
      expect(val).toBeLessThan(fieldOrder);
    }
  });
});
