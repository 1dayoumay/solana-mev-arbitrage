use super::PoolFetcher;
use crate::error::{BotError, Result};
use crate::types::{PoolInfo, TokenMint, DexType};
use crate::config::DexConfig;
use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcProgramAccountsConfig, RpcAccountInfoConfig};
use solana_sdk::commitment_config::CommitmentConfig;
use orca_whirlpools_client::Whirlpool;
use spl_token::state::Mint;
use solana_sdk::program_pack::Pack; // ✅ ADD THIS LINE

pub struct OrcaOnchainFetcher {
    config: DexConfig,
    rpc_url: String,
}

impl OrcaOnchainFetcher {
    pub fn new(config: DexConfig, rpc_url: String) -> Self {
        Self { config, rpc_url }
    }
    
    fn parse_whirlpool(&self, address: &Pubkey, whirlpool: &Whirlpool, rpc_client: &RpcClient) -> Result<PoolInfo> {
        let token_a_mint = whirlpool.token_mint_a;
        let token_b_mint = whirlpool.token_mint_b;
        
        if token_a_mint == Pubkey::default() || token_b_mint == Pubkey::default() {
            return Err(BotError::InvalidPoolData("Zero mint address".to_string()));
        }
        
        let sqrt_price = whirlpool.sqrt_price as f64 / (1u128 << 64) as f64;
        let price = sqrt_price * sqrt_price;
        
        let token_a_account = rpc_client.get_account(&token_a_mint)
            .map_err(|e| BotError::RateLimitError(format!("Failed to fetch token A mint: {}", e)))?;
        let token_b_account = rpc_client.get_account(&token_b_mint)
            .map_err(|e| BotError::RateLimitError(format!("Failed to fetch token B mint: {}", e)))?;
        
        let token_a_mint_data = Mint::unpack(&token_a_account.data)
            .map_err(|e| BotError::InvalidPoolData(format!("Failed to unpack token A mint: {}", e)))?;
        let token_b_mint_data = Mint::unpack(&token_b_account.data)
            .map_err(|e| BotError::InvalidPoolData(format!("Failed to unpack token B mint: {}", e)))?;
        
        let liquidity_a = whirlpool.liquidity as f64 / (10u64.pow(token_a_mint_data.decimals as u32) as f64);
        let liquidity_b = whirlpool.liquidity as f64 / (10u64.pow(token_b_mint_data.decimals as u32) as f64);
        let liquidity_usd = liquidity_a * price + liquidity_b;
        
        let fee_bps = whirlpool.fee_rate / 100;
        
        Ok(PoolInfo {
            address: *address,
            dex: DexType::Orca,
            token_a: TokenMint(token_a_mint),
            token_b: TokenMint(token_b_mint),
            price,
            liquidity_usd,
            fee_bps,
            last_updated: std::time::Instant::now(),
        })
    }
}

#[async_trait]
impl PoolFetcher for OrcaOnchainFetcher {
    fn dex_type(&self) -> DexType {
        DexType::Orca
    }
    
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>> {
        tracing::info!("Fetching Orca Whirlpools on-chain...");
        
        let rpc_client = RpcClient::new(self.rpc_url.clone());
        let config = RpcProgramAccountsConfig {
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let accounts = rpc_client.get_program_accounts_with_config(&self.config.program_id, config)
            .map_err(|e| BotError::RateLimitError(format!("RPC error: {}", e)))?;
        
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            match Whirlpool::from_bytes(&account.data) {
                Ok(whirlpool) => {
                    match self.parse_whirlpool(&pubkey, &whirlpool, &rpc_client) {
                        Ok(pool) => pools.push(pool),
                        Err(e) => tracing::debug!("Failed to parse pool {}: {}", pubkey, e),
                    }
                }
                Err(_) => {
                    continue;
                }
            }
        }
        
        tracing::info!("Successfully fetched {} Orca pools", pools.len());
        Ok(pools)
    }
    
    async fn fetch_pool_by_address(&self, address: &Pubkey) -> Result<Option<PoolInfo>> {
        let rpc_client = RpcClient::new(self.rpc_url.clone());
        
        match rpc_client.get_account(address) {
            Ok(account) => {
                match Whirlpool::from_bytes(&account.data) {
                    Ok(whirlpool) => self.parse_whirlpool(address, &whirlpool, &rpc_client).map(Some),
                    Err(_) => Ok(None),
                }
            }
            Err(_) => Ok(None),
        }
    }
}