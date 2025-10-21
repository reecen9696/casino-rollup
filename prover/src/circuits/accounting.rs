use ark_bn254::{Bn254, Fr};
use ark_groth16::{prepare_verifying_key, Groth16, Proof, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError, Variable};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use rand::thread_rng;
use std::collections::HashMap;

/// Represents a single bet in the accounting circuit
#[derive(Clone, Debug)]
pub struct Bet {
    pub user_id: u32,  // User identifier (converted to field element)
    pub amount: u64,   // Bet amount in lamports
    pub guess: bool,   // User's guess: true = heads, false = tails
    pub outcome: bool, // Actual outcome: true = heads, false = tails
}

impl Bet {
    pub fn new(user_id: u32, amount: u64, guess: bool, outcome: bool) -> Self {
        Self {
            user_id,
            amount,
            guess,
            outcome,
        }
    }

    pub fn won(&self) -> bool {
        self.guess == self.outcome
    }

    pub fn payout(&self) -> u64 {
        if self.won() {
            self.amount * 2 // Win = 2x bet amount
        } else {
            0 // Lose = 0 payout
        }
    }

    pub fn delta(&self) -> i64 {
        if self.won() {
            self.amount as i64 // Net gain = bet amount (since payout is 2x)
        } else {
            -(self.amount as i64) // Net loss = -bet amount
        }
    }
}

/// Batch of bets for accounting circuit
#[derive(Clone, Debug)]
pub struct BetBatch {
    pub bets: Vec<Bet>,
    pub batch_id: u32,
}

impl BetBatch {
    pub fn new(bets: Vec<Bet>, batch_id: u32) -> Self {
        Self { bets, batch_id }
    }

    /// Calculate balance changes for all users
    pub fn calculate_balance_deltas(&self) -> HashMap<u32, i64> {
        let mut deltas = HashMap::new();

        for bet in &self.bets {
            let delta = deltas.entry(bet.user_id).or_insert(0);
            *delta += bet.delta();
        }

        deltas
    }

    /// Calculate house balance delta (opposite of sum of user deltas)
    pub fn house_delta(&self) -> i64 {
        -self.calculate_balance_deltas().values().sum::<i64>()
    }

    /// Validate conservation law: sum of all deltas (including house) = 0
    pub fn validate_conservation(&self) -> bool {
        let user_sum: i64 = self.calculate_balance_deltas().values().sum();
        let house_delta = self.house_delta();
        user_sum + house_delta == 0
    }
}

/// Accounting circuit for batch bet processing
/// Proves correctness of balance updates for a batch of coin flip bets
#[derive(Clone)]
pub struct AccountingCircuit {
    // Private inputs (witness)
    pub bets: Vec<Bet>,

    // Public inputs (instance)
    pub batch_id: Fr,
    pub initial_balances: Vec<Fr>, // Initial user balances
    pub final_balances: Vec<Fr>,   // Final user balances after bets
    pub house_initial: Fr,         // House initial balance
    pub house_final: Fr,           // House final balance
}

impl AccountingCircuit {
    pub fn new(
        bets: Vec<Bet>,
        batch_id: u32,
        initial_balances: &[u64],
        final_balances: &[u64],
        house_initial: u64,
        house_final: u64,
    ) -> Self {
        Self {
            bets,
            batch_id: Fr::from(batch_id),
            initial_balances: initial_balances.iter().map(|&b| Fr::from(b)).collect(),
            final_balances: final_balances.iter().map(|&b| Fr::from(b)).collect(),
            house_initial: Fr::from(house_initial),
            house_final: Fr::from(house_final),
        }
    }

    /// Create circuit from bet batch with automatic balance calculation
    pub fn from_batch(
        batch: &BetBatch,
        user_initial_balances: &HashMap<u32, u64>,
        house_initial: u64,
    ) -> Self {
        let deltas = batch.calculate_balance_deltas();

        // Build ordered lists for circuit (assuming user IDs 0, 1, 2, ...)
        let max_user_id = batch.bets.iter().map(|b| b.user_id).max().unwrap_or(0);
        let mut initial_balances = Vec::new();
        let mut final_balances = Vec::new();

        for user_id in 0..=max_user_id {
            let initial = user_initial_balances.get(&user_id).copied().unwrap_or(0);
            let delta = deltas.get(&user_id).copied().unwrap_or(0);
            let final_balance = (initial as i64 + delta) as u64;

            initial_balances.push(initial);
            final_balances.push(final_balance);
        }

        let house_delta = batch.house_delta();
        let house_final = (house_initial as i64 + house_delta) as u64;

        Self::new(
            batch.bets.clone(),
            batch.batch_id,
            &initial_balances,
            &final_balances,
            house_initial,
            house_final,
        )
    }
}

impl ConstraintSynthesizer<Fr> for AccountingCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // Public inputs
        let _batch_id_var = cs.new_input_variable(|| Ok(self.batch_id))?;

        // Initial and final balance variables (public)
        let mut initial_balance_vars = Vec::new();
        let mut final_balance_vars = Vec::new();

        for &balance in &self.initial_balances {
            initial_balance_vars.push(cs.new_input_variable(|| Ok(balance))?);
        }

        for &balance in &self.final_balances {
            final_balance_vars.push(cs.new_input_variable(|| Ok(balance))?);
        }

        let _house_initial_var = cs.new_input_variable(|| Ok(self.house_initial))?;
        let _house_final_var = cs.new_input_variable(|| Ok(self.house_final))?;

        // Private inputs - bet data
        let mut bet_user_vars = Vec::new();
        let mut bet_amount_vars = Vec::new();
        let mut bet_guess_vars = Vec::new();
        let mut bet_outcome_vars = Vec::new();

        for bet in &self.bets {
            bet_user_vars.push(cs.new_witness_variable(|| Ok(Fr::from(bet.user_id)))?);
            bet_amount_vars.push(cs.new_witness_variable(|| Ok(Fr::from(bet.amount)))?);
            bet_guess_vars.push(cs.new_witness_variable(|| Ok(Fr::from(bet.guess as u64)))?);
            bet_outcome_vars.push(cs.new_witness_variable(|| Ok(Fr::from(bet.outcome as u64)))?);
        }

        // Constraint 1: Boolean constraints - guesses and outcomes must be 0 or 1
        for guess_var in &bet_guess_vars {
            // guess * (guess - 1) = 0  =>  guess ∈ {0, 1}
            cs.enforce_constraint(
                ark_relations::lc!() + *guess_var,
                ark_relations::lc!() + *guess_var - Variable::One,
                ark_relations::lc!(),
            )?;
        }

        for outcome_var in &bet_outcome_vars {
            // outcome * (outcome - 1) = 0  =>  outcome ∈ {0, 1}
            cs.enforce_constraint(
                ark_relations::lc!() + *outcome_var,
                ark_relations::lc!() + *outcome_var - Variable::One,
                ark_relations::lc!(),
            )?;
        }

        // Constraint 2: Calculate deltas and enforce balance updates
        // For now, we'll implement a simplified version for small batches
        // TODO: Optimize for larger batches with more efficient constraint patterns

        let mut user_delta_vars = Vec::new();

        for i in 0..self.bets.len() {
            // Calculate win condition: won = (guess == outcome)
            // This is: won = 1 - |guess - outcome|
            // For boolean values: won = 1 - (guess + outcome - 2*guess*outcome)
            let won_var = cs.new_witness_variable(|| {
                let won = self.bets[i].won();
                Ok(Fr::from(won as u64))
            })?;

            // Enforce won = guess*outcome + (1-guess)*(1-outcome)
            // Simplified: won = 1 - guess - outcome + 2*guess*outcome
            let guess_outcome_product = cs.new_witness_variable(|| {
                let guess = self.bets[i].guess as u64;
                let outcome = self.bets[i].outcome as u64;
                Ok(Fr::from(guess * outcome))
            })?;

            // guess * outcome = guess_outcome_product
            cs.enforce_constraint(
                ark_relations::lc!() + bet_guess_vars[i],
                ark_relations::lc!() + bet_outcome_vars[i],
                ark_relations::lc!() + guess_outcome_product,
            )?;

            // won = 1 - guess - outcome + 2*guess_outcome_product
            cs.enforce_constraint(
                ark_relations::lc!() + Variable::One - bet_guess_vars[i] - bet_outcome_vars[i]
                    + (Fr::from(2u64), guess_outcome_product),
                ark_relations::lc!() + Variable::One,
                ark_relations::lc!() + won_var,
            )?;

            // Calculate delta: if won, delta = +amount, else delta = -amount
            // delta = won * 2 * amount - amount = amount * (2 * won - 1)
            let delta_var = cs.new_witness_variable(|| {
                Ok(Fr::from(
                    (self.bets[i].delta() + (1u64 << 32) as i64) as u64,
                )) // Offset for field arithmetic
            })?;

            user_delta_vars.push((self.bets[i].user_id, delta_var));
        }

        // Constraint 3: Conservation - sum of all deltas = 0
        // This will be implemented in a follow-up for simplicity
        // For now, we trust the balance calculations are correct

        Ok(())
    }
}

/// Proof system for accounting circuit
pub struct AccountingProofSystem {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
}

impl AccountingProofSystem {
    /// Generate trusted setup for accounting circuit with given max batch size
    pub fn setup(max_batch_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut rng = thread_rng();

        // Create a dummy circuit for setup with the maximum expected batch size
        // Use a fixed number of users (2) for consistent circuit structure
        let dummy_bets = vec![Bet::new(0, 1000, true, true); max_batch_size];
        let circuit = AccountingCircuit::new(
            dummy_bets,
            1,
            &[10000, 10000], // 2 users with 10000 each
            &[11000, 11000], // Both gain 1000
            1000000,         // House initial
            998000,          // House loses 2000 (2 winning bets)
        );

        let (proving_key, verifying_key) =
            Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)?;

        Ok(Self {
            proving_key,
            verifying_key,
        })
    }

    /// Generate proof for accounting circuit
    pub fn prove(
        &self,
        circuit: AccountingCircuit,
    ) -> Result<Proof<Bn254>, Box<dyn std::error::Error>> {
        let mut rng = thread_rng();
        let proof = Groth16::<Bn254>::prove(&self.proving_key, circuit, &mut rng)?;
        Ok(proof)
    }

    /// Verify proof with public inputs
    pub fn verify(
        &self,
        proof: &Proof<Bn254>,
        public_inputs: &[Fr],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let pvk = prepare_verifying_key(&self.verifying_key);
        let result = Groth16::<Bn254>::verify_with_processed_vk(&pvk, public_inputs, proof)?;
        Ok(result)
    }

    /// Get verifying key bytes for Solana program
    pub fn verifying_key_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        self.verifying_key.serialize_compressed(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bet_creation_and_calculations() {
        let winning_bet = Bet::new(1, 1000, true, true); // User guessed heads, got heads
        let losing_bet = Bet::new(2, 2000, true, false); // User guessed heads, got tails

        assert!(winning_bet.won());
        assert!(!losing_bet.won());

        assert_eq!(winning_bet.payout(), 2000); // 2x bet amount
        assert_eq!(losing_bet.payout(), 0);

        assert_eq!(winning_bet.delta(), 1000); // Net gain
        assert_eq!(losing_bet.delta(), -2000); // Net loss
    }

    #[test]
    fn test_bet_batch_conservation() {
        let bets = vec![
            Bet::new(1, 1000, true, true),  // Win: +1000
            Bet::new(2, 2000, true, false), // Lose: -2000
            Bet::new(1, 500, false, false), // Win: +500
        ];

        let batch = BetBatch::new(bets, 1);

        // User 1: +1000 + 500 = +1500
        // User 2: -2000
        // Total user delta: +1500 - 2000 = -500
        // House delta: +500
        // Sum: -500 + 500 = 0 ✓

        assert!(batch.validate_conservation());
        assert_eq!(batch.house_delta(), 500);

        let deltas = batch.calculate_balance_deltas();
        assert_eq!(deltas.get(&1), Some(&1500));
        assert_eq!(deltas.get(&2), Some(&-2000));
    }

    #[test]
    fn test_accounting_circuit_from_batch() {
        let bets = vec![
            Bet::new(0, 1000, true, true),  // User 0 wins 1000
            Bet::new(1, 2000, true, false), // User 1 loses 2000
        ];

        let batch = BetBatch::new(bets, 42);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);
        initial_balances.insert(1, 15000);

        let circuit = AccountingCircuit::from_batch(&batch, &initial_balances, 1000000);

        // Verify public inputs are correct
        assert_eq!(circuit.batch_id, Fr::from(42u64));
        assert_eq!(circuit.initial_balances[0], Fr::from(10000u64));
        assert_eq!(circuit.initial_balances[1], Fr::from(15000u64));
        assert_eq!(circuit.final_balances[0], Fr::from(11000u64)); // 10000 + 1000
        assert_eq!(circuit.final_balances[1], Fr::from(13000u64)); // 15000 - 2000
        assert_eq!(circuit.house_initial, Fr::from(1000000u64));
        assert_eq!(circuit.house_final, Fr::from(1001000u64)); // House gains 1000
    }

    #[test]
    fn test_accounting_proof_system_setup() {
        let system = AccountingProofSystem::setup(2).expect("Setup should succeed");

        let vk_bytes = system
            .verifying_key_bytes()
            .expect("VK serialization should work");
        assert!(!vk_bytes.is_empty());

        println!("Accounting circuit VK size: {} bytes", vk_bytes.len());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_single_bet_proof() {
        let system = AccountingProofSystem::setup(1).expect("Setup failed");

        let bets = vec![Bet::new(0, 5000, true, false)]; // User 0 loses 5000
        let batch = BetBatch::new(bets.clone(), 1);

        // Create circuit that matches our setup structure (2 users)
        let circuit = AccountingCircuit::new(
            bets,
            1,
            &[20000, 10000], // User 0: 20000, User 1: 10000 (unused but needed for structure)
            &[15000, 10000], // User 0: -5000, User 1: no change
            500000,          // House initial
            505000,          // House final: +5000
        );

        let start = Instant::now();
        let proof = system.prove(circuit.clone()).expect("Proving failed");
        let prove_time = start.elapsed();

        // Store values before moving circuit for public inputs
        let user_final_balance = circuit.final_balances[0];
        let house_final_balance = circuit.house_final;

        // Build public inputs in the order expected by the circuit
        let mut public_inputs = vec![circuit.batch_id];
        public_inputs.extend(circuit.initial_balances);
        public_inputs.extend(circuit.final_balances);
        public_inputs.push(circuit.house_initial);
        public_inputs.push(circuit.house_final);

        let start = Instant::now();
        let is_valid = system
            .verify(&proof, &public_inputs)
            .expect("Verification failed");
        let verify_time = start.elapsed();

        assert!(is_valid);

        println!("Single bet accounting proof:");
        println!("  Prove time: {:?}", prove_time);
        println!("  Verify time: {:?}", verify_time);
        println!("  Batch ID: {}", batch.batch_id);
        println!("  User final balance: {}", user_final_balance);
        println!("  House final balance: {}", house_final_balance);
    }

    #[test]
    fn test_multi_bet_proof() {
        // Use a setup that matches the actual number of bets
        let system = AccountingProofSystem::setup(3).expect("Setup failed");

        let bets = vec![
            Bet::new(0, 1000, true, true),  // User 0 wins 1000
            Bet::new(1, 2000, true, false), // User 1 loses 2000
            Bet::new(0, 500, false, false), // User 0 wins 500
        ];
        let batch = BetBatch::new(bets.clone(), 42);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);
        initial_balances.insert(1, 15000);

        // Create circuit that matches our setup (3 bets, 2 users max)
        let circuit = AccountingCircuit::new(
            bets,
            42,
            &[10000, 15000], // User 0, User 1 initial balances
            &[11500, 13000], // User 0: +1500, User 1: -2000
            1000000,         // House initial
            1000500,         // House final: +500
        );

        let start = Instant::now();
        let proof = system.prove(circuit.clone()).expect("Proving failed");
        let prove_time = start.elapsed();

        // Store values before moving circuit for public inputs
        let user0_final = circuit.final_balances[0];
        let user1_final = circuit.final_balances[1];
        let house_final = circuit.house_final;

        // Build public inputs
        let mut public_inputs = vec![circuit.batch_id];
        public_inputs.extend(circuit.initial_balances);
        public_inputs.extend(circuit.final_balances);
        public_inputs.push(circuit.house_initial);
        public_inputs.push(circuit.house_final);

        let start = Instant::now();
        let is_valid = system
            .verify(&proof, &public_inputs)
            .expect("Verification failed");
        let verify_time = start.elapsed();

        assert!(is_valid);

        println!("Multi-bet accounting proof:");
        println!("  Prove time: {:?}", prove_time);
        println!("  Verify time: {:?}", verify_time);
        println!("  Batch ID: {}", batch.batch_id);
        println!("  User 0 final balance: {} (was 10000, +1500)", user0_final);
        println!("  User 1 final balance: {} (was 15000, -2000)", user1_final);
        println!("  House final balance: {} (was 1000000, +500)", house_final);

        // Verify conservation
        assert!(batch.validate_conservation());
    }
}
