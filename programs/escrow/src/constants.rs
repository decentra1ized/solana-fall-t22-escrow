use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// Minimum lifetime of an escrow, in seconds, before it can be cancelled.
#[constant]
pub const CANCEL_DELAY_SECONDS: i64 = 300; // 5 minutes
