use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Time Lock has not elapsed")]
    TimeLockActive,
    #[msg("Escrow has overflowed")]
    EscrowOverflow,
}
