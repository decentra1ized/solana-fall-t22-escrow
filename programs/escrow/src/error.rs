use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("The escrow cannot be cancelled yet. The maker must wait 300 seconds after creation.")]
    TimeLockActive,
}
