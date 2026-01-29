use crate::discovery::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::{error, info, warn};

// Constants
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// Whitelisted DEX Program IDs
const RAYDIUM_V4_PROGRAM: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const RAYDIUM_CLMM_PROGRAM: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
const RAYDIUM_CP_PROGRAM: &str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";
const METEORA_DLMM_PROGRAM: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
const METEORA_DAMM_PROGRAM: &str = "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB";
const ORCA_WHIRLPOOL_PROGRAM: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const PUMP_PROGRAM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

// API Configuration
const GECKO_API_BASE: &str = "https://api.geckoterminal.com/api/v2";
const DEXSCREENER_API_BASE: &str = "https://api.dexscreener.com/token-pairs/v1";
const SOLANA_NETWORK: &str = "solana";
const CONCURRENT_RPC_CHECKS: usize = 5;
const RPC_RATE_LIMIT_MS: u64 = 200;

// API Response Structures
#[derive(Deserialize, Debug)]
struct GeckoResponse {
    data: Vec<GeckoPoolData>,
}

#[derive(Deserialize, Debug, Clone)]
struct GeckoPoolData {
    attributes: GeckoAttributes,
    relationships: Option<GeckoRelationships>,
}

#[derive(Deserialize, Debug, Clone)]
struct GeckoAttributes {
    name: String,
    address: String,
    #[serde(default)]
    reserve_in_usd: Option<String>,
    #[serde(default)]
    volume_usd: Option<GeckoVolume>,
}

#[derive(Deserialize, Debug, Clone)]
struct GeckoVolume {
    h24: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct GeckoRelationships {
    base_token: GeckoTokenRelation,
}

#[derive(Deserialize, Debug, Clone)]
struct GeckoTokenRelation {
    data: GeckoTokenData,
}

#[derive(Deserialize, Debug, Clone)]
struct GeckoTokenData {
    id: String,
}

#[derive(Deserialize, Debug)]
struct DexscreenerResponse(Vec<DexscreenerPair>);

#[derive(Deserialize, Debug, Clone)]
struct DexscreenerPair {
    #[serde(rename = "pairAddress")]
    pair_address: String,
    #[serde(rename = "dexId")]
    dex_id: String,
    #[serde(default)]
    liquidity: Option<DexscreenerLiquidity>,
    #[serde(default)]
    volume: Option<DexscreenerVolume>,
    #[serde(rename = "baseToken")]
    base_token: Option<TokenInfo>,
    #[serde(rename = "quoteToken")]
    quote_token: Option<TokenInfo>,
}

#[derive(Deserialize, Debug, Clone)]
struct TokenInfo {
    address: String,
    name: Option<String>,
    symbol: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct DexscreenerLiquidity {
    usd: Option<f64>,
}

#[derive(Deserialize, Debug, Clone)]
struct DexscreenerVolume {
    h24: Option<f64>,
}

/// Map program ID to dex type
fn identify_specific_dex_type(owner: &Pubkey) -> Option<(String, String)> {
    let owner_str = owner.to_string();
    
    match owner_str.as_str() {
        RAYDIUM_V4_PROGRAM => Some(("raydium-v4".to_string(), RAYDIUM_V4_PROGRAM.to_string())),
        RAYDIUM_CLMM_PROGRAM => Some(("raydium-clmm".to_string(), RAYDIUM_CLMM_PROGRAM.to_string())),
        RAYDIUM_CP_PROGRAM => Some(("raydium-cp".to_string(), RAYDIUM_CP_PROGRAM.to_string())),
        METEORA_DLMM_PROGRAM => Some(("meteora-dlmm".to_string(), METEORA_DLMM_PROGRAM.to_string())),
        METEORA_DAMM_PROGRAM => Some(("meteora-damm-v2".to_string(), METEORA_DAMM_PROGRAM.to_string())),
        ORCA_WHIRLPOOL_PROGRAM => Some(("orca-whirlpool".to_string(), ORCA_WHIRLPOOL_PROGRAM.to_string())),
        PUMP_PROGRAM => Some(("pump".to_string(), PUMP_PROGRAM.to_string())),
        _ => None,
    }
}

/// Verify pool on-chain with rate limiting
async fn verify_pool_on_chain(
    rpc_client: &RpcClient,
    pool_address: &str,
) -> Result<Option<(String, String)>> {
    let pubkey = Pubkey::from_str(pool_address)
        .context("Invalid pool address format")?;
    
    let account = match rpc_client.get_account(&pubkey) {
        Ok(acc) => acc,
        Err(e) => {
            warn!("Failed to fetch account {}: {}", pool_address, e);
            return Ok(None);
        }
    };

    sleep(Duration::from_millis(RPC_RATE_LIMIT_MS)).await;
    Ok(identify_specific_dex_type(&account.owner))
}

/// Discovery engine implementation
pub struct DiscoveryEngine {
    http_client: Client,
    rpc_client: Arc<RpcClient>,
    config: DiscoveryConfig,
}

impl DiscoveryEngine {
    pub fn new(rpc_url: String, config: DiscoveryConfig) -> Self {
        let http_client = Client::builder()
            .user_agent("Mozilla/5.0 (Compatible; PoolDiscoveryBot/1.0)")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            rpc_url,
            CommitmentConfig::confirmed(),
        ));

        Self {
            http_client,
            rpc_client,
            config,
        }
    }

    /// Run discovery and return results
    pub async fn run_discovery(&self) -> Result<DiscoveredPools> {
        info!("🚀 Starting Pool Discovery...");
        info!("⏱️  RPC Rate Limit: {} ms/req", RPC_RATE_LIMIT_MS);

        // Get tokens from GeckoTerminal
        let mut initial_pools = Vec::new();
        
        info!("📡 Fetching trending pools...");
        let mut trending = self.fetch_gecko_pools(&format!("networks/{}/trending_pools", SOLANA_NETWORK)).await?;
        initial_pools.append(&mut trending);
        
        info!("📡 Fetching top pools...");
        let mut top = self.fetch_gecko_pools(&format!("networks/{}/pools", SOLANA_NETWORK)).await?;
        initial_pools.append(&mut top);

        // Extract unique tokens
        let ignored_mints = [SOL_MINT, USDC_MINT, "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"];
        
        let discovered_tokens: HashSet<String> = initial_pools
            .iter()
            .filter_map(|pool| {
                pool.relationships.as_ref().map(|r| {
                    r.base_token.data.id.replace("solana_", "")
                })
            })
            .filter(|addr| !ignored_mints.contains(&addr.as_str()))
            .collect();

        info!("💎 Found {} unique tokens", discovered_tokens.len());
        
        let discovered_tokens: Vec<String> = discovered_tokens.into_iter().collect();
        let total_tokens = discovered_tokens.len();
        
        info!("🔍 Verifying pools on-chain (rate limited)...\n");

        // Process with concurrency limit
        let semaphore = Arc::new(Semaphore::new(CONCURRENT_RPC_CHECKS));
        let mut futures = FuturesUnordered::new();

        for (idx, token_addr) in discovered_tokens.into_iter().enumerate() {
            let permit = semaphore.clone().acquire_owned().await?;
            let rpc_client = self.rpc_client.clone();
            let config = self.config.clone();
            
            let future = tokio::spawn(async move {
                let _permit = permit;
                Self::process_token(idx, total_tokens, &rpc_client, &config, &token_addr).await
            });
            
            futures.push(future);
        }

        let mut all_results: Vec<DiscoveredToken> = Vec::new();
        while let Some(result) = futures.next().await {
            match result {
                Ok(Ok(Some(token_group))) => {
                    if token_group.pools.len() >= 2 {
                        all_results.push(token_group);
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => error!("Task error: {}", e),
                Err(e) => error!("Join error: {}", e),
            }
        }

        all_results.sort_by(|a, b| b.total_liquidity.partial_cmp(&a.total_liquidity).unwrap());

        let output = DiscoveredPools {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            token_count: all_results.len(),
            tokens: all_results,
        };

        info!("🏆 Discovery complete! Found {} tokens with >= 2 verified SOL pools", output.token_count);
        Ok(output)
    }

    /// Save discovery results to JSON file
    pub async fn save_results(&self, results: &DiscoveredPools) -> Result<()> {
        let path = &self.config.output_file;
        let json = serde_json::to_string_pretty(results)
            .context("Failed to serialize results")?;
        
        tokio::fs::write(path, json).await
            .context(format!("Failed to write to {}", path))?;
        
        info!("💾 Saved discovered pools to {}", path);
        Ok(())
    }

    /// Load previously saved results (for bot startup)
    pub async fn load_results(&self) -> Result<Option<DiscoveredPools>> {
        let path = &self.config.output_file;
        
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let pools: DiscoveredPools = serde_json::from_str(&content)
                    .context("Failed to parse discovered pools JSON")?;
                info!("📂 Loaded {} tokens from {}", pools.token_count, path);
                Ok(Some(pools))
            }
            Err(_) => {
                info!("⚠️ No existing discovered pools file found at {}", path);
                Ok(None)
            }
        }
    }

    /// Convert discovered pools to markets format for bot
    pub fn convert_to_markets(pools: &DiscoveredPools) -> Vec<String> {
        let mut market_addresses = Vec::new();
        
        for token in &pools.tokens {
            for pool in &token.pools {
                market_addresses.push(pool.pool_address.clone());
            }
        }
        
        info!("🔄 Converted {} pools to market format", market_addresses.len());
        market_addresses
    }

    async fn fetch_gecko_pools(&self, endpoint: &str) -> Result<Vec<GeckoPoolData>> {
        let url = format!("{}/{}", GECKO_API_BASE, endpoint);
        let resp = self.http_client.get(&url).send().await?.error_for_status()?;
        
        if resp.status() == 404 {
            return Ok(Vec::new());
        }
        
        sleep(Duration::from_millis(500)).await;
        let data: GeckoResponse = resp.json().await?;
        Ok(data.data)
    }

    // FIXED: Added total_tokens parameter
    async fn process_token(
        idx: usize,
        total_tokens: usize,
        rpc_client: &Arc<RpcClient>,
        config: &DiscoveryConfig,
        token_addr: &str,
    ) -> Result<Option<DiscoveredToken>> {
        let dexscreener_url = format!("{}/{}/{}", DEXSCREENER_API_BASE, SOLANA_NETWORK, token_addr);
        
        let pairs = match reqwest::get(&dexscreener_url).await {
            Ok(resp) => {
                match resp.json::<DexscreenerResponse>().await {
                    Ok(p) => p.0,
                    Err(e) => {
                        error!("[{}/{}] Parse error for {}: {}", idx + 1, total_tokens, &token_addr[..8], e);
                        return Ok(None);
                    }
                }
            }
            Err(e) => {
                error!("[{}/{}] Fetch error for {}: {}", idx + 1, total_tokens, &token_addr[..8], e);
                return Ok(None);
            }
        };

        let mut verified_pools: Vec<DiscoveredPool> = Vec::new();
        let mut token_name = "Unknown".to_string();
        let mut token_symbol = "UNK".to_string();

        for pair in pairs {
            let (is_sol_pair, sol_side) = match (&pair.base_token, &pair.quote_token) {
                (Some(base), Some(quote)) => {
                    if base.address == SOL_MINT {
                        token_name = base.name.clone().unwrap_or_default();
                        token_symbol = base.symbol.clone().unwrap_or("UNK".to_string());
                        (true, "base")
                    } else if quote.address == SOL_MINT {
                        token_name = base.name.clone().unwrap_or_default();
                        token_symbol = base.symbol.clone().unwrap_or("UNK".to_string());
                        (true, "quote")
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };

            if !is_sol_pair {
                continue;
            }

            let (dex_type, program_id) = match verify_pool_on_chain(rpc_client, &pair.pair_address).await? {
                Some(result) => result,
                None => continue,
            };

            let liq = pair.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
            let vol = pair.volume.as_ref().and_then(|v| v.h24).unwrap_or(0.0);

            if liq >= config.min_liquidity_usd && vol >= config.min_volume_h24 {
                verified_pools.push(DiscoveredPool {
                    pool_address: pair.pair_address.clone(),
                    dex_type,
                    program_id,
                    liquidity_usd: liq,
                    volume_h24: vol,
                    sol_side: sol_side.to_string(),
                });
            }
        }

        if verified_pools.len() < 2 {
            return Ok(None);
        }

        verified_pools.sort_by(|a, b| b.liquidity_usd.partial_cmp(&a.liquidity_usd).unwrap());
        let total_liq: f64 = verified_pools.iter().map(|p| p.liquidity_usd).sum();

        info!("[{}/{}] {}: {} pools, ${:.0} liquidity", 
            idx + 1, total_tokens, token_symbol, verified_pools.len(), total_liq);

        Ok(Some(DiscoveredToken {
            token_address: token_addr.to_string(),
            token_name,
            token_symbol,
            total_liquidity: total_liq,
            pools: verified_pools,
        }))
    }
}