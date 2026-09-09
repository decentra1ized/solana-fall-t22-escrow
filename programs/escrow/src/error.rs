use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Timelock is still active")]
    TimeLockActive,
    #[msg("Arithmetic overflow")]
    Overflow,
}
