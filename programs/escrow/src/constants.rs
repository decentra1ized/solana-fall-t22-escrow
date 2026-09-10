use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// The maker must wait this long after `make` before `cancel` is allowed.
#[constant]
pub const CANCEL_DELAY_SECONDS: i64 = 300;
