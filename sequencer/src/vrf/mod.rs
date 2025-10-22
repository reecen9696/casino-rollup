//! VRF (Verifiable Random Functions) module for ZK Casino
//! 
//! This module provides cryptographically verifiable randomness for fair coin flip outcomes.
//! It implements ed25519-based VRF signatures that can be independently verified by clients.

pub mod keypair;

pub use keypair::{VRFKeypair, VRFKeypairError};

use serde::{Deserialize, Serialize};

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
}