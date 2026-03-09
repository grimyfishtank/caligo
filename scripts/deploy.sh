#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────
# Caligo — Testnet Deployment Script
#
# Deploys the MixerPool and RelayerRegistry contracts to Soroban testnet.
#
# Prerequisites:
#   1. Install soroban-cli: cargo install --locked soroban-cli
#   2. Fund a testnet account: https://friendbot.stellar.org
#   3. Copy .env.example to .env and set DEPLOYER_SECRET_KEY
#   4. Build contracts: see below
#
# Usage:
#   bash scripts/deploy.sh
# ─────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load environment variables
if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    source "$ROOT_DIR/.env"
    set +a
fi

# Defaults
NETWORK="${STELLAR_NETWORK:-testnet}"
RPC_URL="${STELLAR_RPC_URL:-https://soroban-testnet.stellar.org}"
DEPLOYER="${DEPLOYER_SECRET_KEY:-}"

if [ -z "$DEPLOYER" ]; then
    echo "ERROR: DEPLOYER_SECRET_KEY not set. Copy .env.example to .env and configure it."
    exit 1
fi

echo "=== Caligo Testnet Deployment ==="
echo "Network:  $NETWORK"
echo "RPC URL:  $RPC_URL"
echo ""

# ── Step 1: Build WASM artifacts ──

echo "Step 1: Building contracts..."
cd "$ROOT_DIR"

# Note: Building for wasm32-unknown-unknown requires the soroban toolchain.
# If this fails with duplicate panic_impl, use:
#   stellar contract build
# instead of cargo build directly.
echo "  Building mixer_pool..."
stellar contract build --manifest-path contracts/mixer_pool/Cargo.toml 2>/dev/null || \
    cargo build --manifest-path contracts/mixer_pool/Cargo.toml \
        --target wasm32-unknown-unknown --release --features vk

echo "  Building relayer_registry..."
stellar contract build --manifest-path contracts/relayer_registry/Cargo.toml 2>/dev/null || \
    cargo build --manifest-path contracts/relayer_registry/Cargo.toml \
        --target wasm32-unknown-unknown --release

# ── Step 2: Configure identity ──

echo ""
echo "Step 2: Configuring deployer identity..."
soroban keys add deployer --secret-key "$DEPLOYER" 2>/dev/null || true

# ── Step 3: Deploy MixerPool ──

MIXER_WASM="$ROOT_DIR/target/wasm32-unknown-unknown/release/mixer_pool.wasm"
if [ ! -f "$MIXER_WASM" ]; then
    echo "ERROR: MixerPool WASM not found at $MIXER_WASM"
    echo "  Run: stellar contract build"
    exit 1
fi

echo ""
echo "Step 3: Deploying MixerPool contract..."
MIXER_ID=$(soroban contract deploy \
    --wasm "$MIXER_WASM" \
    --source deployer \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    2>&1)
echo "  MixerPool deployed: $MIXER_ID"

# ── Step 4: Deploy RelayerRegistry ──

REGISTRY_WASM="$ROOT_DIR/target/wasm32-unknown-unknown/release/relayer_registry.wasm"
if [ ! -f "$REGISTRY_WASM" ]; then
    echo "ERROR: RelayerRegistry WASM not found at $REGISTRY_WASM"
    exit 1
fi

echo ""
echo "Step 4: Deploying RelayerRegistry contract..."
REGISTRY_ID=$(soroban contract deploy \
    --wasm "$REGISTRY_WASM" \
    --source deployer \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    2>&1)
echo "  RelayerRegistry deployed: $REGISTRY_ID"

# ── Step 5: Initialize contracts ──

echo ""
echo "Step 5: Initializing MixerPool..."

# Get deployer public key for admin
DEPLOYER_PUB=$(soroban keys address deployer 2>&1)

DENOM="${POOL_DENOMINATION:-10000000000}"
MAX_FEE="${POOL_MAX_FEE:-1000000000}"
DEPTH="${POOL_TREE_DEPTH:-20}"
HISTORY="${POOL_ROOT_HISTORY_SIZE:-500}"
TOKEN="${TOKEN_CONTRACT_ID:-}"

if [ -z "$TOKEN" ]; then
    echo "  WARNING: TOKEN_CONTRACT_ID not set."
    echo "  You must wrap native XLM first with:"
    echo "    stellar contract asset deploy --asset native --network $NETWORK"
    echo "  Then set TOKEN_CONTRACT_ID in .env and re-run initialization manually."
else
    soroban contract invoke \
        --id "$MIXER_ID" \
        --source deployer \
        --network "$NETWORK" \
        --rpc-url "$RPC_URL" \
        -- initialize \
        --admin "$DEPLOYER_PUB" \
        --token "$TOKEN" \
        --denomination "$DENOM" \
        --max_fee "$MAX_FEE" \
        --tree_depth "$DEPTH" \
        --root_history_size "$HISTORY"
    echo "  MixerPool initialized."
fi

echo ""
echo "Step 6: Initializing RelayerRegistry..."
RELAYER_MAX="${RELAYER_MAX_FEE_BPS:-100}"

soroban contract invoke \
    --id "$REGISTRY_ID" \
    --source deployer \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    -- initialize \
    --admin "$DEPLOYER_PUB" \
    --max_fee_bps "$RELAYER_MAX"
echo "  RelayerRegistry initialized."

# ── Step 6: Output ──

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Add these to your .env:"
echo "  MIXER_POOL_CONTRACT_ID=$MIXER_ID"
echo "  RELAYER_REGISTRY_CONTRACT_ID=$REGISTRY_ID"
echo ""
echo "Next steps:"
echo "  1. Wrap native XLM:  stellar contract asset deploy --asset native --network $NETWORK"
echo "  2. Set TOKEN_CONTRACT_ID in .env"
echo "  3. Initialize MixerPool with the token (if not done above)"
echo "  4. Start the indexer:  cd indexer && CONTRACT_ID=$MIXER_ID cargo run"
echo "  5. Run E2E tests"
