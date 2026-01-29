use crate::ata::ensure_base_atas_exist;
use crate::config::Config;
use crate::constants::sol_mint;
use crate::discovery::{DiscoveryEngine, DiscoveryConfig};
use crate::engine::*;
use crate::refresh::initialize_pools_from_markets;
use anyhow::Context;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn, debug};

/// Shared bot state for dynamic market updates
pub struct BotState {
    markets: Arc<RwLock<Vec<String>>>,
    discovery_engine: Option<DiscoveryEngine>,
}

pub async fn run_bot(config_path: &str) -> anyhow::Result<()> {
    let config = Config::load(config_path)?;
    info!("Configuration loaded successfully");

    let rpc_client = Arc::new(RpcClient::new(config.rpc.url.clone()));
    let wallet_kp = load_keypair(&config.wallet.private_key)?;
    info!("Wallet loaded: {}", wallet_kp.pubkey());

    // Initialize shared bot state
    let bot_state = Arc::new(BotState {
        markets: Arc::new(RwLock::new(Vec::new())),
        discovery_engine: None,
    });

    // Setup and run discovery if enabled in config
    if let Some(discovery_config) = config.discovery.as_ref().filter(|d| d.enabled) {
        // Convert config::DiscoveryConfig to discovery::DiscoveryConfig
        let discovery_config = DiscoveryConfig {
            enabled: discovery_config.enabled,
            interval_minutes: discovery_config.interval_minutes,
            min_liquidity_usd: discovery_config.min_liquidity_usd,
            min_volume_h24: discovery_config.min_volume_h24,
            output_file: discovery_config.output_file.clone(),
        };
        
        let discovery_engine = DiscoveryEngine::new(config.rpc.url.clone(), discovery_config);
        
        // Run initial discovery on startup
        info!("🔄 Running initial pool discovery...");
        match discovery_engine.run_discovery().await {
            Ok(results) => {
                // Save results to JSON file
                if let Err(e) = discovery_engine.save_results(&results).await {
                    error!("Failed to save discovery results: {}", e);
                }
                
                // Load markets from discovery and update state
                let markets = DiscoveryEngine::convert_to_markets(&results);
                *bot_state.markets.write().await = markets;
            }
            Err(e) => {
                error!("❌ Initial discovery failed: {}, falling back to config markets", e);
                // Fallback to config markets if discovery fails
                *bot_state.markets.write().await = config.routing.markets.markets.clone();
            }
        }
        
        // Store engine and start background thread
        // We need to clone the Arc to move into the spawn
        let state_clone = Arc::new(BotState {
            markets: bot_state.markets.clone(),
            discovery_engine: Some(discovery_engine),
        });
        
        tokio::spawn(async move {
            run_background_discovery(state_clone).await;
        });
    } else {
        // Discovery disabled - use static markets from config
        info!("📋 Using static markets from config (discovery disabled)");
        *bot_state.markets.write().await = config.routing.markets.markets.clone();
    }

    // Initialize engine components for arbitrage detection
    let price_graph = Arc::new(PriceGraph::new());
    let amount_optimizer = AmountOptimizer::new(price_graph.clone());

    // Main bot loop
    let mut main_interval = interval(Duration::from_secs(60));
    
    loop {
        main_interval.tick().await;
        
        // Get current markets (may be updated by discovery)
        let markets = bot_state.markets.read().await.clone();
        if markets.is_empty() {
            warn!("⚠️ No markets configured, skipping cycle");
            continue;
        }

        info!("🔍 Processing {} markets", markets.len());

        // Initialize pools from current markets
        let mint_pool_data = match initialize_pools_from_markets(
            &crate::config::MarketsConfig { 
                markets: markets.clone(),  // Use the Vec<String>, not MarketsConfig
                lookup_table_accounts: config.routing.markets.lookup_table_accounts.clone(), 
                process_delay: config.routing.markets.process_delay 
            },
            &wallet_kp.pubkey(),
            rpc_client.clone(),
        ).await {
            Ok(data) => data,
            Err(e) => {
                error!("❌ Failed to initialize pools: {}", e);
                continue;
            }
        };

        info!("✅ Initialized {} mints from markets", mint_pool_data.len());

        // Build price graph from pool data
        for (_, pool_data) in mint_pool_data.iter() {
            price_graph.update_from_mint_pool_data(pool_data, &rpc_client);
        }

        // Run detection cycle (existing logic)
        let cycles = CycleDetector::find_negative_cycles(
            &price_graph,
            sol_mint(),
            2,  // min hops
            5,  // max hops
            50, // min profit bps
        );

        let mut profitable_cycles = 0;
        for mut cycle in cycles {
            if let Some(amount) = amount_optimizer.optimize_amount(
                &mut cycle,
                2_000_000_000, // $2000 in lamports
                20,            // 20% capital per cycle
                500_000,       // 0.005 SOL min profit
            ) {
                profitable_cycles += 1;
                info!("💰 Cycle: {} hops, {} bps, {} SOL profit, {} SOL input",
                    cycle.total_hops,
                    cycle.total_profit_bps,
                    cycle.estimated_profit_lamports as f64 / 1e9,
                    amount as f64 / 1e9
                );
            }
        }

        if profitable_cycles == 0 {
            debug!("No profitable cycles detected this iteration");
        }

        info!("⏱️  Bot heartbeat: {} active mints, {} cycles", mint_pool_data.len(), profitable_cycles);
    }
}

/// Background discovery thread - runs every 15 minutes
async fn run_background_discovery(state: Arc<BotState>) {
    let engine = state.discovery_engine.as_ref().unwrap();
    let mut discovery_interval = interval(Duration::from_secs(60 * 15)); // 15 minutes
    
    info!("🤖 Background discovery thread started (15 min interval)");
    
    loop {
        discovery_interval.tick().await;
        info!("🔄 Running scheduled pool discovery...");
        
        match engine.run_discovery().await {
            Ok(results) => {
                // Save to JSON file
                if let Err(e) = engine.save_results(&results).await {
                    error!("❌ Failed to save discovery results: {}", e);
                    continue;
                }
                
                // Update markets in bot state
                let new_markets = crate::discovery::DiscoveryEngine::convert_to_markets(&results);
                let old_count = state.markets.read().await.len();
                *state.markets.write().await = new_markets.clone();
                
                info!("📈 Markets updated: {} → {} pools", old_count, new_markets.len());
            }
            Err(e) => {
                error!("❌ Background discovery failed: {}", e);
            }
        }
    }
}

/// Load keypair from base58 string or file path
fn load_keypair(private_key: &str) -> anyhow::Result<Keypair> {
    if let Ok(keypair) = bs58::decode(private_key)
        .into_vec()
        .map_err(|e| anyhow::anyhow!("Failed to decode base58: {}", e))
        .and_then(|bytes| {
            Keypair::from_bytes(&bytes).map_err(|e| anyhow::anyhow!("Invalid keypair bytes: {}", e))
        })
    {
        return Ok(keypair);
    }

    if let Ok(keypair) = solana_sdk::signature::read_keypair_file(private_key) {
        return Ok(keypair);
    }

    anyhow::bail!("Failed to load keypair from: {}", private_key)
}