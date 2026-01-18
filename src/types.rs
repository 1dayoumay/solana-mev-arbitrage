use solana_sdk::pubkey::Pubkey;
use std::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TokenMint(pub Pubkey);

impl fmt::Display for TokenMint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct PoolInfo {
    pub address: Pubkey,
    pub dex: DexType,
    pub token_a: TokenMint,
    pub token_b: TokenMint,
    pub price: f64,
    pub liquidity_usd: f64,
    pub fee_bps: u16,
    pub last_updated: std::time::Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DexType {
    Raydium,
    Meteora,
    Orca,
}

impl fmt::Display for DexType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DexType::Raydium => write!(f, "Raydium"),
            DexType::Meteora => write!(f, "Meteora"),
            DexType::Orca => write!(f, "Orca"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PathEdge {
    pub pool_address: Pubkey,
    pub dex: DexType,
    pub price: f64,
    pub liquidity_usd: f64,
    pub fee_bps: u16,
}