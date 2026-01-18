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

pub struct RaydiumOnchainFetcher {
    rpc_client: Arc<RpcClient>,
}

impl RaydiumOnchainFetcher {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_client: Arc::new(RpcClient::new(rpc_url)),
        }
    }
    
    fn parse_pool(&self, address: &Pubkey, data: &[u8]) -> Result<Option<PoolInfo>> {
        // Try parsing as AMM v4
        if let Ok(amm_info) = dex::raydium::RaydiumAmmInfo::load_checked(data) {
            let price = self.calculate_price_from_vaults(&amm_info)?;
            return Ok(Some(PoolInfo {
                address: *address,
                dex: DexType::Raydium,
                token_a: TokenMint(amm_info.coin_mint),
                token_b: TokenMint(amm_info.pc_mint),
                price,
                liquidity_usd: self.get_tvl_from_rpc(&amm_info)?,
                fee_bps: 25, // Default AMM fee
                last_updated: std::time::Instant::now(),
            }));
        }
        
        // Try parsing as CP-AMM
        if let Ok(cp_info) = dex::raydium::RaydiumCpAmmInfo::load_checked(data) {
            let price = self.calculate_price_from_vaults_cp(&cp_info)?;
            return Ok(Some(PoolInfo {
                address: *address,
                dex: DexType::Raydium,
                token_a: TokenMint(cp_info.token_0_mint),
                token_b: TokenMint(cp_info.token_1_mint),
                price,
                liquidity_usd: self.get_tvl_from_rpc_cp(&cp_info)?,
                fee_bps: self.get_fee_rate(&cp_info)?,
                last_updated: std::time::Instant::now(),
            }));
        }
        
        // Try parsing as CLMM
        if let Ok(clmm_info) = dex::raydium::clmm_info::PoolState::load_checked(data) {
            let price = self.calculate_price_clmm(&clmm_info)?;
            return Ok(Some(PoolInfo {
                address: *address,
                dex: DexType::Raydium,
                token_a: TokenMint(clmm_info.token_mint_0),
                token_b: TokenMint(clmm_info.token_mint_1),
                price,
                liquidity_usd: self.get_tvl_from_rpc_clmm(&clmm_info)?,
                fee_bps: clmm_info.tick_spacing as u16, // Use tick spacing as proxy
                last_updated: std::time::Instant::now(),
            }));
        }
        
        Ok(None)
    }
    
    fn calculate_price_from_vaults(&self, amm_info: &dex::raydium::RaydiumAmmInfo) -> Result<f64> {
        // Implementation: fetch vault balances and calculate price
        // For brevity, returning placeholder - implement actual logic
        Ok(1.0)
    }
    
    fn calculate_price_from_vaults_cp(&self, cp_info: &dex::raydium::RaydiumCpAmmInfo) -> Result<f64> {
        // Implementation: fetch CP vault balances
        Ok(1.0)
    }
    
    fn calculate_price_clmm(&self, clmm_info: &dex::raydium::clmm_info::PoolState) -> Result<f64> {
        let sqrt_price = clmm_info.sqrt_price_x64 as f64 / (1u128 << 64) as f64;
        Ok(sqrt_price * sqrt_price)
    }
    
    fn get_tvl_from_rpc(&self, amm_info: &dex::raydium::RaydiumAmmInfo) -> Result<f64> {
        // Fetch token balances from vaults and calculate USD value
        // Placeholder - implement with Jupiter price API or on-chain oracle
        Ok(100000.0)
    }
    
    fn get_tvl_from_rpc_cp(&self, cp_info: &dex::raydium::RaydiumCpAmmInfo) -> Result<f64> {
        Ok(100000.0)
    }
    
    fn get_tvl_from_rpc_clmm(&self, clmm_info: &dex::raydium::clmm_info::PoolState) -> Result<f64> {
        Ok(100000.0)
    }
    
    fn get_fee_rate(&self, cp_info: &dex::raydium::RaydiumCpAmmInfo) -> Result<u16> {
        // Fetch fee from amm_config account
        Ok(25)
    }
}

#[async_trait]
impl PoolFetcher for RaydiumOnchainFetcher {
    fn dex_type(&self) -> DexType {
        DexType::Raydium
    }
    
    async fn fetch_pools(&self) -> Result<Vec<PoolInfo>> {
        tracing::info!("Fetching Raydium pools on-chain...");
        
        let config = RpcProgramAccountsConfig {
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let mut all_pools = Vec::new();
        let program_ids = [
            dex::raydium::raydium_program_id(),
            dex::raydium::raydium_cp_program_id(),
            dex::raydium::raydium_clmm_program_id(),
        ];
        
        for program_id in program_ids {
            match self.rpc_client.get_program_accounts_with_config(&program_id, config.clone()) {
                Ok(accounts) => {
                    tracing::info!("Found {} accounts for program {}", accounts.len(), program_id);
                    for (pubkey, account) in accounts {
                        if let Ok(Some(pool)) = self.parse_pool(&pubkey, &account.data) {
                            all_pools.push(pool);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch program accounts for {}: {}", program_id, e);
                }
            }
        }
        
        tracing::info!("Successfully parsed {} Raydium pools", all_pools.len());
        Ok(all_pools)
    }
    
    async fn fetch_pool_by_address(&self, address: &Pubkey) -> Result<Option<PoolInfo>> {
        match self.rpc_client.get_account(address) {
            Ok(account) => self.parse_pool(address, &account.data),
            Err(_) => Ok(None),
        }
    }
}