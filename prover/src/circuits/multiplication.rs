use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, ProvingKey, VerifyingKey, Proof, prepare_verifying_key};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use rand::thread_rng;

/// A simple circuit: prove knowledge of two factors a, b such that a * b = c.
/// This is analogous to verifying a balance update: old_balance - bet = new_balance
#[derive(Clone)]
pub struct MulCircuit {
    // Private inputs (witness)
    pub a: Fr,
    pub b: Fr,
    // Public input (instance)
    pub c: Fr,
}

impl ConstraintSynthesizer<Fr> for MulCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // Allocate private witness variables
        let a_var = cs.new_witness_variable(|| Ok(self.a))?;
        let b_var = cs.new_witness_variable(|| Ok(self.b))?;
        
        // Allocate public instance variable
        let c_var = cs.new_input_variable(|| Ok(self.c))?;
        
        // Enforce constraint: a * b = c
        cs.enforce_constraint(
            ark_relations::lc!() + a_var,
            ark_relations::lc!() + b_var,
            ark_relations::lc!() + c_var,
        )?;
        
        Ok(())
    }
}

impl MulCircuit {
    pub fn new(a: u64, b: u64) -> Self {
        let a_fr = Fr::from(a);
        let b_fr = Fr::from(b);
        let c_fr = a_fr * b_fr;
        
        Self {
            a: a_fr,
            b: b_fr,
            c: c_fr,
        }
    }
    
    pub fn new_invalid(a: u64, b: u64, wrong_c: u64) -> Self {
        Self {
            a: Fr::from(a),
            b: Fr::from(b),
            c: Fr::from(wrong_c), // This will make the circuit unsatisfiable
        }
    }
}

/// Complete Groth16 proof system for the multiplication circuit
pub struct MulProofSystem {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
}

impl MulProofSystem {
    /// Generate trusted setup for the multiplication circuit
    /// In production, this would be done via a ceremony
    pub fn setup() -> Result<Self, Box<dyn std::error::Error>> {
        let mut rng = thread_rng();
        
        // Create a dummy circuit for setup
        let circuit = MulCircuit::new(1, 1);
        
        // Generate proving and verifying keys
        let (proving_key, verifying_key) = Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)?;
        
        Ok(Self {
            proving_key,
            verifying_key,
        })
    }
    
    /// Generate a proof for the given circuit
    pub fn prove(&self, circuit: MulCircuit) -> Result<Proof<Bn254>, Box<dyn std::error::Error>> {
        let mut rng = thread_rng();
        let proof = Groth16::<Bn254>::prove(&self.proving_key, circuit, &mut rng)?;
        Ok(proof)
    }
    
    /// Verify a proof with the given public inputs
    pub fn verify(&self, proof: &Proof<Bn254>, public_inputs: &[Fr]) -> Result<bool, Box<dyn std::error::Error>> {
        let pvk = prepare_verifying_key(&self.verifying_key);
        let result = Groth16::<Bn254>::verify_with_processed_vk(&pvk, public_inputs, proof)?;
        Ok(result)
    }
    
    /// Get the verifying key as bytes for embedding in Solana program
    pub fn verifying_key_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        self.verifying_key.serialize_compressed(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_multiplication_circuit_valid() {
        let circuit = MulCircuit::new(3, 7); // 3 * 7 = 21
        let system = MulProofSystem::setup().expect("Setup failed");
        
        // Generate proof
        let start = Instant::now();
        let proof = system.prove(circuit.clone()).expect("Proving failed");
        let prove_time = start.elapsed();
        
        // Verify proof
        let start = Instant::now();
        let public_inputs = vec![circuit.c]; // Only c is public
        let is_valid = system.verify(&proof, &public_inputs).expect("Verification failed");
        let verify_time = start.elapsed();
        
        assert!(is_valid, "Valid proof should verify");
        
        println!("Multiplication circuit validation:");
        println!("  Prove time: {:?}", prove_time);
        println!("  Verify time: {:?}", verify_time);
        println!("  Public input (c = 3 * 7): {}", circuit.c);
    }
    
    #[test]
    #[should_panic(expected = "cs.is_satisfied")]
    fn test_multiplication_circuit_invalid() {
        let circuit = MulCircuit::new_invalid(3, 7, 22); // 3 * 7 ≠ 22
        let system = MulProofSystem::setup().expect("Setup failed");
        
        // This should panic during proving because the circuit constraint is not satisfied
        let _result = system.prove(circuit);
        // If we reach here without panicking, the test should fail
        panic!("Expected constraint validation to panic");
    }
    
    #[test]
    fn test_wrong_public_input() {
        let circuit = MulCircuit::new(5, 4); // 5 * 4 = 20
        let system = MulProofSystem::setup().expect("Setup failed");
        
        let proof = system.prove(circuit.clone()).expect("Proving failed");
        
        // Try to verify with wrong public input
        let wrong_public_inputs = vec![Fr::from(19u64)]; // Wrong result
        let is_valid = system.verify(&proof, &wrong_public_inputs).expect("Verification failed");
        
        assert!(!is_valid, "Proof with wrong public input should not verify");
    }
    
    #[test]
    fn test_verifying_key_serialization() {
        let system = MulProofSystem::setup().expect("Setup failed");
        let vk_bytes = system.verifying_key_bytes().expect("Serialization failed");
        
        assert!(!vk_bytes.is_empty(), "Verifying key bytes should not be empty");
        println!("Verifying key size: {} bytes", vk_bytes.len());
    }
    
    #[test]
    fn test_large_numbers() {
        // Test with larger numbers to ensure field arithmetic works correctly
        let circuit = MulCircuit::new(12345, 67890); // Large multiplication
        let system = MulProofSystem::setup().expect("Setup failed");
        
        let proof = system.prove(circuit.clone()).expect("Proving failed");
        let public_inputs = vec![circuit.c];
        let is_valid = system.verify(&proof, &public_inputs).expect("Verification failed");
        
        assert!(is_valid, "Large number multiplication should verify");
        
        // Verify the actual computation
        let expected = 12345u64 * 67890u64;
        assert_eq!(circuit.c, Fr::from(expected));
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn benchmark_setup_time() {
        let iterations = 5;
        let mut total_time = std::time::Duration::default();
        
        for _ in 0..iterations {
            let start = Instant::now();
            let _system = MulProofSystem::setup().expect("Setup failed");
            total_time += start.elapsed();
        }
        
        let avg_time = total_time / iterations as u32;
        println!("Average setup time: {:?}", avg_time);
        
        // Setup should be reasonably fast for a simple circuit
        assert!(avg_time.as_millis() < 5000, "Setup taking too long: {:?}", avg_time);
    }
    
    #[test]
    fn benchmark_proving_time() {
        let system = MulProofSystem::setup().expect("Setup failed");
        let iterations = 10;
        let mut total_time = std::time::Duration::default();
        
        for i in 0..iterations {
            let circuit = MulCircuit::new(i + 1, i + 2);
            let start = Instant::now();
            let _proof = system.prove(circuit).expect("Proving failed");
            total_time += start.elapsed();
        }
        
        let avg_time = total_time / iterations as u32;
        println!("Average proving time: {:?}", avg_time);
        
        // Proving should be fast for a simple circuit
        assert!(avg_time.as_millis() < 1000, "Proving taking too long: {:?}", avg_time);
    }
    
    #[test]
    fn benchmark_verification_time() {
        let system = MulProofSystem::setup().expect("Setup failed");
        let circuit = MulCircuit::new(42, 24);
        let proof = system.prove(circuit.clone()).expect("Proving failed");
        let public_inputs = vec![circuit.c];
        
        let iterations = 100;
        let mut total_time = std::time::Duration::default();
        
        for _ in 0..iterations {
            let start = Instant::now();
            let _result = system.verify(&proof, &public_inputs).expect("Verification failed");
            total_time += start.elapsed();
        }
        
        let avg_time = total_time / iterations as u32;
        println!("Average verification time: {:?}", avg_time);
        
        // Verification should be fast
        assert!(avg_time.as_millis() < 100, "Verification taking too long: {:?}", avg_time);
    }
}