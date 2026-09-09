use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub maker: Pubkey,      // The maker of the escrow
    pub mint_a: Pubkey,     // The token being offered
    pub mint_b: Pubkey,     // The token being requested
    pub amount_a: u64,      // Amount being offered
    pub amount_b: u64,      // Amount being requested
    pub seed: u16,          // Seed used for PDA derivation
    pub bump: u8,           // Bump used for PDA derivation
    pub created_at: i64,    // Unix timestamp when the escrow was created
}