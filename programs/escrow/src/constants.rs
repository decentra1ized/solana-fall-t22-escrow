use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// How long a maker must wait after creating an escrow before they can cancel it.
/// Cancellation is allowed once `now >= created_at + CANCEL_DELAY_SECONDS`.
#[constant]
pub const CANCEL_DELAY_SECONDS: i64 = 300;
