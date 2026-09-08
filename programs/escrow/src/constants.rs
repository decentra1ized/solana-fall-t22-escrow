use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// Minimum number of seconds that must elapse after `make` before `cancel` is allowed.
pub const CANCEL_DELAY_SECONDS: i64 = 300; // 5 minutes
