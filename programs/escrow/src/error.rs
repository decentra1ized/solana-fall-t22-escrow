use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Withdraw timeout has not elapsed. Wait 5 minutes after creating the escrow.")]
    WithdrawTimeout,
}
