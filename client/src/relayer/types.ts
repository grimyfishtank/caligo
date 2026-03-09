/**
 * Shared types for relayer communication.
 */

/** Relayer info as returned by the RelayerRegistry contract. */
export interface RelayerInfo {
  address: string;
  endpoint: string;
  feeBps: number;
  active: boolean;
}

/** Withdrawal relay request sent from client to relayer. */
export interface RelayRequest {
  /** Groth16 proof encoded as hex (256 bytes = 512 hex chars). */
  proofHex: string;
  /** Merkle root (32 bytes hex). */
  rootHex: string;
  /** Nullifier hash (32 bytes hex). */
  nullifierHashHex: string;
  /** Recipient Stellar address. */
  recipient: string;
  /** Fee in stroops. */
  fee: string;
  /** Mixer pool contract ID. */
  poolContractId: string;
}

/** Relay response from the relayer. */
export interface RelayResponse {
  /** Whether the relay was successful. */
  success: boolean;
  /** Transaction hash if successful. */
  txHash?: string;
  /** Error message if failed. */
  error?: string;
}
