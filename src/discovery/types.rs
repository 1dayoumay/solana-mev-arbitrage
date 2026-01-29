use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscoveredPools {
    pub timestamp: u64,
    pub token_count: usize,
    pub tokens: Vec<DiscoveredToken>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscoveredToken {
    pub token_address: String,
    pub token_name: String,
    pub token_symbol: String,
    pub total_liquidity: f64,
    pub pools: Vec<DiscoveredPool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscoveredPool {
    pub pool_address: String,
    pub dex_type: String,
    pub program_id: String,
    pub liquidity_usd: f64,
    pub volume_h24: f64,
    pub sol_side: String,
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub min_liquidity_usd: f64,
    pub min_volume_h24: f64,
    pub output_file: String,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 15,
            min_liquidity_usd: 5000.0,
            min_volume_h24: 1000.0,
            output_file: "discovered_pools.json".to_string(),
        }
    }
}