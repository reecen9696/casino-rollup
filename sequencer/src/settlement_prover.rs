/// Settlement Prover Module
///
/// Bridges the sequencer settlement queue with ZK proof generation.
/// Converts SettlementItem data into SettlementBatch format for the prover,
/// generates Groth16 proofs, and handles the proving pipeline.
use anyhow::{anyhow, Result};
use prover::{
    proof_generator::{ProofGenerator, SerializableProof},
    witness_generator::{SettlementBatch, SettlementBet},
};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::SettlementItem;

/// Settlement prover configuration
#[derive(Debug, Clone)]
pub struct SettlementProverConfig {
    /// Maximum number of users supported in a single batch
    pub max_users: usize,
    /// Maximum number of bets per batch
    pub max_bets_per_batch: usize,
    /// Initial house balance for proof generation
    pub house_initial_balance: u64,
}

impl Default for SettlementProverConfig {
    fn default() -> Self {
        Self {
            max_users: 5,                     // Start small for testing
            max_bets_per_batch: 3,            // Match circuit constraints
            house_initial_balance: 1_000_000, // 1M units house bankroll
        }
    }
}

/// Settlement prover that bridges sequencer and ZK prover
pub struct SettlementProver {
    /// Groth16 proof generator
    proof_generator: Arc<Mutex<ProofGenerator>>,
    /// Configuration parameters
    config: SettlementProverConfig,
    /// User balance tracking (in real implementation, this would come from database)
    user_balances: Arc<Mutex<HashMap<u32, u64>>>,
    /// House balance tracking
    house_balance: Arc<Mutex<u64>>,
    /// Global batch counter for unique batch IDs
    batch_counter: Arc<Mutex<u32>>,
}

impl SettlementProver {
    /// Create new settlement prover with given configuration
    pub async fn new(config: SettlementProverConfig) -> Result<Self> {
        let mut proof_generator = ProofGenerator::new(config.max_users, config.max_bets_per_batch);

        // Initialize the proof generator (setup Groth16 parameters)
        proof_generator
            .setup()
            .map_err(|e| anyhow!("Failed to setup proof generator: {}", e))?;

        let prover = Self {
            proof_generator: Arc::new(Mutex::new(proof_generator)),
            config: config.clone(),
            user_balances: Arc::new(Mutex::new(HashMap::new())),
            house_balance: Arc::new(Mutex::new(config.house_initial_balance)),
            batch_counter: Arc::new(Mutex::new(0)),
        };

        info!("SettlementProver initialized with config: {:?}", config);
        Ok(prover)
    }

    /// Initialize user balance (for demo purposes)
    pub async fn init_user_balance(&self, user_id: u32, balance: u64) {
        let mut balances = self.user_balances.lock().await;
        balances.insert(user_id, balance);
        info!("Initialized user {} with balance {}", user_id, balance);
    }

    /// Convert SettlementItem array to SettlementBatch for proof generation
    async fn convert_to_settlement_batch(
        &self,
        settlement_items: &[SettlementItem],
    ) -> Result<SettlementBatch> {
        let mut batch_counter = self.batch_counter.lock().await;
        *batch_counter += 1;
        let batch_id = *batch_counter;

        // Get current balances (snapshot)
        let initial_balances = self.user_balances.lock().await.clone();
        let house_initial_balance = *self.house_balance.lock().await;

        // Convert settlement items to settlement bets
        let mut bets = Vec::new();
        for item in settlement_items {
            // Parse user address to get user_id (simplified mapping)
            let user_id = self.parse_user_id(&item.player_address)?;

            // Determine bet outcome
            let is_win = item.payout > item.amount.abs();
            let amount = item.amount.abs() as u64;

            // For coin flip: assume heads=true, tails=false
            // In real implementation, this would come from bet data
            let guess = true; // Simplified for demo
            let outcome = is_win; // If they won, outcome matches guess

            let settlement_bet =
                SettlementBet::new(user_id, amount, guess, outcome, item.bet_id.clone());

            bets.push(settlement_bet);
        }

        // Validate batch size
        if bets.len() > self.config.max_bets_per_batch {
            return Err(anyhow!(
                "Batch too large: {} bets exceeds maximum {}",
                bets.len(),
                self.config.max_bets_per_batch
            ));
        }

        let settlement_batch = SettlementBatch {
            batch_id,
            bets,
            initial_balances,
            house_initial_balance,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        debug!(
            "Converted {} settlement items to batch {}",
            settlement_items.len(),
            batch_id
        );
        Ok(settlement_batch)
    }

    /// Parse user address to get user_id (simplified mapping for demo)
    fn parse_user_id(&self, player_address: &str) -> Result<u32> {
        // Try to parse as Pubkey first
        if let Ok(pubkey) = Pubkey::from_str(player_address) {
            // Use first 4 bytes of pubkey as user_id (simplified)
            let bytes = pubkey.to_bytes();
            let user_id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            return Ok(user_id % 1000); // Keep it reasonable for demo
        }

        // Fallback: hash the string
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        player_address.hash(&mut hasher);
        let hash = hasher.finish();
        Ok((hash % 1000) as u32) // Keep user IDs under 1000
    }

    /// Generate ZK proof for settlement batch
    pub async fn generate_proof(
        &self,
        settlement_items: &[SettlementItem],
    ) -> Result<SerializableProof> {
        let start_time = std::time::Instant::now();

        // Convert to settlement batch format
        let settlement_batch = self.convert_to_settlement_batch(settlement_items).await?;

        info!(
            "Generating proof for batch {} with {} bets",
            settlement_batch.batch_id,
            settlement_batch.bets.len()
        );

        // Generate proof using the prover library
        let proof_generator = self.proof_generator.lock().await;
        let proof = proof_generator
            .generate_proof(&settlement_batch)
            .map_err(|e| anyhow!("Proof generation failed: {}", e))?;

        let generation_time = start_time.elapsed();
        info!(
            "Proof generated for batch {} in {:?}",
            settlement_batch.batch_id, generation_time
        );

        // Update balances based on settlement
        self.update_balances(&settlement_batch).await?;

        Ok(proof)
    }

    /// Update user and house balances after successful proof generation
    async fn update_balances(&self, settlement_batch: &SettlementBatch) -> Result<()> {
        let mut user_balances = self.user_balances.lock().await;
        let mut house_balance = self.house_balance.lock().await;

        let mut total_user_delta: i64 = 0;

        for bet in &settlement_batch.bets {
            // Get current balance or initialize if new user
            let current_balance = user_balances.get(&bet.user_id).copied().unwrap_or(0);

            // Calculate balance change
            let balance_delta = if bet.outcome == bet.guess {
                // Win: get bet amount back plus payout
                bet.amount as i64
            } else {
                // Lose: lose bet amount
                -(bet.amount as i64)
            };

            let new_balance = (current_balance as i64 + balance_delta).max(0) as u64;
            user_balances.insert(bet.user_id, new_balance);
            total_user_delta += balance_delta;

            debug!(
                "User {} balance: {} -> {} (delta: {})",
                bet.user_id, current_balance, new_balance, balance_delta
            );
        }

        // Update house balance (house gains what users lose, loses what users win)
        let house_delta = -total_user_delta;
        let new_house_balance = (*house_balance as i64 + house_delta).max(0) as u64;
        *house_balance = new_house_balance;

        debug!(
            "House balance updated by {} to {}",
            house_delta, new_house_balance
        );

        Ok(())
    }

    /// Get current user balance
    pub async fn get_user_balance(&self, user_id: u32) -> u64 {
        self.user_balances
            .lock()
            .await
            .get(&user_id)
            .copied()
            .unwrap_or(0)
    }

    /// Get current house balance
    pub async fn get_house_balance(&self) -> u64 {
        *self.house_balance.lock().await
    }

    /// Verify a proof (for testing)
    pub async fn verify_proof(&self, proof: &SerializableProof) -> Result<bool> {
        let proof_generator = self.proof_generator.lock().await;
        let is_valid = proof_generator
            .verify_proof(proof)
            .map_err(|e| anyhow!("Proof verification failed: {}", e))?;
        Ok(is_valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_settlement_prover_creation() {
        let config = SettlementProverConfig::default();
        let prover = SettlementProver::new(config).await;
        assert!(prover.is_ok());
    }

    #[tokio::test]
    async fn test_user_id_parsing() {
        let config = SettlementProverConfig::default();
        let prover = SettlementProver::new(config).await.unwrap();

        // Test with valid pubkey
        let pubkey = Pubkey::new_unique();
        let user_id = prover.parse_user_id(&pubkey.to_string()).unwrap();
        assert!(user_id < 1000);

        // Test with arbitrary string
        let user_id2 = prover.parse_user_id("test_user").unwrap();
        assert!(user_id2 < 1000);
    }

    #[tokio::test]
    async fn test_settlement_batch_conversion() {
        let config = SettlementProverConfig::default();
        let prover = SettlementProver::new(config).await.unwrap();

        // Initialize some user balances
        prover.init_user_balance(100, 10000).await;
        prover.init_user_balance(200, 5000).await;

        let settlement_items = vec![
            SettlementItem {
                bet_id: "bet1".to_string(),
                player_address: "user100".to_string(),
                amount: -1000, // Lost bet
                payout: 0,
                timestamp: Utc::now(),
            },
            SettlementItem {
                bet_id: "bet2".to_string(),
                player_address: "user200".to_string(),
                amount: 500, // Won bet
                payout: 1000,
                timestamp: Utc::now(),
            },
        ];

        let batch = prover
            .convert_to_settlement_batch(&settlement_items)
            .await
            .unwrap();
        assert_eq!(batch.bets.len(), 2);
        assert_eq!(batch.batch_id, 1);
        assert!(batch.initial_balances.len() >= 2);
    }

    #[tokio::test]
    async fn test_proof_generation() {
        let config = SettlementProverConfig::default();
        let prover = SettlementProver::new(config).await.unwrap();

        // Initialize user balance
        prover.init_user_balance(100, 10000).await;

        let settlement_items = vec![SettlementItem {
            bet_id: "bet1".to_string(),
            player_address: "user100".to_string(),
            amount: -1000, // Lost bet
            payout: 0,
            timestamp: Utc::now(),
        }];

        let result = prover.generate_proof(&settlement_items).await;
        assert!(result.is_ok());

        let proof = result.unwrap();
        assert_eq!(proof.batch_id, 1);

        // Verify the proof
        let is_valid = prover.verify_proof(&proof).await.unwrap();
        assert!(is_valid);
    }
}
