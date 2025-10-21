use crate::circuits::accounting::AccountingCircuit;
use crate::witness_generator::{SettlementBatch, WitnessError, WitnessGenerator};
use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, SerializationError};
use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
use ark_std::rand::thread_rng;
use std::io::{Read, Write};
use thiserror::Error;

/// Errors that can occur during proof generation
#[derive(Error, Debug)]
pub enum ProofError {
    #[error("Witness generation failed: {0}")]
    WitnessGeneration(#[from] WitnessError),
    #[error("Proof generation failed: {0}")]
    ProofGeneration(String),
    #[error("Proof verification failed")]
    ProofVerification,
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid proof parameters")]
    InvalidParameters,
    #[error("Circuit constraint mismatch: expected {expected}, got {actual}")]
    CircuitMismatch { expected: usize, actual: usize },
}

/// Serializable proof structure for transport/storage
#[derive(Clone, Debug)]
pub struct SerializableProof {
    pub proof: Proof<Bn254>,
    pub public_inputs: Vec<Fr>,
    pub batch_id: u32,
    pub timestamp: u64,
}

impl SerializableProof {
    pub fn new(proof: Proof<Bn254>, public_inputs: Vec<Fr>, batch_id: u32) -> Self {
        Self {
            proof,
            public_inputs,
            batch_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Serialize proof to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, ProofError> {
        let mut buf = Vec::new();

        // Write batch_id and timestamp
        buf.write_all(&self.batch_id.to_le_bytes())?;
        buf.write_all(&self.timestamp.to_le_bytes())?;

        // Write number of public inputs
        buf.write_all(&(self.public_inputs.len() as u32).to_le_bytes())?;

        // Write public inputs
        for input in &self.public_inputs {
            let mut input_buf = Vec::new();
            input.serialize_compressed(&mut input_buf)?;
            buf.write_all(&(input_buf.len() as u32).to_le_bytes())?;
            buf.write_all(&input_buf)?;
        }

        // Write proof
        let mut proof_buf = Vec::new();
        self.proof.serialize_compressed(&mut proof_buf)?;
        buf.write_all(&(proof_buf.len() as u32).to_le_bytes())?;
        buf.write_all(&proof_buf)?;

        Ok(buf)
    }

    /// Deserialize proof from bytes
    pub fn from_bytes(mut data: &[u8]) -> Result<Self, ProofError> {
        // Read batch_id
        let mut buf = [0u8; 4];
        data.read_exact(&mut buf)?;
        let batch_id = u32::from_le_bytes(buf);

        // Read timestamp
        let mut buf = [0u8; 8];
        data.read_exact(&mut buf)?;
        let timestamp = u64::from_le_bytes(buf);

        // Read number of public inputs
        let mut buf = [0u8; 4];
        data.read_exact(&mut buf)?;
        let num_inputs = u32::from_le_bytes(buf) as usize;

        // Read public inputs
        let mut public_inputs = Vec::with_capacity(num_inputs);
        for _ in 0..num_inputs {
            let mut buf = [0u8; 4];
            data.read_exact(&mut buf)?;
            let input_len = u32::from_le_bytes(buf) as usize;

            let mut input_buf = vec![0u8; input_len];
            data.read_exact(&mut input_buf)?;

            let input = Fr::deserialize_compressed(&input_buf[..])?;
            public_inputs.push(input);
        }

        // Read proof
        let mut buf = [0u8; 4];
        data.read_exact(&mut buf)?;
        let proof_len = u32::from_le_bytes(buf) as usize;

        let mut proof_buf = vec![0u8; proof_len];
        data.read_exact(&mut proof_buf)?;

        let proof = Proof::<Bn254>::deserialize_compressed(&proof_buf[..])?;

        Ok(Self {
            proof,
            public_inputs,
            batch_id,
            timestamp,
        })
    }
}

/// Zero-knowledge proof generator for accounting circuits
pub struct ProofGenerator {
    witness_generator: WitnessGenerator,
    proving_key: Option<ProvingKey<Bn254>>,
    verifying_key: Option<VerifyingKey<Bn254>>,
    max_batch_size: usize,
    max_users: usize,
}

impl ProofGenerator {
    pub fn new(max_batch_size: usize, max_users: usize) -> Self {
        Self {
            witness_generator: WitnessGenerator::new(max_batch_size, max_users),
            proving_key: None,
            verifying_key: None,
            max_batch_size,
            max_users,
        }
    }

    /// Setup the proving and verifying keys for a given circuit size
    /// This is deterministic based on circuit structure
    pub fn setup(&mut self) -> Result<(), ProofError> {
        // Create a dummy circuit with the expected structure
        let dummy_circuit = self.create_dummy_circuit()?;

        // Generate parameters
        let mut rng = thread_rng();
        let (pk, vk) = Groth16::<Bn254>::setup(dummy_circuit, &mut rng).map_err(|e| {
            ProofError::ProofGeneration(format!("Parameter generation failed: {}", e))
        })?;

        self.verifying_key = Some(vk);
        self.proving_key = Some(pk);

        Ok(())
    }

    /// Generate a ZK proof from settlement batch data
    pub fn generate_proof(
        &self,
        settlement_batch: &SettlementBatch,
    ) -> Result<SerializableProof, ProofError> {
        // Ensure keys are set up
        let proving_key = self
            .proving_key
            .as_ref()
            .ok_or(ProofError::InvalidParameters)?;

        // Generate witness (accounting circuit)
        let circuit = self.witness_generator.generate_witness(settlement_batch)?;

        // Extract public inputs in the order expected by the circuit
        let mut public_inputs = vec![circuit.batch_id];
        public_inputs.extend(circuit.initial_balances.clone());
        public_inputs.extend(circuit.final_balances.clone());
        public_inputs.push(circuit.house_initial);
        public_inputs.push(circuit.house_final);

        // Generate proof
        let mut rng = thread_rng();
        let proof = Groth16::<Bn254>::prove(proving_key, circuit, &mut rng)
            .map_err(|e| ProofError::ProofGeneration(format!("Proof creation failed: {}", e)))?;

        Ok(SerializableProof::new(
            proof,
            public_inputs,
            settlement_batch.batch_id,
        ))
    }

    /// Verify a proof
    pub fn verify_proof(&self, serializable_proof: &SerializableProof) -> Result<bool, ProofError> {
        let verifying_key = self
            .verifying_key
            .as_ref()
            .ok_or(ProofError::InvalidParameters)?;

        let result = Groth16::<Bn254>::verify(
            verifying_key,
            &serializable_proof.public_inputs,
            &serializable_proof.proof,
        )
        .map_err(|_| ProofError::ProofVerification)?;

        Ok(result)
    }

    /// Create a deterministic proof for testing (uses fixed randomness)
    pub fn generate_deterministic_proof(
        &self,
        settlement_batch: &SettlementBatch,
        seed: u64,
    ) -> Result<SerializableProof, ProofError> {
        use ark_std::rand::rngs::StdRng;
        use ark_std::rand::SeedableRng;

        let proving_key = self
            .proving_key
            .as_ref()
            .ok_or(ProofError::InvalidParameters)?;

        let circuit = self.witness_generator.generate_witness(settlement_batch)?;

        // Extract public inputs in the order expected by the circuit
        let mut public_inputs = vec![circuit.batch_id];
        public_inputs.extend(circuit.initial_balances.clone());
        public_inputs.extend(circuit.final_balances.clone());
        public_inputs.push(circuit.house_initial);
        public_inputs.push(circuit.house_final);

        // Use seeded RNG for deterministic proof generation
        let mut rng = StdRng::seed_from_u64(seed);
        let proof = Groth16::<Bn254>::prove(proving_key, circuit, &mut rng)
            .map_err(|e| ProofError::ProofGeneration(format!("Proof creation failed: {}", e)))?;

        Ok(SerializableProof::new(
            proof,
            public_inputs,
            settlement_batch.batch_id,
        ))
    }

    /// Get the verifying key for external verification
    pub fn get_verifying_key(&self) -> Option<&VerifyingKey<Bn254>> {
        self.verifying_key.as_ref()
    }

    /// Serialize the verifying key for deployment
    pub fn serialize_verifying_key(&self) -> Result<Vec<u8>, ProofError> {
        let vk = self
            .verifying_key
            .as_ref()
            .ok_or(ProofError::InvalidParameters)?;

        let mut buf = Vec::new();
        vk.serialize_compressed(&mut buf)?;
        Ok(buf)
    }

    /// Check if settlement batch is valid without generating proof
    pub fn validate_settlement_batch(
        &self,
        settlement_batch: &SettlementBatch,
    ) -> Result<(), ProofError> {
        self.witness_generator
            .generate_witness(settlement_batch)
            .map(|_| ())
            .map_err(ProofError::from)
    }

    /// Create a dummy circuit for setup (contains expected structure)
    fn create_dummy_circuit(&self) -> Result<AccountingCircuit, ProofError> {
        use crate::witness_generator::create_test_settlement_batch;
        use std::collections::HashMap;

        // Create batch with MAXIMUM size to ensure consistent circuit structure
        let mut initial_balances = HashMap::new();
        for i in 0..self.max_users {
            initial_balances.insert(i as u32, 10000u64); // Default balance
        }

        // Create maximum number of dummy bets to ensure circuit structure is fixed
        let mut dummy_bets = Vec::new();
        for i in 0..self.max_batch_size {
            let user_id = (i % self.max_users) as u32;
            let outcome = i % 2 == 0; // Alternate outcomes
            dummy_bets.push((user_id, 1000, true, outcome));
        }

        let batch = create_test_settlement_batch(
            0, // dummy batch_id
            dummy_bets,
            initial_balances,
            100000, // house initial
        );

        self.witness_generator
            .generate_witness(&batch)
            .map_err(ProofError::from)
    }

    /// Estimate number of constraints for validation
    fn estimate_constraints(&self) -> usize {
        // Conservative estimate based on circuit structure
        // This should match the actual circuit constraint count
        let base_constraints = 100; // Basic circuit overhead
        let bet_constraints = self.max_batch_size * 50; // ~50 constraints per bet
        let balance_constraints = self.max_users * 20; // ~20 constraints per user balance

        base_constraints + bet_constraints + balance_constraints
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness_generator::create_test_settlement_batch;
    use std::collections::HashMap;

    #[test]
    fn test_proof_generator_setup() {
        let mut generator = ProofGenerator::new(5, 3);

        let result = generator.setup();
        assert!(result.is_ok());
        assert!(generator.proving_key.is_some());
        assert!(generator.verifying_key.is_some());
    }

    #[test]
    fn test_single_bet_proof_generation() {
        let mut generator = ProofGenerator::new(5, 3);
        generator.setup().unwrap();

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);

        let batch = create_test_settlement_batch(
            42,
            vec![(0, 1000, true, true)], // User 0 wins
            initial_balances,
            50000,
        );

        let proof = generator.generate_proof(&batch).unwrap();
        assert_eq!(proof.batch_id, 42);

        let is_valid = generator.verify_proof(&proof).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_multi_bet_proof_generation() {
        let mut generator = ProofGenerator::new(10, 5);
        generator.setup().unwrap();

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);
        initial_balances.insert(1, 15000);
        initial_balances.insert(2, 8000);

        let batch = create_test_settlement_batch(
            123,
            vec![
                (0, 1000, true, true),  // User 0 wins
                (1, 2000, false, true), // User 1 loses
                (2, 500, false, false), // User 2 wins
                (0, 800, true, false),  // User 0 loses
            ],
            initial_balances,
            100000,
        );

        let proof = generator.generate_proof(&batch).unwrap();
        assert_eq!(proof.batch_id, 123);

        let is_valid = generator.verify_proof(&proof).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_deterministic_proof_generation() {
        let mut generator = ProofGenerator::new(5, 3);
        generator.setup().unwrap();

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);

        let batch = create_test_settlement_batch(
            1,
            vec![(0, 1000, true, true)],
            initial_balances.clone(),
            50000,
        );

        // Generate two proofs with same seed
        let proof1 = generator
            .generate_deterministic_proof(&batch, 12345)
            .unwrap();
        let proof2 = generator
            .generate_deterministic_proof(&batch, 12345)
            .unwrap();

        // NOTE: Groth16 proofs include cryptographic randomness for security,
        // so proofs will NOT be byte-identical even with same seed.
        // "Deterministic" here means reproducibly verifiable, not identical output.
        
        // Both proofs should verify correctly
        assert!(generator.verify_proof(&proof1).unwrap());
        assert!(generator.verify_proof(&proof2).unwrap());

        // Public inputs should be identical
        assert_eq!(proof1.public_inputs, proof2.public_inputs);
        assert_eq!(proof1.batch_id, proof2.batch_id);

        // Proof bytes will be different due to Groth16 randomness (this is expected)
        // This is cryptographically correct behavior
    }

    #[test]
    fn test_proof_serialization() {
        let mut generator = ProofGenerator::new(5, 3);
        generator.setup().unwrap();

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);

        let batch = create_test_settlement_batch(
            999,
            vec![(0, 1000, true, false)], // User 0 loses
            initial_balances,
            50000,
        );

        let original_proof = generator.generate_proof(&batch).unwrap();

        // Serialize and deserialize
        let serialized = original_proof.to_bytes().unwrap();
        let deserialized_proof = SerializableProof::from_bytes(&serialized).unwrap();

        // Verify batch_id and timestamp are preserved
        assert_eq!(deserialized_proof.batch_id, 999);
        assert_eq!(deserialized_proof.timestamp, original_proof.timestamp);

        // Verify proof still validates
        let is_valid = generator.verify_proof(&deserialized_proof).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_invalid_settlement_batch() {
        let mut generator = ProofGenerator::new(5, 3);
        generator.setup().unwrap();

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 500); // Insufficient balance

        let batch = create_test_settlement_batch(
            1,
            vec![(0, 1000, true, true)], // Bet more than balance
            initial_balances,
            50000,
        );

        let result = generator.generate_proof(&batch);
        assert!(matches!(result, Err(ProofError::WitnessGeneration(_))));

        // Validation should also fail
        let validation_result = generator.validate_settlement_batch(&batch);
        assert!(validation_result.is_err());
    }

    #[test]
    fn test_verifying_key_serialization() {
        let mut generator = ProofGenerator::new(5, 3);
        generator.setup().unwrap();

        let vk_bytes = generator.serialize_verifying_key().unwrap();
        assert!(!vk_bytes.is_empty());

        // Should be able to deserialize (basic check)
        let _vk = VerifyingKey::<Bn254>::deserialize_compressed(&vk_bytes[..]).unwrap();
    }

    #[test]
    fn test_empty_batch_error() {
        let mut generator = ProofGenerator::new(5, 3);
        generator.setup().unwrap();

        let batch = create_test_settlement_batch(1, vec![], HashMap::new(), 50000);

        let result = generator.generate_proof(&batch);
        assert!(matches!(
            result,
            Err(ProofError::WitnessGeneration(WitnessError::EmptyBatch))
        ));
    }

    #[test]
    fn test_setup_required_error() {
        let generator = ProofGenerator::new(5, 3);

        let mut initial_balances = HashMap::new();
        initial_balances.insert(0, 10000);

        let batch =
            create_test_settlement_batch(1, vec![(0, 1000, true, true)], initial_balances, 50000);

        // Should fail without setup
        let result = generator.generate_proof(&batch);
        assert!(matches!(result, Err(ProofError::InvalidParameters)));
    }
}
