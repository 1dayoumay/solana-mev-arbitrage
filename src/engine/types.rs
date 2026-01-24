use solana_program::pubkey::Pubkey;
use std::fmt::Debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DexType {
    Pump, RaydiumV4, RaydiumCp, RaydiumClmm,
    MeteoraDlmm, MeteoraDamm, MeteoraDammV2,
    Whirlpool, Vertigo, Heaven, Futarchy, Humidifi,
    PancakeSwap, Byreal,
}

#[derive(Debug, Clone)]
pub struct PoolEdge {
    pub pool_pubkey: Pubkey,
    pub dex_type: DexType,
    pub price: f64,              // price = output_mint / input_mint
    pub liquidity_usd: f64,      // Available liquidity depth
    pub fee_bps: u64,            // Fee in basis points
    pub inverse_fee_bps: u64,    // Fee for reverse direction
    pub token_program: Pubkey,   // Token or Token-2022
}

#[derive(Debug, Clone)]
pub struct SwapLeg {
    pub from_mint: Pubkey,
    pub to_mint: Pubkey,
    pub pool_pubkey: Pubkey,
    pub dex_type: DexType,
    pub amount_in: u64,
    pub estimated_amount_out: u64,
}

#[derive(Debug, Clone)]
pub struct ArbitrageCycle {
    pub legs: Vec<SwapLeg>,
    pub total_profit_bps: i64,   // Profit in basis points
    pub estimated_profit_lamports: u64,
    pub total_hops: usize,
}

#[derive(Debug, Clone)]
pub struct TokenNode {
    pub mint: Pubkey,
}