use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod vault {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        msg!("Hello World from Vault Program!");
        Ok(())
    }

    pub fn test_instruction(_ctx: Context<TestContext>, value: u64) -> Result<()> {
        msg!("Test instruction called with value: {}", value);
        require!(value > 0, VaultError::InvalidValue);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct TestContext {}

#[error_code]
pub enum VaultError {
    #[msg("Invalid value provided")]
    InvalidValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_error_codes() {
        let error = VaultError::InvalidValue;
        assert_eq!(error.to_string(), "Invalid value provided");
    }
}