# Private Stellar Protocol
## ZK Mixer Architecture Plan — Claude Code Reference

Version: 0.2  
Status: Architecture Specification (Revised)  
Goal: Implement a privacy layer for Stellar using zk-SNARK powered mixer pools.

---

## License

```
MIT License

Copyright (c) 2025 Private Stellar Protocol Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## Quick Reference for Claude Code

When working in this repo, always keep these rules in mind:

- Soroban contracts are written in **Rust** — never Solidity or other EVM languages.
- ZK circuits are written in **Circom 2** and compiled with **snarkjs**.
- All hashing **inside circuits** must use **Poseidon** — never SHA-256 or Keccak (constraint cost is prohibitive).
- All hashing **outside circuits** (e.g., nullifier tracking in contract state) may use SHA-256 via Soroban's native crypto host functions.
- The proving scheme is **Groth16**. See Section 4 for the rationale and trusted setup implications.
- Soroban's instruction limit is the single biggest technical risk. Never add on-chain logic without estimating compute cost first.
- Every new contract entrypoint needs a corresponding integration test in `/tests`.

---

## 1. Protocol Overview

Private Stellar is a zero-knowledge mixer protocol enabling private transfers of XLM and Stellar assets.

The protocol breaks the link between sender and receiver by allowing users to:

1. Deposit a fixed-denomination amount into a mixer pool
2. Receive a cryptographic commitment (stored locally as a deposit note)
3. Later withdraw funds to a fresh address using a zero-knowledge proof
4. Optionally route the withdrawal through a relayer to hide their network identity

Observers cannot determine who deposited, who withdrew, or which deposit corresponds to which withdrawal. The system is inspired by Tornado Cash but adapted for Stellar and Soroban smart contracts.

---

## 2. Tech Stack

| Layer | Technology |
|---|---|
| Smart Contracts | Rust (Soroban SDK) |
| ZK Circuits | Circom 2 |
| Proof Generation | snarkjs (WASM, browser/Node) |
| Proving Scheme | Groth16 |
| Hash Function (in-circuit) | Poseidon |
| Hash Function (on-chain) | SHA-256 (Soroban host fn) |
| Client SDK | TypeScript |
| Stellar SDK | `@stellar/stellar-sdk` |
| Indexer | Rust or Node.js + PostgreSQL |
| API Layer | GraphQL or REST |

---

## 3. System Components

### 3.1 MixerPool Contract (Rust/Soroban)

One contract is deployed per denomination pool (e.g., 10 XLM, 100 XLM, 1000 XLM). Fixed denominations are required — variable amounts would shrink the anonymity set.

Responsibilities:
- Accept deposits and verify the fixed amount
- Append commitments to the on-chain Merkle tree
- Store a rolling root history window
- Verify Groth16 withdrawal proofs
- Track spent nullifiers to prevent double-spend
- Pay recipient (and optionally relayer fee) on valid withdrawal

### 3.2 Merkle Tree (Embedded in MixerPool)

The Merkle tree is stored directly in pool contract state. A separate contract is only warranted if multiple pools need to share a tree, which is not a requirement for V1.

### 3.3 Relayer Registry Contract (Rust/Soroban)

Maintains a permissioned or open registry of relayer addresses. Used by the client to discover active relayers and by the pool contract to validate fee recipients.

Relayer responsibilities:
- Broadcast withdrawal transactions on behalf of the user
- Receive a fee (capped by the contract — see Section 7)
- Never learn the user's deposit secrets (they only receive the proof payload)

---

## 4. Cryptographic Architecture

### 4.1 Hash Function Decision — CRITICAL

**All hashing inside zk circuits MUST use Poseidon.**

SHA-256 and Keccak cost ~25,000+ R1CS constraints per call. Poseidon costs ~240 constraints per call for a 2-input hash. Using the wrong hash function makes proof generation impractical.

- Commitment hash: `Poseidon(secret, nullifier)` — computed in-circuit and off-chain
- Merkle tree nodes: `Poseidon(left, right)` — computed in-circuit and in the client SDK
- Nullifier hash: `Poseidon(nullifier)` — public output of the circuit

On-chain (in Soroban contract code outside the circuit verifier), use Soroban's native SHA-256 host function for any non-circuit hashing needs. Do not implement Poseidon in Rust on-chain — it is only needed in circuits and the client SDK.

### 4.2 Commitments

Each deposit generates a commitment:

```
secret    = random 32 bytes (kept private forever)
nullifier = random 32 bytes (revealed at withdrawal as a hash)
commitment = Poseidon(secret, nullifier)
```

The commitment is stored on-chain in the Merkle tree. The secret and nullifier are stored locally by the user as a deposit note.

**If the deposit note is lost, the funds are permanently unrecoverable.** The client must prompt users to back up their note (see Section 9 — Note Encryption).

### 4.3 Merkle Tree

All commitments are stored in an incremental Merkle tree.

```
depth    = 20
capacity = 1,048,576 deposits
leaf     = Poseidon(commitment)
```

The root is updated after each deposit. All historical roots within the root history window remain valid for withdrawals.

**Root history window:** Store the last **N = 500** roots. This means a user has up to 500 deposits (across all users in the pool) to complete their withdrawal before their root expires. 100 is too small for pools with moderate activity — a busy pool could rotate 100 roots in hours. 500 is a safer default. This value should be a configurable contract parameter set at deployment.

### 4.4 Nullifiers

To prevent double-spending, the contract tracks all spent nullifier hashes:

```
nullifier_hash = Poseidon(nullifier)
```

At withdrawal, the circuit proves knowledge of a `nullifier` such that `Poseidon(nullifier)` equals the submitted `nullifier_hash`, and that `Poseidon(secret, nullifier)` is a leaf in the Merkle tree. If `nullifier_hash` is already in the contract's spent set, the transaction is rejected.

### 4.5 Proving Scheme — Groth16 vs PLONK

**Decision: Use Groth16 for V1.**

| | Groth16 | PLONK |
|---|---|---|
| Proof size | ~200 bytes (smallest) | ~400–600 bytes |
| Verifier cost | Lowest (critical for Soroban limits) | Moderate |
| Trusted setup | Circuit-specific ceremony | Universal — one ceremony, any circuit |
| Setup complexity | High (per circuit) | Lower |

Groth16 is chosen because its verifier has the lowest compute cost, which directly addresses the Soroban instruction limit risk. The tradeoff is that **each circuit requires its own trusted setup ceremony**. For V1 there are only two circuits (commitment and withdrawal), so this is manageable.

**PLONK should be reconsidered for V2** if new circuit variants are needed frequently, or if Soroban's instruction limits improve enough to absorb the larger verifier cost.

### 4.6 Soroban Instruction Limit — Top Technical Risk

Soroban enforces a per-transaction CPU instruction budget. Verifying a Groth16 proof involves pairing operations on BN254 (or BLS12-381) elliptic curves, which are computationally expensive.

**This must be benchmarked before writing any other contract logic.** If Groth16 verification exceeds the instruction budget, the entire withdrawal mechanism fails.

Mitigation options (in order of preference):
1. Use a precompile or host function if Soroban exposes one for pairing operations (check Soroban roadmap)
2. Implement an optimized Rust verifier and measure instructions with `soroban contract invoke --cost`
3. If limits are hit, evaluate PLONK with a recursive proof, or a split verification approach

**Action item:** Spike on Groth16 verifier cost before Phase 2 begins.

---

## 5. ZK Circuit Design

### 5.1 Withdrawal Circuit (`circuits/withdraw.circom`)

This circuit proves a user is entitled to withdraw from the pool without revealing which deposit is theirs.

**Public inputs:**
```
root           // Merkle root from contract state
nullifier_hash // Poseidon(nullifier) — prevents double spend
recipient      // withdrawal destination address
relayer        // relayer address (0 if direct)
fee            // relayer fee amount
```

**Private inputs:**
```
secret         // random secret from deposit note
nullifier      // random nullifier from deposit note
path_elements  // Merkle sibling nodes along the proof path
path_indices   // left/right direction bits for each level
```

**Circuit assertions:**
```
Poseidon(secret, nullifier) == commitment
MerkleProof(commitment, path_elements, path_indices) == root
Poseidon(nullifier) == nullifier_hash
```

The `recipient`, `relayer`, and `fee` are included as public inputs to bind the proof to a specific withdrawal. This prevents a malicious relayer from substituting their own address — the proof would be invalid for any recipient other than the one specified.

**Privacy note on recipient:** Because `recipient` is a public input, the relayer and any chain observer will know the destination address. This is acceptable if the recipient is a fresh address with no prior history. Users must be warned to always withdraw to a freshly generated address.

### 5.2 Merkle Circuit (`circuits/merkle.circom`)

A reusable component for Merkle inclusion proofs. Used as a subcircuit within `withdraw.circom`. Implements Poseidon-based node hashing at each level.

### 5.3 Compile & Setup Commands

```bash
# Compile circuit
circom circuits/withdraw.circom --r1cs --wasm --sym -o build/

# Phase 1 (powers of tau — done once, reusable across Groth16 circuits up to the constraint count)
snarkjs powersoftau new bn128 16 build/pot16_0000.ptau
snarkjs powersoftau contribute build/pot16_0000.ptau build/pot16_0001.ptau --name="Contributor 1"
snarkjs powersoftau prepare phase2 build/pot16_0001.ptau build/pot16_final.ptau

# Phase 2 (circuit-specific — must redo for each circuit change)
snarkjs groth16 setup build/withdraw.r1cs build/pot16_final.ptau build/withdraw_0000.zkey
snarkjs zkey contribute build/withdraw_0000.zkey build/withdraw_0001.zkey --name="Contributor 1"
snarkjs zkey export verificationkey build/withdraw_0001.zkey build/verification_key.json

# Export Solidity verifier as a starting point for the Rust port
snarkjs zkey export solidityverifier build/withdraw_0001.zkey build/verifier.sol
# Note: adapt the Solidity verifier to Rust/Soroban manually or use a zk-Soroban library
```

---

## 6. Transaction Flows

### 6.1 Deposit Flow

**Step 1 — Client generates secrets locally:**
```typescript
const secret     = crypto.getRandomValues(new Uint8Array(32));
const nullifier  = crypto.getRandomValues(new Uint8Array(32));
const commitment = poseidon([secret, nullifier]);
```

**Step 2 — Client prompts user to save deposit note** (before broadcasting):
```json
{
  "secret": "0x...",
  "nullifier": "0x...",
  "commitment": "0x...",
  "pool_id": "pool_100_xlm",
  "deposit_tx": "..."
}
```
The client encrypts this note with a user-provided password before local storage. See Section 9.

**Step 3 — Client sends deposit transaction:**
```
deposit(commitment)
```
Transaction must include exactly the pool's fixed XLM denomination. No other amount is accepted.

**Step 4 — Contract appends commitment to Merkle tree and updates root history.**

### 6.2 Withdrawal Flow

**Step 1 — Client fetches Merkle path from indexer:**
```
GET /merkle-path?commitment=0x...
→ { path_elements: [...], path_indices: [...], root: "0x..." }
```

**Step 2 — Client generates proof (in-browser WASM or Node):**
```typescript
const { proof, publicSignals } = await snarkjs.groth16.fullProve(
  { secret, nullifier, path_elements, path_indices },
  "build/withdraw_js/withdraw.wasm",
  "build/withdraw_0001.zkey"
);
```
Proof generation takes 1–10 seconds depending on device. Display a loading indicator and run in a Web Worker to avoid blocking the UI thread.

**Step 3 — Client submits withdrawal (direct or via relayer):**
```
withdraw(proof, root, nullifier_hash, recipient, relayer, fee)
```

**Step 4 — Contract verifies:**
- Root exists in root history
- `nullifier_hash` not in spent set
- Groth16 proof is valid for the public inputs

**Step 5 — On success:**
- Mark `nullifier_hash` as spent
- Transfer `pool_amount - fee` to `recipient`
- Transfer `fee` to `relayer` (if non-zero)
- Activate recipient account if it does not exist (fund minimum XLM balance — see Section 8)

---

## 7. Smart Contract API

### `deposit(commitment: Bytes32)`

**Checks:**
- Attached XLM equals pool denomination exactly
- `commitment` not already in the tree

**Updates:**
- Appends leaf `Poseidon(commitment)` to Merkle tree
- Recomputes parent hashes up to root
- Pushes new root to root history (evicts oldest if at capacity)

**Errors:**
- `ErrWrongAmount` — attached XLM does not match denomination
- `ErrDuplicateCommitment` — commitment already exists

### `withdraw(proof, root, nullifier_hash, recipient, relayer, fee)`

**Checks:**
- `root` exists in root history
- `nullifier_hash` not in spent set
- `fee <= MAX_FEE` (contract constant, e.g. 1% of denomination)
- Groth16 proof is valid for `(root, nullifier_hash, recipient, relayer, fee)`

**Updates:**
- Adds `nullifier_hash` to spent set
- Activates recipient account if needed (native XLM minimum balance)
- Transfers `pool_amount - fee` to `recipient`
- Transfers `fee` to `relayer` if `fee > 0`

**Errors:**
- `ErrInvalidRoot` — root not found in history
- `ErrNullifierSpent` — double-spend attempt
- `ErrInvalidProof` — proof verification failed
- `ErrFeeTooHigh` — fee exceeds maximum

### `get_root() → Bytes32`

Returns the current (most recent) Merkle root.

### `get_root_history() → Vec<Bytes32>`

Returns the full root history window.

### `is_nullifier_spent(nullifier_hash: Bytes32) → bool`

Allows clients to pre-check before broadcasting a withdrawal.

---

## 8. Stellar-Specific Considerations

### Account Activation

Stellar requires a minimum XLM balance for an account to exist on-chain (currently ~1 XLM base reserve). When a user withdraws to a **fresh address**, the contract must fund the account creation as part of the withdrawal transaction.

The pool denomination must account for this:
- The contract sends the activation amount as part of the transfer
- The effective received amount is `pool_amount - fee - activation_reserve`
- The UI must display this clearly before the user deposits

### Transaction Fees

Soroban transaction fees are paid by the submitter (or relayer). The relayer fee in the withdrawal call covers this. Ensure the minimum relayer fee is set to cover worst-case Soroban fees at peak network load.

### Minimum Pool Denominations

Given activation costs and Soroban fees, the minimum practical denomination is approximately **10 XLM**. Smaller pools are technically possible but result in a poor UX where fees consume a significant portion of the withdrawal.

---

## 9. Note Encryption & Recovery

**If the deposit note is lost, funds are permanently unrecoverable.** The protocol has no mechanism to prove ownership without `secret` and `nullifier`.

### Client-Side Encryption

Before storing locally, the client encrypts the deposit note:

```typescript
const encryptedNote = await encrypt(depositNote, userPassword);
localStorage.setItem(`note_${commitment}`, encryptedNote);
```

Use AES-256-GCM with a key derived from the user's password via PBKDF2 or Argon2id.

### Recovery Options (V1 requires Option 1 at minimum)

1. **Encrypted file export** — Download an encrypted JSON file. User stores it offline.
2. **Mnemonic encoding** — Encode `secret + nullifier` as a BIP39-style mnemonic for human-readable backup.
3. **On-chain encrypted note (V2)** — Encrypt the note with the user's public key and store ciphertext in contract events. Recoverable from chain history.

---

## 10. Relayer Network

### Trust Model

Relayers are semi-trusted: they learn the `recipient` address and the proof, but they cannot:
- Redirect funds (recipient is bound in the proof)
- Extract the user's secret or nullifier (proof is zero-knowledge)
- Double-spend (nullifier is tracked on-chain)

Relayers can:
- Censor a specific user's withdrawal (refuse to relay)
- Go offline

**Mitigation:** Users must always have the option of direct withdrawal (no relayer) as a fallback. The UI must expose this option clearly.

### Fee Cap

The contract enforces `fee <= MAX_FEE` (suggested: 1% of pool denomination or a fixed XLM cap, whichever is lower). This prevents a malicious relayer from submitting a valid proof with an inflated fee.

### Relayer Discovery

The RelayerRegistry contract maintains a list of active relayers with their fee rates and endpoints. The client fetches this list and presents it to the user before withdrawal.

---

## 11. Indexer

The indexer listens to Soroban contract events and maintains:
- A full list of commitments (for Merkle path computation)
- Current and historical Merkle roots
- Pool state (total deposits, anonymity set size)

### API Endpoints

```
GET /merkle-path?commitment=<hex>
→ { root, path_elements, path_indices, leaf_index }

GET /pool-state?pool_id=<id>
→ { denomination, deposit_count, latest_root }

GET /roots?pool_id=<id>
→ { roots: [{ root, timestamp, index }] }
```

### Tech

- **Rust** preferred (consistent with contract language, good Stellar SDK support)
- PostgreSQL for persistent state
- REST or GraphQL API

---

## 12. Client SDK

### Responsibilities

- Generate secrets and commitments
- Encrypt and store deposit notes
- Fetch Merkle paths from indexer
- Generate Groth16 proofs (snarkjs WASM)
- Submit transactions via Stellar SDK
- Discover and interact with relayers

### Key Modules

```
/client
  /crypto       — Poseidon, note encryption, secret generation
  /proof        — snarkjs wrapper, proof generation, verification key loading
  /wallet       — deposit note storage, encryption/decryption, export/import
  /sdk          — deposit(), withdraw(), fetchMerklePath(), getPoolState()
  /relayer      — relayer discovery, fee estimation, proof relay
  /ui           — deposit flow, withdrawal flow, note backup prompts
```

### Proof Generation Performance

snarkjs WASM proof generation typically takes 1–10 seconds in a browser. For the withdrawal circuit at depth 20:
- Desktop: ~1–3 seconds
- Mobile: ~5–10 seconds

Always run proof generation in a **Web Worker** to avoid blocking the UI thread.

---

## 13. Directory Structure

```
private-stellar/
├── contracts/
│   ├── mixer_pool/
│   │   ├── src/
│   │   │   ├── lib.rs          # Contract entrypoints
│   │   │   ├── merkle.rs       # Merkle tree logic
│   │   │   ├── verifier.rs     # Groth16 verifier (BN254)
│   │   │   └── storage.rs      # State keys and types
│   │   ├── Cargo.toml
│   │   └── tests/
│   │       └── integration.rs
│   └── relayer_registry/
│       ├── src/lib.rs
│       └── Cargo.toml
│
├── circuits/
│   ├── withdraw.circom         # Main withdrawal circuit
│   ├── merkle.circom           # Merkle inclusion proof component
│   ├── poseidon.circom         # Poseidon hash component
│   └── build/                  # Compiled artifacts (gitignored)
│
├── client/
│   ├── src/
│   │   ├── crypto/
│   │   ├── proof/
│   │   ├── wallet/
│   │   ├── sdk/
│   │   └── relayer/
│   ├── package.json
│   └── tsconfig.json
│
├── indexer/
│   ├── src/
│   │   ├── listener.rs         # Soroban event listener
│   │   ├── merkle_service.rs   # Off-chain Merkle path builder
│   │   └── api.rs              # REST/GraphQL API
│   └── Cargo.toml
│
├── tests/
│   ├── e2e/                    # Full deposit-withdraw flow tests
│   └── circuits/               # Circuit constraint tests (snarkjs)
│
├── scripts/
│   ├── setup_ceremony.sh       # Trusted setup automation
│   ├── deploy.sh               # Contract deployment
│   └── seed_testnet.sh         # Testnet seeding
│
├── docs/
│   ├── protocol_spec.md
│   ├── security_model.md
│   └── trusted_setup.md
│
├── plan.md                     # This file
├── Cargo.toml                  # Workspace root
└── .env.example
```

---

## 14. Development Environment Setup

```bash
# Rust + Soroban
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install --locked soroban-cli

# Circom + snarkjs
npm install -g circom
npm install -g snarkjs

# Node dependencies (client SDK)
cd client && npm install

# Run contract tests
cd contracts/mixer_pool && cargo test

# Run circuit verification test
cd circuits && snarkjs groth16 verify build/verification_key.json build/public.json build/proof.json

# Check Groth16 verifier instruction cost on Soroban (run this early)
soroban contract invoke --cost --id <CONTRACT_ID> -- withdraw ...
```

### Environment Variables (`.env`)

```
STELLAR_NETWORK=testnet
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
MIXER_POOL_CONTRACT_ID=C...
RELAYER_REGISTRY_CONTRACT_ID=C...
INDEXER_DATABASE_URL=postgres://...
INDEXER_API_PORT=3000
```

---

## 15. Testing Strategy

### Unit Tests (Rust)
- Merkle tree append, root computation, root history eviction
- Nullifier spent-set logic
- Fee cap enforcement
- All error conditions for each contract entrypoint

### Circuit Tests (snarkjs/TypeScript)
- Valid proof generation and verification
- Invalid proof rejection (wrong secret, wrong path, spent nullifier)
- Edge cases: depth-0 tree, full tree

### Integration Tests
- Full deposit → withdrawal flow on local Soroban sandbox
- Direct withdrawal (no relayer)
- Relayer-routed withdrawal
- Double-spend attempt (must reject)
- Expired root withdrawal (must reject)
- Fresh account activation on withdrawal

### Fuzzing
- Fuzz Merkle tree inputs
- Fuzz circuit inputs for unexpected constraint violations

---

## 16. Development Roadmap

### Phase 1 — Contracts (Weeks 1–4)
- [ ] MixerPool contract skeleton (Rust/Soroban)
- [ ] Incremental Merkle tree in contract state
- [ ] Root history with configurable window (default 500)
- [ ] Nullifier spent set
- [ ] Deposit entrypoint
- [ ] **SPIKE: Groth16 verifier cost on Soroban** ← complete before Phase 2

### Phase 2 — ZK Circuits (Weeks 3–6)
- [ ] Poseidon component (`circom-poseidon` or implement from spec)
- [ ] Merkle inclusion circuit
- [ ] Withdrawal circuit
- [ ] Trusted setup (Phase 1 powers of tau + Phase 2 circuit-specific)
- [ ] Rust verifier in contract (adapted from snarkjs output)
- [ ] Wire verifier into `withdraw()` entrypoint
- [ ] Circuit test suite

### Phase 3 — Client SDK (Weeks 5–8)
- [ ] Secret/commitment generation
- [ ] Note encryption and local storage
- [ ] Deposit note backup (encrypted file export)
- [ ] snarkjs proof generation wrapper (Web Worker)
- [ ] Deposit and withdraw SDK functions
- [ ] Merkle path fetching from indexer

### Phase 4 — Indexer (Weeks 7–9)
- [ ] Soroban event listener
- [ ] Off-chain Merkle tree mirror
- [ ] Merkle path API endpoint
- [ ] Pool state API endpoint

### Phase 5 — Relayer Network (Weeks 9–11)
- [ ] RelayerRegistry contract
- [ ] Relayer server (receives proof payloads, broadcasts transactions)
- [ ] Relayer discovery in client SDK
- [ ] Fee cap enforcement in contract

### Phase 6 — Audit & Mainnet (Weeks 12+)
- [ ] Full security audit (contracts + circuits)
- [ ] Trusted setup ceremony (public, multi-party, 10+ contributors)
- [ ] Testnet public beta
- [ ] Mainnet deployment

---

## 17. Security Requirements

### Double-Spend Prevention
Nullifier hashes stored in contract state. Any withdrawal attempting to reuse a nullifier is rejected before proof verification.

### Front-Running
Commitments hide deposit details on-chain. An observer cannot determine the depositor's identity or secrets from the commitment alone.

### Proof Binding
The recipient, relayer, and fee are bound as public inputs to the proof. Substituting any of these values invalidates the proof.

### Merkle Root Validation
Withdrawals must reference a root present in the contract's root history. Fabricated roots are rejected.

### Fee Cap
Contract enforces `fee <= MAX_FEE` to prevent malicious relayers from inflating fees.

### Trusted Setup
Groth16 requires a trusted setup ceremony. The ceremony must be:
- Multi-party (at least 10+ independent contributors)
- Publicly verifiable (ceremony transcript published)
- Performed after the final circuit is frozen — any circuit change invalidates the zkey

See `docs/trusted_setup.md` for ceremony procedures.

### Contract Immutability
For V1, **contracts must be immutable after deployment**. An upgradeable contract could drain funds or compromise nullifier tracking. If upgradeability is required in the future, use a time-locked governance mechanism.

---

## 18. Known Limitations

- **Anonymity set:** Privacy is only as strong as the number of equal-denomination deposits in the pool. Small pools offer weak privacy.
- **Proof generation time:** Mobile devices may take 5–15 seconds. Must be communicated clearly in UX.
- **Root history window:** Users must complete withdrawal within 500 deposits. High-volume pools should increase this at deployment.
- **Recipient linkability:** Withdrawal destination is a public input. Users must always withdraw to a fresh address. The UI must enforce this with warnings.
- **Note loss:** No on-chain recovery in V1. Lost notes = lost funds.
- **Regulatory risk:** Privacy protocols face regulatory scrutiny. This project is for research and education purposes. Operators must consult legal counsel before deployment.

---

## 19. Future Upgrades (Post-V1)

- **Multi-asset pools** — Support USDC, wBTC, and other Stellar tokens
- **On-chain encrypted note storage** — Recover deposit notes from chain history
- **PLONK upgrade** — Universal trusted setup, easier circuit iteration
- **Confidential amounts** — Hide pool denomination via range proofs
- **Stealth address generation** — Automatically derive fresh recipient addresses
- **Rollup layer** — Batch withdrawals to reduce per-transaction Soroban fees
- **Shielded wallet** — Full private balance management beyond mixer pools

---

## 20. Success Criteria

The protocol is considered complete when:

- Deposits and withdrawals are unlinkable by any on-chain observer
- Double-spend attempts are deterministically rejected
- Withdrawal proofs are verifiable on-chain within Soroban's instruction budget
- A fresh-address withdrawal leaves no on-chain link to the depositor
- The trusted setup ceremony is publicly verifiable
- The full test suite passes on Soroban testnet
- An independent security audit finds no critical vulnerabilities

---

*End of Plan — Private Stellar Protocol v0.2*
