use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("The required waiting period is not finished yet, please wait")]
    TimeLockActive,
}
