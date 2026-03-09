# Caligo — Zero-Knowledge Privacy Protocol for Stellar

**Private, unlinkable XLM transfers powered by zk-SNARKs on Soroban.**

Caligo is a zero-knowledge mixer protocol that enables private transfers of XLM on the Stellar network. Using Groth16 zk-SNARK proofs and Poseidon hashing, Caligo breaks the on-chain link between depositors and recipients — making transactions truly private on Stellar.

> **Experimental Software — Security Review Required**
>
> This protocol is under active development and has **not yet undergone a third-party security audit**. It is provided for research, education, and testing purposes only. **Do not use with real funds on mainnet** until a comprehensive security review has been completed by an independent auditor. Use at your own risk.

---

## Why Caligo?

Stellar transactions are fully transparent by default — every transfer, sender, and recipient is visible on the public ledger. Caligo solves this by introducing a **privacy layer** for Stellar using zero-knowledge cryptography:

- **Deposit** a fixed amount of XLM into a shielded pool
- **Withdraw** to any fresh address using a zk-SNARK proof
- **No link** between deposit and withdrawal is visible on-chain
- **Optional relayer** routing hides your network identity

Caligo is inspired by privacy protocols like Tornado Cash, rebuilt from scratch for Stellar's Soroban smart contract platform.

### Key Features

- **Zero-Knowledge Proofs** — Groth16 proofs on BN254 verify withdrawal eligibility without revealing deposit identity
- **Poseidon Hashing** — Circuit-optimized hash function (~240 R1CS constraints vs ~25,000 for SHA-256)
- **Fixed-Denomination Pools** — Uniform deposit sizes maximize the anonymity set
- **Double-Spend Protection** — Nullifier tracking prevents any deposit from being withdrawn twice
- **Encrypted Note Backup** — AES-256-GCM encrypted deposit notes with PBKDF2 key derivation
- **Relayer Network** — Permissionless relayer registration with on-chain fee caps
- **Soroban Native** — Built entirely on Stellar's Soroban smart contract platform

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        Client SDK (TypeScript)                   │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌──────────────────┐  │
│  │ Crypto   │  │ Prover   │  │ Wallet  │  │ Relayer Discovery │  │
│  │ Poseidon │  │ snarkjs  │  │ Notes   │  │ Fee Estimation    │  │
│  │ Merkle   │  │ Groth16  │  │ Backup  │  │ Relay Submission  │  │
│  └────┬─────┘  └────┬─────┘  └────┬────┘  └────────┬─────────┘  │
└───────┼──────────────┼────────────┼────────────────┼─────────────┘
        │              │            │                │
        ▼              ▼            │                ▼
┌──────────────┐ ┌──────────────┐   │    ┌──────────────────────┐
│  MixerPool   │ │   Indexer    │   │    │  Relayer Registry    │
│  (Soroban)   │ │   (Rust)     │   │    │  (Soroban)           │
│              │ │              │   │    │                      │
│ • deposit()  │ │ • Event poll │   │    │ • register()         │
│ • withdraw() │ │ • Merkle     │   │    │ • get_active()       │
│ • verify()   │ │   mirror     │   │    │ • fee cap            │
│ • nullifiers │ │ • REST API   │   │    │   enforcement        │
└──────────────┘ └──────────────┘   │    └──────────────────────┘
        │                           │
        └───────────────────────────┘
              Stellar Network
```

### Components

| Component | Language | Description |
|-----------|----------|-------------|
| **MixerPool Contract** | Rust (Soroban) | Core privacy pool — accepts deposits, verifies Groth16 proofs, pays withdrawals |
| **RelayerRegistry Contract** | Rust (Soroban) | Permissionless relayer registration with fee cap enforcement |
| **ZK Circuits** | Circom 2 | Groth16 withdrawal proof circuit with Poseidon hashing and Merkle inclusion |
| **Client SDK** | TypeScript | Secret generation, proof creation, note management, relayer discovery |
| **Indexer** | Rust (axum) | Off-chain Soroban event listener, Merkle tree mirror, REST API |

---

## How It Works

### Deposit Flow

1. Client generates a random `secret` and `nullifier` (32 bytes each)
2. Computes `commitment = Poseidon(secret, nullifier)`
3. Sends exactly the pool denomination (e.g., 100 XLM) + commitment to the MixerPool contract
4. Contract appends commitment to the on-chain Merkle tree and updates root history
5. Client saves an encrypted deposit note locally

### Withdrawal Flow

1. Client fetches the Merkle path for their commitment from the indexer
2. Generates a Groth16 proof proving:
   - Knowledge of `secret` and `nullifier` such that `Poseidon(secret, nullifier)` is in the Merkle tree
   - `Poseidon(nullifier) == nullifierHash` (for double-spend tracking)
   - The proof is bound to a specific `recipient`, `relayer`, and `fee`
3. Submits the proof to the MixerPool contract (directly or via relayer)
4. Contract verifies the proof, checks the nullifier hasn't been spent, and transfers funds

**The proof reveals nothing about which deposit is being withdrawn.**

---

## Project Structure

```
caligo/
├── contracts/
│   ├── mixer_pool/           # Core mixer pool contract (Soroban)
│   │   ├── src/lib.rs        # deposit(), withdraw(), verify()
│   │   └── src/tests.rs      # Contract unit tests
│   └── relayer_registry/     # Relayer management contract
│       ├── src/lib.rs         # register(), deactivate(), queries
│       └── src/tests.rs       # Registry unit tests
├── circuits/
│   ├── withdraw.circom       # Main withdrawal proof circuit
│   ├── merkle.circom         # Merkle inclusion proof component
│   ├── poseidon.circom       # Poseidon hash component
│   └── build/                # Compiled circuit artifacts
├── client/
│   ├── src/
│   │   ├── crypto/           # Poseidon, secrets, encryption, Merkle tree
│   │   ├── proof/            # snarkjs Groth16 prover/verifier
│   │   ├── wallet/           # Note store with encrypted backup
│   │   ├── sdk/              # MixerSDK high-level interface
│   │   └── relayer/          # Relayer discovery and fee estimation
│   └── tests/                # Unit, integration, cross-validation, E2E tests
├── indexer/
│   └── src/                  # Event listener, Merkle mirror, REST API
├── scripts/
│   └── deploy.sh             # Testnet deployment automation
├── plan.md                   # Full architecture specification
├── .env.example              # Configuration template
└── Cargo.toml                # Rust workspace root
```

---

## Getting Started

### Prerequisites

- **Rust** (latest stable) with `wasm32-unknown-unknown` target
- **Soroban CLI** (`stellar-cli` or `soroban-cli`)
- **Node.js** (v18+) and npm
- **Circom 2** and **snarkjs** (for circuit compilation)

### Installation

```bash
# Clone the repository
git clone https://github.com/GrimyFishTank/caligo.git
cd caligo

# Install Rust + Soroban toolchain
rustup target add wasm32-unknown-unknown
cargo install --locked soroban-cli

# Install circuit tools
npm install -g circom snarkjs

# Install client SDK dependencies
cd client && npm install && cd ..
```

### Build Contracts

```bash
# Build all Soroban contracts
stellar contract build

# Or build individually
stellar contract build --manifest-path contracts/mixer_pool/Cargo.toml
stellar contract build --manifest-path contracts/relayer_registry/Cargo.toml
```

### Build ZK Circuits

```bash
# Compile circuit and run trusted setup
npm run build:circuit
npm run setup

# This produces:
#   circuits/build/withdraw_js/withdraw.wasm  (proving WASM)
#   circuits/build/withdraw_0001.zkey          (proving key)
#   circuits/build/verification_key.json       (verification key)
```

### Run Tests

```bash
# Contract tests (Rust)
cargo test

# Client SDK tests (TypeScript)
cd client && npm test

# E2E tests (requires circuit artifacts)
cd client && npx jest tests/e2e.test.ts
```

---

## Testnet Deployment

1. Copy `.env.example` to `.env` and set your `DEPLOYER_SECRET_KEY`
2. Fund your account via [Stellar Friendbot](https://friendbot.stellar.org)
3. Run the deployment script:

```bash
bash scripts/deploy.sh
```

This will:
- Build both contracts to WASM
- Deploy MixerPool and RelayerRegistry to Soroban testnet
- Initialize contracts with default parameters
- Output the contract IDs for your `.env`

### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `POOL_DENOMINATION` | `10000000000` | Pool size in stroops (100 XLM) |
| `POOL_MAX_FEE` | `1000000000` | Max relayer fee (10 XLM) |
| `POOL_TREE_DEPTH` | `20` | Merkle tree depth (2^20 = ~1M deposits) |
| `POOL_ROOT_HISTORY_SIZE` | `500` | Number of historical roots kept valid |
| `RELAYER_MAX_FEE_BPS` | `100` | Max relayer fee in basis points (1%) |

---

## Cryptographic Design

### Hash Functions

| Context | Hash Function | Rationale |
|---------|--------------|-----------|
| Inside ZK circuits | Poseidon | ~240 R1CS constraints per hash |
| Client-side Merkle tree | Poseidon (circomlibjs) | Must match circuit exactly |
| On-chain (non-circuit) | SHA-256 (Soroban host fn) | Native, efficient |
| Address encoding | SHA-256 → mod p | Deterministic field element from Stellar address |

### Proving System

**Groth16** on the **BN254** curve was chosen for V1 because:
- Smallest proof size (~256 bytes)
- Lowest verifier cost (critical for Soroban's instruction budget)
- Benchmarked at ~23ms native, ~70-117ms estimated WASM — well within Soroban limits

The tradeoff is a **circuit-specific trusted setup ceremony** — required before mainnet deployment.

### Cross-Component Consistency

All three layers (contract, circuit, client SDK) must produce identical outputs for:
- **Poseidon hashing**: `light-poseidon` (Rust) ↔ `circomlibjs` (TypeScript) ↔ `circomlib` (Circom)
- **Address-to-field conversion**: `SHA-256(strkey_utf8) mod BN254_FIELD_ORDER`
- **Merkle tree computation**: Identical zero-value initialization and Poseidon node hashing

Cross-validation tests verify Rust and TypeScript implementations produce matching outputs for pinned test vectors.

---

## Security Model

### Guarantees

- **Unlinkability**: Deposits and withdrawals cannot be correlated by on-chain observers
- **Double-spend prevention**: Nullifier hashes are stored permanently; reuse is rejected
- **Proof binding**: Recipient, relayer, and fee are public inputs — proof is invalid if any are changed
- **Root validation**: Only roots in the contract's history window are accepted
- **Fee caps**: On-chain enforcement prevents relayer fee inflation

### Assumptions

- The Groth16 trusted setup ceremony is performed honestly (at least 1 honest participant)
- Users withdraw to fresh addresses with no prior transaction history
- Users securely back up their encrypted deposit notes
- The BN254 curve and Poseidon hash function remain cryptographically secure

### Known Limitations

- **Anonymity set** — Privacy strength depends on pool activity. Low-volume pools offer weaker privacy.
- **Note loss** — Lost deposit notes mean permanently lost funds (no on-chain recovery in V1)
- **Recipient visibility** — Withdrawal destination is a public input (use fresh addresses)
- **Root expiry** — Users must withdraw within the root history window (default: 500 deposits)
- **Proof generation time** — Client-side proof generation takes 5-15 seconds on mobile devices

---

## API Reference

### Indexer REST API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/merkle-path?commitment=0x...` | GET | Returns Merkle proof for a commitment |
| `/pool-state` | GET | Returns pool info (deposit count, root, denomination) |
| `/roots` | GET | Returns the current root history |
| `/health` | GET | Health check |

### Client SDK

```typescript
import { MixerSDK } from 'caligo-client';

const sdk = new MixerSDK(config);

// Deposit
const { note, commitment } = await sdk.prepareDeposit();
await sdk.finalizeDeposit(commitment, depositorKeypair);

// Withdraw (direct)
const withdrawal = await sdk.prepareWithdrawal(note, recipientAddress);
await sdk.finalizeWithdrawal(withdrawal, recipientKeypair);

// Withdraw (via relayer)
const relayer = await selectCheapestRelayer(registryRelayers);
await submitRelayRequest(relayer, withdrawalPayload);
```

### Contract Interface

```rust
// MixerPool
fn deposit(env, depositor: Address, commitment: BytesN<32>)
fn withdraw(env, proof: BytesN<256>, root: BytesN<32>,
            nullifier_hash: BytesN<32>, recipient: Address,
            relayer: Address, fee: i128)
fn get_root(env) -> BytesN<32>
fn is_nullifier_spent(env, nullifier_hash: BytesN<32>) -> bool

// RelayerRegistry
fn register(env, relayer: Address, endpoint: String, fee_bps: u32)
fn get_active_relayers(env) -> Vec<RelayerInfo>
fn deactivate(env, caller: Address, relayer: Address)
```

---

## Testing

Caligo includes 112+ tests across all components:

| Suite | Count | Coverage |
|-------|-------|----------|
| Contract unit tests (MixerPool) | 15+ | Deposits, withdrawals, nullifiers, root history, fee caps |
| Contract unit tests (RelayerRegistry) | 15 | Registration, deactivation, fee limits, queries |
| Client crypto tests | 34 | Poseidon, encryption, Merkle tree, address encoding |
| Client wallet tests | 6 | Note store, encrypted backup/restore |
| Client relayer tests | 8 | Discovery, fee estimation, selection |
| Cross-validation tests | 6 | Rust ↔ TypeScript hash consistency |
| E2E integration tests | 6 | Full deposit → proof → verify cycle |
| Indexer benchmarks | 1 | BN254 pairing cost measurement |

```bash
# Run all tests
cargo test                          # Rust contracts + indexer
cd client && npm test               # TypeScript SDK
```

---

## Roadmap & Next Steps

### Short-Term (V1 Completion)

- [ ] **Trusted setup ceremony** — Multi-party computation with 10+ independent contributors
- [ ] **Third-party security audit** — Contracts, circuits, and client SDK
- [ ] **Testnet public beta** — Deploy to Stellar testnet with monitoring
- [ ] **Mainnet deployment** — After audit completion and ceremony

### Optimizations

- [ ] **Web Worker proof generation** — Move snarkjs to a Web Worker to unblock the UI
- [ ] **PostgreSQL indexer storage** — Replace in-memory state with persistent database
- [ ] **Batch withdrawal processing** — Aggregate multiple withdrawals to reduce per-tx cost
- [ ] **Circuit optimization** — Reduce R1CS constraint count for faster proof generation
- [ ] **WASM verifier optimization** — Profile and optimize the on-chain Groth16 verifier

### Future Features (V2+)

- [ ] **Multi-asset pools** — Support USDC, wBTC, and other Stellar tokens via SAC
- [ ] **On-chain encrypted note storage** — Recover deposit notes from chain history
- [ ] **PLONK upgrade** — Universal trusted setup, easier circuit iteration
- [ ] **Confidential amounts** — Variable deposit sizes with range proofs
- [ ] **Stealth address generation** — Automatically derive fresh recipient addresses
- [ ] **Rollup layer** — Batch proofs to reduce per-transaction Soroban fees
- [ ] **Shielded wallet** — Full private balance management beyond mixer pools
- [ ] **Cross-chain bridges** — Privacy-preserving transfers between Stellar and other chains
- [ ] **Mobile SDK** — Native iOS/Android proof generation

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Smart Contracts | Rust, Soroban SDK v22 |
| ZK Circuits | Circom 2, snarkjs |
| Proving Scheme | Groth16 (BN254) |
| Hash (in-circuit) | Poseidon |
| Hash (on-chain) | SHA-256 (Soroban host fn) |
| Client SDK | TypeScript, circomlibjs |
| Indexer | Rust, axum, tokio |
| Encryption | AES-256-GCM, PBKDF2 (600K iterations) |
| Stellar SDK | `@stellar/stellar-sdk` |

---

## Contributing

Contributions are welcome. Please open an issue to discuss proposed changes before submitting a pull request.

### Development Setup

```bash
# Run contract tests with output
cargo test -- --nocapture

# Run client tests in watch mode
cd client && npx jest --watch

# Benchmark Groth16 verifier cost
cargo test --manifest-path indexer/Cargo.toml --test bench_pairing -- --nocapture
```

---

## License

[MIT](LICENSE) — Copyright (c) 2026 GrimyFishTank

---

## Acknowledgments

- [Tornado Cash](https://tornado.cash) — Original mixer protocol design inspiration
- [circomlib](https://github.com/iden3/circomlib) — Poseidon hash circuit implementation
- [snarkjs](https://github.com/iden3/snarkjs) — Groth16 proving system
- [Soroban](https://soroban.stellar.org) — Stellar smart contract platform
- [arkworks](https://github.com/arkworks-rs) — Rust elliptic curve and pairing library

---

<sub>Caligo — Privacy for Stellar. Zero-knowledge proofs. Unlinkable transactions. Private XLM transfers on Soroban.</sub>
