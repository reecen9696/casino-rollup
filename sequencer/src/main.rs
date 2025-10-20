use anyhow::Result;
use axum::{
    extract::{State, Path},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::info;
use chrono::{DateTime, Utc};
use rand::Rng;
use uuid::Uuid;

mod database;
use database::{Database, Bet, PlayerBalance, DatabaseError};

mod oracle;
use oracle::{OracleManager, OracleConfig, OracleClient};

// Settlement queue for ZK proof batching (VF Node pattern)
#[derive(Debug, Clone)]
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
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub settlement_sender: SettlementSender,
    pub oracle_client: OracleClient,
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
    Router::new()
        .route("/health", get(health_check))
        .route("/v1/bet", post(bet_handler))
        .route("/v1/balance/:address", get(get_balance))
        .route("/v1/deposit", post(deposit_handler))
        .route("/v1/withdraw", post(withdraw_handler))
        .route("/v1/bets/:address", get(get_player_bets))
        .route("/v1/recent-bets", get(get_recent_bets))
        .with_state(state)
}

pub async fn health_check() -> &'static str {
    "OK"
}

// Settlement batch processor for ZK proof preparation (VF Node pattern)
async fn process_settlement_batch(batch: &[SettlementItem]) {
    let start_time = std::time::Instant::now();
    
    tracing::info!(
        "Processing settlement batch of {} items for ZK proof generation", 
        batch.len()
    );
    
    // TODO: Future ZK proof generation and oracle verification
    // For now, log the batch processing for oracle integration readiness
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
    }).await.ok();
    
    tracing::info!(
        "Settlement batch processed in {}μs (ready for oracle/ZK integration)", 
        start_time.elapsed().as_micros()
    );
}

pub async fn bet_handler(
    State(state): State<AppState>,
    Json(bet_request): Json<BetRequest>,
) -> Result<Json<BetResponse>, StatusCode> {
    let start_time = std::time::Instant::now();
    
    // Validate bet amount (minimum 1000 lamports = 0.000001 SOL)
    if bet_request.amount < 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // CPU-intensive random generation in background thread (VF Node pattern)
    let coin_result = tokio::task::spawn_blocking(move || {
        let mut rng = rand::thread_rng();
        rng.gen::<bool>()
    }).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate unique bet ID
    let bet_id = format!("bet_{}", Uuid::new_v4().simple());

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
        if let Err(e) = state_clone.db.update_player_balance_after_bet(
            &bet_request.player_address,
            bet_request.amount as i64,
            payout as i64,
        ).await {
            tracing::error!("Failed to update balance for player {}: {}", bet_request.player_address, e);
        }

        // Add to settlement queue for ZK proof batching (VF Node pattern)
        let settlement_item = SettlementItem {
            bet_id: bet_id.clone(),
            player_address: bet_request.player_address.clone(),
            amount: bet_request.amount as i64,
            payout: payout as i64,
            timestamp: response_clone.timestamp,
        };
        
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
    let balance = state.db.get_player_balance(&address).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        ))?;

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
    Json(deposit_request): Json<DepositRequest>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    if deposit_request.amount == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deposit amount must be greater than 0".to_string(),
            }),
        ));
    }

    let balance = state.db.deposit(&deposit_request.player_address, deposit_request.amount as i64).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to deposit: {}", e),
            }),
        ))?;

    Ok(Json(BalanceResponse::from(&balance)))
}

pub async fn withdraw_handler(
    State(state): State<AppState>,
    Json(withdraw_request): Json<WithdrawRequest>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    if withdraw_request.amount == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Withdrawal amount must be greater than 0".to_string(),
            }),
        ));
    }

    let balance = state.db.withdraw(&withdraw_request.player_address, withdraw_request.amount as i64).await
        .map_err(|e| match e {
            DatabaseError::PlayerNotFound(_) => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Player not found".to_string(),
                }),
            ),
            DatabaseError::InsufficientBalance { required, available } => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Insufficient balance. Required: {}, Available: {}", required, available),
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
    let bets = state.db.get_player_bets(&address, Some(50)).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        ))?;

    let bet_responses: Vec<BetResponse> = bets.iter().map(BetResponse::from).collect();
    
    Ok(Json(BetsResponse {
        total_count: bet_responses.len(),
        bets: bet_responses,
    }))
}

pub async fn get_recent_bets(
    State(state): State<AppState>,
) -> Result<Json<BetsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let bets = state.db.get_recent_bets(Some(50)).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        ))?;

    let bet_responses: Vec<BetResponse> = bets.iter().map(BetResponse::from).collect();
    
    Ok(Json(BetsResponse {
        total_count: bet_responses.len(),
        bets: bet_responses,
    }))
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // Initialize database
    let db = Database::new(&args.database_url).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;
    
    db.create_tables().await
        .map_err(|e| anyhow::anyhow!("Failed to create database tables: {}", e))?;

    // Initialize settlement queue for ZK proof batching (VF Node pattern)
    let (settlement_sender, settlement_receiver) = mpsc::unbounded_channel();

    // Initialize oracle manager for proof fetching (as requested by user)
    let oracle_config = OracleConfig::default();
    let oracle_manager = OracleManager::new(oracle_config.clone());
    let oracle_client = OracleClient::new(oracle_config);
    
    // Start oracle proof fetching service
    oracle_manager.start_proof_service().await
        .map_err(|e| anyhow::anyhow!("Failed to start oracle service: {}", e))?;

    let state = AppState {
        db: Arc::new(db),
        settlement_sender,
        oracle_client,
    };

    // Settlement processor for ZK proof batching (VF Node background pattern)
    let settlement_processor_handle = tokio::spawn(async move {
        let mut settlement_receiver = settlement_receiver;
        let mut batch = Vec::new();
        let mut interval = interval(Duration::from_millis(100)); // 100ms batching window
        
        loop {
            tokio::select! {
                // Receive settlement items
                item = settlement_receiver.recv() => {
                    match item {
                        Some(settlement_item) => {
                            batch.push(settlement_item);
                            
                            // Process batch when it reaches size limit (prepare for ZK rollup)
                            if batch.len() >= 50 {
                                process_settlement_batch(&batch).await;
                                batch.clear();
                            }
                        }
                        None => break, // Channel closed
                    }
                }
                
                // Process batch on timer (ensure regular processing)
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        process_settlement_batch(&batch).await;
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
        
        let state = AppState {
            db: Arc::new(db),
        };
        
        let app = create_app(state.clone());
        (app, state)
    }

    #[tokio::test]
    async fn test_health_check() {
        let (app, _state) = setup_test_app().await;

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let bets_response: BetsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(bets_response.total_count, 3);
    }

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(&["sequencer", "--port", "8080", "--database-url", "sqlite:test.db"]);
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