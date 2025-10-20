use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111112");

#[program]
pub mod verifier {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        msg!("Hello World from Verifier Program!");
        Ok(())
    }

    pub fn verify_proof(_ctx: Context<VerifyContext>, proof_data: Vec<u8>) -> Result<()> {
        msg!("Verifying proof with {} bytes", proof_data.len());
        require!(!proof_data.is_empty(), VerifierError::EmptyProof);
        require!(proof_data.len() <= 1000, VerifierError::ProofTooLarge);
        
        // Placeholder verification logic
        msg!("Proof verification successful");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct VerifyContext {}

#[error_code]
pub enum VerifierError {
    #[msg("Proof data cannot be empty")]
    EmptyProof,
    #[msg("Proof data is too large")]
    ProofTooLarge,
    #[msg("Invalid proof format")]
    InvalidProofFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_error_codes() {
        let error = VerifierError::EmptyProof;
        assert_eq!(error.to_string(), "Proof data cannot be empty");

        let error = VerifierError::ProofTooLarge;
        assert_eq!(error.to_string(), "Proof data is too large");

        let error = VerifierError::InvalidProofFormat;
        assert_eq!(error.to_string(), "Invalid proof format");
    }
}