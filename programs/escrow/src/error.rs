use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Time lock is still active; this escrow cannot be cancelled yet")]
    TimeLockActive,
}
