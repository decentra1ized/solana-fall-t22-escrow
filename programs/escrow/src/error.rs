use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("The time lock has not elapsed yet. Cannot cancel before 5 minutes have passed.")]
    TimeLockActive,
}
