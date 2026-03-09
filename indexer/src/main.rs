//! Caligo Indexer — Soroban event indexer for the ZK mixer protocol.
//!
//! Listens to deposit events from the mixer pool contract, maintains
//! an off-chain Poseidon Merkle tree mirror, and serves REST API
//! endpoints for Merkle path queries and pool state.
//!
//! Usage:
//!   RUST_LOG=info caligo-indexer
//!
//! Environment variables:
//!   SOROBAN_RPC_URL      — Soroban RPC endpoint (default: https://soroban-testnet.stellar.org)
//!   CONTRACT_ID          — Mixer pool contract ID (required)
//!   DENOMINATION         — Pool denomination in stroops (default: 100_000_000_00 = 100 XLM)
//!   TREE_DEPTH           — Merkle tree depth (default: 20)
//!   API_PORT             — REST API port (default: 3001)
//!   POLL_INTERVAL_SECS   — Event polling interval in seconds (default: 5)
//!   DATABASE_URL         — PostgreSQL URL (optional, enables persistent storage)
//!                          Example: postgres://user:pass@localhost:5432/caligo

mod api;
pub mod db;
mod listener;
mod merkle;
mod poseidon;
mod state;

use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "caligo_indexer=info".into()),
        )
        .init();

    let rpc_url = std::env::var("SOROBAN_RPC_URL")
        .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string());
    let contract_id = std::env::var("CONTRACT_ID")
        .expect("CONTRACT_ID environment variable is required");
    let denomination: i128 = std::env::var("DENOMINATION")
        .unwrap_or_else(|_| "10000000000".to_string())
        .parse()
        .expect("DENOMINATION must be a valid integer");
    let tree_depth: u32 = std::env::var("TREE_DEPTH")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .expect("TREE_DEPTH must be a valid integer");
    let api_port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .expect("API_PORT must be a valid port number");
    let poll_interval: u64 = std::env::var("POLL_INTERVAL_SECS")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .expect("POLL_INTERVAL_SECS must be a valid integer");

    let shared_state = state::new_shared_state(contract_id, denomination, tree_depth);

    // Optionally connect to PostgreSQL for persistent storage
    #[cfg(feature = "postgres")]
    {
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            match db::Database::connect(&database_url).await {
                Ok(database) => {
                    info!("PostgreSQL storage enabled");

                    // Restore state from database
                    let mut state = shared_state.write().await;
                    match database.get_all_commitments().await {
                        Ok(commitments) => {
                            for (_, commitment) in &commitments {
                                state.tree.insert(*commitment);
                            }
                            if !commitments.is_empty() {
                                info!(
                                    "Restored {} commitments from database",
                                    commitments.len()
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to restore commitments: {}", e);
                        }
                    }

                    match database.get_last_ledger().await {
                        Ok(ledger) if ledger > 0 => {
                            state.last_ledger = ledger;
                            info!("Resuming from ledger {}", ledger);
                        }
                        _ => {}
                    }
                    drop(state);

                    // Store database handle in shared state
                    {
                        let mut state = shared_state.write().await;
                        state.database = Some(std::sync::Arc::new(database));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to PostgreSQL: {}. Running in-memory only.",
                        e
                    );
                }
            }
        } else {
            info!("No DATABASE_URL set — running with in-memory storage only");
        }
    }

    #[cfg(not(feature = "postgres"))]
    {
        info!("PostgreSQL support not compiled — running with in-memory storage");
        info!("  Build with: cargo run --features postgres");
    }

    // Start event listener in background
    let listener_state = shared_state.clone();
    tokio::spawn(async move {
        listener::start_event_listener(rpc_url, listener_state, poll_interval).await;
    });

    // Start API server
    let app = api::router(shared_state);
    let addr = format!("0.0.0.0:{}", api_port);
    info!("Indexer API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
