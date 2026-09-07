use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The cancel delay has not elapsed yet")]
    TimeLockActive,
}
