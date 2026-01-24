use crate::dex::*;
use crate::engine::types::*;
use crate::pools::*;
use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;  // <-- ADD THIS LINE
use std::sync::Arc;
use tracing::{debug, warn};

pub struct PriceGraph {
    pub edges: Arc<DashMap<Pubkey, Vec<PoolEdge>>>, // Key: from_mint
}

impl PriceGraph {
    pub fn new() -> Self {
        Self {
            edges: Arc::new(DashMap::new()),
        }
    }

    pub fn update_from_mint_pool_data(&self, pool_data: &MintPoolData, rpc_client: &solana_client::rpc_client::RpcClient) {
        let sol_mint = crate::constants::sol_mint();
        
        // Process all pool types
        self.process_raydium_pools(pool_data, sol_mint, rpc_client);
        self.process_raydium_cp_pools(pool_data, sol_mint, rpc_client);
        self.process_pump_pools(pool_data, sol_mint, rpc_client);
        self.process_dlmm_pools(pool_data, sol_mint, rpc_client);
        self.process_whirlpool_pools(pool_data, sol_mint, rpc_client);
        self.process_raydium_clmm_pools(pool_data, sol_mint, rpc_client);
        self.process_meteora_damm_pools(pool_data, sol_mint, rpc_client);
        self.process_meteora_damm_v2_pools(pool_data, sol_mint, rpc_client);
        self.process_vertigo_pools(pool_data, sol_mint, rpc_client);
        self.process_heaven_pools(pool_data, sol_mint, rpc_client);
        self.process_futarchy_pools(pool_data, sol_mint, rpc_client);
        self.process_humidifi_pools(pool_data, sol_mint, rpc_client);
        self.process_pancakeswap_pools(pool_data, sol_mint, rpc_client);
        self.process_byreal_pools(pool_data, sol_mint, rpc_client);
    }

    fn process_raydium_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.raydium_pools {
            if let Ok(price) = self.get_amm_price(&pool.token_vault, &pool.sol_vault, rpc_client) {
                let token_liquidity = self.get_token_balance(&pool.token_vault, rpc_client).unwrap_or(0);
                let sol_liquidity = self.get_token_balance(&pool.sol_vault, rpc_client).unwrap_or(0);
                let liquidity_usd = (sol_liquidity as f64 * 200.0) + (token_liquidity as f64 * price * 200.0);

                // TOKEN -> SOL
                self.add_edge(pool_data.mint, sol_mint, PoolEdge {
                    pool_pubkey: pool.pool,
                    dex_type: DexType::RaydiumV4,
                    price,
                    liquidity_usd,
                    fee_bps: 25,
                    inverse_fee_bps: 25,
                    token_program: pool_data.token_program,
                });

                // SOL -> TOKEN
                self.add_edge(sol_mint, pool_data.mint, PoolEdge {
                    pool_pubkey: pool.pool,
                    dex_type: DexType::RaydiumV4,
                    price: 1.0 / price,
                    liquidity_usd,
                    fee_bps: 25,
                    inverse_fee_bps: 25,
                    token_program: pool_data.token_program,
                });
            }
        }
    }

    fn process_raydium_clmm_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.raydium_clmm_pools {
            if let Ok(pool_state) = crate::dex::raydium::clmm_info::PoolState::load_checked(&rpc_client.get_account(&pool.pool).unwrap().data) {
                let price = self.calculate_clmm_price(pool_state.sqrt_price_x64);
                let liquidity_usd = self.estimate_clmm_liquidity(&pool_state, rpc_client);

                // Determine which mint is which
                if pool.token_mint == pool_state.token_mint_0 {
                    // TOKEN -> SOL
                    self.add_edge(pool.token_mint, pool_state.token_mint_1, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::RaydiumClmm,
                        price,
                        liquidity_usd,
                        fee_bps: 5,
                        inverse_fee_bps: 5,
                        token_program: pool_data.token_program,
                    });
                    // SOL -> TOKEN
                    self.add_edge(pool_state.token_mint_1, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::RaydiumClmm,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 5,
                        inverse_fee_bps: 5,
                        token_program: pool_data.token_program,
                    });
                } else {
                    // TOKEN -> SOL (inverse)
                    self.add_edge(pool.token_mint, pool_state.token_mint_0, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::RaydiumClmm,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 5,
                        inverse_fee_bps: 5,
                        token_program: pool_data.token_program,
                    });
                    // SOL -> TOKEN
                    self.add_edge(pool_state.token_mint_0, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::RaydiumClmm,
                        price,
                        liquidity_usd,
                        fee_bps: 5,
                        inverse_fee_bps: 5,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

    fn process_whirlpool_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.whirlpool_pools {
            if let Ok(whirlpool) = crate::dex::whirlpool::state::Whirlpool::try_deserialize(&rpc_client.get_account(&pool.pool).unwrap().data) {
                let price = self.calculate_clmm_price(whirlpool.sqrt_price);
                let liquidity_usd = (whirlpool.liquidity as f64) * 200.0 / 1e9; // Approximate

                if pool.token_mint == whirlpool.token_mint_a {
                    self.add_edge(pool.token_mint, whirlpool.token_mint_b, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Whirlpool,
                        price,
                        liquidity_usd,
                        fee_bps: 2,
                        inverse_fee_bps: 2,
                        token_program: pool_data.token_program,
                    });
                    self.add_edge(whirlpool.token_mint_b, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Whirlpool,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 2,
                        inverse_fee_bps: 2,
                        token_program: pool_data.token_program,
                    });
                } else {
                    self.add_edge(pool.token_mint, whirlpool.token_mint_a, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Whirlpool,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 2,
                        inverse_fee_bps: 2,
                        token_program: pool_data.token_program,
                    });
                    self.add_edge(whirlpool.token_mint_a, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Whirlpool,
                        price,
                        liquidity_usd,
                        fee_bps: 2,
                        inverse_fee_bps: 2,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

    fn get_amm_price(&self, token_vault: &Pubkey, sol_vault: &Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) -> anyhow::Result<f64> {
        let token_account = rpc_client.get_account(token_vault)?;
        let sol_account = rpc_client.get_account(sol_vault)?;
        
        // Parse token account data to get amount
        let token_amount = self.parse_token_amount(&token_account.data);
        let sol_amount = self.parse_token_amount(&sol_account.data);
        
        if sol_amount == 0 {
            return Err(anyhow::anyhow!("Zero SOL liquidity"));
        }
        
        Ok(token_amount as f64 / sol_amount as f64)
    }

    fn calculate_clmm_price(&self, sqrt_price_x64: u128) -> f64 {
        // price = (sqrt_price_x64 / 2^64)^2
        let sqrt_price = sqrt_price_x64 as f64 / (1u128 << 64) as f64;
        sqrt_price * sqrt_price
    }

    fn estimate_clmm_liquidity(&self, pool_state: &crate::dex::raydium::clmm_info::PoolState, _rpc_client: &solana_client::rpc_client::RpcClient) -> f64 {
        // Approximate: liquidity * sqrt_price gives USD value
        (pool_state.liquidity as f64 * self.calculate_clmm_price(pool_state.sqrt_price_x64)) / 1e9 * 200.0
    }

    fn parse_token_amount(&self, data: &[u8]) -> u64 {
        if data.len() < 64 {
            return 0;
        }
        // Standard token account layout: owner (32) + amount (8)
        let mut amount_bytes = [0u8; 8];
        amount_bytes.copy_from_slice(&data[64..72]);
        u64::from_le_bytes(amount_bytes)
    }

    fn process_raydium_cp_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        // Implementation similar to Raydium V4
        for pool in &pool_data.raydium_cp_pools {
            if let Ok(price) = self.get_amm_price(&pool.token_vault, &pool.sol_vault, rpc_client) {
                let liquidity_usd = self.estimate_amm_liquidity(&pool.token_vault, &pool.sol_vault, rpc_client, price);
                
                self.add_edge(pool_data.mint, sol_mint, PoolEdge {
                    pool_pubkey: pool.pool,
                    dex_type: DexType::RaydiumCp,
                    price,
                    liquidity_usd,
                    fee_bps: 5,
                    inverse_fee_bps: 5,
                    token_program: pool_data.token_program,
                });
                
                self.add_edge(sol_mint, pool_data.mint, PoolEdge {
                    pool_pubkey: pool.pool,
                    dex_type: DexType::RaydiumCp,
                    price: 1.0 / price,
                    liquidity_usd,
                    fee_bps: 5,
                    inverse_fee_bps: 5,
                    token_program: pool_data.token_program,
                });
            }
        }
    }

    fn process_pump_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.pump_pools {
            if let Ok(price) = self.get_amm_price(&pool.token_vault, &pool.sol_vault, rpc_client) {
                let liquidity_usd = self.estimate_amm_liquidity(&pool.token_vault, &pool.sol_vault, rpc_client, price);
                
                self.add_edge(pool_data.mint, sol_mint, PoolEdge {
                    pool_pubkey: pool.pool,
                    dex_type: DexType::Pump,
                    price,
                    liquidity_usd,
                    fee_bps: 100, // Pump has higher fees
                    inverse_fee_bps: 100,
                    token_program: pool_data.token_program,
                });
                
                self.add_edge(sol_mint, pool_data.mint, PoolEdge {
                    pool_pubkey: pool.pool,
                    dex_type: DexType::Pump,
                    price: 1.0 / price,
                    liquidity_usd,
                    fee_bps: 100,
                    inverse_fee_bps: 100,
                    token_program: pool_data.token_program,
                });
            }
        }
    }

    fn estimate_amm_liquidity(&self, token_vault: &Pubkey, sol_vault: &Pubkey, rpc_client: &solana_client::rpc_client::RpcClient, price: f64) -> f64 {
        let token_amount = self.get_token_balance(token_vault, rpc_client).unwrap_or(0);
        let sol_amount = self.get_token_balance(sol_vault, rpc_client).unwrap_or(0);
        (token_amount as f64 * price * 200.0) + (sol_amount as f64 * 200.0)
    }

    fn get_token_balance(&self, vault: &Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) -> anyhow::Result<u64> {
        let account = rpc_client.get_account(vault)?;
        Ok(self.parse_token_amount(&account.data))
    }

    // Stub implementations for other DEX types - add full implementations in Phase 1.x
    fn process_dlmm_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pair in &pool_data.dlmm_pairs {
            match rpc_client.get_account(&pair.pair) {
                Ok(account) => {
                    match crate::dex::meteora::dlmm_info::DlmmInfo::load_checked(&account.data) {
                        Ok(dlmm_info) => {
                            // Calculate price from active bin
                            // price = (1 + bin_step/10000)^active_id
                            let bin_step = dlmm_info.lb_pair.bin_step as f64 / 10_000.0;
                            let price = (1.0 + bin_step).powi(dlmm_info.active_id);
                            
                            // Estimate liquidity from bin arrays (simplified)
                            let liquidity_usd = (dlmm_info.active_id.abs() as f64) * 1000.0; // Approximate

                            // Determine token order
                            if pair.token_mint == dlmm_info.token_x_mint {
                                // TOKEN_X -> TOKEN_Y
                                self.add_edge(dlmm_info.token_x_mint, dlmm_info.token_y_mint, PoolEdge {
                                    pool_pubkey: pair.pair,
                                    dex_type: DexType::MeteoraDlmm,
                                    price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                                // TOKEN_Y -> TOKEN_X
                                self.add_edge(dlmm_info.token_y_mint, dlmm_info.token_x_mint, PoolEdge {
                                    pool_pubkey: pair.pair,
                                    dex_type: DexType::MeteoraDlmm,
                                    price: 1.0 / price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                            } else {
                                // TOKEN_Y -> TOKEN_X
                                self.add_edge(dlmm_info.token_y_mint, dlmm_info.token_x_mint, PoolEdge {
                                    pool_pubkey: pair.pair,
                                    dex_type: DexType::MeteoraDlmm,
                                    price: 1.0 / price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                                // TOKEN_X -> TOKEN_Y
                                self.add_edge(dlmm_info.token_x_mint, dlmm_info.token_y_mint, PoolEdge {
                                    pool_pubkey: pair.pair,
                                    dex_type: DexType::MeteoraDlmm,
                                    price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                            }
                        }
                        Err(e) => warn!("Failed to parse DLMM pool {}: {}", pair.pair, e),
                    }
                }
                Err(e) => warn!("Failed to fetch DLMM pool {}: {}", pair.pair, e),
            }
        }
    }

    fn process_meteora_damm_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.meteora_damm_pools {
            // Use token accounts for price calculation
            if let (Ok(token_x_balance), Ok(sol_balance)) = (
                self.get_token_balance(&pool.token_x_token_vault, rpc_client),
                self.get_token_balance(&pool.token_sol_token_vault, rpc_client)
            ) {
                if sol_balance > 0 {
                    let price = token_x_balance as f64 / sol_balance as f64;
                    let liquidity_usd = (token_x_balance as f64 * price * 200.0) + (sol_balance as f64 * 200.0);

                    self.add_edge(pool.token_mint, sol_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::MeteoraDamm,
                        price,
                        liquidity_usd,
                        fee_bps: 10,
                        inverse_fee_bps: 10,
                        token_program: pool_data.token_program,
                    });

                    self.add_edge(sol_mint, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::MeteoraDamm,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 10,
                        inverse_fee_bps: 10,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

    fn process_meteora_damm_v2_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.meteora_damm_v2_pools {
            // DAMM v2 uses direct vault balances
            if let (Ok(token_x_balance), Ok(sol_balance)) = (
                self.get_token_balance(&pool.token_x_vault, rpc_client),
                self.get_token_balance(&pool.token_sol_vault, rpc_client)
            ) {
                if sol_balance > 0 {
                    let price = token_x_balance as f64 / sol_balance as f64;
                    let liquidity_usd = (token_x_balance as f64 * price * 200.0) + (sol_balance as f64 * 200.0);

                    self.add_edge(pool.token_mint, sol_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::MeteoraDammV2,
                        price,
                        liquidity_usd,
                        fee_bps: 8,
                        inverse_fee_bps: 8,
                        token_program: pool_data.token_program,
                    });

                    self.add_edge(sol_mint, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::MeteoraDammV2,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 8,
                        inverse_fee_bps: 8,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

    fn process_vertigo_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.vertigo_pools {
            if let (Ok(token_x_balance), Ok(sol_balance)) = (
                self.get_token_balance(&pool.token_x_vault, rpc_client),
                self.get_token_balance(&pool.token_sol_vault, rpc_client)
            ) {
                if sol_balance > 0 {
                    let price = token_x_balance as f64 / sol_balance as f64;
                    let liquidity_usd = (token_x_balance as f64 * price * 200.0) + (sol_balance as f64 * 200.0);

                    self.add_edge(pool.token_mint, sol_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Vertigo,
                        price,
                        liquidity_usd,
                        fee_bps: 15,
                        inverse_fee_bps: 15,
                        token_program: pool_data.token_program,
                    });

                    self.add_edge(sol_mint, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Vertigo,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 15,
                        inverse_fee_bps: 15,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

fn process_heaven_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.heaven_pools {
            match rpc_client.get_account(&pool.pool) {
                Ok(account) => {
                    // <-- FIXED: Changed from `if let Ok` to `if let Some`
                    if let Some(heaven_state) = crate::dex::heaven::info::HeavenPoolState::parse(
                        &account.data
                    ) {
                        // Heaven uses reserve ratios
                        if heaven_state.reserve_b > 0 {
                            let price = heaven_state.reserve_a as f64 / heaven_state.reserve_b as f64;
                            let liquidity_usd = (heaven_state.reserve_a as f64 * price * 200.0) + 
                                               (heaven_state.reserve_b as f64 * 200.0);

                            self.add_edge(pool.token_mint, pool.base_mint, PoolEdge {
                                pool_pubkey: pool.pool,
                                dex_type: DexType::Heaven,
                                price,
                                liquidity_usd,
                                fee_bps: 20,
                                inverse_fee_bps: 20,
                                token_program: pool_data.token_program,
                            });

                            self.add_edge(pool.base_mint, pool.token_mint, PoolEdge {
                                pool_pubkey: pool.pool,
                                dex_type: DexType::Heaven,
                                price: 1.0 / price,
                                liquidity_usd,
                                fee_bps: 20,
                                inverse_fee_bps: 20,
                                token_program: pool_data.token_program,
                            });
                        }
                    }
                }
                Err(e) => warn!("Failed to fetch Heaven pool {}: {}", pool.pool, e),
            }
        }
    }

    fn process_futarchy_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.futarchy_pools {
            // Futarchy uses simple vault balances
            if let (Ok(token_x_balance), Ok(sol_balance)) = (
                self.get_token_balance(&pool.token_x_vault, rpc_client),
                self.get_token_balance(&pool.token_sol_vault, rpc_client)
            ) {
                if sol_balance > 0 {
                    let price = token_x_balance as f64 / sol_balance as f64;
                    let liquidity_usd = (token_x_balance as f64 * price * 200.0) + (sol_balance as f64 * 200.0);

                    self.add_edge(pool.token_mint, sol_mint, PoolEdge {
                        pool_pubkey: pool.dao,
                        dex_type: DexType::Futarchy,
                        price,
                        liquidity_usd,
                        fee_bps: 25,
                        inverse_fee_bps: 25,
                        token_program: pool_data.token_program,
                    });

                    self.add_edge(sol_mint, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.dao,
                        dex_type: DexType::Futarchy,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 25,
                        inverse_fee_bps: 25,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

    fn process_humidifi_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        for pool in &pool_data.humidifi_pools {
            // Humidifi uses vault balances
            if let (Ok(token_x_balance), Ok(sol_balance)) = (
                self.get_token_balance(&pool.token_x_vault, rpc_client),
                self.get_token_balance(&pool.token_sol_vault, rpc_client)
            ) {
                if sol_balance > 0 {
                    let price = token_x_balance as f64 / sol_balance as f64;
                    let liquidity_usd = (token_x_balance as f64 * price * 200.0) + (sol_balance as f64 * 200.0);

                    self.add_edge(pool.token_mint, sol_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Humidifi,
                        price,
                        liquidity_usd,
                        fee_bps: 12,
                        inverse_fee_bps: 12,
                        token_program: pool_data.token_program,
                    });

                    self.add_edge(sol_mint, pool.token_mint, PoolEdge {
                        pool_pubkey: pool.pool,
                        dex_type: DexType::Humidifi,
                        price: 1.0 / price,
                        liquidity_usd,
                        fee_bps: 12,
                        inverse_fee_bps: 12,
                        token_program: pool_data.token_program,
                    });
                }
            }
        }
    }

    fn process_pancakeswap_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        // PancakeSwap uses same CLMM as Raydium - duplicate logic
        for pool in &pool_data.pancakeswap_pools {
            match rpc_client.get_account(&pool.pool) {
                Ok(account) => {
                    if account.owner != crate::dex::pancakeswap::pancakeswap_program_id() {
                        warn!("PancakeSwap pool owner mismatch: {}", pool.pool);
                        continue;
                    }
                    
                    match crate::dex::raydium::clmm_info::PoolState::load_checked(&account.data) {
                        Ok(pool_state) => {
                            let price = self.calculate_clmm_price(pool_state.sqrt_price_x64);
                            let liquidity_usd = self.estimate_clmm_liquidity(&pool_state, rpc_client);

                            if pool.token_mint == pool_state.token_mint_0 {
                                self.add_edge(pool.token_mint, pool_state.token_mint_1, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::PancakeSwap,
                                    price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                                self.add_edge(pool_state.token_mint_1, pool.token_mint, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::PancakeSwap,
                                    price: 1.0 / price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                            } else {
                                self.add_edge(pool.token_mint, pool_state.token_mint_0, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::PancakeSwap,
                                    price: 1.0 / price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                                self.add_edge(pool_state.token_mint_0, pool.token_mint, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::PancakeSwap,
                                    price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                            }
                        }
                        Err(e) => warn!("Failed to parse PancakeSwap pool {}: {}", pool.pool, e),
                    }
                }
                Err(e) => warn!("Failed to fetch PancakeSwap pool {}: {}", pool.pool, e),
            }
        }
    }

    fn process_byreal_pools(&self, pool_data: &MintPoolData, sol_mint: Pubkey, rpc_client: &solana_client::rpc_client::RpcClient) {
        // Byreal uses same CLMM as Raydium - duplicate logic
        for pool in &pool_data.byreal_pools {
            match rpc_client.get_account(&pool.pool) {
                Ok(account) => {
                    if account.owner != crate::dex::byreal::byreal_program_id() {
                        warn!("Byreal pool owner mismatch: {}", pool.pool);
                        continue;
                    }
                    
                    match crate::dex::raydium::clmm_info::PoolState::load_checked(&account.data) {
                        Ok(pool_state) => {
                            let price = self.calculate_clmm_price(pool_state.sqrt_price_x64);
                            let liquidity_usd = self.estimate_clmm_liquidity(&pool_state, rpc_client);

                            if pool.token_mint == pool_state.token_mint_0 {
                                self.add_edge(pool.token_mint, pool_state.token_mint_1, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::Byreal,
                                    price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                                self.add_edge(pool_state.token_mint_1, pool.token_mint, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::Byreal,
                                    price: 1.0 / price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                            } else {
                                self.add_edge(pool.token_mint, pool_state.token_mint_0, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::Byreal,
                                    price: 1.0 / price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                                self.add_edge(pool_state.token_mint_0, pool.token_mint, PoolEdge {
                                    pool_pubkey: pool.pool,
                                    dex_type: DexType::Byreal,
                                    price,
                                    liquidity_usd,
                                    fee_bps: 5,
                                    inverse_fee_bps: 5,
                                    token_program: pool_data.token_program,
                                });
                            }
                        }
                        Err(e) => warn!("Failed to parse Byreal pool {}: {}", pool.pool, e),
                    }
                }
                Err(e) => warn!("Failed to fetch Byreal pool {}: {}", pool.pool, e),
            }
        }
    }

    fn add_edge(&self, from_mint: Pubkey, to_mint: Pubkey, edge: PoolEdge) {
        debug!("Adding edge: {} -> {} (price: {}, dex: {:?})", from_mint, to_mint, edge.price, edge.dex_type);
        self.edges.entry(from_mint).or_insert_with(Vec::new).push(edge);
    }
}