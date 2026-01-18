use super::PoolFetcher;
use crate::error::{BotError, Result};
use crate::types::{PoolInfo, TokenMint, DexType};
use crate::config::DexConfig;
use async_trait::async_trait;
use governor::RateLimiter;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub struct RaydiumApiFetcher {
    config: DexConfig,
    client: reqwest::Client,
    rate_limiter: crate::utils::DirectRateLimiter,
}

impl RaydiumApiFetcher {
    pub fn new(config: DexConfig) -> Self {
        let rate_limiter = crate::utils::create_rate_limiter(config.rate_limit_rps);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            config,
            client,
            rate_limiter,
        }
    }
    
    async fn fetch_pairs(&self) -> Result<serde_json::Value> {
        self.rate_limiter.until_ready().await;
        
        let url = format!("{}/pairs", self.config.api_base_url.as_ref().unwrap());
        tracing::debug!("Fetching Raydium pairs from: {}", url);
        
        let response = self.client
            .get(&url)
            .query(&[("limit", "1000")])
            .send()
            .await
            .map_err(|e| BotError::HttpError(e))?;
            
        if !response.status().is_success() {
            return Err(BotError::RateLimitError(format!("API returned status: {}", response.status())));
        }
        
        response.json().await.map_err(|e| BotError::HttpError(e))
    }
}

#[async_trait]
impl PoolFetcher for RaydiumApiFetcher {
    fn dex_type(&self) -> DexType {
        DexType::Raydium
    }
    
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>> {
        let data = self.fetch_pairs().await?;
        let pools_array = data["data"]["data"].as_array()
            .ok_or_else(|| BotError::InvalidPoolData("Missing data.data array".to_string()))?;
            
        let mut pools = Vec::new();
        
        for pool_data in pools_array {
            if let Ok(pool) = parse_raydium_pool(pool_data) {
                pools.push(pool);
            }
        }
        
        Ok(pools)
    }
    
    async fn fetch_pool_by_address(&self, _address: &Pubkey) -> Result<Option<PoolInfo>> {
        // Phase 2: Implement single pool fetch
        Ok(None)
    }
}

fn parse_raydium_pool(data: &serde_json::Value) -> Result<PoolInfo> {
    let address_str = data["id"].as_str()
        .ok_or_else(|| BotError::InvalidPoolData("Missing pool ID".to_string()))?;
    let address = Pubkey::from_str(address_str)
        .map_err(|_| BotError::InvalidPoolData("Invalid pool pubkey".to_string()))?;
        
    let token_a_mint = data["mintA"].as_str()
        .ok_or_else(|| BotError::InvalidPoolData("Missing mintA".to_string()))?;
    let token_b_mint = data["mintB"].as_str()
        .ok_or_else(|| BotError::InvalidPoolData("Missing mintB".to_string()))?;
        
    let reserve_a = data["reserveA"].as_f64().unwrap_or(0.0);
    let reserve_b = data["reserveB"].as_f64().unwrap_or(0.0);
    
    let price = crate::utils::calculate_price_from_tvl(reserve_a, reserve_b);
    
    let liquidity = data["liquidity"].as_f64().unwrap_or(0.0);
    let fee_bps = data["feeBps"].as_u64().unwrap_or(25) as u16;
    
    Ok(PoolInfo {
        address,
        dex: DexType::Raydium,
        token_a: TokenMint(Pubkey::from_str(token_a_mint).unwrap()),
        token_b: TokenMint(Pubkey::from_str(token_b_mint).unwrap()),
        price,
        liquidity_usd: liquidity,
        fee_bps,
        last_updated: std::time::Instant::now(),
    })
}