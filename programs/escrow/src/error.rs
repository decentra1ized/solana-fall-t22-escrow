use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("The escrow cancellation time lock is still active")]
    TimeLockActive,
}
