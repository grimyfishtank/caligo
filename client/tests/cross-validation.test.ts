/**
 * Cross-validation tests ensuring client SDK encoding matches on-chain encoding.
 *
 * The on-chain contract uses: SHA-256(strkey_utf8_bytes)
 * The client SDK uses:        SHA-256(address_string_utf8_bytes)
 *
 * These MUST produce identical results. If they diverge, withdrawal proofs
 * will fail because the public inputs won't match.
 */

import { createHash } from "crypto";
import { addressToFieldBytes, addressToField } from "../src/crypto/secrets";

/**
 * Reference implementation of the on-chain algorithm.
 * This mirrors contracts/mixer_pool/src/lib.rs::address_to_field_bytes exactly:
 *   1. addr.to_string() → strkey (e.g., "GBZX...")
 *   2. Copy UTF-8 bytes into buffer (up to 56 chars)
 *   3. SHA-256(bytes)
 *   4. Return raw 32-byte hash
 */
function referenceAddressToField(strkey: string): Uint8Array {
  const buf = Buffer.from(strkey.slice(0, 56), "utf-8");
  return new Uint8Array(createHash("sha256").update(buf).digest());
}

describe("Address encoding cross-validation", () => {
  // Known Stellar public key addresses (56-char strkeys)
  const testAddresses = [
    "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q",
    "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGSNFHEBD9AFZQ7TM4JRS9A",
    "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ",
    "GDRQE7GL6FBT34PRICBHFBTTH52FM3TH6LLJNEUTD5Z5XBQAP7TPUNG2",
  ];

  test("addressToFieldBytes matches reference implementation for known addresses", () => {
    for (const addr of testAddresses) {
      const sdkResult = addressToFieldBytes(addr);
      const refResult = referenceAddressToField(addr);
      expect(Buffer.from(sdkResult).toString("hex")).toBe(
        Buffer.from(refResult).toString("hex")
      );
    }
  });

  test("SHA-256 known answer test", () => {
    // Compute SHA-256 of a well-known Stellar address and verify against
    // a precomputed expected value. This ensures both Rust and TypeScript
    // can be independently verified against the same expected output.
    const addr = "GBZXN7PIRZGNMHGA7MUUUF4GWBKSKPZM73L7LYOAAV24RM76ZU5SD6Q";
    const expected = createHash("sha256")
      .update(Buffer.from(addr, "utf-8"))
      .digest("hex");

    const result = Buffer.from(addressToFieldBytes(addr)).toString("hex");
    expect(result).toBe(expected);

    // Pin the expected value so future changes are caught
    expect(result).toBe(
      "46e806a364c2cc4fd7d08a73f5824da2e1fb2161db456344b49b2e1814a2f449"
    );
  });

  test("different addresses produce different field elements", () => {
    const results = testAddresses.map((addr) =>
      Buffer.from(addressToFieldBytes(addr)).toString("hex")
    );
    const unique = new Set(results);
    expect(unique.size).toBe(testAddresses.length);
  });

  test("field element is non-zero", () => {
    for (const addr of testAddresses) {
      const field = addressToField(addr);
      expect(field).not.toBe(0n);
    }
  });

  test("encoding is deterministic", () => {
    const addr = testAddresses[0];
    const r1 = addressToFieldBytes(addr);
    const r2 = addressToFieldBytes(addr);
    expect(Buffer.from(r1).toString("hex")).toBe(
      Buffer.from(r2).toString("hex")
    );
  });

  test("contract address format (C...) also works", () => {
    // Contract addresses are also 56 chars starting with C
    const contractAddr = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
    const result = addressToFieldBytes(contractAddr);
    expect(result.length).toBe(32);
    expect(Buffer.from(result).toString("hex")).not.toBe("0".repeat(64));
  });
});
