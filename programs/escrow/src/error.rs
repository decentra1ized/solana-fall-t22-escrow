use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The escrow cannot be deleted within 5 mins of creation!")]
    TimeLockActive,
}
