#!/usr/bin/env bash
# Private Stellar Protocol — Trusted Setup Script
#
# This script compiles the withdrawal circuit, runs a LOCAL trusted setup
# (powers of tau + circuit-specific phase 2), and exports the verification key.
#
# WARNING: This local setup is for DEVELOPMENT ONLY.
# For production, a multi-party ceremony with 10+ contributors is required.
# See docs/trusted_setup.md for the production ceremony procedure.

set -euo pipefail

BUILD_DIR="circuits/build"
CIRCUIT="circuits/withdraw.circom"
PTAU_POWER=16  # 2^16 = 65536 constraints (sufficient for depth-20 Merkle + Poseidon)

mkdir -p "$BUILD_DIR"

echo "=== Step 1: Compile circuit ==="
circom "$CIRCUIT" --r1cs --wasm --sym -o "$BUILD_DIR/"

echo ""
echo "=== Step 2: Circuit info ==="
npx snarkjs r1cs info "$BUILD_DIR/withdraw.r1cs"

echo ""
echo "=== Step 3: Powers of Tau (Phase 1) ==="
echo "Starting new ceremony with 2^${PTAU_POWER} constraints..."
npx snarkjs powersoftau new bn128 "$PTAU_POWER" "$BUILD_DIR/pot_0000.ptau" -v
npx snarkjs powersoftau contribute "$BUILD_DIR/pot_0000.ptau" "$BUILD_DIR/pot_0001.ptau" \
  --name="Dev Contributor 1" -e="$(head -c 32 /dev/urandom | xxd -p)"
npx snarkjs powersoftau prepare phase2 "$BUILD_DIR/pot_0001.ptau" "$BUILD_DIR/pot_final.ptau" -v

echo ""
echo "=== Step 4: Circuit-Specific Setup (Phase 2) ==="
npx snarkjs groth16 setup "$BUILD_DIR/withdraw.r1cs" "$BUILD_DIR/pot_final.ptau" "$BUILD_DIR/withdraw_0000.zkey"
npx snarkjs zkey contribute "$BUILD_DIR/withdraw_0000.zkey" "$BUILD_DIR/withdraw_0001.zkey" \
  --name="Dev Contributor 1" -e="$(head -c 32 /dev/urandom | xxd -p)"

echo ""
echo "=== Step 5: Export verification key ==="
npx snarkjs zkey export verificationkey "$BUILD_DIR/withdraw_0001.zkey" "$BUILD_DIR/verification_key.json"

echo ""
echo "=== Step 6: Export Rust verification key constants ==="
node scripts/export_vk_rust.js

echo ""
echo "=== Setup complete ==="
echo "Artifacts in $BUILD_DIR/:"
echo "  withdraw.r1cs       — Circuit constraints"
echo "  withdraw_js/        — WASM prover"
echo "  withdraw_0001.zkey  — Proving key"
echo "  verification_key.json — Verification key (JSON)"
echo ""
echo "IMPORTANT: This is a DEV setup. For production, run a multi-party ceremony."
