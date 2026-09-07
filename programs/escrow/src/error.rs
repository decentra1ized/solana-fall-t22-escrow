use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("You cannot cancel the escrow yet. Please wait for the cancellation delay to pass.")]
    CancelTooEarly,
}
