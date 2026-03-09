/**
 * AES-256-GCM encryption for deposit notes.
 *
 * Uses PBKDF2 to derive a 256-bit key from a user password.
 * The encrypted note can be stored locally or exported as a file.
 *
 * Security properties:
 * - AES-256-GCM provides authenticated encryption (confidentiality + integrity)
 * - PBKDF2 with 600,000 iterations resists brute-force password attacks
 * - Random salt and IV ensure unique ciphertext for each encryption
 */

import { randomBytes, createCipheriv, createDecipheriv, pbkdf2Sync } from "crypto";

const ALGORITHM = "aes-256-gcm";
const KEY_LENGTH = 32; // 256 bits
const IV_LENGTH = 12; // 96 bits (recommended for GCM)
const SALT_LENGTH = 32; // 256 bits
const AUTH_TAG_LENGTH = 16; // 128 bits
const PBKDF2_ITERATIONS = 600_000; // OWASP recommended minimum for SHA-256
const PBKDF2_DIGEST = "sha256";

/** Encrypted payload with all data needed for decryption (except the password). */
export interface EncryptedPayload {
  /** Version tag for forward compatibility */
  version: 1;
  /** PBKDF2 salt (hex) */
  salt: string;
  /** GCM initialization vector (hex) */
  iv: string;
  /** GCM authentication tag (hex) */
  authTag: string;
  /** Encrypted data (hex) */
  ciphertext: string;
}

/**
 * Derive a 256-bit AES key from a password using PBKDF2.
 */
function deriveKey(password: string, salt: Buffer): Buffer {
  return pbkdf2Sync(password, salt, PBKDF2_ITERATIONS, KEY_LENGTH, PBKDF2_DIGEST);
}

/**
 * Encrypt a plaintext string with AES-256-GCM using a password-derived key.
 */
export function encrypt(plaintext: string, password: string): EncryptedPayload {
  if (!password || password.length < 8) {
    throw new Error("Password must be at least 8 characters");
  }

  const salt = randomBytes(SALT_LENGTH);
  const iv = randomBytes(IV_LENGTH);
  const key = deriveKey(password, salt);

  const cipher = createCipheriv(ALGORITHM, key, iv, { authTagLength: AUTH_TAG_LENGTH });
  const encrypted = Buffer.concat([
    cipher.update(plaintext, "utf8"),
    cipher.final(),
  ]);
  const authTag = cipher.getAuthTag();

  return {
    version: 1,
    salt: salt.toString("hex"),
    iv: iv.toString("hex"),
    authTag: authTag.toString("hex"),
    ciphertext: encrypted.toString("hex"),
  };
}

/**
 * Decrypt an AES-256-GCM encrypted payload using a password.
 * Throws if the password is wrong or the data has been tampered with.
 */
export function decrypt(payload: EncryptedPayload, password: string): string {
  if (payload.version !== 1) {
    throw new Error(`Unsupported encryption version: ${payload.version}`);
  }

  const salt = Buffer.from(payload.salt, "hex");
  const iv = Buffer.from(payload.iv, "hex");
  const authTag = Buffer.from(payload.authTag, "hex");
  const ciphertext = Buffer.from(payload.ciphertext, "hex");
  const key = deriveKey(password, salt);

  const decipher = createDecipheriv(ALGORITHM, key, iv, { authTagLength: AUTH_TAG_LENGTH });
  decipher.setAuthTag(authTag);

  const decrypted = Buffer.concat([
    decipher.update(ciphertext),
    decipher.final(),
  ]);

  return decrypted.toString("utf8");
}
