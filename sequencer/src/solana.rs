use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use std::str::FromStr;
use tokio::time::{sleep, Duration};
use log::{info, warn};

/// Solana client configuration
#[derive(Debug, Clone)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub commitment: CommitmentConfig,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:8899".to_string(), // Local validator by default
            commitment: CommitmentConfig::confirmed(),
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl SolanaConfig {
    /// Create config for Solana Testnet
    pub fn testnet() -> Self {
        Self {
            rpc_url: "https://api.testnet.solana.com".to_string(),
            commitment: CommitmentConfig::confirmed(),
            retry_attempts: 5,
            retry_delay_ms: 2000,
        }
    }
    
    /// Create config for Solana Devnet
    pub fn devnet() -> Self {
        Self {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            commitment: CommitmentConfig::confirmed(),
            retry_attempts: 5,
            retry_delay_ms: 2000,
        }
    }
}

/// Solana client for submitting settlement transactions
pub struct SolanaClient {
    client: RpcClient,
    config: SolanaConfig,
    sequencer_keypair: Keypair,
    vault_program_id: Pubkey,
    verifier_program_id: Pubkey,
}

impl SolanaClient {
    /// Create a new Solana client
    pub fn new(
        config: SolanaConfig,
        sequencer_keypair: Keypair,
        vault_program_id: &str,
        verifier_program_id: &str,
    ) -> Result<Self> {
        let client = RpcClient::new_with_commitment(config.rpc_url.clone(), config.commitment);
        
        let vault_program_id = Pubkey::from_str(vault_program_id)
            .map_err(|e| anyhow!("Invalid vault program ID: {}", e))?;
        let verifier_program_id = Pubkey::from_str(verifier_program_id)
            .map_err(|e| anyhow!("Invalid verifier program ID: {}", e))?;
        
        Ok(Self {
            client,
            config,
            sequencer_keypair,
            vault_program_id,
            verifier_program_id,
        })
    }

    /// Get the sequencer's public key
    pub fn sequencer_pubkey(&self) -> Pubkey {
        self.sequencer_keypair.pubkey()
    }

    /// Check if the Solana connection is healthy
    pub async fn health_check(&self) -> Result<()> {
        tokio::task::spawn_blocking({
            let rpc_url = self.config.rpc_url.clone();
            let commitment = self.config.commitment;
            move || {
                let client = RpcClient::new_with_commitment(rpc_url, commitment);
                let version = client.get_version()?;
                info!("Connected to Solana cluster version: {}", version.solana_core);
                Ok::<(), anyhow::Error>(())
            }
        }).await??;
        Ok(())
    }

    /// Get the sequencer's SOL balance
    pub async fn get_sequencer_balance(&self) -> Result<u64> {
        let balance = tokio::task::spawn_blocking({
            let rpc_url = self.config.rpc_url.clone();
            let commitment = self.config.commitment;
            let pubkey = self.sequencer_pubkey();
            move || {
                let client = RpcClient::new_with_commitment(rpc_url, commitment);
                client.get_balance(&pubkey)
            }
        }).await??;
        Ok(balance)
    }

    /// Submit a settlement batch to the verifier program
    pub async fn submit_settlement_batch(
        &self,
        batch_data: BatchSettlementData,
        proof: Vec<u8>,
    ) -> Result<Signature> {
        info!("Submitting settlement batch {} with {} bets", 
              batch_data.batch_id, batch_data.bets.len());

        let instruction = self.create_verify_and_settle_instruction(batch_data, proof)?;
        
        let signature = self.send_transaction_with_retry(vec![instruction]).await?;
        
        info!("Settlement batch submitted successfully: {}", signature);
        Ok(signature)
    }

    /// Create verify_and_settle instruction for the verifier program
    fn create_verify_and_settle_instruction(
        &self,
        batch_data: BatchSettlementData,
        proof: Vec<u8>,
    ) -> Result<Instruction> {
        // Derive verifier state PDA
        let (verifier_state, _) = Pubkey::find_program_address(
            &[b"verifier_state"],
            &self.verifier_program_id,
        );

        // Create instruction data
        let mut instruction_data = Vec::new();
        
        // Add instruction discriminator (8 bytes for verify_and_settle)
        // This would be computed from the method name hash in a real implementation
        instruction_data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0xab, 0xcd, 0xef, 0x90]);
        
        // Serialize batch data and proof (simplified for Phase 2)
        let serialized_batch = serde_json::to_vec(&batch_data)
            .map_err(|e| anyhow!("Failed to serialize batch data: {}", e))?;
        instruction_data.extend_from_slice(&(serialized_batch.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(&serialized_batch);
        
        instruction_data.extend_from_slice(&(proof.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(&proof);

        let instruction = Instruction {
            program_id: self.verifier_program_id,
            accounts: vec![
                AccountMeta::new(verifier_state, false),
                AccountMeta::new_readonly(self.sequencer_pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::sysvar::instructions::id(), false),
            ],
            data: instruction_data,
        };

        Ok(instruction)
    }

    /// Send transaction with retry logic
    async fn send_transaction_with_retry(&self, instructions: Vec<Instruction>) -> Result<Signature> {
        for attempt in 1..=self.config.retry_attempts {
            match self.send_transaction(instructions.clone()).await {
                Ok(signature) => return Ok(signature),
                Err(e) => {
                    if attempt == self.config.retry_attempts {
                        return Err(e);
                    }
                    warn!("Transaction attempt {} failed: {}. Retrying...", attempt, e);
                    sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                }
            }
        }
        unreachable!()
    }

    /// Send a single transaction
    async fn send_transaction(&self, instructions: Vec<Instruction>) -> Result<Signature> {
        let (recent_blockhash, signature) = tokio::task::spawn_blocking({
            let rpc_url = self.config.rpc_url.clone();
            let commitment = self.config.commitment;
            let sequencer_keypair = Keypair::from_bytes(&self.sequencer_keypair.to_bytes())
                .map_err(|e| anyhow!("Failed to clone keypair: {}", e))?;
            
            move || -> Result<(solana_sdk::hash::Hash, Signature)> {
                let client = RpcClient::new_with_commitment(rpc_url, commitment);
                
                // Get recent blockhash
                let recent_blockhash = client.get_latest_blockhash()?;
                
                // Create and sign transaction
                let transaction = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&sequencer_keypair.pubkey()),
                    &[&sequencer_keypair],
                    recent_blockhash,
                );
                
                // Send transaction
                let signature = client.send_and_confirm_transaction(&transaction)?;
                Ok((recent_blockhash, signature))
            }
        }).await??;

        info!("Transaction confirmed: {} (blockhash: {})", signature, recent_blockhash);
        Ok(signature)
    }

    /// Submit a placeholder transaction for Phase 2 testing
    pub async fn submit_placeholder_settlement(&self, batch_id: u64) -> Result<Signature> {
        info!("Submitting placeholder settlement for batch {}", batch_id);
        
        // Create dummy batch data for testing
        let batch_data = BatchSettlementData {
            batch_id,
            sequencer_nonce: batch_id,
            bets: vec![
                BetSettlement {
                    bet_id: batch_id * 100 + 1,
                    user: Pubkey::new_unique(),
                    bet_amount: 1000000, // 0.001 SOL
                    user_guess: 1,
                    outcome: 1,
                    payout: 2000000, // Win: 2x
                },
                BetSettlement {
                    bet_id: batch_id * 100 + 2,
                    user: Pubkey::new_unique(),
                    bet_amount: 500000, // 0.0005 SOL
                    user_guess: 0,
                    outcome: 1,
                    payout: 0, // Loss: 0x
                },
            ],
        };
        
        // Create dummy proof
        let dummy_proof = vec![0u8; 64]; // 64 bytes of zeros for Phase 2
        
        self.submit_settlement_batch(batch_data, dummy_proof).await
    }

    /// Get transaction status and logs
    pub async fn get_transaction_logs(&self, signature: &Signature) -> Result<Vec<String>> {
        let logs = tokio::task::spawn_blocking({
            let rpc_url = self.config.rpc_url.clone();
            let commitment = self.config.commitment;
            let signature = *signature;
            move || {
                let client = RpcClient::new_with_commitment(rpc_url, commitment);
                let config = solana_client::rpc_config::RpcTransactionConfig {
                    encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                };
                
                let transaction = client.get_transaction_with_config(&signature, config)?;
                
                // Extract logs from the transaction metadata
                let logs = if let Some(meta) = transaction.transaction.meta {
                    match meta.log_messages {
                        solana_transaction_status::option_serializer::OptionSerializer::Some(logs) => logs,
                        solana_transaction_status::option_serializer::OptionSerializer::None => Vec::new(),
                        solana_transaction_status::option_serializer::OptionSerializer::Skip => Vec::new(),
                    }
                } else {
                    Vec::new()
                };
                
                Ok::<Vec<String>, anyhow::Error>(logs)
            }
        }).await??;
        
        Ok(logs)
    }
}

/// Batch settlement data structure (matches verifier program)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchSettlementData {
    pub batch_id: u64,
    pub sequencer_nonce: u64,
    pub bets: Vec<BetSettlement>,
}

/// Individual bet settlement (matches verifier program)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BetSettlement {
    pub bet_id: u64,
    pub user: Pubkey,
    pub bet_amount: u64,
    pub user_guess: u8, // 0 or 1 for coin flip
    pub outcome: u8,    // 0 or 1 actual outcome
    pub payout: u64,    // Calculated payout amount
}

/// Settlement transaction result
#[derive(Debug)]
pub struct SettlementResult {
    pub signature: Signature,
    pub batch_id: u64,
    pub bets_count: usize,
    pub transaction_logs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_config() {
        let config = SolanaConfig::default();
        assert_eq!(config.rpc_url, "http://127.0.0.1:8899");
        assert_eq!(config.retry_attempts, 3);
        
        let testnet_config = SolanaConfig::testnet();
        assert_eq!(testnet_config.rpc_url, "https://api.testnet.solana.com");
    }

    #[test]
    fn test_batch_settlement_data() {
        let batch = BatchSettlementData {
            batch_id: 123,
            sequencer_nonce: 456,
            bets: vec![
                BetSettlement {
                    bet_id: 1,
                    user: Pubkey::new_unique(),
                    bet_amount: 1000,
                    user_guess: 1,
                    outcome: 1,
                    payout: 2000,
                },
            ],
        };
        
        assert_eq!(batch.batch_id, 123);
        assert_eq!(batch.bets.len(), 1);
    }

    #[test]
    fn test_keypair_generation() {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();
        assert_ne!(pubkey, Pubkey::default());
    }
}