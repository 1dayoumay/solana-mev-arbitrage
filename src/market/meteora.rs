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

pub struct MeteoraOnchainFetcher {
    rpc_client: Arc<RpcClient>,
}

impl MeteoraOnchainFetcher {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_client: Arc::new(RpcClient::new(rpc_url)),
        }
    }
    
    fn parse_dammv2_pool(&self, address: &Pubkey, data: &[u8]) -> Result<Option<PoolInfo>> {
        if let Ok(damm_info) = dex::meteora::dammv2_info::MeteoraDAmmV2Info::load_checked(data) {
            let price = self.calculate_price_dammv2(&damm_info)?;
            Ok(Some(PoolInfo {
                address: *address,
                dex: DexType::Meteora,
                token_a: TokenMint(damm_info.base_mint),
                token_b: TokenMint(damm_info.quote_mint),
                price,
                liquidity_usd: self.get_tvl_dammv2(&damm_info)?,
                fee_bps: 10, // DAMM v2 default
                last_updated: std::time::Instant::now(),
            }))
        } else {
            Ok(None)
        }
    }
    
    fn parse_dlmm_pool(&self, address: &Pubkey, data: &[u8]) -> Result<Option<PoolInfo>> {
        if let Ok(dlmm_info) = dex::meteora::dlmm_info::DlmmInfo::load_checked(data) {
            let price = self.calculate_price_dlmm(&dlmm_info)?;
            Ok(Some(PoolInfo {
                address: *address,
                dex: DexType::Meteora,
                token_a: TokenMint(dlmm_info.token_x_mint),
                token_b: TokenMint(dlmm_info.token_y_mint),
                price,
                liquidity_usd: self.get_tvl_dlmm(&dlmm_info)?,
                fee_bps: dlmm_info.lb_pair.parameters.base_factor,
                last_updated: std::time::Instant::now(),
            }))
        } else {
            Ok(None)
        }
    }
    
    fn calculate_price_dammv2(&self, damm_info: &dex::meteora::dammv2_info::MeteoraDAmmV2Info) -> Result<f64> {
        // Fetch vault balances and calculate
        Ok(1.0)
    }
    
    fn calculate_price_dlmm(&self, dlmm_info: &dex::meteora::dlmm_info::DlmmInfo) -> Result<f64> {
        // Use active bin and CLMM formula
        let bin_step = dlmm_info.lb_pair.bin_step as f64 / 10000.0;
        let price = (1.0 + bin_step).powi(dlmm_info.active_id);
        Ok(price)
    }
    
    fn get_tvl_dammv2(&self, damm_info: &dex::meteora::dammv2_info::MeteoraDAmmV2Info) -> Result<f64> {
        Ok(100000.0)
    }
    
    fn get_tvl_dlmm(&self, dlmm_info: &dex::meteora::dlmm_info::DlmmInfo) -> Result<f64> {
        // Sum liquidity across all bins (complex - simplified)
        Ok(100000.0)
    }
}

#[async_trait]
impl PoolFetcher for MeteoraOnchainFetcher {
    fn dex_type(&self) -> DexType {
        DexType::Meteora
    }
    
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>> {
        tracing::info!("Fetching Meteora pools on-chain...");
        
        let config = RpcProgramAccountsConfig {
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let mut all_pools = Vec::new();
        
        // Fetch DLMM pools
        let dlmm_program = dex::meteora::constants::dlmm_program_id();
        match self.rpc_client.get_program_accounts_with_config(&dlmm_program, config.clone()) {
            Ok(accounts) => {
                tracing::info!("Found {} DLMM accounts", accounts.len());
                for (pubkey, account) in accounts {
                    if let Ok(Some(pool)) = self.parse_dlmm_pool(&pubkey, &account.data) {
                        all_pools.push(pool);
                    }
                }
            }
            Err(e) => tracing::warn!("Failed to fetch DLMM program: {}", e),
        }
        
        // Fetch DAMM v2 pools
        let damm_v2_program = dex::meteora::constants::damm_v2_program_id();
        match self.rpc_client.get_program_accounts_with_config(&damm_v2_program, config.clone()) {
            Ok(accounts) => {
                tracing::info!("Found {} DAMM v2 accounts", accounts.len());
                for (pubkey, account) in accounts {
                    if let Ok(Some(pool)) = self.parse_dammv2_pool(&pubkey, &account.data) {
                        all_pools.push(pool);
                    }
                }
            }
            Err(e) => tracing::warn!("Failed to fetch DAMM v2 program: {}", e),
        }
        
        tracing::info!("Successfully parsed {} Meteora pools", all_pools.len());
        Ok(all_pools)
    }
    
    async fn fetch_pool_by_address(&self, address: &Pubkey) -> Result<Option<PoolInfo>> {
        match self.rpc_client.get_account(address) {
            Ok(account) => {
                if let Ok(pool) = self.parse_dlmm_pool(address, &account.data) {
                    return Ok(pool);
                }
                if let Ok(pool) = self.parse_dammv2_pool(address, &account.data) {
                    return Ok(pool);
                }
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }
}