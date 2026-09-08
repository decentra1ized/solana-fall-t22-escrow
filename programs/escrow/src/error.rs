use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("This escrow's time lock is yet to elapse thus it cannot be cancelled or retracted currently — try again later")]
    TimeLockActive
}
