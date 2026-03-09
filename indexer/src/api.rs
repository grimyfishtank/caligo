//! REST API for the indexer.
//!
//! Endpoints:
//!   GET /merkle-path?commitment=<hex>  → MerkleProof
//!   GET /pool-state                    → PoolInfo
//!   GET /roots                         → Vec<RootEntry>
//!   GET /health                        → health check

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tower_http::cors::CorsLayer;

use crate::state::SharedState;

/// Build the API router.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/merkle-path", get(merkle_path))
        .route("/pool-state", get(pool_state))
        .route("/roots", get(roots))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ── Request/Response types ──

#[derive(Deserialize)]
struct MerklePathQuery {
    commitment: String,
}


// ── Handlers ──

/// GET /merkle-path?commitment=<hex>
///
/// Returns the Merkle inclusion proof for a given commitment.
/// The commitment should be a 64-character hex string (32 bytes).
async fn merkle_path(
    State(state): State<SharedState>,
    Query(query): Query<MerklePathQuery>,
) -> impl IntoResponse {
    let commitment_hex = query.commitment.trim_start_matches("0x");

    let commitment_bytes = match hex::decode(commitment_hex) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            arr
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid commitment: expected 64 hex characters"})),
            )
                .into_response();
        }
    };

    let state = state.read().await;

    match state.tree.find_commitment(&commitment_bytes) {
        Some(index) => {
            let proof = state.tree.proof(index).unwrap();
            let path_elements: Vec<String> = proof.path_elements.iter().map(hex::encode).collect();
            Json(serde_json::json!({
                "root": hex::encode(proof.root),
                "path_elements": path_elements,
                "path_indices": proof.path_indices,
                "leaf_index": proof.leaf_index,
            }))
            .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Commitment not found in tree"})),
        )
            .into_response(),
    }
}

/// GET /pool-state
///
/// Returns current pool information.
async fn pool_state(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    Json(state.pool_info())
}

/// GET /roots
///
/// Returns the full root history.
async fn roots(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    Json(state.roots.clone())
}

/// GET /health
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}
