use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

#[constant]					// This is new
pub const CANCEL_DELAY_SECONDS: i64 = 300;	// 