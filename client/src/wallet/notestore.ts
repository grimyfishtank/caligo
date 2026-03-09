/**
 * Deposit note storage and encrypted backup/restore.
 *
 * Notes are stored in-memory and can be exported as encrypted JSON files.
 * The encryption uses AES-256-GCM with a password-derived key (see crypto/encryption.ts).
 *
 * CRITICAL: If all copies of a note are lost, the deposited funds are
 * permanently unrecoverable. The SDK must prompt users to export backups.
 */

import { encrypt, decrypt } from "../crypto/encryption.js";
import type { EncryptedPayload } from "../crypto/encryption.js";
import { serializeNote, deserializeNote } from "../crypto/secrets.js";
import type { DepositNote } from "../crypto/secrets.js";

/** Metadata attached to a stored note. */
export interface StoredNote {
  note: DepositNote;
  poolId: string;
  depositTimestamp: number;
  leafIndex?: number;
  spent: boolean;
}

/** Encrypted backup file format. */
export interface NoteBackup {
  version: 1;
  poolId: string;
  depositTimestamp: number;
  leafIndex?: number;
  encrypted: EncryptedPayload;
}

/**
 * In-memory note store. In a real application, this would be backed
 * by localStorage, IndexedDB, or a secure native keystore.
 */
export class NoteStore {
  private notes: Map<string, StoredNote> = new Map();

  /** Add a note to the store. Key is the commitment hex. */
  add(note: DepositNote, poolId: string, leafIndex?: number): void {
    const key = "0x" + note.commitment.toString(16).padStart(64, "0");
    this.notes.set(key, {
      note,
      poolId,
      depositTimestamp: Date.now(),
      leafIndex,
      spent: false,
    });
  }

  /** Retrieve a note by commitment hex. */
  get(commitmentHex: string): StoredNote | undefined {
    return this.notes.get(commitmentHex);
  }

  /** Mark a note as spent (after successful withdrawal). */
  markSpent(commitmentHex: string): void {
    const stored = this.notes.get(commitmentHex);
    if (stored) {
      stored.spent = true;
    }
  }

  /** Get all unspent notes for a given pool. */
  getUnspent(poolId: string): StoredNote[] {
    return Array.from(this.notes.values()).filter(
      (n) => n.poolId === poolId && !n.spent
    );
  }

  /** Get all notes (spent and unspent). */
  getAll(): StoredNote[] {
    return Array.from(this.notes.values());
  }

  /**
   * Export a note as an encrypted backup.
   * The user must provide a password to protect the backup.
   */
  exportNote(commitmentHex: string, password: string): NoteBackup {
    const stored = this.notes.get(commitmentHex);
    if (!stored) {
      throw new Error(`Note not found: ${commitmentHex}`);
    }

    const serialized = serializeNote(stored.note);
    const plaintext = JSON.stringify(serialized);
    const encrypted = encrypt(plaintext, password);

    return {
      version: 1,
      poolId: stored.poolId,
      depositTimestamp: stored.depositTimestamp,
      leafIndex: stored.leafIndex,
      encrypted,
    };
  }

  /**
   * Import a note from an encrypted backup.
   * The user must provide the correct password.
   */
  importNote(backup: NoteBackup, password: string): DepositNote {
    if (backup.version !== 1) {
      throw new Error(`Unsupported backup version: ${backup.version}`);
    }

    const plaintext = decrypt(backup.encrypted, password);
    const serialized = JSON.parse(plaintext);
    const note = deserializeNote(serialized);

    this.notes.set(
      "0x" + note.commitment.toString(16).padStart(64, "0"),
      {
        note,
        poolId: backup.poolId,
        depositTimestamp: backup.depositTimestamp,
        leafIndex: backup.leafIndex,
        spent: false,
      }
    );

    return note;
  }

  /**
   * Export all notes as encrypted backups.
   */
  exportAll(password: string): NoteBackup[] {
    const backups: NoteBackup[] = [];
    for (const [key] of this.notes) {
      backups.push(this.exportNote(key, password));
    }
    return backups;
  }
}
