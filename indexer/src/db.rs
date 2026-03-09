//! PostgreSQL storage backend for the indexer.
//!
//! Persists commitments, Merkle tree state, and root history to PostgreSQL.
//! This replaces the in-memory storage for production deployments where
//! the indexer needs to survive restarts without re-scanning all events.
//!
//! Enable with: cargo run --features postgres
//!
//! Schema is auto-migrated on startup via `initialize_schema()`.

#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "postgres")]
use tracing::info;

/// PostgreSQL connection and query interface.
#[cfg(feature = "postgres")]
pub struct Database {
    pool: PgPool,
}

#[cfg(feature = "postgres")]
impl Database {
    /// Connect to PostgreSQL and run schema migrations.
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        let db = Self { pool };
        db.initialize_schema().await?;
        info!("PostgreSQL connected and schema initialized");
        Ok(db)
    }

    /// Create tables if they don't exist.
    async fn initialize_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS commitments (
                leaf_index  INTEGER PRIMARY KEY,
                commitment  BYTEA NOT NULL UNIQUE,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS merkle_roots (
                id              SERIAL PRIMARY KEY,
                root            BYTEA NOT NULL,
                deposit_index   INTEGER NOT NULL,
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS indexer_state (
                key     TEXT PRIMARY KEY,
                value   TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_commitments_commitment
                ON commitments (commitment);

            CREATE INDEX IF NOT EXISTS idx_merkle_roots_root
                ON merkle_roots (root);
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a new commitment. Returns the leaf index.
    pub async fn insert_commitment(
        &self,
        leaf_index: u32,
        commitment: &[u8; 32],
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO commitments (leaf_index, commitment) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(leaf_index as i32)
        .bind(&commitment[..])
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a new Merkle root into the history.
    pub async fn insert_root(
        &self,
        root: &[u8; 32],
        deposit_index: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO merkle_roots (root, deposit_index) VALUES ($1, $2)")
            .bind(&root[..])
            .bind(deposit_index as i32)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get all commitments ordered by leaf index (for rebuilding the Merkle tree).
    pub async fn get_all_commitments(&self) -> Result<Vec<(u32, [u8; 32])>, sqlx::Error> {
        let rows: Vec<(i32, Vec<u8>)> = sqlx::query_as(
            "SELECT leaf_index, commitment FROM commitments ORDER BY leaf_index ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(idx, bytes)| {
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Some((idx as u32, arr))
                } else {
                    None
                }
            })
            .collect())
    }

    /// Get the last processed ledger number.
    pub async fn get_last_ledger(&self) -> Result<u32, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM indexer_state WHERE key = 'last_ledger'",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .and_then(|(v,)| v.parse::<u32>().ok())
            .unwrap_or(0))
    }

    /// Update the last processed ledger number.
    pub async fn set_last_ledger(&self, ledger: u32) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO indexer_state (key, value) VALUES ('last_ledger', $1)
             ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(ledger.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get the total number of commitments.
    pub async fn commitment_count(&self) -> Result<u32, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM commitments")
                .fetch_one(&self.pool)
                .await?;
        Ok(count as u32)
    }

    /// Find a commitment's leaf index.
    pub async fn find_commitment(&self, commitment: &[u8; 32]) -> Result<Option<u32>, sqlx::Error> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT leaf_index FROM commitments WHERE commitment = $1",
        )
        .bind(&commitment[..])
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(idx,)| idx as u32))
    }

    /// Get recent roots (for the /roots endpoint).
    pub async fn get_roots(
        &self,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, u32)>, sqlx::Error> {
        let rows: Vec<(Vec<u8>, i32)> = sqlx::query_as(
            "SELECT root, deposit_index FROM merkle_roots ORDER BY id DESC LIMIT $1",
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(root, idx)| (root, idx as u32))
            .collect())
    }
}

// When postgres feature is disabled, provide a stub so the module compiles
#[cfg(not(feature = "postgres"))]
pub struct Database;

#[cfg(not(feature = "postgres"))]
impl Database {
    /// Stub — returns error when postgres feature is not enabled.
    pub async fn connect(_database_url: &str) -> Result<Self, String> {
        Err("PostgreSQL support not enabled. Build with --features postgres".to_string())
    }
}
