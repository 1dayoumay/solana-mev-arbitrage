use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub rpc_url: String,
    pub rpc_ws_url: String,
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
            min_liquidity_usd,
            update_interval_secs,
        })
    }
}