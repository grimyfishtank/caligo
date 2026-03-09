//! Soroban event listener.
//!
//! Polls the Soroban RPC `getEvents` endpoint for deposit events
//! emitted by the mixer pool contract. Each deposit event contains
//! the commitment and leaf index, which are used to update the
//! off-chain Merkle tree mirror.

use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::state::SharedState;

/// Soroban RPC JSON-RPC request.
#[derive(Serialize)]
struct RpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: serde_json::Value,
}

/// Soroban RPC getEvents response.
#[derive(Deserialize, Debug)]
struct RpcResponse {
    result: Option<GetEventsResult>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetEventsResult {
    events: Vec<EventEntry>,
    latest_ledger: u32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct EventEntry {
    #[serde(rename = "type")]
    event_type: String,
    ledger: u32,
    topic: Vec<serde_json::Value>,
    value: serde_json::Value,
}

/// Start the event polling loop.
///
/// Polls the Soroban RPC for deposit events every `poll_interval` seconds.
/// New commitments are inserted into the off-chain Merkle tree.
pub async fn start_event_listener(
    rpc_url: String,
    state: SharedState,
    poll_interval_secs: u64,
) {
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(poll_interval_secs));

    info!("Event listener started, polling every {}s", poll_interval_secs);

    loop {
        interval.tick().await;

        match poll_events(&client, &rpc_url, &state).await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} deposit events", count);
                }
            }
            Err(e) => {
                warn!("Event poll error: {}", e);
            }
        }
    }
}

/// Poll for new deposit events and process them.
async fn poll_events(
    client: &reqwest::Client,
    rpc_url: &str,
    state: &SharedState,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (contract_id, start_ledger) = {
        let s = state.read().await;
        (s.contract_id.clone(), s.last_ledger)
    };

    // If we haven't seen any ledger yet, get the latest ledger first
    let start = if start_ledger == 0 {
        // Get latest ledger from getHealth or getLatestLedger
        let req = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "getLatestLedger",
            params: serde_json::json!({}),
        };

        let resp: serde_json::Value = client
            .post(rpc_url)
            .json(&req)
            .send()
            .await?
            .json()
            .await?;

        let ledger = resp["result"]["sequence"]
            .as_u64()
            .unwrap_or(1) as u32;

        // Start from a reasonable lookback (e.g., last 1000 ledgers)
        ledger.saturating_sub(1000)
    } else {
        start_ledger + 1
    };

    let req = RpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "getEvents",
        params: serde_json::json!({
            "startLedger": start,
            "filters": [{
                "type": "contract",
                "contractIds": [contract_id],
                "topics": [["*"]]
            }],
            "pagination": {
                "limit": 1000
            }
        }),
    };

    let resp: RpcResponse = client
        .post(rpc_url)
        .json(&req)
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = resp.error {
        error!("RPC error: {:?}", err);
        return Ok(0);
    }

    let result = match resp.result {
        Some(r) => r,
        None => return Ok(0),
    };

    let mut count = 0;
    let mut state = state.write().await;
    state.update_last_ledger(result.latest_ledger);

    for event in &result.events {
        // Look for deposit events: topic contains "deposit"
        if is_deposit_event(event) {
            if let Some(commitment) = extract_commitment(event) {
                let index = state.insert_commitment(commitment);
                info!(
                    "Indexed deposit: leaf_index={}, commitment={}",
                    index,
                    hex::encode(commitment)
                );
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Check if an event is a deposit event.
fn is_deposit_event(event: &EventEntry) -> bool {
    // Soroban contract events have topics as XDR-encoded SCVal
    // The deposit event topic is the symbol "deposit"
    for topic in &event.topic {
        if let Some(s) = topic.as_str() {
            // Topics may be base64-encoded XDR SCVal or string representations
            if s.contains("deposit") {
                return true;
            }
        }
    }
    false
}

/// Extract the commitment bytes from a deposit event value.
///
/// The deposit event value is a tuple (commitment: BytesN<32>, leaf_index: u32).
/// The exact XDR decoding depends on the Soroban SDK version.
fn extract_commitment(event: &EventEntry) -> Option<[u8; 32]> {
    // The event value structure depends on the Soroban event encoding.
    // For now, attempt to extract from common formats.
    //
    // In production, this would use proper XDR decoding via stellar-xdr crate.
    // The event value is encoded as an SCVal tuple.

    let value = &event.value;

    // Try to extract from a string hex representation
    if let Some(s) = value.as_str() {
        if let Ok(bytes) = hex::decode(s.trim_start_matches("0x")) {
            if bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                return Some(arr);
            }
        }
    }

    // Try to extract from an object with a "bytes" field
    if let Some(bytes_str) = value.get("bytes").and_then(|v| v.as_str()) {
        if let Ok(bytes) = hex::decode(bytes_str) {
            if bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                return Some(arr);
            }
        }
    }

    // Try to extract from an XDR base64-encoded value
    // This would need proper stellar-xdr decoding in production
    if let Some(xdr_str) = value.get("xdr").and_then(|v| v.as_str()) {
        return decode_commitment_from_xdr(xdr_str);
    }

    warn!("Could not extract commitment from event: {:?}", value);
    None
}

/// Decode commitment from XDR-encoded SCVal.
///
/// Placeholder for proper XDR decoding. In production, use the stellar-xdr crate
/// to decode the SCVal tuple and extract the BytesN<32> commitment.
fn decode_commitment_from_xdr(xdr_base64: &str) -> Option<[u8; 32]> {
    use base64::Engine;
    let xdr_bytes = base64::engine::general_purpose::STANDARD
        .decode(xdr_base64)
        .ok()?;

    // SCVal encoding for a BytesN<32> is:
    // - Type discriminant (4 bytes): SCV_BYTES = 14
    // - Length (4 bytes): 32
    // - Data (32 bytes): the commitment
    //
    // For a tuple (commitment, leaf_index), the structure is more complex.
    // This is a simplified extractor that looks for a 32-byte sequence.
    //
    // TODO: Use stellar-xdr crate for proper decoding in production.
    if xdr_bytes.len() >= 40 {
        // Try to find the bytes value in the XDR
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&xdr_bytes[8..40]);
        return Some(arr);
    }

    None
}
