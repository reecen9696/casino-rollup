use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("11111111111111111111111111111111");

#[program]
pub mod vault {
    use super::*;

    /// Initialize the global vault state
    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.authority = ctx.accounts.authority.key();
        vault_state.total_users = 0;
        vault_state.total_sol_deposited = 0;
        vault_state.total_usdc_deposited = 0;
        vault_state.is_paused = false;
        
        msg!("Vault initialized with authority: {}", vault_state.authority);
        Ok(())
    }

    /// Create a user vault account
    pub fn initialize_user_vault(ctx: Context<InitializeUserVault>) -> Result<()> {
        let user_vault = &mut ctx.accounts.user_vault;
        user_vault.owner = ctx.accounts.user.key();
        user_vault.sol_balance = 0;
        user_vault.usdc_balance = 0;
        user_vault.bet_count = 0;
        user_vault.total_winnings = 0;
        user_vault.total_losses = 0;
        user_vault.created_at = Clock::get()?.unix_timestamp;
        
        // Update global vault state
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.total_users = vault_state.total_users.checked_add(1)
            .ok_or(VaultError::MathOverflow)?;
        
        msg!("User vault created for: {}", user_vault.owner);
        Ok(())
    }

    /// Deposit SOL into user vault (mocked for Phase 2)
    pub fn deposit_sol(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
        require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);
        require!(amount > 0, VaultError::InvalidAmount);
        
        let user_vault = &mut ctx.accounts.user_vault;
        user_vault.sol_balance = user_vault.sol_balance.checked_add(amount)
            .ok_or(VaultError::MathOverflow)?;
        
        // Update global state
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.total_sol_deposited = vault_state.total_sol_deposited.checked_add(amount)
            .ok_or(VaultError::MathOverflow)?;
        
        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            token_type: TokenType::Sol,
            amount,
            new_balance: user_vault.sol_balance,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("SOL deposit: {} lamports for user: {}", amount, ctx.accounts.user.key());
        Ok(())
    }

    /// Deposit USDC into user vault (mocked for Phase 2)
    pub fn deposit_usdc(ctx: Context<DepositUsdc>, amount: u64) -> Result<()> {
        require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);
        require!(amount > 0, VaultError::InvalidAmount);
        
        let user_vault = &mut ctx.accounts.user_vault;
        user_vault.usdc_balance = user_vault.usdc_balance.checked_add(amount)
            .ok_or(VaultError::MathOverflow)?;
        
        // Update global state
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.total_usdc_deposited = vault_state.total_usdc_deposited.checked_add(amount)
            .ok_or(VaultError::MathOverflow)?;
        
        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            token_type: TokenType::Usdc,
            amount,
            new_balance: user_vault.usdc_balance,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("USDC deposit: {} tokens for user: {}", amount, ctx.accounts.user.key());
        Ok(())
    }

    /// Withdraw SOL from user vault
    pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);
        require!(amount > 0, VaultError::InvalidAmount);
        
        let user_vault = &mut ctx.accounts.user_vault;
        require!(user_vault.sol_balance >= amount, VaultError::InsufficientBalance);
        
        user_vault.sol_balance = user_vault.sol_balance.checked_sub(amount)
            .ok_or(VaultError::MathUnderflow)?;
        
        // Update global state
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.total_sol_deposited = vault_state.total_sol_deposited.checked_sub(amount)
            .ok_or(VaultError::MathUnderflow)?;
        
        emit!(WithdrawEvent {
            user: ctx.accounts.user.key(),
            token_type: TokenType::Sol,
            amount,
            new_balance: user_vault.sol_balance,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("SOL withdrawal: {} lamports for user: {}", amount, ctx.accounts.user.key());
        Ok(())
    }

    /// Withdraw USDC from user vault
    pub fn withdraw_usdc(ctx: Context<WithdrawUsdc>, amount: u64) -> Result<()> {
        require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);
        require!(amount > 0, VaultError::InvalidAmount);
        
        let user_vault = &mut ctx.accounts.user_vault;
        require!(user_vault.usdc_balance >= amount, VaultError::InsufficientBalance);
        
        user_vault.usdc_balance = user_vault.usdc_balance.checked_sub(amount)
            .ok_or(VaultError::MathUnderflow)?;
        
        // Update global state
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.total_usdc_deposited = vault_state.total_usdc_deposited.checked_sub(amount)
            .ok_or(VaultError::MathUnderflow)?;
        
        emit!(WithdrawEvent {
            user: ctx.accounts.user.key(),
            token_type: TokenType::Usdc,
            amount,
            new_balance: user_vault.usdc_balance,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("USDC withdrawal: {} tokens for user: {}", amount, ctx.accounts.user.key());
        Ok(())
    }

    /// Update user vault after settlement (called by verifier program)
    pub fn update_balances(
        ctx: Context<UpdateBalances>,
        sol_delta: i64,
        usdc_delta: i64,
        is_win: bool,
        bet_amount: u64,
    ) -> Result<()> {
        require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);
        
        let user_vault = &mut ctx.accounts.user_vault;
        
        // Update SOL balance
        if sol_delta >= 0 {
            user_vault.sol_balance = user_vault.sol_balance.checked_add(sol_delta as u64)
                .ok_or(VaultError::MathOverflow)?;
        } else {
            let abs_delta = (-sol_delta) as u64;
            require!(user_vault.sol_balance >= abs_delta, VaultError::InsufficientBalance);
            user_vault.sol_balance = user_vault.sol_balance.checked_sub(abs_delta)
                .ok_or(VaultError::MathUnderflow)?;
        }
        
        // Update USDC balance
        if usdc_delta >= 0 {
            user_vault.usdc_balance = user_vault.usdc_balance.checked_add(usdc_delta as u64)
                .ok_or(VaultError::MathOverflow)?;
        } else {
            let abs_delta = (-usdc_delta) as u64;
            require!(user_vault.usdc_balance >= abs_delta, VaultError::InsufficientBalance);
            user_vault.usdc_balance = user_vault.usdc_balance.checked_sub(abs_delta)
                .ok_or(VaultError::MathUnderflow)?;
        }
        
        // Update bet statistics
        user_vault.bet_count = user_vault.bet_count.checked_add(1)
            .ok_or(VaultError::MathOverflow)?;
        
        if is_win {
            user_vault.total_winnings = user_vault.total_winnings.checked_add(bet_amount)
                .ok_or(VaultError::MathOverflow)?;
        } else {
            user_vault.total_losses = user_vault.total_losses.checked_add(bet_amount)
                .ok_or(VaultError::MathOverflow)?;
        }
        
        emit!(BalanceUpdateEvent {
            user: user_vault.owner,
            sol_delta,
            usdc_delta,
            new_sol_balance: user_vault.sol_balance,
            new_usdc_balance: user_vault.usdc_balance,
            is_win,
            bet_count: user_vault.bet_count,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("Balance updated for user: {}, SOL: {}, USDC: {}", 
             user_vault.owner, sol_delta, usdc_delta);
        Ok(())
    }

    /// Pause/unpause vault operations (admin only)
    pub fn set_pause_state(ctx: Context<SetPauseState>, is_paused: bool) -> Result<()> {
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.is_paused = is_paused;
        
        msg!("Vault pause state set to: {}", is_paused);
        Ok(())
    }
}

// Account structures
#[account]
pub struct VaultState {
    pub authority: Pubkey,
    pub total_users: u64,
    pub total_sol_deposited: u64,
    pub total_usdc_deposited: u64,
    pub is_paused: bool,
}

#[account]
pub struct UserVault {
    pub owner: Pubkey,
    pub sol_balance: u64,
    pub usdc_balance: u64,
    pub bet_count: u64,
    pub total_winnings: u64,
    pub total_losses: u64,
    pub created_at: i64,
}

// Context structures
#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<VaultState>(),
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeUserVault<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + std::mem::size_of::<UserVault>(),
        seeds = [b"user_vault", user.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(
        mut,
        seeds = [b"user_vault", user.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct DepositUsdc<'info> {
    #[account(
        mut,
        seeds = [b"user_vault", user.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(
        mut,
        seeds = [b"user_vault", user.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawUsdc<'info> {
    #[account(
        mut,
        seeds = [b"user_vault", user.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateBalances<'info> {
    #[account(
        mut,
        seeds = [b"user_vault", user_vault.owner.as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    /// CHECK: This should be the verifier program calling this instruction
    pub verifier_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetPauseState<'info> {
    #[account(
        mut,
        seeds = [b"vault_state"],
        bump,
        has_one = authority
    )]
    pub vault_state: Account<'info, VaultState>,
    pub authority: Signer<'info>,
}

// Events
#[event]
pub struct DepositEvent {
    pub user: Pubkey,
    pub token_type: TokenType,
    pub amount: u64,
    pub new_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct WithdrawEvent {
    pub user: Pubkey,
    pub token_type: TokenType,
    pub amount: u64,
    pub new_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct BalanceUpdateEvent {
    pub user: Pubkey,
    pub sol_delta: i64,
    pub usdc_delta: i64,
    pub new_sol_balance: u64,
    pub new_usdc_balance: u64,
    pub is_win: bool,
    pub bet_count: u64,
    pub timestamp: i64,
}

// Enums
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum TokenType {
    Sol,
    Usdc,
}

// Error codes
#[error_code]
pub enum VaultError {
    #[msg("Invalid amount provided")]
    InvalidAmount,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Vault operations are paused")]
    VaultPaused,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Math underflow")]
    MathUnderflow,
    #[msg("Unauthorized access")]
    Unauthorized,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_error_codes() {
        let error = VaultError::InvalidAmount;
        assert_eq!(error.to_string(), "Invalid amount provided");
    }

    #[test]
    fn test_token_type_serialization() {
        let sol_type = TokenType::Sol;
        let usdc_type = TokenType::Usdc;
        
        assert_ne!(sol_type, usdc_type);
        assert_eq!(sol_type, TokenType::Sol);
    }
}