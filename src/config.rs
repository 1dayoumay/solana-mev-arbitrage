use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct DexConfig {
    pub name: &'static str,
    pub program_id: Pubkey,
    pub api_base_url: Option<String>,
    pub rate_limit_rps: u32,
}

impl DexConfig {
    pub fn raydium_mainnet() -> Self {
        Self {
            name: "Raydium",
            program_id: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
            api_base_url: Some("https://api-v3.raydium.io".to_string()),
            rate_limit_rps: 10,
        }
    }
    
    pub fn meteora_mainnet() -> Self {
        Self {
            name: "Meteora",
            program_id: Pubkey::from_str("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG").unwrap(),
            api_base_url: Some("https://api.meteora.ag".to_string()),
            rate_limit_rps: 10,
        }
    }
    
    pub fn orca_mainnet() -> Self {
        Self {
            name: "Orca",
            program_id: Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap(),
            api_base_url: None, // On-chain only
            rate_limit_rps: 10,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub rpc_url: String,
    pub rpc_ws_url: String,
    pub dex_configs: Vec<DexConfig>,
    pub min_liquidity_usd: f64,
    pub update_interval_secs: u64,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, crate::error::BotError> {
        dotenvy::dotenv().ok();
        
        let rpc_url = std::env::var("RPC_URL")
            .map_err(|_| crate::error::BotError::ConfigError("RPC_URL not set".to_string()))?;
            
        let rpc_ws_url = std::env::var("RPC_WS_URL")
            .map_err(|_| crate::error::BotError::ConfigError("RPC_WS_URL not set".to_string()))?;
            
        let min_liquidity_usd = std::env::var("MIN_LIQUIDITY_USD")
            .unwrap_or_else(|_| "50000".to_string())
            .parse()
            .map_err(|e| crate::error::BotError::ConfigError(format!("Invalid MIN_LIQUIDITY_USD: {}", e)))?;
            
        let update_interval_secs = std::env::var("UPDATE_INTERVAL_SECS")
            .unwrap_or_else(|_| "15".to_string())
            .parse()
            .map_err(|e| crate::error::BotError::ConfigError(format!("Invalid UPDATE_INTERVAL_SECS: {}", e)))?;
        
        Ok(Self {
            rpc_url,
            rpc_ws_url,
            dex_configs: vec![
                DexConfig::raydium_mainnet(),
                DexConfig::meteora_mainnet(),
                DexConfig::orca_mainnet(),
            ],
            min_liquidity_usd,
            update_interval_secs,
        })
    }
}