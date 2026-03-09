/**
 * Relayer discovery and fee estimation.
 *
 * Fetches active relayers from the RelayerRegistry contract
 * and provides fee estimation for withdrawal transactions.
 */

import type { RelayerInfo, RelayRequest, RelayResponse } from "./types.js";

/** Configuration for relayer discovery. */
export interface RelayerDiscoveryConfig {
  /** Soroban RPC URL. */
  rpcUrl: string;
  /** RelayerRegistry contract ID. */
  registryContractId: string;
}

/**
 * Discover active relayers from the registry contract.
 *
 * In production, this calls the RelayerRegistry.get_active_relayers()
 * contract method via Soroban RPC. For now, it accepts a direct list
 * for testing and development.
 */
export async function fetchActiveRelayers(
  config: RelayerDiscoveryConfig
): Promise<RelayerInfo[]> {
  // In production, this would invoke the Soroban contract:
  //   const result = await server.call(registryContractId, "get_active_relayers", []);
  //   return parseRelayerInfoList(result);
  //
  // For now, return empty — caller should use setRelayerList() for testing.
  //
  // TODO: Implement Soroban RPC contract invocation when deploying.
  const _ = config;
  return [];
}

/**
 * Select the cheapest active relayer from a list.
 */
export function selectCheapestRelayer(relayers: RelayerInfo[]): RelayerInfo | null {
  const active = relayers.filter((r) => r.active);
  if (active.length === 0) return null;
  return active.reduce((cheapest, r) =>
    r.feeBps < cheapest.feeBps ? r : cheapest
  );
}

/**
 * Estimate the relayer fee for a withdrawal.
 *
 * @param denomination - Pool denomination in stroops
 * @param feeBps - Relayer fee in basis points (e.g., 100 = 1%)
 * @returns Fee amount in stroops
 */
export function estimateRelayerFee(denomination: bigint, feeBps: number): bigint {
  return (denomination * BigInt(feeBps)) / 10000n;
}

/**
 * Submit a withdrawal relay request to a relayer.
 *
 * @param relayerEndpoint - The relayer's API endpoint URL
 * @param request - The relay request payload
 * @returns The relay response
 */
export async function submitRelayRequest(
  relayerEndpoint: string,
  request: RelayRequest
): Promise<RelayResponse> {
  const response = await fetch(`${relayerEndpoint}/relay`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });

  if (!response.ok) {
    return {
      success: false,
      error: `Relayer returned HTTP ${response.status}`,
    };
  }

  return response.json() as Promise<RelayResponse>;
}
