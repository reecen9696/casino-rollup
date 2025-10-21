use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash;
use anchor_lang::solana_program::sysvar::instructions;

mod groth16;
mod verifying_key;

use groth16::{verify_groth16_proof, Groth16Proof};
use verifying_key::get_embedded_verifying_key;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod verifier {
    use super::*;

    /// Initialize the verifier state
    pub fn initialize_verifier(ctx: Context<InitializeVerifier>) -> Result<()> {
        let verifier_state = &mut ctx.accounts.verifier_state;
        verifier_state.authority = ctx.accounts.authority.key();
        verifier_state.vault_program = ctx.accounts.vault_program.key();
        verifier_state.total_batches_processed = 0;
        verifier_state.total_bets_settled = 0;
        verifier_state.is_paused = false;

        msg!(
            "Verifier initialized with authority: {}",
            verifier_state.authority
        );
        Ok(())
    }

    /// Verify and settle a batch of bets (Phase 2: stub implementation with events only)
    pub fn verify_and_settle(
        ctx: Context<VerifyAndSettle>,
        batch_data: BatchSettlementData,
        proof: Vec<u8>, // Placeholder proof for Phase 2
    ) -> Result<()> {
        require!(
            !ctx.accounts.verifier_state.is_paused,
            VerifierError::VerifierPaused
        );
        require!(!batch_data.bets.is_empty(), VerifierError::EmptyBatch);
        require!(
            batch_data.bets.len() <= MAX_BATCH_SIZE,
            VerifierError::BatchTooLarge
        );
        require!(!proof.is_empty(), VerifierError::EmptyProof);

        let verifier_state = &mut ctx.accounts.verifier_state;

        // Phase 3d: Real ZK proof verification using Groth16 and BN254 syscalls
        msg!(
            "Verifying batch with {} bets using Groth16 proof ({} bytes)",
            batch_data.bets.len(),
            proof.len()
        );

        // Parse and verify the Groth16 proof
        let groth16_proof =
            Groth16Proof::from_bytes(&proof).map_err(|_| VerifierError::InvalidProofFormat)?;

        // Load embedded verifying key
        let verifying_key =
            get_embedded_verifying_key().map_err(|_| VerifierError::InvalidVerifyingKey)?;

        // Prepare public inputs for verification
        // For our circuit, we expect one public input: the batch hash
        let batch_hash = compute_batch_hash(&batch_data);
        let public_inputs = vec![batch_hash];

        // Perform Groth16 verification
        let verification_result =
            verify_groth16_proof(&groth16_proof, &verifying_key, &public_inputs);

        match verification_result {
            Ok(true) => {
                msg!("✓ Groth16 proof verification successful");
            }
            Ok(false) => {
                msg!("✗ Groth16 proof verification failed: invalid proof");
                return Err(VerifierError::InvalidProof.into());
            }
            Err(e) => {
                msg!("✗ Groth16 proof verification error: {:?}", e);
                return Err(VerifierError::ProofVerificationFailed.into());
            }
        }

        // Validate batch arithmetic (basic checks for Phase 2)
        let mut total_house_delta: i64 = 0;
        for bet_settlement in &batch_data.bets {
            require!(
                bet_settlement.bet_amount > 0,
                VerifierError::InvalidBetAmount
            );

            // Validate outcome is boolean (0 or 1)
            require!(
                bet_settlement.outcome == 0 || bet_settlement.outcome == 1,
                VerifierError::InvalidOutcome
            );

            // Calculate payout based on outcome and bet amount
            let expected_payout = if bet_settlement.outcome == bet_settlement.user_guess {
                bet_settlement.bet_amount * 2 // Win: 2x payout
            } else {
                0 // Loss: no payout
            };

            require!(
                bet_settlement.payout == expected_payout,
                VerifierError::InvalidPayout
            );

            // Calculate delta for house (negative when user wins)
            let house_delta = bet_settlement.bet_amount as i64 - bet_settlement.payout as i64;
            total_house_delta = total_house_delta
                .checked_add(house_delta)
                .ok_or(VerifierError::MathOverflow)?;
        }

        // Emit settlement event for each bet
        for bet_settlement in &batch_data.bets {
            emit!(BetSettlementEvent {
                bet_id: bet_settlement.bet_id,
                user: bet_settlement.user,
                bet_amount: bet_settlement.bet_amount,
                user_guess: bet_settlement.user_guess,
                outcome: bet_settlement.outcome,
                payout: bet_settlement.payout,
                is_win: bet_settlement.outcome == bet_settlement.user_guess,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        // Emit batch settlement event
        emit!(BatchSettlementEvent {
            batch_id: batch_data.batch_id,
            sequencer: ctx.accounts.sequencer.key(),
            batch_size: batch_data.bets.len() as u32,
            house_delta: total_house_delta,
            proof_hash: hash::hash(&proof).to_bytes(),
            settlement_timestamp: Clock::get()?.unix_timestamp,
        });

        // Update verifier state
        verifier_state.total_batches_processed = verifier_state
            .total_batches_processed
            .checked_add(1)
            .ok_or(VerifierError::MathOverflow)?;

        verifier_state.total_bets_settled = verifier_state
            .total_bets_settled
            .checked_add(batch_data.bets.len() as u64)
            .ok_or(VerifierError::MathOverflow)?;

        msg!(
            "Batch {} settled successfully: {} bets, house delta: {}",
            batch_data.batch_id,
            batch_data.bets.len(),
            total_house_delta
        );

        Ok(())
    }

    /// Verify a single ZK proof (Phase 3d implementation with real Groth16)
    pub fn verify_proof(ctx: Context<VerifyProof>, proof: Vec<u8>) -> Result<()> {
        require!(
            !ctx.accounts.verifier_state.is_paused,
            VerifierError::VerifierPaused
        );
        require!(!proof.is_empty(), VerifierError::EmptyProof);
        require!(proof.len() <= MAX_PROOF_SIZE, VerifierError::ProofTooLarge);

        // Phase 3d: Real Groth16 verification using Solana's BN254 syscalls
        msg!(
            "Performing Groth16 proof verification: {} bytes",
            proof.len()
        );

        // Parse the Groth16 proof
        let groth16_proof =
            Groth16Proof::from_bytes(&proof).map_err(|_| VerifierError::InvalidProofFormat)?;

        // Load embedded verifying key
        let verifying_key =
            get_embedded_verifying_key().map_err(|_| VerifierError::InvalidVerifyingKey)?;

        // For standalone proof verification, we need public inputs
        // This would typically be provided as additional parameters
        // For now, use empty public inputs (circuit dependent)
        let public_inputs: Vec<[u8; 32]> = vec![];

        // Perform Groth16 verification
        let is_valid = match verify_groth16_proof(&groth16_proof, &verifying_key, &public_inputs) {
            Ok(valid) => {
                if valid {
                    msg!("✓ Groth16 proof verification successful");
                } else {
                    msg!("✗ Groth16 proof verification failed: invalid proof");
                }
                valid
            }
            Err(e) => {
                msg!("✗ Groth16 proof verification error: {:?}", e);
                return Err(VerifierError::ProofVerificationFailed.into());
            }
        };

        emit!(ProofVerificationEvent {
            proof_hash: hash::hash(&proof).to_bytes(),
            verifier: ctx.accounts.verifier_state.key(),
            is_valid,
            timestamp: Clock::get()?.unix_timestamp,
        });

        if !is_valid {
            return Err(VerifierError::InvalidProof.into());
        }

        Ok(())
    }

    /// Pause/unpause verifier operations (admin only)
    pub fn set_verifier_pause_state(
        ctx: Context<SetVerifierPauseState>,
        is_paused: bool,
    ) -> Result<()> {
        let verifier_state = &mut ctx.accounts.verifier_state;
        verifier_state.is_paused = is_paused;

        msg!("Verifier pause state set to: {}", is_paused);
        Ok(())
    }

    /// Update vault program address (admin only)
    pub fn update_vault_program(
        ctx: Context<UpdateVaultProgram>,
        new_vault_program: Pubkey,
    ) -> Result<()> {
        let verifier_state = &mut ctx.accounts.verifier_state;
        verifier_state.vault_program = new_vault_program;

        msg!("Vault program updated to: {}", new_vault_program);
        Ok(())
    }
}

// Constants
const MAX_BATCH_SIZE: usize = 100;
const MAX_PROOF_SIZE: usize = 2048; // 2KB for Phase 2, will be smaller for Groth16

// Account structures
#[account]
pub struct VerifierState {
    pub authority: Pubkey,
    pub vault_program: Pubkey,
    pub total_batches_processed: u64,
    pub total_bets_settled: u64,
    pub is_paused: bool,
}

// Data structures
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct BatchSettlementData {
    pub batch_id: u64,
    pub sequencer_nonce: u64,
    pub bets: Vec<BetSettlement>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct BetSettlement {
    pub bet_id: u64,
    pub user: Pubkey,
    pub bet_amount: u64,
    pub user_guess: u8, // 0 or 1 for coin flip
    pub outcome: u8,    // 0 or 1 actual outcome
    pub payout: u64,    // Calculated payout amount
}

// Context structures
#[derive(Accounts)]
pub struct InitializeVerifier<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<VerifierState>(),
        seeds = [b"verifier_state"],
        bump
    )]
    pub verifier_state: Account<'info, VerifierState>,
    /// CHECK: The vault program that this verifier will interact with
    pub vault_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyAndSettle<'info> {
    #[account(
        mut,
        seeds = [b"verifier_state"],
        bump
    )]
    pub verifier_state: Account<'info, VerifierState>,
    /// CHECK: The sequencer submitting the batch (signature validation happens in sequencer)
    pub sequencer: Signer<'info>,
    /// CHECK: Instructions sysvar for CPI validation
    #[account(address = instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct VerifyProof<'info> {
    #[account(
        seeds = [b"verifier_state"],
        bump
    )]
    pub verifier_state: Account<'info, VerifierState>,
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetVerifierPauseState<'info> {
    #[account(
        mut,
        seeds = [b"verifier_state"],
        bump,
        has_one = authority
    )]
    pub verifier_state: Account<'info, VerifierState>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateVaultProgram<'info> {
    #[account(
        mut,
        seeds = [b"verifier_state"],
        bump,
        has_one = authority
    )]
    pub verifier_state: Account<'info, VerifierState>,
    pub authority: Signer<'info>,
}

// Events
#[event]
pub struct BetSettlementEvent {
    pub bet_id: u64,
    pub user: Pubkey,
    pub bet_amount: u64,
    pub user_guess: u8,
    pub outcome: u8,
    pub payout: u64,
    pub is_win: bool,
    pub timestamp: i64,
}

#[event]
pub struct BatchSettlementEvent {
    pub batch_id: u64,
    pub sequencer: Pubkey,
    pub batch_size: u32,
    pub house_delta: i64,
    pub proof_hash: [u8; 32],
    pub settlement_timestamp: i64,
}

#[event]
pub struct ProofVerificationEvent {
    pub proof_hash: [u8; 32],
    pub verifier: Pubkey,
    pub is_valid: bool,
    pub timestamp: i64,
}

/// Compute the batch hash for use as public input to the ZK circuit
fn compute_batch_hash(batch_data: &BatchSettlementData) -> [u8; 32] {
    // Serialize batch data for hashing
    let mut hasher_data = Vec::new();

    // Add batch metadata
    hasher_data.extend_from_slice(&batch_data.batch_id.to_le_bytes());
    hasher_data.extend_from_slice(&(batch_data.bets.len() as u32).to_le_bytes());

    // Add each bet settlement data
    for bet in &batch_data.bets {
        hasher_data.extend_from_slice(&bet.bet_id.to_le_bytes());
        hasher_data.extend_from_slice(&bet.user.to_bytes());
        hasher_data.extend_from_slice(&bet.bet_amount.to_le_bytes());
        hasher_data.push(bet.user_guess);
        hasher_data.push(bet.outcome);
        hasher_data.extend_from_slice(&bet.payout.to_le_bytes());
    }

    // Hash the serialized data
    let hash_result = hash::hash(&hasher_data);
    hash_result.to_bytes()
}

// Error codes
#[error_code]
pub enum VerifierError {
    #[msg("Proof data cannot be empty")]
    EmptyProof,
    #[msg("Proof data is too large")]
    ProofTooLarge,
    #[msg("Invalid proof format")]
    InvalidProofFormat,
    #[msg("Batch cannot be empty")]
    EmptyBatch,
    #[msg("Batch size exceeds maximum allowed")]
    BatchTooLarge,
    #[msg("Invalid bet amount")]
    InvalidBetAmount,
    #[msg("Invalid outcome value")]
    InvalidOutcome,
    #[msg("Invalid payout calculation")]
    InvalidPayout,
    #[msg("Verifier operations are paused")]
    VerifierPaused,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Math underflow")]
    MathUnderflow,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Invalid sequencer")]
    InvalidSequencer,
    #[msg("Proof verification failed")]
    ProofVerificationFailed,
    #[msg("Invalid proof - verification returned false")]
    InvalidProof,
    #[msg("Invalid verifying key")]
    InvalidVerifyingKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_error_codes() {
        let error = VerifierError::EmptyProof;
        assert_eq!(error.to_string(), "Proof data cannot be empty");

        let error = VerifierError::BatchTooLarge;
        assert_eq!(error.to_string(), "Batch size exceeds maximum allowed");
    }

    #[test]
    fn test_bet_settlement_data() {
        let bet = BetSettlement {
            bet_id: 123,
            user: Pubkey::default(),
            bet_amount: 1000,
            user_guess: 1,
            outcome: 1,
            payout: 2000,
        };

        assert_eq!(bet.bet_amount, 1000);
        assert_eq!(bet.payout, 2000);
        assert_eq!(bet.outcome, bet.user_guess);
    }

    #[test]
    fn test_batch_size_constraints() {
        // Validate constants are reasonable
        assert!(MAX_BATCH_SIZE > 0, "MAX_BATCH_SIZE must be positive");
        assert!(
            MAX_BATCH_SIZE <= 1000,
            "MAX_BATCH_SIZE should be reasonable"
        );
        assert!(MAX_PROOF_SIZE > 0, "MAX_PROOF_SIZE must be positive");
    }
}
