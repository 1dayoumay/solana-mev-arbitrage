use crate::dex;
use super::PoolFetcher;
use crate::error::{BotError, Result};
use crate::types::{PoolInfo, TokenMint, DexType};
use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcProgramAccountsConfig, RpcAccountInfoConfig};
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;
use spl_token::state::Mint;
use solana_sdk::program_pack::Pack;

pub struct OrcaOnchainFetcher {
    rpc_client: Arc<RpcClient>,
}

impl OrcaOnchainFetcher {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_client: Arc::new(RpcClient::new(rpc_url)),
        }
    }
    
    fn parse_whirlpool(&self, address: &Pubkey, data: &[u8]) -> Result<Option<PoolInfo>> {
        if let Ok(whirlpool) = dex::whirlpool::state::Whirlpool::try_deserialize(data) {
            let sqrt_price = whirlpool.sqrt_price as f64 / (1u128 << 64) as f64;
            let price = sqrt_price * sqrt_price;
            
            // Fetch mint decimals
            let token_a_account = self.rpc_client.get_account(&whirlpool.token_mint_a)
                .map_err(|e| BotError::RateLimitError(format!("Failed to fetch token A mint: {}", e)))?;
            let token_b_account = self.rpc_client.get_account(&whirlpool.token_mint_b)
                .map_err(|e| BotError::RateLimitError(format!("Failed to fetch token B mint: {}", e)))?;
            
            let token_a_mint_data = Mint::unpack(&token_a_account.data)
                .map_err(|e| BotError::InvalidPoolData(format!("Failed to unpack token A mint: {}", e)))?;
            let token_b_mint_data = Mint::unpack(&token_b_account.data)
                .map_err(|e| BotError::InvalidPoolData(format!("Failed to unpack token B mint: {}", e)))?;
            
            // Calculate liquidity in USD (simplified)
            let liquidity_a = whirlpool.liquidity as f64 / (10u64.pow(token_a_mint_data.decimals as u32) as f64);
            let liquidity_b = whirlpool.liquidity as f64 / (10u64.pow(token_b_mint_data.decimals as u32) as f64);
            let liquidity_usd = liquidity_a * price + liquidity_b;
            
            let fee_bps = whirlpool.fee_rate / 100;
            
            Ok(Some(PoolInfo {
                address: *address,
                dex: DexType::Orca,
                token_a: TokenMint(whirlpool.token_mint_a),
                token_b: TokenMint(whirlpool.token_mint_b),
                price,
                liquidity_usd,
                fee_bps,
                last_updated: std::time::Instant::now(),
            }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl PoolFetcher for OrcaOnchainFetcher {
    fn dex_type(&self) -> DexType {
        DexType::Orca
    }
    
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>> {
        tracing::info!("Fetching Orca Whirlpools on-chain...");
        
        let config = RpcProgramAccountsConfig {
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let program_id = dex::whirlpool::constants::whirlpool_program_id();
        let accounts = self.rpc_client.get_program_accounts_with_config(&program_id, config)
            .map_err(|e| BotError::RateLimitError(format!("RPC error: {}", e)))?;
        
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            match self.parse_whirlpool(&pubkey, &account.data) {
                Ok(Some(pool)) => pools.push(pool),
                Ok(None) => continue,
                Err(e) => tracing::debug!("Failed to parse pool {}: {}", pubkey, e),
            }
        }
        
        tracing::info!("Successfully fetched {} Orca pools", pools.len());
        Ok(pools)
    }
    
    async fn fetch_pool_by_address(&self, address: &Pubkey) -> Result<Option<PoolInfo>> {
        match self.rpc_client.get_account(address) {
            Ok(account) => self.parse_whirlpool(address, &account.data),
            Err(_) => Ok(None),
        }
    }
}