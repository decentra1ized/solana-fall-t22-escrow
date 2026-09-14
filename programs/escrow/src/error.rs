use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Escrow cannot be cancelled until the time lock has expired")]
    TimeLockActive,
}
