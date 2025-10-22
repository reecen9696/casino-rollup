//! VRF (Verifiable Random Functions) module for ZK Casino
//! 
//! This module provides cryptographically verifiable randomness for fair coin flip outcomes.
//! It implements ed25519-based VRF signatures that can be independently verified by clients.

pub mod keypair;

pub use keypair::{VRFKeypair, VRFKeypairError};

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// VRF proof containing all data needed for verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VRFProof {
    /// The message that was signed (hash of bet_id||user||nonce)
    pub message: [u8; 32],
    /// The VRF signature (serialized as hex string for serde compatibility)
    #[serde(with = "hex_serde")]
    pub signature: [u8; 64],
    /// The public key used for signing
    pub public_key: [u8; 32],
    /// The derived outcome (true = heads, false = tails)
    pub outcome: bool,
}

// Helper module for serializing large byte arrays as hex
mod hex_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom("Invalid signature length"));
        }
        let mut result = [0u8; 64];
        result.copy_from_slice(&bytes);
        Ok(result)
    }
}

impl VRFProof {
    /// Create a new VRF proof
    pub fn new(message: [u8; 32], signature: [u8; 64], public_key: [u8; 32], outcome: bool) -> Self {
        Self {
            message,
            signature,
            public_key,
            outcome,
        }
    }

    /// Verify this VRF proof using ed25519 signature verification
    pub fn verify(&self) -> bool {
        use ed25519_dalek::{PublicKey, Signature, Verifier};
        
        match (
            PublicKey::from_bytes(&self.public_key),
            Signature::try_from(&self.signature[..])
        ) {
            (Ok(public_key), Ok(signature)) => {
                // Verify the signature
                let verification_result = public_key.verify(&self.message, &signature);
                
                if verification_result.is_ok() {
                    // Verify the outcome derivation matches LSB of signature
                    let derived_outcome = (self.signature[0] & 1) == 0; // LSB = 0 means heads (true)
                    derived_outcome == self.outcome
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get the outcome as a string for display
    pub fn outcome_string(&self) -> &'static str {
        if self.outcome { "heads" } else { "tails" }
    }
}

/// Generate a deterministic VRF message from bet parameters
/// 
/// Creates H(bet_id||user||nonce) where:
/// - bet_id: 8 bytes (u64 big-endian)
/// - user: 32 bytes (Solana public key)
/// - nonce: 8 bytes (u64 big-endian)
/// 
/// This ensures deterministic, reproducible randomness that can be verified
/// by clients and auditors.
pub fn generate_vrf_message(bet_id: u64, user: &[u8; 32], nonce: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    
    // Add bet_id as 8 bytes (big-endian for network consistency)
    hasher.update(bet_id.to_be_bytes());
    
    // Add user address as 32 bytes (Solana pubkey)
    hasher.update(user);
    
    // Add nonce as 8 bytes (big-endian for network consistency)
    hasher.update(nonce.to_be_bytes());
    
    let result = hasher.finalize();
    result.into()
}

/// Generate a VRF message from string bet_id (for backward compatibility)
/// 
/// Converts string bet_id to deterministic u64 using hash truncation
pub fn generate_vrf_message_from_string(bet_id: &str, user: &[u8; 32], nonce: u64) -> [u8; 32] {
    // Convert string bet_id to deterministic u64
    let mut hasher = Sha256::new();
    hasher.update(bet_id.as_bytes());
    let bet_id_hash = hasher.finalize();
    let bet_id_u64 = u64::from_be_bytes([
        bet_id_hash[0], bet_id_hash[1], bet_id_hash[2], bet_id_hash[3],
        bet_id_hash[4], bet_id_hash[5], bet_id_hash[6], bet_id_hash[7],
    ]);
    
    generate_vrf_message(bet_id_u64, user, nonce)
}

/// Create a complete VRF proof by signing a message with the given keypair
/// 
/// This combines message generation and signing into a single operation
pub fn create_vrf_proof(
    keypair: &VRFKeypair,
    bet_id: u64,
    user: &[u8; 32],
    nonce: u64,
) -> Result<VRFProof, VRFKeypairError> {
    let message = generate_vrf_message(bet_id, user, nonce);
    let (signature, outcome) = keypair.sign_and_derive_outcome(&message)?;
    let public_key = keypair.public_key_bytes();
    
    Ok(VRFProof::new(message, signature, public_key, outcome))
}

/// Create a VRF proof from string bet_id (for backward compatibility)
pub fn create_vrf_proof_from_string(
    keypair: &VRFKeypair,
    bet_id: &str,
    user: &[u8; 32],
    nonce: u64,
) -> Result<VRFProof, VRFKeypairError> {
    let message = generate_vrf_message_from_string(bet_id, user, nonce);
    let (signature, outcome) = keypair.sign_and_derive_outcome(&message)?;
    let public_key = keypair.public_key_bytes();
    
    Ok(VRFProof::new(message, signature, public_key, outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vrf_proof_creation() {
        let message = [1u8; 32];
        let signature = [2u8; 64];
        let public_key = [3u8; 32];
        let outcome = true;

        let proof = VRFProof::new(message, signature, public_key, outcome);

        assert_eq!(proof.message, message);
        assert_eq!(proof.signature, signature);
        assert_eq!(proof.public_key, public_key);
        assert_eq!(proof.outcome, outcome);
    }

    #[test]
    fn test_outcome_string() {
        let proof_heads = VRFProof::new([0u8; 32], [0u8; 64], [0u8; 32], true);
        let proof_tails = VRFProof::new([0u8; 32], [0u8; 64], [0u8; 32], false);

        assert_eq!(proof_heads.outcome_string(), "heads");
        assert_eq!(proof_tails.outcome_string(), "tails");
    }

    #[test]
    fn test_vrf_message_generation() {
        let bet_id = 12345u64;
        let user = [0x42u8; 32]; // Test user address
        let nonce = 67890u64;

        let message1 = generate_vrf_message(bet_id, &user, nonce);
        let message2 = generate_vrf_message(bet_id, &user, nonce);

        // Same inputs should produce same message (deterministic)
        assert_eq!(message1, message2);

        // Different inputs should produce different messages
        let different_bet = generate_vrf_message(bet_id + 1, &user, nonce);
        assert_ne!(message1, different_bet);

        let different_user = {
            let mut user2 = user;
            user2[0] = 0x43;
            generate_vrf_message(bet_id, &user2, nonce)
        };
        assert_ne!(message1, different_user);

        let different_nonce = generate_vrf_message(bet_id, &user, nonce + 1);
        assert_ne!(message1, different_nonce);
    }

    #[test]
    fn test_vrf_message_from_string() {
        let bet_id_str = "bet_12345";
        let user = [0x42u8; 32];
        let nonce = 67890u64;

        let message1 = generate_vrf_message_from_string(bet_id_str, &user, nonce);
        let message2 = generate_vrf_message_from_string(bet_id_str, &user, nonce);

        // Same inputs should produce same message (deterministic)
        assert_eq!(message1, message2);

        // Different string bet_ids should produce different messages
        let different_bet = generate_vrf_message_from_string("bet_54321", &user, nonce);
        assert_ne!(message1, different_bet);
    }

    #[test]
    fn test_message_format_consistency() {
        // Test that our message format is consistent and reproducible
        let bet_id = 0x123456789ABCDEFFu64;
        let user = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
        ];
        let nonce = 0xFEDCBA9876543210u64;

        let message = generate_vrf_message(bet_id, &user, nonce);

        // We should get a 32-byte message
        assert_eq!(message.len(), 32);

        // The message should be the SHA-256 of the concatenated inputs
        let mut hasher = Sha256::new();
        hasher.update(bet_id.to_be_bytes());
        hasher.update(&user);
        hasher.update(nonce.to_be_bytes());
        let expected: [u8; 32] = hasher.finalize().into();

        assert_eq!(message, expected);
    }

    #[test]
    fn test_create_vrf_proof_with_keypair() {
        use crate::vrf::keypair::VRFKeypair;

        let keypair = VRFKeypair::generate().expect("Failed to generate keypair");
        let bet_id = 12345u64;
        let user = [0x42u8; 32];
        let nonce = 67890u64;

        let proof = create_vrf_proof(&keypair, bet_id, &user, nonce)
            .expect("Failed to create VRF proof");

        // Verify the proof structure
        assert_eq!(proof.public_key, keypair.public_key_bytes());
        assert_eq!(proof.message, generate_vrf_message(bet_id, &user, nonce));

        // Verify the proof is valid
        assert!(proof.verify(), "Generated VRF proof should be valid");

        // Test deterministic behavior
        let proof2 = create_vrf_proof(&keypair, bet_id, &user, nonce)
            .expect("Failed to create second VRF proof");
        
        assert_eq!(proof.message, proof2.message);
        assert_eq!(proof.signature, proof2.signature);
        assert_eq!(proof.outcome, proof2.outcome);
    }
}