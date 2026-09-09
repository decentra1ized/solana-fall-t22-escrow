use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// How long an escrow must live before the maker may cancel it.
/// Exported to the IDL by `#[constant]`, so clients read the same number we enforce.
#[constant]
pub const CANCEL_DELAY_SECONDS: i64 = 300; // 5 minutes
