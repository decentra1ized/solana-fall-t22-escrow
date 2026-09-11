use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Escrow cancellation timelock is still active")]
    TimelockActive,
}
