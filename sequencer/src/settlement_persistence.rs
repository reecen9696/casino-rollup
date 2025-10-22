/// Settlement persistence module for crash-safe queue and deduplication
/// Implements requirements: "crash-safe queue & dedup on resend"
/// Uses file-based persistence to avoid complex database dependencies
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::RwLock;

use crate::SettlementItem;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SettlementBatchStatus {
    Pending,   // Created but not yet proving
    Proving,   // ZK proof generation in progress
    Proved,    // ZK proof generated successfully
    Submitted, // Submitted to Solana
    Confirmed, // Confirmed on-chain
    Failed,    // Failed permanently
}

impl std::fmt::Display for SettlementBatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Proving => write!(f, "proving"),
            Self::Proved => write!(f, "proved"),
            Self::Submitted => write!(f, "submitted"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for SettlementBatchStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(Self::Pending),
            "proving" => Ok(Self::Proving),
            "proved" => Ok(Self::Proved),
            "submitted" => Ok(Self::Submitted),
            "confirmed" => Ok(Self::Confirmed),
            "failed" => Ok(Self::Failed),
            _ => Err(anyhow::anyhow!("Invalid settlement batch status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementBatch {
    pub batch_id: u64,
    pub status: SettlementBatchStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub proof_data: Option<Vec<u8>>,
    pub transaction_signature: Option<String>,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub items: Vec<SettlementItem>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistenceData {
    batches: HashMap<u64, SettlementBatch>,
    processed_bet_ids: std::collections::HashSet<String>,
    last_batch_id: u64,
}

pub struct SettlementPersistence {
    data: RwLock<PersistenceData>,
    file_path: PathBuf,
}

impl SettlementPersistence {
    /// Initialize file-based persistence for settlement tracking
    pub async fn new(database_url: &str) -> Result<Self> {
        // Convert database URL to file path for our JSON persistence
        let file_path = if database_url.starts_with("sqlite:") {
            let db_path = &database_url[7..];
            Path::new(db_path).with_extension("settlement.json")
        } else if database_url == "sqlite::memory:" {
            // For in-memory database, use a temp file
            std::env::temp_dir().join("settlement_memory.json")
        } else {
            PathBuf::from(database_url).with_extension("settlement.json")
        };

        // Create directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Load existing data or create new
        let data = if file_path.exists() {
            let json_data = fs::read_to_string(&file_path).await?;
            serde_json::from_str(&json_data).unwrap_or_default()
        } else {
            PersistenceData::default()
        };

        Ok(Self {
            data: RwLock::new(data),
            file_path,
        })
    }

    /// Save data to file
    async fn save_to_file(&self) -> Result<()> {
        let data = self.data.read().await;
        let json_data = serde_json::to_string_pretty(&*data)?;
        fs::write(&self.file_path, json_data).await?;
        Ok(())
    }

    /// Save settlement batch for crash-safe processing (Phase 3e requirement)
    pub async fn save_batch(&self, batch_id: &str, items: Vec<SettlementItem>) -> Result<()> {
        // Extract numeric batch ID from string format "batch_N"
        let batch_id_num: u64 = batch_id
            .strip_prefix("batch_")
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                // Generate new ID if parsing fails
                use std::sync::atomic::{AtomicU64, Ordering};
                static NEXT_ID: AtomicU64 = AtomicU64::new(1);
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            });

        self.create_batch(&items).await?;
        Ok(())
    }

    /// Mark batch as completed (Phase 3e requirement)
    pub async fn mark_completed(&self, batch_id: &str) -> Result<()> {
        let batch_id_num: u64 = batch_id
            .strip_prefix("batch_")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid batch ID format: {}", batch_id))?;

        self.update_batch_status(batch_id_num, SettlementBatchStatus::Confirmed, None)
            .await
    }

    /// Load pending batches for processing (crash recovery)
    pub async fn load_pending_batches(&self) -> Result<Vec<SettlementBatch>> {
        self.get_pending_batches().await
    }

    /// Create a new settlement batch with pending status
    pub async fn create_batch(&self, items: &[SettlementItem]) -> Result<u64> {
        let now = Utc::now();

        let mut data = self.data.write().await;
        let batch_id = data.last_batch_id + 1;
        data.last_batch_id = batch_id;

        let batch = SettlementBatch {
            batch_id,
            status: SettlementBatchStatus::Pending,
            created_at: now,
            updated_at: now,
            proof_data: None,
            transaction_signature: None,
            retry_count: 0,
            error_message: None,
            items: items.to_vec(),
        };

        // Add all bet IDs to processed set for deduplication
        for item in items {
            data.processed_bet_ids.insert(item.bet_id.clone());
        }

        data.batches.insert(batch_id, batch);
        drop(data);

        self.save_to_file().await?;

        tracing::info!(
            "Created settlement batch {} with {} items",
            batch_id,
            items.len()
        );
        Ok(batch_id)
    }

    /// Update batch status
    pub async fn update_batch_status(
        &self,
        batch_id: u64,
        status: SettlementBatchStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();

        let mut data = self.data.write().await;
        if let Some(batch) = data.batches.get_mut(&batch_id) {
            batch.status = status.clone();
            batch.updated_at = now;
            batch.error_message = error_message;
        }
        drop(data);

        self.save_to_file().await?;

        tracing::info!("Updated batch {} status to {}", batch_id, status);
        Ok(())
    }

    /// Store proof data for a batch
    pub async fn store_proof(&self, batch_id: u64, proof_data: &[u8]) -> Result<()> {
        let now = Utc::now();

        let mut data = self.data.write().await;
        if let Some(batch) = data.batches.get_mut(&batch_id) {
            batch.status = SettlementBatchStatus::Proved;
            batch.proof_data = Some(proof_data.to_vec());
            batch.updated_at = now;
        }
        drop(data);

        self.save_to_file().await?;

        tracing::info!(
            "Stored proof for batch {} ({} bytes)",
            batch_id,
            proof_data.len()
        );
        Ok(())
    }

    /// Store transaction signature after Solana submission
    pub async fn store_transaction(&self, batch_id: u64, signature: &str) -> Result<()> {
        let now = Utc::now();

        let mut data = self.data.write().await;
        if let Some(batch) = data.batches.get_mut(&batch_id) {
            batch.status = SettlementBatchStatus::Submitted;
            batch.transaction_signature = Some(signature.to_string());
            batch.updated_at = now;
        }
        drop(data);

        self.save_to_file().await?;

        tracing::info!("Stored transaction {} for batch {}", signature, batch_id);
        Ok(())
    }

    /// Get pending batches that need to be retried (crash recovery)
    pub async fn get_pending_batches(&self) -> Result<Vec<SettlementBatch>> {
        let data = self.data.read().await;
        let batches: Vec<SettlementBatch> = data
            .batches
            .values()
            .filter(|batch| {
                matches!(
                    batch.status,
                    SettlementBatchStatus::Pending
                        | SettlementBatchStatus::Proving
                        | SettlementBatchStatus::Proved
                        | SettlementBatchStatus::Submitted
                )
            })
            .cloned()
            .collect();

        Ok(batches)
    }

    /// Check if a bet is already included in any batch (deduplication)
    pub async fn is_bet_processed(&self, bet_id: &str) -> Result<bool> {
        let data = self.data.read().await;
        Ok(data.processed_bet_ids.contains(bet_id))
    }

    /// Increment retry count for a batch
    pub async fn increment_retry_count(&self, batch_id: u64) -> Result<u32> {
        let now = Utc::now();

        let mut data = self.data.write().await;
        let retry_count = if let Some(batch) = data.batches.get_mut(&batch_id) {
            batch.retry_count += 1;
            batch.updated_at = now;
            batch.retry_count
        } else {
            return Err(anyhow::anyhow!("Batch {} not found", batch_id));
        };
        drop(data);

        self.save_to_file().await?;

        Ok(retry_count)
    }

    /// Get settlement statistics
    pub async fn get_settlement_stats(&self) -> Result<SettlementStats> {
        let data = self.data.read().await;

        let total_batches = data.batches.len() as u64;
        let confirmed_batches = data
            .batches
            .values()
            .filter(|b| b.status == SettlementBatchStatus::Confirmed)
            .count() as u64;
        let failed_batches = data
            .batches
            .values()
            .filter(|b| b.status == SettlementBatchStatus::Failed)
            .count() as u64;
        let confirmed_items = data
            .batches
            .values()
            .filter(|b| b.status == SettlementBatchStatus::Confirmed)
            .map(|b| b.items.len() as u64)
            .sum();

        Ok(SettlementStats {
            total_batches,
            confirmed_batches,
            failed_batches,
            confirmed_items,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct SettlementStats {
    pub total_batches: u64,
    pub confirmed_batches: u64,
    pub failed_batches: u64,
    pub confirmed_items: u64,
}
