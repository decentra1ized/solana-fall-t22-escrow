use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Cancel is time-locked until the delay has elapsed")]
    CancelTooEarly,
    #[msg("Timestamp arithmetic overflowed")]
    TimestampOverflow,
}
