use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Cancellation is not allowed until 5 minutes after creation")]
    CancelTooEarly,
}
