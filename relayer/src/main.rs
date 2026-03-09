//! Caligo Relayer Server
//!
//! Receives withdrawal proof payloads from clients and broadcasts
//! withdrawal transactions to the Soroban network on their behalf.
//!
//! The relayer earns a fee (capped by the MixerPool contract) for each
//! successful withdrawal it relays.
//!
//! Usage:
//!   RUST_LOG=info caligo-relayer
//!
//! Environment variables:
//!   RELAYER_SECRET_KEY   — Stellar secret key for the relayer account (required)
//!   SOROBAN_RPC_URL      — Soroban RPC endpoint (default: testnet)
//!   STELLAR_NETWORK      — Network passphrase preset: "testnet" or "mainnet" (default: testnet)
//!   RELAYER_PORT         — HTTP server port (default: 3002)
//!   FEE_BPS              — Fee in basis points to charge (default: 50 = 0.5%)
//!   MAX_PENDING          — Max concurrent relay requests (default: 10)

mod relay;
mod rpc;
mod validate;

use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "caligo_relayer=info".into()),
        )
        .init();

    let secret_key = std::env::var("RELAYER_SECRET_KEY")
        .expect("RELAYER_SECRET_KEY environment variable is required");
    let rpc_url = std::env::var("SOROBAN_RPC_URL")
        .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string());
    let network = std::env::var("STELLAR_NETWORK").unwrap_or_else(|_| "testnet".to_string());
    let port: u16 = std::env::var("RELAYER_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .parse()
        .expect("RELAYER_PORT must be a valid port");
    let fee_bps: u32 = std::env::var("FEE_BPS")
        .unwrap_or_else(|_| "50".to_string())
        .parse()
        .expect("FEE_BPS must be a valid integer");
    let max_pending: usize = std::env::var("MAX_PENDING")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .expect("MAX_PENDING must be a valid integer");

    let network_passphrase = match network.as_str() {
        "mainnet" => "Public Global Stellar Network ; September 2015".to_string(),
        _ => "Test SDF Network ; September 2015".to_string(),
    };

    let config = relay::RelayerConfig {
        secret_key,
        rpc_url,
        network_passphrase,
        fee_bps,
        max_pending,
    };

    let state = relay::RelayerState::new(config);
    let app = relay::router(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Caligo Relayer listening on {}", addr);
    info!("Fee: {} bps ({}%)", fee_bps, fee_bps as f64 / 100.0);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
