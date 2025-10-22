use anyhow::Result;
use axum::{
    async_trait,
    extract::{FromRequest, Path, Request, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use clap::Parser;
use parking_lot::Mutex;
use rand::Rng;
use serde::{Deserialize, Serialize};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

mod database;
use database::{Bet, Database, DatabaseError, PlayerBalance};

mod settlement_persistence;
use settlement_persistence::{SettlementBatchStatus, SettlementPersistence};

mod oracle;
use oracle::{OracleClient, OracleConfig, OracleManager};

mod vrf;
use vrf::VRFKeypair;

mod solana;
use solana::{BatchSettlementData, BetSettlement, SolanaClient, SolanaConfig};

mod settlement_prover;
use settlement_prover::{SettlementProver, SettlementProverConfig};

// Settlement queue for ZK proof batching (VF Node pattern)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SettlementItem {
    pub bet_id: String,
    pub player_address: String,
    pub amount: i64,
    pub payout: i64,
    pub timestamp: DateTime<Utc>,
}

// Oracle proof data structure (future integration)
#[derive(Debug, Clone)]
pub struct OracleProofData {
    pub proof_hash: String,
    pub timestamp: DateTime<Utc>,
    pub verified: bool,
}

// Settlement queue statistics
#[derive(Debug, Clone)]
pub struct SettlementStats {
    pub total_items_queued: Arc<AtomicU64>,
    pub total_batches_processed: Arc<AtomicU64>,
    pub items_in_current_batch: Arc<AtomicU64>,
    pub last_batch_processed_at: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl SettlementStats {
    pub fn new() -> Self {
        Self {
            total_items_queued: Arc::new(AtomicU64::new(0)),
            total_batches_processed: Arc::new(AtomicU64::new(0)),
            items_in_current_batch: Arc::new(AtomicU64::new(0)),
            last_batch_processed_at: Arc::new(Mutex::new(None)),
        }
    }
}

// High-performance channels for background processing
pub type SettlementSender = mpsc::UnboundedSender<SettlementItem>;
pub type SettlementReceiver = mpsc::UnboundedReceiver<SettlementItem>;

#[derive(Parser)]
#[command(name = "sequencer")]
#[command(about = "ZK Casino Sequencer Service")]
pub struct Args {
    #[arg(short, long, default_value = "3000")]
    pub port: u16,

    #[arg(short, long, default_value = "sqlite:zkcasino.db")]
    pub database_url: String,

    #[arg(long, default_value = "vrf-keypair.json")]
    pub vrf_keypair_path: String,

    #[arg(long)]
    pub enable_vrf: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub settlement_sender: SettlementSender,
    pub oracle_client: OracleClient,
    pub settlement_stats: SettlementStats,
    pub solana_client: Option<Arc<SolanaClient>>, // Optional for Phase 2 testing
    pub settlement_prover: Option<Arc<SettlementProver>>, // Phase 3e: ZK proof generation
    pub settlement_persistence: Arc<SettlementPersistence>, // Phase 3e: Crash-safe queue
    pub vrf_keypair: Option<Arc<VRFKeypair>>, // Phase 4a: VRF keypair for verifiable randomness
    pub bet_counter: Arc<AtomicU64>, // Unique counter for VRF nonces
}

#[derive(Deserialize, Serialize)]
pub struct BetRequest {
    pub player_address: String,
    pub amount: u64,
    pub guess: bool, // true for heads, false for tails
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BetResponse {
    pub bet_id: String,
    pub player_address: String,
    pub amount: u64,
    pub guess: bool,
    pub result: bool,
    pub won: bool,
    pub payout: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct DepositRequest {
    pub player_address: String,
    pub amount: u64,
}

#[derive(Serialize, Deserialize)]
pub struct WithdrawRequest {
    pub player_address: String,
    pub amount: u64,
}

#[derive(Serialize, Deserialize)]
pub struct BalanceResponse {
    pub player_address: String,
    pub balance: u64,
    pub total_deposited: u64,
    pub total_withdrawn: u64,
    pub total_wagered: u64,
    pub total_won: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct BetsResponse {
    pub bets: Vec<BetResponse>,
    pub total_count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

// Custom JSON extractor that returns 400 instead of 422 for JSON errors
pub struct CustomJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for CustomJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(CustomJson(value)),
            Err(_) => Err(StatusCode::BAD_REQUEST), // Return 400 instead of 422
        }
    }
}

impl From<&PlayerBalance> for BalanceResponse {
    fn from(balance: &PlayerBalance) -> Self {
        Self {
            player_address: balance.player_address.clone(),
            balance: balance.balance as u64,
            total_deposited: balance.total_deposited as u64,
            total_withdrawn: balance.total_withdrawn as u64,
            total_wagered: balance.total_wagered as u64,
            total_won: balance.total_won as u64,
            created_at: balance.created_at,
            updated_at: balance.updated_at,
        }
    }
}

impl From<&Bet> for BetResponse {
    fn from(bet: &Bet) -> Self {
        Self {
            bet_id: bet.id.clone(),
            player_address: bet.player_address.clone(),
            amount: bet.amount as u64,
            guess: bet.guess,
            result: bet.result,
            won: bet.won,
            payout: bet.payout as u64,
            timestamp: bet.timestamp,
        }
    }
}

pub fn create_app(state: AppState) -> Router {
    // Configure CORS to allow requests from the frontend
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/v1/bet", post(bet_handler))
        .route("/v1/balance/:address", get(get_balance))
        .route("/v1/deposit", post(deposit_handler))
        .route("/v1/withdraw", post(withdraw_handler))
        .route("/v1/bets/:address", get(get_player_bets))
        .route("/v1/recent-bets", get(get_recent_bets))
        .route("/v1/settlement-stats", get(get_settlement_stats))
        .layer(cors)
        .with_state(state)
}

pub async fn health_check() -> &'static str {
    "OK"
}

// Settlement batch processor for ZK proof preparation (VF Node pattern)
async fn process_settlement_batch(
    batch: &[SettlementItem],
    stats: &SettlementStats,
    solana_client: Option<Arc<SolanaClient>>,
    settlement_prover: Option<Arc<SettlementProver>>,
    settlement_persistence: Arc<SettlementPersistence>,
) {
    let start_time = std::time::Instant::now();

    tracing::info!(
        "Processing settlement batch of {} items for ZK proof generation",
        batch.len()
    );

    // Update statistics
    let batch_id = stats
        .total_batches_processed
        .fetch_add(1, Ordering::Relaxed)
        + 1;

    // Phase 3e: Save batch to persistent storage for crash safety
    let batch_id_str = format!("batch_{}", batch_id);
    let actual_batch_id = match settlement_persistence
        .save_batch(&batch_id_str, batch.to_vec())
        .await
    {
        Ok(id) => id,
        Err(e) => {
            error!(
                "Failed to save batch {} to persistent storage: {}",
                batch_id, e
            );
            return; // Don't process if we can't persist (crash safety requirement)
        }
    };

    stats
        .items_in_current_batch
        .fetch_sub(batch.len() as u64, Ordering::Relaxed);
    *stats.last_batch_processed_at.lock() = Some(Utc::now());

    // Phase 3e: Generate ZK proof if prover is available
    let proof_data = if let Some(settlement_prover) = settlement_prover {
        info!(
            "Generating ZK proof for batch {} with {} items",
            actual_batch_id,
            batch.len()
        );

        match settlement_prover.generate_proof(batch).await {
            Ok(proof) => {
                info!("ZK proof generated successfully for batch {}", actual_batch_id);

                // Verify the proof for testing
                match settlement_prover.verify_proof(&proof).await {
                    Ok(true) => {
                        info!("ZK proof verified successfully for batch {}", actual_batch_id);

                        // Convert proof to bytes for Solana submission
                        match proof.to_bytes() {
                            Ok(proof_bytes) => Some(proof_bytes),
                            Err(e) => {
                                error!("Failed to serialize proof for batch {}: {}", actual_batch_id, e);
                                None
                            }
                        }
                    }
                    Ok(false) => {
                        error!("ZK proof verification failed for batch {}", actual_batch_id);
                        None
                    }
                    Err(e) => {
                        error!("Error verifying ZK proof for batch {}: {}", actual_batch_id, e);
                        None
                    }
                }
            }
            Err(e) => {
                error!("Failed to generate ZK proof for batch {}: {}", actual_batch_id, e);
                None
            }
        }
    } else {
        // Fallback to placeholder proof for Phase 2 compatibility
        info!(
            "Using placeholder proof for batch {} (ZK prover not enabled)",
            batch_id
        );
        Some(vec![0u8; 64]) // 64 bytes of zeros
    };

    // Submit to Solana if client is available
    if let Some(solana_client) = solana_client {
        if let Some(proof_bytes) = proof_data {
            match submit_batch_to_solana_with_proof(&*solana_client, actual_batch_id, batch, &proof_bytes)
                .await
            {
                Ok(signature) => {
                    info!(
                        "Batch {} submitted to Solana successfully with proof: {}",
                        actual_batch_id, signature
                    );
                    
                    // Store the transaction signature in settlement persistence
                    if let Err(e) = settlement_persistence.store_transaction(actual_batch_id, &signature.to_string()).await {
                        error!("Failed to store transaction signature for batch {}: {}", actual_batch_id, e);
                    } else {
                        info!("Transaction signature stored for batch {}: {}", actual_batch_id, signature);
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to submit batch {} to Solana: {}. Continuing with local processing.",
                        actual_batch_id, e
                    );
                }
            }
        } else {
            error!(
                "No proof available for batch {}, skipping Solana submission",
                actual_batch_id
            );
        }
    } else {
        // For testing: store a mock transaction signature when Solana is not available
        info!("Solana not available, storing mock transaction signature for batch {}", actual_batch_id);
        let mock_signature = format!("mock_tx_{}_confirmed", actual_batch_id);
        if let Err(e) = settlement_persistence.store_transaction(actual_batch_id, &mock_signature).await {
            error!("Failed to store mock transaction signature for batch {}: {}", actual_batch_id, e);
        } else {
            info!("Mock transaction signature stored for batch {}: {}", actual_batch_id, mock_signature);
        }
    }

    // Log batch details for debugging
    for item in batch {
        tracing::debug!(
            "Settlement item: bet_id={}, player={}, amount={}, payout={}",
            item.bet_id,
            item.player_address,
            item.amount,
            item.payout
        );
    }

    // Simulate batch processing time (prepare for actual ZK proof computation)
    tokio::task::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
    })
    .await
    .ok();

    // Phase 3e: Mark batch as completed in persistent storage
    let actual_batch_id_str = format!("batch_{}", actual_batch_id);
    if let Err(e) = settlement_persistence.mark_completed(&actual_batch_id_str).await {
        error!("Failed to mark batch {} as completed: {}", actual_batch_id, e);
        // Continue anyway - the batch was processed successfully
    }

    tracing::info!(
        "Settlement batch {} processed and persisted in {}μs (ready for oracle/ZK integration)",
        actual_batch_id,
        start_time.elapsed().as_micros()
    );
}

/// Submit settlement batch to Solana (Phase 2 implementation)
async fn submit_batch_to_solana(
    solana_client: &SolanaClient,
    batch_id: u64,
    batch: &[SettlementItem],
) -> Result<solana_sdk::signature::Signature> {
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    // Convert settlement items to Solana batch format
    let bet_settlements: Vec<BetSettlement> = batch
        .iter()
        .enumerate()
        .map(|(i, item)| {
            // Parse user address (in real implementation, this would be validated)
            let user =
                Pubkey::from_str(&item.player_address).unwrap_or_else(|_| Pubkey::new_unique());

            // Determine outcome and payout logic
            let is_win = item.payout > item.amount.abs() as i64;
            let bet_amount = item.amount.abs() as u64;
            let payout = if is_win { item.payout as u64 } else { 0 };

            BetSettlement {
                bet_id: batch_id * 1000 + i as u64,
                user,
                bet_amount,
                user_guess: if is_win { 1 } else { 0 }, // Simplified for Phase 2
                outcome: if is_win { 1 } else { 0 },
                payout,
            }
        })
        .collect();

    let batch_data = BatchSettlementData {
        batch_id,
        sequencer_nonce: batch_id,
        bets: bet_settlements,
    };

    // Create placeholder proof for Phase 2
    let proof = vec![0u8; 64]; // 64 bytes of zeros

    // Submit to Solana
    solana_client
        .submit_settlement_batch(batch_data, proof)
        .await
}

/// Submit settlement batch to Solana with ZK proof (Phase 3e implementation)
async fn submit_batch_to_solana_with_proof(
    solana_client: &SolanaClient,
    batch_id: u64,
    batch: &[SettlementItem],
    proof_data: &[u8],
) -> Result<solana_sdk::signature::Signature> {
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    info!(
        "Submitting batch {} with ZK proof to Solana (size: {} bytes)",
        batch_id,
        proof_data.len()
    );

    // Convert settlement items to Solana batch format
    let bet_settlements: Vec<BetSettlement> = batch
        .iter()
        .enumerate()
        .map(|(i, item)| {
            // Parse user address (in real implementation, this would be validated)
            let user =
                Pubkey::from_str(&item.player_address).unwrap_or_else(|_| Pubkey::new_unique());

            // Convert bet_id string to u64 by hashing
            let bet_id = i as u64; // Simple index-based ID for batch

            // Determine outcome from payout (won if payout > 0)
            let won = item.payout > 0;
            let user_guess = 1u8; // Default guess (in real system this would be stored)
            let outcome = if won { user_guess } else { 1 - user_guess };

            BetSettlement {
                bet_id,
                user,
                bet_amount: item.amount as u64,
                user_guess,
                outcome,
                payout: item.payout as u64,
            }
        })
        .collect();

    let batch_data = BatchSettlementData {
        batch_id,
        sequencer_nonce: batch_id, // Use batch_id as nonce
        bets: bet_settlements,
    };

    // Submit to Solana with real ZK proof
    solana_client
        .submit_settlement_batch(batch_data, proof_data.to_vec())
        .await
}

pub async fn bet_handler(
    State(state): State<AppState>,
    CustomJson(bet_request): CustomJson<BetRequest>,
) -> Result<Json<BetResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    tracing::debug!("Bet request received: player={}, amount={}, guess={}", 
                   bet_request.player_address, bet_request.amount, bet_request.guess);

    // Validate bet amount (minimum 1000 lamports = 0.000001 SOL)
    if bet_request.amount < 1000 {
        tracing::debug!("Bet rejected: amount {} too small", bet_request.amount);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Generate unique bet ID
    let bet_id = format!("bet_{}", Uuid::new_v4().simple());

    // Get unique nonce for VRF (atomic increment for thread safety)
    let sequencer_nonce = state.bet_counter.fetch_add(1, Ordering::Relaxed);

    // Generate coin flip outcome using VRF or fallback to CSPRNG
    let coin_result = if let Some(vrf_keypair) = &state.vrf_keypair {
        // Use VRF for deterministic, verifiable randomness
        use crate::vrf::create_vrf_proof_from_string;
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        // Parse player address to public key bytes
        let user_pubkey = Pubkey::from_str(&bet_request.player_address)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let user_bytes = user_pubkey.to_bytes();

        // Clone necessary data for the blocking task
        let vrf_keypair_clone = vrf_keypair.clone();
        let bet_id_clone = bet_id.clone();
        let player_address_clone = bet_request.player_address.clone();

        tracing::debug!("Starting VRF proof generation for bet_id={}, nonce={}", bet_id, sequencer_nonce);

        // Generate VRF proof in blocking task with timeout to avoid blocking async runtime
        let vrf_proof_future = tokio::task::spawn_blocking(move || {
            tracing::debug!("VRF blocking task started for bet_id={}, nonce={}", bet_id_clone, sequencer_nonce);
            let result = create_vrf_proof_from_string(
                &vrf_keypair_clone,
                &bet_id_clone,
                &user_bytes,
                sequencer_nonce,
            );
            tracing::debug!("VRF blocking task completed for bet_id={}, result={:?}", bet_id_clone, result.is_ok());
            result
        });

        let vrf_proof = tokio::time::timeout(std::time::Duration::from_secs(5), vrf_proof_future)
            .await
            .map_err(|_| {
                error!("VRF task timed out after 5 seconds for bet_id={}, nonce={}", bet_id, sequencer_nonce);
                StatusCode::REQUEST_TIMEOUT
            })?
            .map_err(|e| {
                error!("VRF task spawn failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map_err(|e| {
                error!("VRF proof generation failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Log VRF details for auditability
        info!(
            "VRF: bet_id={}, user={}, nonce={}, outcome={}, signature={}",
            bet_id,
            player_address_clone,
            sequencer_nonce,
            vrf_proof.outcome_string(),
            hex::encode(&vrf_proof.signature[..8]) // First 8 bytes for brevity
        );

        vrf_proof.outcome
    } else {
        // Fallback to CSPRNG when VRF is disabled
        let coin_result = tokio::task::spawn_blocking(move || {
            let mut rng = rand::thread_rng();
            rng.gen::<bool>()
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        info!(
            "CSPRNG: bet_id={}, user={}, outcome={}",
            bet_id,
            bet_request.player_address,
            if coin_result { "heads" } else { "tails" }
        );

        coin_result
    };

    // Determine if player won
    let won = bet_request.guess == coin_result;

    // Calculate payout (2x for winning, 0 for losing)
    let payout = if won { bet_request.amount * 2 } else { 0 };

    // Create immediate response (VF Node instant response pattern)
    let response = BetResponse {
        bet_id: bet_id.clone(),
        player_address: bet_request.player_address.clone(),
        amount: bet_request.amount,
        guess: bet_request.guess,
        result: coin_result,
        won,
        payout,
        timestamp: Utc::now(),
    };

    // Background processing: Save bet and update balances (non-blocking)
    let state_clone = state.clone();
    let response_clone = response.clone();
    tokio::spawn(async move {
        let processing_time = start_time.elapsed();

        // Create bet record
        let bet = Bet {
            id: bet_id.clone(),
            player_address: bet_request.player_address.clone(),
            amount: bet_request.amount as i64,
            guess: bet_request.guess,
            result: coin_result,
            won,
            payout: payout as i64,
            timestamp: response_clone.timestamp,
        };

        // Save bet to database (background)
        if let Err(e) = state_clone.db.save_bet(&bet).await {
            tracing::error!("Failed to save bet {}: {}", bet.id, e);
        }

        // Update player balance (background) - prepare for oracle/ZK processing
        if let Err(e) = state_clone
            .db
            .update_player_balance_after_bet(
                &bet_request.player_address,
                bet_request.amount as i64,
                payout as i64,
            )
            .await
        {
            tracing::error!(
                "Failed to update balance for player {}: {}",
                bet_request.player_address,
                e
            );
        }

        // Add to settlement queue for ZK proof batching (VF Node pattern)
        let settlement_item = SettlementItem {
            bet_id: bet_id.clone(),
            player_address: bet_request.player_address.clone(),
            amount: bet_request.amount as i64,
            payout: payout as i64,
            timestamp: response_clone.timestamp,
        };

        // Update settlement statistics
        state_clone
            .settlement_stats
            .total_items_queued
            .fetch_add(1, Ordering::Relaxed);
        state_clone
            .settlement_stats
            .items_in_current_batch
            .fetch_add(1, Ordering::Relaxed);

        if let Err(e) = state_clone.settlement_sender.send(settlement_item) {
            tracing::error!("Failed to queue settlement item for bet {}: {}", bet_id, e);
        }

        tracing::info!(
            "Bet {} processed in {}μs (background)",
            bet.id,
            processing_time.as_micros()
        );
    });

    // Instant response to client (VF Node pattern)
    Ok(Json(response))
}

pub async fn get_balance(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let balance = state.db.get_player_balance(&address).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    match balance {
        Some(balance) => Ok(Json(BalanceResponse::from(&balance))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Player not found".to_string(),
            }),
        )),
    }
}

pub async fn deposit_handler(
    State(state): State<AppState>,
    CustomJson(deposit_request): CustomJson<DepositRequest>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    if deposit_request.amount == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deposit amount must be greater than 0".to_string(),
            }),
        ));
    }

    let balance = state
        .db
        .deposit(
            &deposit_request.player_address,
            deposit_request.amount as i64,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to deposit: {}", e),
                }),
            )
        })?;

    Ok(Json(BalanceResponse::from(&balance)))
}

pub async fn withdraw_handler(
    State(state): State<AppState>,
    CustomJson(withdraw_request): CustomJson<WithdrawRequest>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    if withdraw_request.amount == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Withdrawal amount must be greater than 0".to_string(),
            }),
        ));
    }

    let balance = state
        .db
        .withdraw(
            &withdraw_request.player_address,
            withdraw_request.amount as i64,
        )
        .await
        .map_err(|e| match e {
            DatabaseError::PlayerNotFound(_) => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Player not found".to_string(),
                }),
            ),
            DatabaseError::InsufficientBalance {
                required,
                available,
            } => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "Insufficient balance. Required: {}, Available: {}",
                        required, available
                    ),
                }),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to withdraw: {}", e),
                }),
            ),
        })?;

    Ok(Json(BalanceResponse::from(&balance)))
}

pub async fn get_player_bets(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<BetsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let bets = state
        .db
        .get_player_bets(&address, Some(50))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    let bet_responses: Vec<BetResponse> = bets.iter().map(BetResponse::from).collect();

    Ok(Json(BetsResponse {
        total_count: bet_responses.len(),
        bets: bet_responses,
    }))
}

pub async fn get_recent_bets(
    State(state): State<AppState>,
) -> Result<Json<BetsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let bets = state.db.get_recent_bets(Some(50)).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let bet_responses: Vec<BetResponse> = bets.iter().map(BetResponse::from).collect();

    Ok(Json(BetsResponse {
        total_count: bet_responses.len(),
        bets: bet_responses,
    }))
}

#[derive(Serialize)]
pub struct SettlementStatsResponse {
    pub total_items_queued: u64,
    pub total_batches_processed: u64,
    pub items_in_current_batch: u64,
    pub last_batch_processed_at: Option<DateTime<Utc>>,
    pub queue_status: String,
}

pub async fn get_settlement_stats(
    State(state): State<AppState>,
) -> Result<Json<SettlementStatsResponse>, StatusCode> {
    let stats = &state.settlement_stats;

    let response = SettlementStatsResponse {
        total_items_queued: stats.total_items_queued.load(Ordering::Relaxed),
        total_batches_processed: stats.total_batches_processed.load(Ordering::Relaxed),
        items_in_current_batch: stats.items_in_current_batch.load(Ordering::Relaxed),
        last_batch_processed_at: *stats.last_batch_processed_at.lock(),
        queue_status: "active".to_string(),
    };

    Ok(Json(response))
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // Initialize database
    let db = Database::new(&args.database_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;

    db.create_tables()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create database tables: {}", e))?;

    // Initialize settlement persistence for crash-safe queue (Phase 3e requirement)
    info!("Initializing settlement persistence for crash-safe queue...");
    let settlement_persistence = Arc::new(
        SettlementPersistence::new(&args.database_url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize settlement persistence: {}", e))?,
    );

    // Phase 3e: Crash recovery - process any pending batches from previous runs
    info!("Checking for pending settlement batches to recover...");
    let pending_batches = settlement_persistence
        .get_pending_batches()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get pending batches: {}", e))?;

    if !pending_batches.is_empty() {
        info!(
            "Found {} pending batches from previous run, starting recovery...",
            pending_batches.len()
        );

        for batch in pending_batches {
            info!(
                "Recovering batch {} with status {:?}",
                batch.batch_id, batch.status
            );

            // TODO: Implement actual recovery logic based on batch status
            // For now, just log the recovery - full implementation would:
            // - Re-process pending batches
            // - Re-submit proved batches
            // - Verify submitted batches on-chain
            match batch.status {
                SettlementBatchStatus::Pending => {
                    info!("Batch {} needs re-processing", batch.batch_id);
                }
                SettlementBatchStatus::Proving => {
                    info!("Batch {} was interrupted during proving", batch.batch_id);
                }
                SettlementBatchStatus::Proved => {
                    info!("Batch {} has proof ready for submission", batch.batch_id);
                }
                SettlementBatchStatus::Submitted => {
                    info!(
                        "Batch {} was submitted, checking on-chain status",
                        batch.batch_id
                    );
                }
                _ => {}
            }
        }
    } else {
        info!("No pending batches found, starting fresh");
    }

    // Phase 4a: Initialize VRF keypair for verifiable randomness
    let vrf_keypair = if args.enable_vrf {
        info!("Initializing VRF keypair for verifiable randomness...");
        match VRFKeypair::load_or_generate("VRF_KEYPAIR_PATH", &args.vrf_keypair_path) {
            Ok(keypair) => {
                info!("VRF keypair loaded successfully. Public key: {:?}", 
                      hex::encode(keypair.public_key_bytes()));
                Some(Arc::new(keypair))
            }
            Err(e) => {
                error!("Failed to initialize VRF keypair: {}", e);
                return Err(anyhow::anyhow!("VRF keypair initialization failed: {}", e));
            }
        }
    } else {
        info!("VRF disabled, using CSPRNG for randomness");
        None
    };

    // Initialize settlement queue for ZK proof batching (VF Node pattern)
    let (settlement_sender, settlement_receiver) = mpsc::unbounded_channel();
    let settlement_stats = SettlementStats::new();

    // Initialize oracle manager for proof fetching (as requested by user)
    let oracle_config = OracleConfig::default();
    let oracle_manager = OracleManager::new(oracle_config.clone());
    let oracle_client = OracleClient::new(oracle_config);

    // Start oracle proof fetching service
    oracle_manager
        .start_proof_service()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start oracle service: {}", e))?;

    // Initialize Solana client (Phase 2: localnet first, then testnet)
    let solana_client = if std::env::var("ENABLE_SOLANA").unwrap_or_default() == "true" {
        info!("Initializing Solana client...");

        // Generate or load sequencer keypair (in production, load from secure storage)
        let sequencer_keypair = Keypair::new();
        info!("Sequencer public key: {}", sequencer_keypair.pubkey());

        // Configure for local validator by default, switch to testnet with env var
        let solana_config = if std::env::var("SOLANA_TESTNET").unwrap_or_default() == "true" {
            SolanaConfig::testnet()
        } else {
            SolanaConfig::default() // Local validator
        };

        // Program IDs (these should match the deployed programs)
        let vault_program_id = std::env::var("VAULT_PROGRAM_ID")
            .unwrap_or_else(|_| "11111111111111111111111111111111".to_string());
        let verifier_program_id = std::env::var("VERIFIER_PROGRAM_ID")
            .unwrap_or_else(|_| "11111111111111111111111111111112".to_string());

        match SolanaClient::new(
            solana_config,
            sequencer_keypair,
            &vault_program_id,
            &verifier_program_id,
        ) {
            Ok(client) => {
                info!("Solana client initialized successfully");
                // Test connection
                if let Err(e) = client.health_check().await {
                    warn!(
                        "Solana health check failed: {}. Continuing without Solana integration.",
                        e
                    );
                    None
                } else {
                    Some(Arc::new(client))
                }
            }
            Err(e) => {
                warn!("Failed to initialize Solana client: {}. Continuing without Solana integration.", e);
                None
            }
        }
    } else {
        info!("Solana integration disabled. Set ENABLE_SOLANA=true to enable.");
        None
    };

    // Initialize Settlement Prover for Phase 3e (ZK proof generation)
    let settlement_prover = if std::env::var("ENABLE_ZK_PROOFS").unwrap_or_default() == "true" {
        info!("Initializing Settlement Prover for ZK proof generation...");

        let prover_config = SettlementProverConfig::default();
        match SettlementProver::new(prover_config).await {
            Ok(prover) => {
                // Initialize some demo user balances for testing
                prover.init_user_balance(100, 10000).await;
                prover.init_user_balance(200, 5000).await;
                prover.init_user_balance(300, 15000).await;

                info!("Settlement Prover initialized successfully");
                Some(Arc::new(prover))
            }
            Err(e) => {
                warn!("Failed to initialize Settlement Prover: {}. Continuing with placeholder proofs.", e);
                None
            }
        }
    } else {
        info!("ZK proof generation disabled. Set ENABLE_ZK_PROOFS=true to enable real proof generation.");
        None
    };

    let state = AppState {
        db: Arc::new(db),
        settlement_sender,
        oracle_client,
        settlement_stats: settlement_stats.clone(),
        solana_client,
        settlement_prover,
        settlement_persistence: settlement_persistence.clone(),
        vrf_keypair,
        bet_counter: Arc::new(AtomicU64::new(0)),
    };

    // Settlement processor for ZK proof batching (VF Node background pattern)
    let stats_clone = settlement_stats.clone();
    let solana_client_clone = state.solana_client.clone();
    let settlement_prover_clone = state.settlement_prover.clone();
    let settlement_persistence_clone = state.settlement_persistence.clone();
    let _settlement_processor_handle = tokio::spawn(async move {
        let mut settlement_receiver = settlement_receiver;
        let mut batch = Vec::new();
        let mut interval = interval(Duration::from_millis(100)); // 100ms batching window

        loop {
            tokio::select! {
                // Receive settlement items
                item = settlement_receiver.recv() => {
                    match item {
                        Some(settlement_item) => {
                            // Phase 3e: Check for deduplication before adding to batch
                            match settlement_persistence_clone.is_bet_processed(&settlement_item.bet_id).await {
                                Ok(already_processed) => {
                                    if already_processed {
                                        warn!("Bet {} already processed, skipping to prevent double settlement", settlement_item.bet_id);
                                        continue;
                                    }

                                    // Add to batch if not already processed
                                    batch.push(settlement_item);

                                    // Process batch when it reaches size limit (prepare for ZK rollup)
                                    if batch.len() >= 50 {
                                        process_settlement_batch(&batch, &stats_clone, solana_client_clone.clone(), settlement_prover_clone.clone(), settlement_persistence_clone.clone()).await;
                                        batch.clear();
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to check if bet {} is already processed: {}. Proceeding anyway.", settlement_item.bet_id, e);
                                    // If deduplication check fails, proceed anyway to avoid blocking settlement
                                    batch.push(settlement_item);
                                    if batch.len() >= 50 {
                                        process_settlement_batch(&batch, &stats_clone, solana_client_clone.clone(), settlement_prover_clone.clone(), settlement_persistence_clone.clone()).await;
                                        batch.clear();
                                    }
                                }
                            }
                        }
                        None => break, // Channel closed
                    }
                }

                // Process batch on timer (ensure regular processing)
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        process_settlement_batch(&batch, &stats_clone, solana_client_clone.clone(), settlement_prover_clone.clone(), settlement_persistence_clone.clone()).await;
                        batch.clear();
                    }
                }
            }
        }
    });

    let app = create_app(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    info!("Sequencer listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt; // for `oneshot`

    async fn setup_test_app() -> (Router, AppState) {
        let db = Database::new("").await.unwrap();
        db.create_tables().await.unwrap();

        // Initialize test settlement persistence
        let settlement_persistence = Arc::new(
            SettlementPersistence::new("sqlite::memory:")
                .await
                .expect("Failed to initialize test settlement persistence"),
        );

        let (settlement_sender, _) = mpsc::unbounded_channel();
        let oracle_config = OracleConfig::default();
        let oracle_client = OracleClient::new(oracle_config);
        let settlement_stats = SettlementStats::new();

        let state = AppState {
            db: Arc::new(db),
            settlement_sender,
            oracle_client,
            settlement_stats,
            solana_client: None,     // No Solana client for tests
            settlement_prover: None, // No ZK prover for tests
            settlement_persistence,
            vrf_keypair: None,       // No VRF keypair for tests
            bet_counter: Arc::new(AtomicU64::new(0)),
        };

        let app = create_app(state.clone());
        (app, state)
    }

    #[tokio::test]
    async fn test_health_check() {
        let (app, _state) = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"OK");
    }

    #[tokio::test]
    async fn test_deposit_and_balance() {
        let (app, _state) = setup_test_app().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        // Test deposit
        let deposit_request = DepositRequest {
            player_address: player_address.to_string(),
            amount: 10000,
        };

        let request_body = serde_json::to_string(&deposit_request).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/deposit")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let balance_response: BalanceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(balance_response.balance, 10000);
        assert_eq!(balance_response.total_deposited, 10000);
    }

    #[tokio::test]
    async fn test_bet_with_balance() {
        let (app, state) = setup_test_app().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        // First deposit funds
        state.db.deposit(player_address, 10000).await.unwrap();

        // Then place bet
        let bet_request = BetRequest {
            player_address: player_address.to_string(),
            amount: 5000,
            guess: true,
        };

        let request_body = serde_json::to_string(&bet_request).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/bet")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let bet_response: BetResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(bet_response.player_address, player_address);
        assert_eq!(bet_response.amount, 5000);
        assert_eq!(bet_response.guess, true);
        assert!(bet_response.bet_id.starts_with("bet_"));

        // Check payout logic
        if bet_response.won {
            assert_eq!(bet_response.payout, 10000);
        } else {
            assert_eq!(bet_response.payout, 0);
        }
    }

    #[tokio::test]
    async fn test_bet_insufficient_balance() {
        let (app, _state) = setup_test_app().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        // Try to bet without depositing first
        let bet_request = BetRequest {
            player_address: player_address.to_string(),
            amount: 5000,
            guess: true,
        };

        let request_body = serde_json::to_string(&bet_request).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/bet")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error_response: ErrorResponse = serde_json::from_slice(&body).unwrap();
        assert!(error_response.error.contains("Player not found"));
    }

    #[tokio::test]
    async fn test_withdraw() {
        let (app, state) = setup_test_app().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        // First deposit funds
        state.db.deposit(player_address, 10000).await.unwrap();

        // Then withdraw
        let withdraw_request = WithdrawRequest {
            player_address: player_address.to_string(),
            amount: 3000,
        };

        let request_body = serde_json::to_string(&withdraw_request).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/withdraw")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let balance_response: BalanceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(balance_response.balance, 7000);
        assert_eq!(balance_response.total_withdrawn, 3000);
    }

    #[tokio::test]
    async fn test_get_balance() {
        let (app, state) = setup_test_app().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        // First deposit funds
        state.db.deposit(player_address, 5000).await.unwrap();

        // Get balance
        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/v1/balance/{}", player_address))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let balance_response: BalanceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(balance_response.balance, 5000);
        assert_eq!(balance_response.player_address, player_address);
    }

    #[tokio::test]
    async fn test_get_player_bets() {
        let (app, state) = setup_test_app().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        // Create test bets
        for i in 0..3 {
            let bet = Bet {
                id: format!("test_bet_{}", i),
                player_address: player_address.to_string(),
                amount: 1000,
                guess: true,
                result: false,
                won: false,
                payout: 0,
                timestamp: Utc::now(),
            };
            state.db.save_bet(&bet).await.unwrap();
        }

        // Get player bets
        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/v1/bets/{}", player_address))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let bets_response: BetsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(bets_response.total_count, 3);
    }

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(&[
            "sequencer",
            "--port",
            "8080",
            "--database-url",
            "sqlite:test.db",
        ]);
        assert_eq!(args.port, 8080);
        assert_eq!(args.database_url, "sqlite:test.db");

        let args = Args::parse_from(&["sequencer"]);
        assert_eq!(args.port, 3000); // default value
        assert_eq!(args.database_url, "sqlite:zkcasino.db"); // default value
    }

    #[test]
    fn test_health_check_function() {
        let result = tokio_test::block_on(health_check());
        assert_eq!(result, "OK");
    }
}
