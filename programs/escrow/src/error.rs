use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Cancel is time-locked: the offer must stay open for CANCEL_DELAY_SECONDS after make")]
    CancelTooEarly,
    #[msg("Timestamp arithmetic overflowed")]
    Overflow,
}