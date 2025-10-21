// ZK Casino Prover Library
// Phase 3a: ZK Framework Decision - Arkworks Groth16 (BN254)

pub mod circuits;
pub mod proof_generator;
pub mod witness_generator;

// Legacy module for backward compatibility - will be phased out
mod legacy;

pub use circuits::*;
pub use legacy::*;

// Re-export core types for convenience
pub use ark_bn254::{Bn254, Fr};
pub use ark_ff::PrimeField;
pub use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
