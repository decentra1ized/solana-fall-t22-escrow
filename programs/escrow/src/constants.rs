use anchor_lang::prelude::*;

/// How long (in seconds) an offer stays uncancellable after `make`.
#[constant]
pub const CANCEL_DELAY_SECONDS: i64 = 300;