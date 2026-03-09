/**
 * Tests for the wallet module: note storage, export, import.
 */

import { initPoseidon } from "../src/crypto/poseidon";
import { generateDepositNote } from "../src/crypto/secrets";
import { NoteStore } from "../src/wallet/notestore";

beforeAll(async () => {
  await initPoseidon();
});

describe("NoteStore", () => {
  const poolId = "CTEST_POOL_CONTRACT_ID";
  const password = "secure-test-pass-123";

  test("add and retrieve note", () => {
    const store = new NoteStore();
    const note = generateDepositNote();
    const key = "0x" + note.commitment.toString(16).padStart(64, "0");

    store.add(note, poolId, 0);
    const stored = store.get(key);
    expect(stored).toBeDefined();
    expect(stored!.note.commitment).toBe(note.commitment);
    expect(stored!.poolId).toBe(poolId);
    expect(stored!.spent).toBe(false);
  });

  test("mark note as spent", () => {
    const store = new NoteStore();
    const note = generateDepositNote();
    const key = "0x" + note.commitment.toString(16).padStart(64, "0");

    store.add(note, poolId);
    store.markSpent(key);
    expect(store.get(key)!.spent).toBe(true);
  });

  test("getUnspent filters correctly", () => {
    const store = new NoteStore();
    const note1 = generateDepositNote();
    const note2 = generateDepositNote();
    const note3 = generateDepositNote();
    const key1 = "0x" + note1.commitment.toString(16).padStart(64, "0");

    store.add(note1, poolId);
    store.add(note2, poolId);
    store.add(note3, "other-pool");

    store.markSpent(key1);

    const unspent = store.getUnspent(poolId);
    expect(unspent.length).toBe(1);
    expect(unspent[0].note.commitment).toBe(note2.commitment);
  });

  test("export and import note roundtrip", () => {
    const store = new NoteStore();
    const note = generateDepositNote();
    const key = "0x" + note.commitment.toString(16).padStart(64, "0");

    store.add(note, poolId, 42);
    const backup = store.exportNote(key, password);

    // Import into a fresh store
    const store2 = new NoteStore();
    const imported = store2.importNote(backup, password);

    expect(imported.secret).toBe(note.secret);
    expect(imported.nullifier).toBe(note.nullifier);
    expect(imported.commitment).toBe(note.commitment);
    expect(imported.nullifierHash).toBe(note.nullifierHash);

    // Verify metadata preserved
    const stored = store2.get(key);
    expect(stored!.poolId).toBe(poolId);
    expect(stored!.leafIndex).toBe(42);
  });

  test("import with wrong password fails", () => {
    const store = new NoteStore();
    const note = generateDepositNote();
    const key = "0x" + note.commitment.toString(16).padStart(64, "0");

    store.add(note, poolId);
    const backup = store.exportNote(key, password);

    const store2 = new NoteStore();
    expect(() => store2.importNote(backup, "wrong-password-1234")).toThrow();
  });

  test("exportAll creates backups for all notes", () => {
    const store = new NoteStore();
    store.add(generateDepositNote(), poolId);
    store.add(generateDepositNote(), poolId);
    store.add(generateDepositNote(), poolId);

    const backups = store.exportAll(password);
    expect(backups.length).toBe(3);
    backups.forEach((b) => {
      expect(b.version).toBe(1);
      expect(b.poolId).toBe(poolId);
    });
  });
});
