use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Funds locked for withdrawal.")]
    FundsTimeLock,
}
