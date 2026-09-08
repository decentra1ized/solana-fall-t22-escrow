pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("DkyDSZ2gMCTp4qYGdZk98bTW4RysH32x9LAX8MkDFFYw");

#[program]
pub mod escrow {
    use super::*;

    pub fn make(ctx: Context<Make>, seed: u16, amount_a: u64, amount_b: u64) -> Result<()> {
        make::handler(ctx, seed, amount_a, amount_b)
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        take::handler(ctx)
    }

    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        cancel::handler(ctx)
    }
}
