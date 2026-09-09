use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    InvalidAmount,

    #[msg("Escrow is still locked")]
    TimeLockActive,

    #[msg("Timestamp overflow")]
    Overflow,
}