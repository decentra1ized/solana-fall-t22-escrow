use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Timelock is active. Try again after sometime!")]
    TimeLockActive,
}
