pub mod raydium;
pub mod meteora;
pub mod orca;

use async_trait::async_trait;
use crate::error::Result;
use crate::types::PoolInfo;
use solana_sdk::pubkey::Pubkey;

#[async_trait]
pub trait PoolFetcher: Send + Sync {
    fn dex_type(&self) -> crate::types::DexType;
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>>;
    async fn fetch_pool_by_address(&self, address: &Pubkey) -> Result<Option<PoolInfo>>;
}

pub struct MarketOrchestrator {
    fetchers: Vec<Box<dyn PoolFetcher>>,
}

impl MarketOrchestrator {
    pub fn new(rpc_url: String) -> Self {
        let mut fetchers: Vec<Box<dyn PoolFetcher>> = Vec::new();
        
        // Initialize all on-chain fetchers
        fetchers.push(Box::new(raydium::RaydiumOnchainFetcher::new(rpc_url.clone())));
        fetchers.push(Box::new(meteora::MeteoraOnchainFetcher::new(rpc_url.clone())));
        fetchers.push(Box::new(orca::OrcaOnchainFetcher::new(rpc_url)));
        
        Self { fetchers }
    }
    
    pub async fn fetch_all_pools(&self) -> Result<Vec<PoolInfo>> {
        let mut all_pools = Vec::new();
        
        for fetcher in &self.fetchers {
            match fetcher.fetch_pools().await {
                Ok(pools) => {
                    tracing::info!("Fetched {} pools from {}", pools.len(), fetcher.dex_type());
                    all_pools.extend(pools);
                }
                Err(e) => {
                    tracing::error!("Failed to fetch pools from {}: {}", fetcher.dex_type(), e);
                }
            }
        }
        
        Ok(all_pools)
    }
}