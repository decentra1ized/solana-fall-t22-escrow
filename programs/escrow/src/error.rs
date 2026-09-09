use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Cant cancel within 5 mins of creation")]
    TimeLockActive,
}
