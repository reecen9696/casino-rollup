use crate::circuits::accounting::{AccountingCircuit, Bet, BetBatch};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during witness generation
#[derive(Error, Debug)]
pub enum WitnessError {
    #[error("Invalid balance: user {user_id} has balance {balance} but bet amount {bet_amount}")]
    InsufficientBalance {
        user_id: u32,
        balance: u64,
        bet_amount: u64,
    },
    #[error("Negative balance detected: user {user_id} would have balance {balance}")]
    NegativeBalance { user_id: u32, balance: i64 },
    #[error("Conservation law violated: user deltas sum to {user_sum}, house delta is {house_delta}")]
    ConservationViolation { user_sum: i64, house_delta: i64 },
    #[error("Empty batch: no bets provided")]
    EmptyBatch,
    #[error("Batch too large: {size} bets exceeds maximum {max_size}")]
    BatchTooLarge { size: usize, max_size: usize },
    #[error("Unknown user: user_id {user_id} not found in initial balances")]
    UnknownUser { user_id: u32 },
}

/// Settlement batch data from the sequencer
#[derive(Debug, Clone)]
pub struct SettlementBatch {
    pub batch_id: u32,
    pub bets: Vec<SettlementBet>,
    pub initial_balances: HashMap<u32, u64>, // user_id -> balance
    pub house_initial_balance: u64,
    pub timestamp: u64, // Unix timestamp when batch was created
}

/// Individual bet in a settlement batch
#[derive(Debug, Clone)]
pub struct SettlementBet {
    pub user_id: u32,
    pub amount: u64,
    pub guess: bool,    // true = heads, false = tails
    pub outcome: bool,  // true = heads, false = tails
    pub bet_id: String, // For tracking/correlation
}

impl SettlementBet {
    pub fn new(user_id: u32, amount: u64, guess: bool, outcome: bool, bet_id: String) -> Self {
        Self {
            user_id,
            amount,
            guess,
            outcome,
            bet_id,
        }
    }

    /// Check if the user won this bet
    pub fn won(&self) -> bool {
        self.guess == self.outcome
    }

    /// Calculate the payout for this bet (2x amount if won, 0 if lost)
    pub fn payout(&self) -> u64 {
        if self.won() {
            self.amount * 2
        } else {
            0
        }
    }

    /// Calculate the delta for this bet (net change to user balance)
    pub fn delta(&self) -> i64 {
        if self.won() {
            self.amount as i64 // Net gain = bet amount (payout is 2x, so net is +amount)
        } else {
            -(self.amount as i64) // Net loss = -bet amount
        }
    }
}

/// Witness generator for accounting circuits
pub struct WitnessGenerator {
    max_batch_size: usize,
    max_users: usize,
}

impl Default for WitnessGenerator {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_users: 10,
        }
    }
}

impl WitnessGenerator {
    pub fn new(max_batch_size: usize, max_users: usize) -> Self {
        Self {
            max_batch_size,
            max_users,
        }
    }

    /// Generate a witness (accounting circuit) from settlement batch data
    pub fn generate_witness(
        &self,
        settlement_batch: &SettlementBatch,
    ) -> Result<AccountingCircuit, WitnessError> {
        // Validate batch
        self.validate_batch(settlement_batch)?;

        // Convert settlement bets to accounting bets and pad to max size
        let mut accounting_bets: Vec<Bet> = settlement_batch
            .bets
            .iter()
            .map(|sb| Bet::new(sb.user_id, sb.amount, sb.guess, sb.outcome))
            .collect();

        // Pad with dummy bets to reach max_batch_size for consistent circuit structure
        while accounting_bets.len() < self.max_batch_size {
            // Add dummy bet: user 0, amount 0, always loses (no effect on balances)
            accounting_bets.push(Bet::new(0, 0, true, false));
        }

        // Calculate balance deltas (only for real bets)
        let mut user_deltas: HashMap<u32, i64> = HashMap::new();
        for bet in &settlement_batch.bets {  // Only process real bets
            *user_deltas.entry(bet.user_id).or_insert(0) += bet.delta();
        }

        // Calculate final balances
        let mut final_balances = HashMap::new();
        for (&user_id, &initial_balance) in &settlement_batch.initial_balances {
            let delta = user_deltas.get(&user_id).copied().unwrap_or(0);
            let final_balance = (initial_balance as i64 + delta) as u64;

            // Validate no negative balances
            if final_balance as i64 != initial_balance as i64 + delta {
                return Err(WitnessError::NegativeBalance {
                    user_id,
                    balance: initial_balance as i64 + delta,
                });
            }

            final_balances.insert(user_id, final_balance);
        }

        // Calculate house delta and final balance
        let house_delta: i64 = -user_deltas.values().sum::<i64>();
        let house_final_balance = (settlement_batch.house_initial_balance as i64 + house_delta) as u64;

        // Validate conservation law
        let user_sum: i64 = user_deltas.values().sum();
        if user_sum + house_delta != 0 {
            return Err(WitnessError::ConservationViolation {
                user_sum,
                house_delta,
            });
        }

        // Create ordered balance arrays for circuit (must be consistent with max_users)
        let max_user_id = settlement_batch
            .initial_balances
            .keys()
            .max()
            .copied()
            .unwrap_or(0);

        if max_user_id as usize >= self.max_users {
            return Err(WitnessError::BatchTooLarge {
                size: max_user_id as usize + 1,
                max_size: self.max_users,
            });
        }

        let mut initial_balance_array = vec![0u64; self.max_users];
        let mut final_balance_array = vec![0u64; self.max_users];

        for user_id in 0..self.max_users as u32 {
            if let Some(&initial) = settlement_batch.initial_balances.get(&user_id) {
                initial_balance_array[user_id as usize] = initial;
                final_balance_array[user_id as usize] =
                    final_balances.get(&user_id).copied().unwrap_or(initial);
            }
        }

        // Create accounting circuit with padded bets
        let circuit = AccountingCircuit::new(
            accounting_bets,  // Padded to max_batch_size
            settlement_batch.batch_id,
            &initial_balance_array,
            &final_balance_array,
            settlement_batch.house_initial_balance,
            house_final_balance,
        );

        Ok(circuit)
    }

    /// Validate settlement batch data
    fn validate_batch(&self, batch: &SettlementBatch) -> Result<(), WitnessError> {
        // Check for empty batch
        if batch.bets.is_empty() {
            return Err(WitnessError::EmptyBatch);
        }

        // Check batch size limit
        if batch.bets.len() > self.max_batch_size {
            return Err(WitnessError::BatchTooLarge {
                size: batch.bets.len(),
                max_size: self.max_batch_size,
            });
        }

        // Validate all users exist in initial balances
        for bet in &batch.bets {
            if !batch.initial_balances.contains_key(&bet.user_id) {
                return Err(WitnessError::UnknownUser {
                    user_id: bet.user_id,
                });
            }

            // Check sufficient balance for bet
            let balance = batch.initial_balances[&bet.user_id];
            if balance < bet.amount {
                return Err(WitnessError::InsufficientBalance {
                    user_id: bet.user_id,
                    balance,
                    bet_amount: bet.amount,
                });
            }
        }

        Ok(())
    }

    /// Generate witness from the BetBatch format (for compatibility)
    pub fn generate_witness_from_bet_batch(
        &self,
        bet_batch: &BetBatch,
        initial_balances: &HashMap<u32, u64>,
        house_initial_balance: u64,
    ) -> Result<AccountingCircuit, WitnessError> {
        // Convert BetBatch to SettlementBatch
        let settlement_bets: Vec<SettlementBet> = bet_batch
            .bets
            .iter()
            .enumerate()
            .map(|(i, bet)| {
                SettlementBet::new(
                    bet.user_id,
                    bet.amount,
                    bet.guess,
                    bet.outcome,
                    format!("bet_{}", i),
                )
            })
            .collect();

        let settlement_batch = SettlementBatch {
            batch_id: bet_batch.batch_id,
            bets: settlement_bets,
            initial_balances: initial_balances.clone(),
            house_initial_balance,
            timestamp: 0, // Not used in circuit generation
        };

        self.generate_witness(&settlement_batch)
    }
}

/// Helper function to create a settlement batch for testing
pub fn create_test_settlement_batch(
    batch_id: u32,
    bets: Vec<(u32, u64, bool, bool)>, // (user_id, amount, guess, outcome)
    initial_balances: HashMap<u32, u64>,
    house_initial: u64,
) -> SettlementBatch {
    let settlement_bets: Vec<SettlementBet> = bets
        .into_iter()
        .enumerate()
        .map(|(i, (user_id, amount, guess, outcome))| {
            SettlementBet::new(user_id, amount, guess, outcome, format!("test_bet_{}", i))
        })
        .collect();

    SettlementBatch {
        batch_id,
        bets: settlement_bets,
        initial_balances,
        house_initial_balance: house_initial,
        timestamp: 1698000000, // Fixed timestamp for testing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;

    #[test]
    fn test_settlement_bet_calculations() {
        let winning_bet = SettlementBet::new(1, 1000, true, true, "bet1".to_string());
        let losing_bet = SettlementBet::new(2, 2000, true, false, "bet2".to_string());

        assert!(winning_bet.won());
        assert!(!losing_bet.won());

        assert_eq!(winning_bet.payout(), 2000);
        assert_eq!(losing_bet.payout(), 0);

        assert_eq!(winning_bet.delta(), 1000);
        assert_eq!(losing_bet.delta(), -2000);
    }

    #[test]
    fn test_witness_generation_single_bet() {
        let generator = WitnessGenerator::new(10, 5);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);

        let batch = create_test_settlement_batch(
            1,
            vec![(0, 1000, true, true)], // User 0 bets 1000, wins
            initial_balances,
            50000,
        );

        let circuit = generator.generate_witness(&batch).unwrap();

        assert_eq!(circuit.batch_id, Fr::from(1u32));
        assert_eq!(circuit.initial_balances[0], Fr::from(10000u64));
        assert_eq!(circuit.final_balances[0], Fr::from(11000u64)); // +1000 from win
        assert_eq!(circuit.house_initial, Fr::from(50000u64));
        assert_eq!(circuit.house_final, Fr::from(49000u64)); // -1000 to user
    }

    #[test]
    fn test_witness_generation_multi_bet() {
        let generator = WitnessGenerator::new(10, 5);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);
        initial_balances.insert(1, 15000);

        let batch = create_test_settlement_batch(
            42,
            vec![
                (0, 1000, true, true),   // User 0 wins 1000
                (1, 2000, false, true),  // User 1 loses 2000
                (0, 500, false, false),  // User 0 wins 500
            ],
            initial_balances,
            100000,
        );

        let circuit = generator.generate_witness(&batch).unwrap();

        assert_eq!(circuit.batch_id, Fr::from(42u32));
        assert_eq!(circuit.initial_balances[0], Fr::from(10000u64));
        assert_eq!(circuit.initial_balances[1], Fr::from(15000u64));
        assert_eq!(circuit.final_balances[0], Fr::from(11500u64)); // +1500 total
        assert_eq!(circuit.final_balances[1], Fr::from(13000u64)); // -2000
        assert_eq!(circuit.house_initial, Fr::from(100000u64));
        assert_eq!(circuit.house_final, Fr::from(100500u64)); // +500 net
    }

    #[test]
    fn test_insufficient_balance_error() {
        let generator = WitnessGenerator::new(10, 5);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 500); // Only 500 balance

        let batch = create_test_settlement_batch(
            1,
            vec![(0, 1000, true, true)], // User tries to bet 1000
            initial_balances,
            50000,
        );

        let result = generator.generate_witness(&batch);
        assert!(matches!(result, Err(WitnessError::InsufficientBalance { .. })));
    }

    #[test]
    fn test_empty_batch_error() {
        let generator = WitnessGenerator::new(10, 5);

        let batch = create_test_settlement_batch(1, vec![], HashMap::new(), 50000);

        let result = generator.generate_witness(&batch);
        assert!(matches!(result, Err(WitnessError::EmptyBatch)));
    }

    #[test]
    fn test_conservation_validation() {
        let generator = WitnessGenerator::new(10, 5);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);
        initial_balances.insert(1, 10000);

        // Create a batch where conservation should hold
        let batch = create_test_settlement_batch(
            1,
            vec![
                (0, 1000, true, true),   // User 0 wins 1000
                (1, 1000, false, true),  // User 1 loses 1000
            ],
            initial_balances,
            50000,
        );

        let circuit = generator.generate_witness(&batch).unwrap();

        // Verify conservation: User 0 gains 1000, User 1 loses 1000, House breaks even
        assert_eq!(circuit.final_balances[0], Fr::from(11000u64));
        assert_eq!(circuit.final_balances[1], Fr::from(9000u64));
        assert_eq!(circuit.house_final, Fr::from(50000u64)); // No change
    }
}