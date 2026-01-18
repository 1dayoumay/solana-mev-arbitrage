use super::PoolFetcher;
use crate::error::{BotError, Result};
use crate::types::{PoolInfo, TokenMint, DexType};
use crate::config::DexConfig;
use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub struct MeteoraApiFetcher {
    config: DexConfig,
    client: reqwest::Client,
    damm_rate_limiter: crate::utils::DirectRateLimiter,
    dlmm_rate_limiter: crate::utils::DirectRateLimiter,
}

impl MeteoraApiFetcher {
    pub fn new(config: DexConfig) -> Self {
        let damm_rate_limiter = crate::utils::create_rate_limiter(10);
        let dlmm_rate_limiter = crate::utils::create_rate_limiter(30);
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            config,
            client,
            damm_rate_limiter,
            dlmm_rate_limiter,
        }
    }
    
    async fn fetch_damm_pools(&self) -> Result<serde_json::Value> {
        self.damm_rate_limiter.until_ready().await;
        
        let url = format!("{}/damm/v2/pools", self.config.api_base_url.as_ref().unwrap());
        tracing::debug!("Fetching Meteora DAMM pools from: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| BotError::HttpError(e))?;
            
        if !response.status().is_success() {
            return Err(BotError::RateLimitError(format!("API returned status: {}", response.status())));
        }
        
        response.json().await.map_err(|e| BotError::HttpError(e))
    }
    
    async fn fetch_dlmm_pools(&self) -> Result<serde_json::Value> {
        self.dlmm_rate_limiter.until_ready().await;
        
        let url = format!("{}/dlmm/pools", self.config.api_base_url.as_ref().unwrap());
        tracing::debug!("Fetching Meteora DLMM pools from: {}", url);
        
        let response = self.client
            .get(&url)
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
impl PoolFetcher for MeteoraApiFetcher {
    fn dex_type(&self) -> DexType {
        DexType::Meteora
    }
    
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>> {
        let mut pools = Vec::new();
        
        // Fetch DAMM v2 pools
        match self.fetch_damm_pools().await {
            Ok(data) => {
                if let Some(pools_array) = data["data"].as_array() {
                    for pool_data in pools_array {
                        if let Ok(pool) = parse_meteora_pool(pool_data, "damm") {
                            pools.push(pool);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("Failed to fetch Meteora DAMM pools: {}", e),
        }
        
        // Fetch DLMM pools
        match self.fetch_dlmm_pools().await {
            Ok(data) => {
                if let Some(pools_array) = data["data"].as_array() {
                    for pool_data in pools_array {
                        if let Ok(pool) = parse_meteora_pool(pool_data, "dlmm") {
                            pools.push(pool);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("Failed to fetch Meteora DLMM pools: {}", e),
        }
        
        Ok(pools)
    }
    
    async fn fetch_pool_by_address(&self, _address: &Pubkey) -> Result<Option<PoolInfo>> {
        // Phase 2: Implement single pool fetch
        Ok(None)
    }
}

fn parse_meteora_pool(data: &serde_json::Value, _pool_type: &str) -> Result<PoolInfo> {
    let address_str = data["address"].as_str()
        .ok_or_else(|| BotError::InvalidPoolData("Missing pool address".to_string()))?;
    let address = Pubkey::from_str(address_str)
        .map_err(|_| BotError::InvalidPoolData("Invalid pool pubkey".to_string()))?;
        
    let token_a_mint = data["tokenAMint"].as_str()
        .ok_or_else(|| BotError::InvalidPoolData("Missing tokenAMint".to_string()))?;
    let token_b_mint = data["tokenBMint"].as_str()
        .ok_or_else(|| BotError::InvalidPoolData("Missing tokenBMint".to_string()))?;
        
    let reserve_a = data["reserveA"].as_f64().unwrap_or(0.0);
    let reserve_b = data["reserveB"].as_f64().unwrap_or(0.0);
    
    let price = crate::utils::calculate_price_from_tvl(reserve_a, reserve_b);
    
    let liquidity = data["liquidityUsd"].as_f64().unwrap_or(0.0);
    let fee_bps = data["feeBps"].as_u64().unwrap_or(10) as u16;
    
    Ok(PoolInfo {
        address,
        dex: DexType::Meteora,
        token_a: TokenMint(Pubkey::from_str(token_a_mint).unwrap()),
        token_b: TokenMint(Pubkey::from_str(token_b_mint).unwrap()),
        price,
        liquidity_usd: liquidity,
        fee_bps,
        last_updated: std::time::Instant::now(),
    })
}