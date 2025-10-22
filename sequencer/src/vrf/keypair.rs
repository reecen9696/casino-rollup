//! VRF Keypair management for ed25519-based verifiable random functions
//! 
//! This module handles generation, storage, and loading of ed25519 keypairs
//! used for VRF signature generation in the ZK Casino system.

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during VRF keypair operations
#[derive(Error, Debug)]
pub enum VRFKeypairError {
    #[error("Failed to generate keypair: {0}")]
    GenerationError(String),
    
    #[error("Failed to load keypair from file: {0}")]
    LoadError(String),
    
    #[error("Failed to save keypair to file: {0}")]
    SaveError(String),
    
    #[error("Invalid keypair data: {0}")]
    InvalidKeypair(String),
    
    #[error("Environment variable error: {0}")]
    EnvironmentError(String),
    
    #[error("Signature generation failed: {0}")]
    SignatureError(String),
}

/// VRF keypair wrapper providing high-level operations
#[derive(Debug)]
pub struct VRFKeypair {
    keypair: Keypair,
}

/// Serializable format for keypair storage
#[derive(Serialize, Deserialize)]
struct KeypairData {
    secret_key: [u8; 32],
    public_key: [u8; 32],
}

impl VRFKeypair {
    /// Generate a new VRF keypair using cryptographically secure randomness
    pub fn generate() -> Result<Self, VRFKeypairError> {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        
        Ok(Self { keypair })
    }

    /// Load a VRF keypair from file path specified in environment variable
    /// Environment variable should contain the path to the keypair file
    pub fn from_env(env_var: &str) -> Result<Self, VRFKeypairError> {
        let keypair_path = std::env::var(env_var)
            .map_err(|e| VRFKeypairError::EnvironmentError(
                format!("Environment variable '{}' not found: {}", env_var, e)
            ))?;
        
        Self::from_file(&keypair_path)
    }

    /// Load a VRF keypair from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, VRFKeypairError> {
        let path = path.as_ref();
        
        let data = fs::read_to_string(path)
            .map_err(|e| VRFKeypairError::LoadError(
                format!("Failed to read file '{}': {}", path.display(), e)
            ))?;
        
        let keypair_data: KeypairData = serde_json::from_str(&data)
            .map_err(|e| VRFKeypairError::LoadError(
                format!("Failed to parse keypair data: {}", e)
            ))?;
        
        Self::from_bytes(&keypair_data.secret_key)
    }

    /// Create a VRF keypair from raw secret key bytes
    pub fn from_bytes(secret_bytes: &[u8; 32]) -> Result<Self, VRFKeypairError> {
        let secret_key = SecretKey::from_bytes(secret_bytes)
            .map_err(|e| VRFKeypairError::InvalidKeypair(
                format!("Invalid secret key: {}", e)
            ))?;
        
        let public_key = PublicKey::from(&secret_key);
        let keypair = Keypair { secret: secret_key, public: public_key };
        
        Ok(Self { keypair })
    }

    /// Save the keypair to a file in JSON format
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), VRFKeypairError> {
        let path = path.as_ref();
        
        let keypair_data = KeypairData {
            secret_key: self.keypair.secret.to_bytes(),
            public_key: self.keypair.public.to_bytes(),
        };
        
        let json_data = serde_json::to_string_pretty(&keypair_data)
            .map_err(|e| VRFKeypairError::SaveError(
                format!("Failed to serialize keypair: {}", e)
            ))?;
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| VRFKeypairError::SaveError(
                    format!("Failed to create directory '{}': {}", parent.display(), e)
                ))?;
        }
        
        fs::write(path, json_data)
            .map_err(|e| VRFKeypairError::SaveError(
                format!("Failed to write file '{}': {}", path.display(), e)
            ))?;
        
        Ok(())
    }

    /// Get the public key as bytes for storage and verification
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.keypair.public.to_bytes()
    }

    /// Get the public key for external use
    pub fn public_key(&self) -> &PublicKey {
        &self.keypair.public
    }

    /// Sign a message using VRF (ed25519 signature)
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64], VRFKeypairError> {
        let signature = self.keypair.sign(message);
        Ok(signature.to_bytes())
    }

    /// Generate VRF signature and derive outcome for a given message
    /// Returns (signature_bytes, outcome) where outcome is derived from LSB of signature
    pub fn sign_and_derive_outcome(&self, message: &[u8]) -> Result<([u8; 64], bool), VRFKeypairError> {
        let signature_bytes = self.sign(message)?;
        
        // Derive outcome from least significant bit of first byte of signature
        // LSB = 0 means heads (true), LSB = 1 means tails (false)
        let outcome = (signature_bytes[0] & 1) == 0;
        
        Ok((signature_bytes, outcome))
    }

    /// Validate that this keypair can sign and the signature can be verified
    pub fn validate(&self) -> Result<(), VRFKeypairError> {
        let test_message = b"vrf_keypair_validation_test";
        let signature_bytes = self.sign(test_message)?;
        
        let signature = Signature::try_from(&signature_bytes[..])
            .map_err(|e| VRFKeypairError::InvalidKeypair(
                format!("Generated invalid signature: {}", e)
            ))?;
        
        use ed25519_dalek::Verifier;
        self.keypair.public.verify(test_message, &signature)
            .map_err(|e| VRFKeypairError::InvalidKeypair(
                format!("Keypair failed validation: {}", e)
            ))?;
        
        Ok(())
    }

    /// Generate a new keypair and save it to the specified path
    pub fn generate_and_save<P: AsRef<Path>>(path: P) -> Result<Self, VRFKeypairError> {
        let keypair = Self::generate()?;
        keypair.save_to_file(&path)?;
        Ok(keypair)
    }

    /// Load keypair from environment variable, or generate and save a new one if not found
    pub fn load_or_generate(env_var: &str, default_path: &str) -> Result<Self, VRFKeypairError> {
        match Self::from_env(env_var) {
            Ok(keypair) => {
                // Validate the loaded keypair
                keypair.validate()?;
                Ok(keypair)
            }
            Err(VRFKeypairError::EnvironmentError(_)) => {
                // Environment variable not set, try default path
                match Self::from_file(default_path) {
                    Ok(keypair) => {
                        keypair.validate()?;
                        Ok(keypair)
                    }
                    Err(_) => {
                        // Generate new keypair and save to default path
                        log::info!("Generating new VRF keypair at: {}", default_path);
                        let keypair = Self::generate_and_save(default_path)?;
                        keypair.validate()?;
                        Ok(keypair)
                    }
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_keypair_generation() {
        let keypair = VRFKeypair::generate().expect("Should generate keypair");
        assert!(keypair.validate().is_ok());
    }

    #[test]
    fn test_keypair_signing() {
        let keypair = VRFKeypair::generate().expect("Should generate keypair");
        let message = b"test message";
        
        let signature = keypair.sign(message).expect("Should sign message");
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn test_sign_and_derive_outcome() {
        let keypair = VRFKeypair::generate().expect("Should generate keypair");
        let message = b"test message for outcome";
        
        let (signature, outcome) = keypair.sign_and_derive_outcome(message)
            .expect("Should sign and derive outcome");
        
        // Verify outcome derivation is consistent
        let expected_outcome = (signature[0] & 1) == 0;
        assert_eq!(outcome, expected_outcome);
    }

    #[test]
    fn test_keypair_save_and_load() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let keypair_path = temp_dir.path().join("test_keypair.json");
        
        // Generate and save keypair
        let original_keypair = VRFKeypair::generate().expect("Should generate keypair");
        original_keypair.save_to_file(&keypair_path).expect("Should save keypair");
        
        // Load keypair
        let loaded_keypair = VRFKeypair::from_file(&keypair_path).expect("Should load keypair");
        
        // Verify they're the same
        assert_eq!(
            original_keypair.public_key_bytes(),
            loaded_keypair.public_key_bytes()
        );
        
        // Verify loaded keypair works
        assert!(loaded_keypair.validate().is_ok());
    }

    #[test]
    fn test_keypair_from_bytes() {
        let original_keypair = VRFKeypair::generate().expect("Should generate keypair");
        let secret_bytes = original_keypair.keypair.secret.to_bytes();
        
        let restored_keypair = VRFKeypair::from_bytes(&secret_bytes)
            .expect("Should create from bytes");
        
        assert_eq!(
            original_keypair.public_key_bytes(),
            restored_keypair.public_key_bytes()
        );
    }

    #[test]
    fn test_load_or_generate() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let keypair_path = temp_dir.path().join("auto_keypair.json");
        
        // Should generate new keypair when file doesn't exist
        let keypair1 = VRFKeypair::load_or_generate("NONEXISTENT_VAR", &keypair_path.to_string_lossy())
            .expect("Should load or generate");
        
        assert!(keypair_path.exists());
        assert!(keypair1.validate().is_ok());
        
        // Should load existing keypair
        let keypair2 = VRFKeypair::load_or_generate("NONEXISTENT_VAR", &keypair_path.to_string_lossy())
            .expect("Should load existing keypair");
        
        assert_eq!(keypair1.public_key_bytes(), keypair2.public_key_bytes());
    }

    #[test]
    fn test_deterministic_outcomes() {
        let keypair = VRFKeypair::generate().expect("Should generate keypair");
        let message = b"deterministic test message";
        
        // Sign the same message multiple times
        let (sig1, outcome1) = keypair.sign_and_derive_outcome(message)
            .expect("Should sign message");
        let (sig2, outcome2) = keypair.sign_and_derive_outcome(message)
            .expect("Should sign message again");
        
        // Results should be identical (deterministic)
        assert_eq!(sig1, sig2);
        assert_eq!(outcome1, outcome2);
    }

    #[test]
    fn test_public_key_bytes() {
        let keypair = VRFKeypair::generate().expect("Should generate keypair");
        let pub_bytes = keypair.public_key_bytes();
        
        assert_eq!(pub_bytes.len(), 32);
        assert_eq!(pub_bytes, keypair.keypair.public.to_bytes());
    }
}