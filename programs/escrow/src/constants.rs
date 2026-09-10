use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// How long after creation the maker must wait before cancelling.
///
/// A zero-notice offer is a free option: the maker keeps the upside and cancels
/// out of the downside, while takers pay the fee on transactions a cancel
/// front-runs. Holding the door shut makes the offer mean what it says.
pub const CANCEL_DELAY_SECONDS: i64 = 300;
