use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Escrow cannot be cancelled yet: the 5-minute lock period has not elapsed")]
    CancelTooEarly,
}
