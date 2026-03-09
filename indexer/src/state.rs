//! Shared indexer state.
//!
//! Holds the off-chain Merkle tree, root history, and pool metadata.
//! Protected by a RwLock for concurrent read access from API handlers.
//!
//! When the `postgres` feature is enabled, state changes are also
//! persisted to PostgreSQL for durability across restarts.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::merkle::MerkleTree;

/// Pool state tracked by the indexer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolInfo {
    /// Contract ID of the mixer pool.
    pub contract_id: String,
    /// Pool denomination in stroops.
    pub denomination: i128,
    /// Number of deposits.
    pub deposit_count: u32,
    /// Current Merkle root (hex).
    pub latest_root: String,
}

/// Root history entry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RootEntry {
    /// Merkle root (hex).
    pub root: String,
    /// Deposit index when this root was created.
    pub deposit_index: u32,
}

/// Shared application state.
pub struct AppState {
    pub tree: MerkleTree,
    pub roots: Vec<RootEntry>,
    pub contract_id: String,
    pub denomination: i128,
    /// Ledger cursor for event polling.
    pub last_ledger: u32,
    /// Optional PostgreSQL database handle.
    #[cfg(feature = "postgres")]
    pub database: Option<Arc<crate::db::Database>>,
}

impl AppState {
    pub fn new(contract_id: String, denomination: i128, tree_depth: u32) -> Self {
        let tree = MerkleTree::new(tree_depth);
        let initial_root = hex::encode(tree.root());
        AppState {
            tree,
            roots: vec![RootEntry {
                root: initial_root,
                deposit_index: 0,
            }],
            contract_id,
            denomination,
            last_ledger: 0,
            #[cfg(feature = "postgres")]
            database: None,
        }
    }

    /// Insert a commitment and record the new root.
    ///
    /// When PostgreSQL is enabled and connected, the commitment and
    /// root are also persisted asynchronously.
    pub fn insert_commitment(&mut self, commitment: [u8; 32]) -> u32 {
        let index = self.tree.insert(commitment);
        let root_bytes = self.tree.root();
        let root_hex = hex::encode(root_bytes);
        let deposit_index = index + 1;
        self.roots.push(RootEntry {
            root: root_hex,
            deposit_index,
        });

        #[cfg(feature = "postgres")]
        if let Some(db) = &self.database {
            let db = db.clone();
            tokio::spawn(async move {
                if let Err(e) = db.insert_commitment(index, &commitment).await {
                    tracing::error!("Failed to persist commitment: {}", e);
                }
                if let Err(e) = db.insert_root(&root_bytes, deposit_index).await {
                    tracing::error!("Failed to persist root: {}", e);
                }
            });
        }

        index
    }

    /// Update the last processed ledger and persist if database is available.
    pub fn update_last_ledger(&mut self, ledger: u32) {
        self.last_ledger = ledger;

        #[cfg(feature = "postgres")]
        if let Some(db) = &self.database {
            let db = db.clone();
            tokio::spawn(async move {
                if let Err(e) = db.set_last_ledger(ledger).await {
                    tracing::error!("Failed to persist last_ledger: {}", e);
                }
            });
        }
    }

    /// Get current pool info.
    pub fn pool_info(&self) -> PoolInfo {
        PoolInfo {
            contract_id: self.contract_id.clone(),
            denomination: self.denomination,
            deposit_count: self.tree.leaf_count(),
            latest_root: hex::encode(self.tree.root()),
        }
    }
}

/// Thread-safe shared state handle.
pub type SharedState = Arc<RwLock<AppState>>;

pub fn new_shared_state(contract_id: String, denomination: i128, tree_depth: u32) -> SharedState {
    Arc::new(RwLock::new(AppState::new(contract_id, denomination, tree_depth)))
}
