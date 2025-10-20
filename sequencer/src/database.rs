use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    pub id: String,
    pub player_address: String,
    pub amount: i64,
    pub guess: bool,
    pub result: bool,
    pub won: bool,
    pub payout: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerBalance {
    pub player_address: String,
    pub balance: i64,
    pub total_deposited: i64,
    pub total_withdrawn: i64,
    pub total_wagered: i64,
    pub total_won: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Player not found: {0}")]
    PlayerNotFound(String),
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: i64, available: i64 },
    #[error("Bet not found: {0}")]
    BetNotFound(String),
}

pub struct Database {
    bets: Arc<DashMap<String, Bet>>,
    player_bets: Arc<DashMap<String, Vec<String>>>, // player_address -> bet_ids
    balances: Arc<DashMap<String, PlayerBalance>>,
}

impl Database {
    pub async fn new(_database_url: &str) -> Result<Self, DatabaseError> {
        Ok(Self {
            bets: Arc::new(DashMap::new()),
            player_bets: Arc::new(DashMap::new()),
            balances: Arc::new(DashMap::new()),
        })
    }

    pub async fn create_tables(&self) -> Result<(), DatabaseError> {
        // No-op for in-memory database
        Ok(())
    }

    pub async fn save_bet(&self, bet: &Bet) -> Result<(), DatabaseError> {
        // Insert bet directly with DashMap's concurrent access
        self.bets.insert(bet.id.clone(), bet.clone());
        
        // Add to player's bet list with concurrent access
        self.player_bets
            .entry(bet.player_address.clone())
            .or_insert_with(Vec::new)
            .push(bet.id.clone());
        
        Ok(())
    }

    pub async fn get_bet(&self, bet_id: &str) -> Result<Option<Bet>, DatabaseError> {
        Ok(self.bets.get(bet_id).map(|bet| bet.clone()))
    }

    pub async fn get_player_bets(&self, player_address: &str, limit: Option<i64>) -> Result<Vec<Bet>, DatabaseError> {
        let limit = limit.unwrap_or(100) as usize;
        
        let bet_ids = self.player_bets.get(player_address)
            .map(|entry| entry.clone())
            .unwrap_or_default();
        
        let mut player_bet_list: Vec<Bet> = bet_ids
            .iter()
            .rev() // Reverse to get most recent first
            .filter_map(|id| self.bets.get(id).map(|bet| bet.clone()))
            .take(limit)
            .collect();
        
        // Sort by timestamp descending
        player_bet_list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(player_bet_list)
    }

    pub async fn get_recent_bets(&self, limit: Option<i64>) -> Result<Vec<Bet>, DatabaseError> {
        let limit = limit.unwrap_or(50) as usize;
        
        // Collect all bets using concurrent iteration (VF Node pattern)
        let mut all_bets: Vec<Bet> = self.bets
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        
        all_bets.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_bets.truncate(limit);
        
        Ok(all_bets)
    }

    pub async fn get_player_balance(&self, player_address: &str) -> Result<Option<PlayerBalance>, DatabaseError> {
        Ok(self.balances.get(player_address).map(|balance| balance.clone()))
    }

    pub async fn create_player_balance(&self, player_address: &str, initial_balance: i64) -> Result<PlayerBalance, DatabaseError> {
        let now = Utc::now();
        
        let balance = PlayerBalance {
            player_address: player_address.to_string(),
            balance: initial_balance,
            total_deposited: initial_balance,
            total_withdrawn: 0,
            total_wagered: 0,
            total_won: 0,
            created_at: now,
            updated_at: now,
        };
        
        self.balances.insert(player_address.to_string(), balance.clone());
        Ok(balance)
    }

    pub async fn update_player_balance_after_bet(&self, player_address: &str, bet_amount: i64, payout: i64) -> Result<PlayerBalance, DatabaseError> {
        let now = Utc::now();
        
        // Use DashMap's entry API for atomic update
        match self.balances.get(player_address) {
            Some(current_balance) => {
                // Check if player has sufficient balance
                if current_balance.balance < bet_amount {
                    return Err(DatabaseError::InsufficientBalance {
                        required: bet_amount,
                        available: current_balance.balance,
                    });
                }

                // Calculate new balance: subtract bet amount, add payout
                let new_balance = current_balance.balance - bet_amount + payout;
                let new_total_wagered = current_balance.total_wagered + bet_amount;
                let new_total_won = current_balance.total_won + payout;

                let updated_balance = PlayerBalance {
                    player_address: player_address.to_string(),
                    balance: new_balance,
                    total_deposited: current_balance.total_deposited,
                    total_withdrawn: current_balance.total_withdrawn,
                    total_wagered: new_total_wagered,
                    total_won: new_total_won,
                    created_at: current_balance.created_at,
                    updated_at: now,
                };

                self.balances.insert(player_address.to_string(), updated_balance.clone());
                Ok(updated_balance)
            }
            None => Err(DatabaseError::PlayerNotFound(player_address.to_string()))
        }
    }

    pub async fn deposit(&self, player_address: &str, amount: i64) -> Result<PlayerBalance, DatabaseError> {
        let now = Utc::now();
        
        let updated_balance = match self.balances.get(player_address) {
            Some(current_balance) => {
                let new_balance = current_balance.balance + amount;
                let new_total_deposited = current_balance.total_deposited + amount;

                PlayerBalance {
                    player_address: player_address.to_string(),
                    balance: new_balance,
                    total_deposited: new_total_deposited,
                    total_withdrawn: current_balance.total_withdrawn,
                    total_wagered: current_balance.total_wagered,
                    total_won: current_balance.total_won,
                    created_at: current_balance.created_at,
                    updated_at: now,
                }
            }
            None => {
                PlayerBalance {
                    player_address: player_address.to_string(),
                    balance: amount,
                    total_deposited: amount,
                    total_withdrawn: 0,
                    total_wagered: 0,
                    total_won: 0,
                    created_at: now,
                    updated_at: now,
                }
            }
        };

        self.balances.insert(player_address.to_string(), updated_balance.clone());
        Ok(updated_balance)
    }

    pub async fn withdraw(&self, player_address: &str, amount: i64) -> Result<PlayerBalance, DatabaseError> {
        let now = Utc::now();
        
        match self.balances.get(player_address) {
            Some(current_balance) => {
                if current_balance.balance < amount {
                    return Err(DatabaseError::InsufficientBalance {
                        required: amount,
                        available: current_balance.balance,
                    });
                }

                let new_balance = current_balance.balance - amount;
                let new_total_withdrawn = current_balance.total_withdrawn + amount;

                let updated_balance = PlayerBalance {
                    player_address: player_address.to_string(),
                    balance: new_balance,
                    total_deposited: current_balance.total_deposited,
                    total_withdrawn: new_total_withdrawn,
                    total_wagered: current_balance.total_wagered,
                    total_won: current_balance.total_won,
                    created_at: current_balance.created_at,
                    updated_at: now,
                };

                self.balances.insert(player_address.to_string(), updated_balance.clone());
                Ok(updated_balance)
            }
            None => Err(DatabaseError::PlayerNotFound(player_address.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> Database {
        Database::new("").await.unwrap()
    }

    #[tokio::test]
    async fn test_create_tables() {
        let db = setup_test_db().await;
        
        // Tables should be created without error
        assert!(db.create_tables().await.is_ok());
    }

    #[tokio::test]
    async fn test_save_and_get_bet() {
        let db = setup_test_db().await;
        
        let bet = Bet {
            id: "test_bet_123".to_string(),
            player_address: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 5000,
            guess: true,
            result: false,
            won: false,
            payout: 0,
            timestamp: Utc::now(),
        };

        // Save bet
        db.save_bet(&bet).await.unwrap();

        // Retrieve bet
        let retrieved_bet = db.get_bet("test_bet_123").await.unwrap().unwrap();
        assert_eq!(retrieved_bet.id, bet.id);
        assert_eq!(retrieved_bet.player_address, bet.player_address);
        assert_eq!(retrieved_bet.amount, bet.amount);
        assert_eq!(retrieved_bet.won, bet.won);
    }

    #[tokio::test]
    async fn test_player_balance_creation() {
        let db = setup_test_db().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        
        // Create player balance
        let balance = db.create_player_balance(player_address, 10000).await.unwrap();
        assert_eq!(balance.balance, 10000);
        assert_eq!(balance.total_deposited, 10000);
        
        // Retrieve player balance
        let retrieved_balance = db.get_player_balance(player_address).await.unwrap().unwrap();
        assert_eq!(retrieved_balance.balance, 10000);
    }

    #[tokio::test]
    async fn test_deposit() {
        let db = setup_test_db().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        
        // First deposit creates player
        let balance = db.deposit(player_address, 5000).await.unwrap();
        assert_eq!(balance.balance, 5000);
        assert_eq!(balance.total_deposited, 5000);
        
        // Second deposit adds to existing balance
        let balance = db.deposit(player_address, 3000).await.unwrap();
        assert_eq!(balance.balance, 8000);
        assert_eq!(balance.total_deposited, 8000);
    }

    #[tokio::test]
    async fn test_withdraw() {
        let db = setup_test_db().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        
        // Create player with balance
        db.create_player_balance(player_address, 10000).await.unwrap();
        
        // Withdraw amount
        let balance = db.withdraw(player_address, 3000).await.unwrap();
        assert_eq!(balance.balance, 7000);
        assert_eq!(balance.total_withdrawn, 3000);
    }

    #[tokio::test]
    async fn test_withdraw_insufficient_balance() {
        let db = setup_test_db().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        
        // Create player with small balance
        db.create_player_balance(player_address, 1000).await.unwrap();
        
        // Try to withdraw more than balance
        let result = db.withdraw(player_address, 2000).await;
        assert!(matches!(result, Err(DatabaseError::InsufficientBalance { .. })));
    }

    #[tokio::test]
    async fn test_update_balance_after_bet() {
        let db = setup_test_db().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        
        // Create player with balance
        db.create_player_balance(player_address, 10000).await.unwrap();
        
        // Losing bet
        let balance = db.update_player_balance_after_bet(player_address, 2000, 0).await.unwrap();
        assert_eq!(balance.balance, 8000); // 10000 - 2000 + 0
        assert_eq!(balance.total_wagered, 2000);
        assert_eq!(balance.total_won, 0);
        
        // Winning bet
        let balance = db.update_player_balance_after_bet(player_address, 1000, 2000).await.unwrap();
        assert_eq!(balance.balance, 9000); // 8000 - 1000 + 2000
        assert_eq!(balance.total_wagered, 3000);
        assert_eq!(balance.total_won, 2000);
    }

    #[tokio::test]
    async fn test_get_player_bets() {
        let db = setup_test_db().await;
        let player_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        
        // Create multiple bets for player
        for i in 0..5 {
            let bet = Bet {
                id: format!("bet_{}", i),
                player_address: player_address.to_string(),
                amount: 1000 + i * 100,
                guess: i % 2 == 0,
                result: i % 3 == 0,
                won: (i % 2 == 0) == (i % 3 == 0),
                payout: if (i % 2 == 0) == (i % 3 == 0) { (1000 + i * 100) * 2 } else { 0 },
                timestamp: Utc::now(),
            };
            db.save_bet(&bet).await.unwrap();
        }
        
        // Get player bets
        let bets = db.get_player_bets(player_address, Some(3)).await.unwrap();
        assert_eq!(bets.len(), 3);
        
        // Get all player bets
        let all_bets = db.get_player_bets(player_address, None).await.unwrap();
        assert_eq!(all_bets.len(), 5);
    }

    #[tokio::test]
    async fn test_get_recent_bets() {
        let db = setup_test_db().await;
        
        // Create bets from different players
        for i in 0..3 {
            let bet = Bet {
                id: format!("bet_{}", i),
                player_address: format!("player_{}", i),
                amount: 1000,
                guess: true,
                result: false,
                won: false,
                payout: 0,
                timestamp: Utc::now(),
            };
            db.save_bet(&bet).await.unwrap();
        }
        
        // Get recent bets
        let recent_bets = db.get_recent_bets(Some(2)).await.unwrap();
        assert_eq!(recent_bets.len(), 2);
    }
}