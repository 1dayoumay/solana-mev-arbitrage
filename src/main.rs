mod config;
mod error;
mod types;
mod market;
mod graph;
mod utils;

use crate::config::AppConfig;
use crate::graph::{GraphEngine, PetgraphEngine};
use crate::market::{MarketOrchestrator, PoolFetcher};
use crate::market::raydium::RaydiumApiFetcher;
use crate::market::meteora::MeteoraApiFetcher;
use crate::market::orca::OrcaOnchainFetcher;
use std::sync::Arc;
use tokio::signal;
use tokio::time::{interval, Duration};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing - FIXED: .json() not .with_json()
    tracing_subscriber::fmt()
        .json()
        .with_level(true)
        .init();
    
    info!("Starting MEV Bot Phase 1 - Hybrid Pool Indexer");
    
    // Load configuration
    let app_config = AppConfig::from_env()?;
    
    // Initialize graph engine
    let mut graph_engine = PetgraphEngine::new();
    
    // Initialize DEX fetchers
    let mut fetchers: Vec<Box<dyn PoolFetcher>> = Vec::new();
    
    for dex_config in &app_config.dex_configs {
        match dex_config.name {
            "Raydium" => {
                fetchers.push(Box::new(RaydiumApiFetcher::new(dex_config.clone())));
                info!("Initialized Raydium API fetcher");
            }
            "Meteora" => {
                fetchers.push(Box::new(MeteoraApiFetcher::new(dex_config.clone())));
                info!("Initialized Meteora API fetcher");
            }
            "Orca" => {
                fetchers.push(Box::new(OrcaOnchainFetcher::new(
                    dex_config.clone(),
                    app_config.rpc_url.clone()
                )));
                info!("Initialized Orca on-chain fetcher");
            }
            _ => {
                error!("Unknown DEX: {}", dex_config.name);
            }
        }
    }
    
    let orchestrator = MarketOrchestrator::new(fetchers);
    
    // Graceful shutdown handling
    let shutdown_token = Arc::new(tokio::sync::Notify::new());
    let shutdown_token_clone = shutdown_token.clone();
    
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Shutdown signal received");
        shutdown_token_clone.notify_waiters();
    });
    
    // Main update loop
    let mut ticker = interval(Duration::from_secs(app_config.update_interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    info!("Starting pool update loop ({} second interval)", app_config.update_interval_secs);
    
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                info!("Fetching pool updates...");
                
                match orchestrator.fetch_all_pools().await {
                    Ok(pools) => {
                        let filtered_pools: Vec<_> = pools.into_iter()
                            .filter(|p| p.liquidity_usd >= app_config.min_liquidity_usd)
                            .collect();
                        
                        info!("Retrieved {} pools ({} meet liquidity threshold)", 
                            filtered_pools.len(), filtered_pools.len());
                        
                        // Update graph
                        for pool in filtered_pools {
                            if let Err(e) = graph_engine.add_or_update_pool(pool) {
                                error!("Failed to add pool to graph: {}", e);
                            }
                        }
                        
                        // Log graph stats
                        let total_pools = graph_engine.get_all_pools().len();
                        info!("Graph contains {} active pools", total_pools);
                    }
                    Err(e) => {
                        error!("Failed to fetch pools: {}", e);
                    }
                }
            }
            _ = shutdown_token.notified() => {
                info!("Shutting down gracefully...");
                break;
            }
        }
    }
    
    info!("Pool indexer stopped");
    Ok(())
}