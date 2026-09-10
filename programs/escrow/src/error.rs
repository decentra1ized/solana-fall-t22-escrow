use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,

    #[msg("Not so fast! The 5-min time lock is still active.")]
    TimeLockActive,

}
