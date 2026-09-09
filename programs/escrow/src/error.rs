use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Nah bro, escrow still locked")]
    CancelTooEarly,
}
