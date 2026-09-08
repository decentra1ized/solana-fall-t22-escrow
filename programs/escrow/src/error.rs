use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("The time lock has not elapsed yet: this escrow cannot be cancelled for five minutes after creation")]
    TimeLockActive,
}
