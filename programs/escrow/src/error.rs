use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Escrow in timelock")]
    TimeLockError,
}
