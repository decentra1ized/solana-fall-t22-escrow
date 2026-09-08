use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Custom error message")]
    CustomError,
    #[msg("The time lock has not elapsed")]
    TimeLockActive,
}