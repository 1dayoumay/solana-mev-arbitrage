mod engine;

pub use engine::PetgraphEngine;
use crate::types::PoolInfo;
use crate::error::Result;

pub trait GraphEngine: Send + Sync {
    fn add_or_update_pool(&mut self, pool: PoolInfo) -> Result<()>;
    fn get_pool(&self, address: &solana_sdk::pubkey::Pubkey) -> Option<&PoolInfo>;
    fn remove_pool(&mut self, address: &solana_sdk::pubkey::Pubkey) -> Result<()>;
    fn get_all_pools(&self) -> Vec<&PoolInfo>;
    fn clear(&mut self);
}