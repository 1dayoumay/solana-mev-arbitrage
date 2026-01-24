use crate::ata::ensure_base_atas_exist;
use crate::config::Config;
use crate::constants::sol_mint;
use crate::engine::*;
use crate::refresh::initialize_pools_from_markets;
use tokio::time::interval; 
use anyhow::Context;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub async fn run_bot(config_path: &str) -> anyhow::Result<()> {
    let config = Config::load(config_path)?;
    info!("Configuration loaded successfully");

    let rpc_client = Arc::new(RpcClient::new(config.rpc.url.clone()));
    let wallet_kp = load_keypair(&config.wallet.private_key)?;
    info!("Wallet loaded: {}", wallet_kp.pubkey());

    // Initialize engine components
    let price_graph = Arc::new(PriceGraph::new());
    let amount_optimizer = AmountOptimizer::new(price_graph.clone());  // <-- FIXED: Use ::new()

    // Initialize pools
    let mint_pool_data = initialize_pools_from_markets(
        &config.routing.markets,
        &wallet_kp.pubkey(),
        rpc_client.clone(),
    ).await?;

    info!("Initialized {} mints from markets config", mint_pool_data.len());

    // Build initial graph
    for (_, pool_data) in mint_pool_data.iter() {
        price_graph.update_from_mint_pool_data(pool_data, &rpc_client);
    }

    // Spawn detection task
    let graph_clone = price_graph.clone();
    tokio::spawn(async move {
        let mut detect_interval = interval(Duration::from_millis(500));
        loop {
            detect_interval.tick().await;
            
            // <-- FIXED: Call as associated function
            let cycles = CycleDetector::find_negative_cycles(
                &graph_clone,
                sol_mint(),  // <-- Now works because we imported it
                2,
                5,
                50,
            );

            for mut cycle in cycles {
                if let Some(amount) = amount_optimizer.optimize_amount(
                    &mut cycle,
                    2_000_000_000,
                    20,
                    500_000,
                ) {
                    info!("Detected profitable cycle: {} hops, {} bps profit, {} SOL", 
                        cycle.total_hops, 
                        cycle.total_profit_bps,
                        cycle.estimated_profit_lamports as f64 / 1e9
                    );
                }
            }
        }
    });

    // Keep main thread alive
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        info!("Bot heartbeat: {} active mints", mint_pool_data.len());
    }
}

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