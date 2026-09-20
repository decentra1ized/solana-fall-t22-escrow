use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,

    #[msg("Cannot cancel escrow before 5 mins have passed")]
    TooEarlyToCancel,
}
