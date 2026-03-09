#!/usr/bin/env rust-script
//! Groth16 BN254 verifier cost benchmark.
//!
//! Measures native execution time of the pairing operations
//! used in Groth16 verification on BN254.
//!
//! Run with:
//!   cargo install rust-script
//!   rust-script scripts/bench_verifier.rs
//!
//! Or just read the output from the CI/testing notes.

//! ```cargo
//! [dependencies]
//! ark-bn254 = { version = "0.4.0", features = ["curve"] }
//! ark-ec = "0.4.2"
//! ark-ff = "0.4.2"
//! ```

use ark_bn254::{Bn254, Fr, G1Affine, G1Projective, G2Affine};
use ark_ec::pairing::Pairing;
use ark_ec::CurveGroup;
use ark_ff::Zero;
use std::time::Instant;

fn main() {
    let g1 = G1Affine::generator();
    let g2 = G2Affine::generator();
    let scalar = Fr::from(12345u64);

    println!("=== BN254 Groth16 Verifier Cost Benchmark ===\n");

    // Warm up
    let _ = Bn254::pairing(g1, g2);

    // Single pairing
    let start = Instant::now();
    let iterations = 10;
    for _ in 0..iterations {
        let _ = Bn254::pairing(g1, g2);
    }
    let single = start.elapsed() / iterations;
    println!("Single pairing:              {:?}", single);

    // 4-pair multi-pairing (Groth16 verification)
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = Bn254::multi_pairing([g1, -g1, g1, -g1], [g2, g2, g2, g2]);
    }
    let multi = start.elapsed() / iterations;
    println!("4-pair multi-pairing:        {:?}", multi);

    // IC linear combination (5 scalar muls)
    let start = Instant::now();
    for _ in 0..iterations {
        let mut acc: G1Projective = g1.into();
        for _ in 0..5 {
            acc += g1 * scalar;
        }
        let _ = acc.into_affine();
    }
    let ic = start.elapsed() / iterations;
    println!("5x scalar mul (IC combine):  {:?}", ic);

    // Full simulated verification
    let start = Instant::now();
    for _ in 0..iterations {
        let mut vk_x: G1Projective = g1.into();
        for _ in 0..5 {
            vk_x += g1 * scalar;
        }
        let vk_x_affine = vk_x.into_affine();
        let result = Bn254::multi_pairing([g1, -g1, -vk_x_affine, -g1], [g2, g2, g2, g2]);
        let _ = result.is_zero();
    }
    let full = start.elapsed() / iterations;
    println!("Full verification:           {:?}", full);

    println!("\n--- Soroban Cost Estimation ---");
    println!("Native execution:  {:?}", full);
    println!("WASM overhead:     ~3-5x");
    println!("Est. WASM time:    {:?} - {:?}", full * 3, full * 5);
    println!();
    println!("Soroban limits:");
    println!("  CPU budget:  ~100M instructions");
    println!("  Memory:      ~40 MB");
    println!();
    println!("Measure actual on-chain cost with:");
    println!("  soroban contract invoke --cost --id <ID> -- withdraw ...");
}
