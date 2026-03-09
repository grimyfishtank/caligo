/**
 * Tests for the relayer discovery and fee estimation module.
 */

import {
  selectCheapestRelayer,
  estimateRelayerFee,
} from "../src/relayer/discovery";
import type { RelayerInfo } from "../src/relayer/types";

describe("selectCheapestRelayer", () => {
  const relayers: RelayerInfo[] = [
    { address: "GAAA", endpoint: "https://r1.com", feeBps: 50, active: true },
    { address: "GBBB", endpoint: "https://r2.com", feeBps: 20, active: true },
    { address: "GCCC", endpoint: "https://r3.com", feeBps: 80, active: true },
  ];

  test("selects the cheapest active relayer", () => {
    const cheapest = selectCheapestRelayer(relayers);
    expect(cheapest).not.toBeNull();
    expect(cheapest!.address).toBe("GBBB");
    expect(cheapest!.feeBps).toBe(20);
  });

  test("skips inactive relayers", () => {
    const mixed: RelayerInfo[] = [
      { address: "GAAA", endpoint: "https://r1.com", feeBps: 10, active: false },
      { address: "GBBB", endpoint: "https://r2.com", feeBps: 50, active: true },
    ];
    const cheapest = selectCheapestRelayer(mixed);
    expect(cheapest!.address).toBe("GBBB");
  });

  test("returns null for empty list", () => {
    expect(selectCheapestRelayer([])).toBeNull();
  });

  test("returns null when all inactive", () => {
    const inactive: RelayerInfo[] = [
      { address: "GAAA", endpoint: "https://r1.com", feeBps: 10, active: false },
    ];
    expect(selectCheapestRelayer(inactive)).toBeNull();
  });
});

describe("estimateRelayerFee", () => {
  test("1% of 100 XLM (in stroops)", () => {
    // 100 XLM = 1_000_000_000 stroops, 1% = 100 bps
    const fee = estimateRelayerFee(1_000_000_000n, 100);
    expect(fee).toBe(10_000_000n); // 1 XLM
  });

  test("0.5% of 10 XLM", () => {
    const fee = estimateRelayerFee(100_000_000n, 50);
    expect(fee).toBe(500_000n); // 0.05 XLM
  });

  test("0% fee", () => {
    const fee = estimateRelayerFee(1_000_000_000n, 0);
    expect(fee).toBe(0n);
  });

  test("max 100% fee", () => {
    const fee = estimateRelayerFee(1_000_000_000n, 10000);
    expect(fee).toBe(1_000_000_000n);
  });
});
