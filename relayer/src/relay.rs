//! Core relay logic — request handling, rate limiting, and transaction submission.
//!
//! The relay server accepts POST /relay requests with withdrawal proof payloads,
//! validates them, and broadcasts the withdrawal transaction to the Soroban network.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

use crate::rpc;
use crate::validate;

/// Relayer configuration.
#[derive(Debug, Clone)]
pub struct RelayerConfig {
    pub secret_key: String,
    pub rpc_url: String,
    pub network_passphrase: String,
    pub fee_bps: u32,
    pub max_pending: usize,
}

/// Relay request from client (matches client SDK RelayRequest type).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayRequest {
    /// Groth16 proof encoded as hex (256 bytes = 512 hex chars).
    pub proof_hex: String,
    /// Merkle root (32 bytes hex).
    pub root_hex: String,
    /// Nullifier hash (32 bytes hex).
    pub nullifier_hash_hex: String,
    /// Recipient Stellar address.
    pub recipient: String,
    /// Fee in stroops.
    pub fee: String,
    /// Mixer pool contract ID.
    pub pool_contract_id: String,
}

/// Relay response to client (matches client SDK RelayResponse type).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Relayer status info.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayerStatus {
    pub address: String,
    pub fee_bps: u32,
    pub pending_count: usize,
    pub max_pending: usize,
    pub total_relayed: u64,
    pub total_earned_stroops: u64,
}

/// Shared relayer state.
pub struct RelayerState {
    pub config: RelayerConfig,
    /// Relayer's public Stellar address (derived from secret key).
    pub relayer_address: String,
    /// Set of nullifier hashes currently being processed (prevents duplicate submissions).
    pub pending_nullifiers: RwLock<HashSet<String>>,
    /// HTTP client for Soroban RPC.
    pub client: reqwest::Client,
    /// Stats
    pub total_relayed: RwLock<u64>,
    pub total_earned: RwLock<u64>,
}

impl RelayerState {
    pub fn new(config: RelayerConfig) -> Arc<Self> {
        // Derive public address from secret key
        let relayer_address = derive_public_address(&config.secret_key);

        info!("Relayer address: {}", relayer_address);

        Arc::new(Self {
            config,
            relayer_address,
            pending_nullifiers: RwLock::new(HashSet::new()),
            client: reqwest::Client::new(),
            total_relayed: RwLock::new(0),
            total_earned: RwLock::new(0),
        })
    }
}

type AppState = Arc<RelayerState>;

/// Build the HTTP router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/relay", post(handle_relay))
        .route("/status", get(handle_status))
        .route("/health", get(handle_health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// POST /relay — Handle a withdrawal relay request.
async fn handle_relay(
    State(state): State<AppState>,
    Json(request): Json<RelayRequest>,
) -> impl IntoResponse {
    // Check pending capacity
    {
        let pending = state.pending_nullifiers.read().await;
        if pending.len() >= state.config.max_pending {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(RelayResponse {
                    success: false,
                    tx_hash: None,
                    error: Some("Relayer at capacity, try again later".to_string()),
                }),
            );
        }

        // Check if this nullifier is already being processed
        if pending.contains(&request.nullifier_hash_hex) {
            return (
                StatusCode::CONFLICT,
                Json(RelayResponse {
                    success: false,
                    tx_hash: None,
                    error: Some("This nullifier is already being processed".to_string()),
                }),
            );
        }
    }

    // Validate request fields
    if let Err(e) = validate::validate_hex(&request.proof_hex, 256) {
        return (
            StatusCode::BAD_REQUEST,
            Json(RelayResponse {
                success: false,
                tx_hash: None,
                error: Some(format!("Invalid proof: {}", e)),
            }),
        );
    }

    if let Err(e) = validate::validate_hex(&request.root_hex, 32) {
        return (
            StatusCode::BAD_REQUEST,
            Json(RelayResponse {
                success: false,
                tx_hash: None,
                error: Some(format!("Invalid root: {}", e)),
            }),
        );
    }

    if let Err(e) = validate::validate_hex(&request.nullifier_hash_hex, 32) {
        return (
            StatusCode::BAD_REQUEST,
            Json(RelayResponse {
                success: false,
                tx_hash: None,
                error: Some(format!("Invalid nullifier hash: {}", e)),
            }),
        );
    }

    if let Err(e) = validate::validate_stellar_address(&request.recipient) {
        return (
            StatusCode::BAD_REQUEST,
            Json(RelayResponse {
                success: false,
                tx_hash: None,
                error: Some(format!("Invalid recipient: {}", e)),
            }),
        );
    }

    // Add nullifier to pending set
    {
        let mut pending = state.pending_nullifiers.write().await;
        pending.insert(request.nullifier_hash_hex.clone());
    }

    info!(
        "Processing relay request: recipient={}, nullifier={}...",
        request.recipient,
        &request.nullifier_hash_hex[..16.min(request.nullifier_hash_hex.len())]
    );

    // Submit the withdrawal transaction
    let result = rpc::submit_withdrawal(
        &state.client,
        &state.config.rpc_url,
        &state.config.network_passphrase,
        &state.config.secret_key,
        &request.pool_contract_id,
        &request.proof_hex,
        &request.root_hex,
        &request.nullifier_hash_hex,
        &request.recipient,
        &state.relayer_address,
        &request.fee,
    )
    .await;

    // Remove from pending set
    {
        let mut pending = state.pending_nullifiers.write().await;
        pending.remove(&request.nullifier_hash_hex);
    }

    if result.success {
        // Update stats
        let fee_stroops: u64 = request.fee.parse().unwrap_or(0);
        {
            let mut total = state.total_relayed.write().await;
            *total += 1;
        }
        {
            let mut earned = state.total_earned.write().await;
            *earned += fee_stroops;
        }

        info!("Relay successful: tx={}", result.tx_hash.as_deref().unwrap_or("unknown"));
        (
            StatusCode::OK,
            Json(RelayResponse {
                success: true,
                tx_hash: result.tx_hash,
                error: None,
            }),
        )
    } else {
        warn!("Relay failed: {}", result.error.as_deref().unwrap_or("unknown"));
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RelayResponse {
                success: false,
                tx_hash: None,
                error: result.error,
            }),
        )
    }
}

/// GET /status — Return relayer info and stats.
async fn handle_status(State(state): State<AppState>) -> impl IntoResponse {
    let pending = state.pending_nullifiers.read().await;
    let total_relayed = *state.total_relayed.read().await;
    let total_earned = *state.total_earned.read().await;

    Json(RelayerStatus {
        address: state.relayer_address.clone(),
        fee_bps: state.config.fee_bps,
        pending_count: pending.len(),
        max_pending: state.config.max_pending,
        total_relayed,
        total_earned_stroops: total_earned,
    })
}

/// GET /health — Health check.
async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

/// Derive the public Stellar address from a secret key.
fn derive_public_address(secret_key: &str) -> String {
    use ed25519_dalek::SigningKey;
    use stellar_strkey::ed25519::{PrivateKey, PublicKey};

    match PrivateKey::from_string(secret_key) {
        Ok(sk) => {
            let signing_key = SigningKey::from_bytes(&sk.0);
            let public_bytes = signing_key.verifying_key().to_bytes();
            PublicKey(public_bytes).to_string()
        }
        Err(_) => {
            // If the key doesn't parse as a standard S... key, return as-is
            // This allows passing a public key directly for testing
            secret_key.to_string()
        }
    }
}
