use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub maker: Pubkey,          // The maker of the escrow
    pub mint_a: Pubkey,         // The mint of the token being offered by the maker
    pub mint_b: Pubkey,         // The mint of the token being requested by the maker   
    pub amount_a: u64,          // The amount of the token being offered by the maker
    pub amount_b: u64,          // The amount of the token being requested by the maker
    pub seed: u16,              // The seed used for PDA derivation
    pub bump: u8,               // The bump used for PDA derivation
    pub created_at: i64,        // new — Unix timestamp, appended at the end (same reasoning as max_withdraw: keep existing byte offsets stable)
}