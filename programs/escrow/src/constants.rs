use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// Minimum time (in seconds) an escrow must exist before the maker can cancel it.
/// Exposed via #[constant] so it also lands in the IDL for client-side use.
#[constant]
pub const MIN_ESCROW_LIFETIME: i64 = 300; // 5 minutes
