use crate::engine::graph::PriceGraph;
use crate::engine::types::*;
use dashmap::DashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{debug, info};

pub struct AmountOptimizer {
    graph: Arc<PriceGraph>,
}

impl AmountOptimizer {
    pub fn new(graph: Arc<PriceGraph>) -> Self {
        Self { graph }
    }

    pub fn optimize_amount(
        &self,
        cycle: &mut ArbitrageCycle,
        max_capital_lamports: u64,
        capital_per_cycle_percent: u64,
        min_profit_lamports: u64,
    ) -> Option<u64> {
        let max_amount = (max_capital_lamports * capital_per_cycle_percent) / 100;
        let mut low = 1_000_000; // 0.001 SOL min
        let mut high = max_amount;

        if high < low {
            return None;
        }

        let mut best_amount = 0;
        let mut best_profit = 0;

        // Binary search for optimal amount
        for _ in 0..20 {
            let mid = (low + high) / 2;
            
            if let Some(profit) = self.simulate_cycle_with_amount(cycle, mid) {
                if profit > best_profit && profit > min_profit_lamports {
                    best_profit = profit;
                    best_amount = mid;
                    low = mid; // Try larger amounts
                } else {
                    high = mid; // Try smaller amounts
                }
            } else {
                high = mid; // Reduce amount if simulation fails
            }
        }

        if best_amount > 0 && best_profit > min_profit_lamports {
            self.update_leg_amounts(cycle, best_amount);
            info!("Optimized cycle: initial={} lamports, profit={} lamports", best_amount, cycle.estimated_profit_lamports);
            Some(best_amount)
        } else {
            None
        }
    }

    fn simulate_cycle_with_amount(&self, cycle: &ArbitrageCycle, initial_amount: u64) -> Option<u64> {
        let mut current_amount = initial_amount;
        
        for leg in &cycle.legs {
            // Look up the edge from graph to get price and fee
            if let Some(edge) = self.find_edge_in_graph(leg) {
                let slippage_bps = self.calculate_slippage_bps(current_amount, leg.pool_pubkey);
                let effective_fee_bps = edge.fee_bps + slippage_bps;
                let fee_multiplier = (10_000 - effective_fee_bps) as f64 / 10_000.0;
                
                current_amount = (current_amount as f64 * edge.price * fee_multiplier) as u64;
                
                if current_amount == 0 {
                    return None;
                }
            } else {
                return None; // Edge not found
            }
        }
        
        if current_amount > initial_amount {
            Some(current_amount - initial_amount)
        } else {
            None
        }
    }

    /// Look up an edge in the graph using the leg information
    fn find_edge_in_graph(&self, leg: &SwapLeg) -> Option<PoolEdge> {
        if let Some(edges) = self.graph.edges.get(&leg.from_mint) {
            for edge in edges.value().iter() {
                if edge.pool_pubkey == leg.pool_pubkey && edge.dex_type == leg.dex_type {
                    return Some(edge.clone());
                }
            }
        }
        None
    }

    /// Calculate real slippage based on pool liquidity ratio
    /// 
    /// Phase 1.x: Now uses actual pool liquidity from the graph instead of estimates
    fn calculate_slippage_bps(&self, amount_in: u64, pool_pubkey: Pubkey) -> u64 {
        // Iterate graph edges to find the pool and get liquidity
        for entry in self.graph.edges.iter() {
            for edge in entry.value() {
                if edge.pool_pubkey == pool_pubkey {
                    let trade_size_usd = (amount_in as f64) / 1e9 * 200.0; // Convert to USD
                    let pool_liquidity_usd = edge.liquidity_usd.max(1.0); // Avoid division by zero
                    
                    let liquidity_ratio = trade_size_usd / pool_liquidity_usd;
                    let dynamic_slippage = (liquidity_ratio * 0.5 * 100.0) as u64; // 0.5 bps per %
                    
                    let total_slippage = 10u64 + dynamic_slippage;
                    
                    debug!(
                        "Pool {}: trade_size=${:.2}, pool_liq=${:.2}, ratio={:.4}%, slippage={} bps",
                        pool_pubkey, trade_size_usd, pool_liquidity_usd, 
                        liquidity_ratio * 100.0, total_slippage
                    );
                    
                    return total_slippage.min(100); // Cap at 100 bps (1%)
                }
            }
        }
        
        debug!("Pool {} not found in graph for slippage calculation", pool_pubkey);
        50u64 // Default 50 bps slippage
    }

    fn update_leg_amounts(&self, cycle: &mut ArbitrageCycle, initial_amount: u64) {
        let mut current_amount = initial_amount;
        
        for leg in cycle.legs.iter_mut() {
            leg.amount_in = current_amount;
            
            if let Some(edge) = self.find_edge_in_graph(leg) {
                let slippage_bps = self.calculate_slippage_bps(current_amount, leg.pool_pubkey);
                let effective_fee_bps = edge.fee_bps + slippage_bps;
                let fee_multiplier = (10_000 - effective_fee_bps) as f64 / 10_000.0;
                
                current_amount = (current_amount as f64 * edge.price * fee_multiplier) as u64;
                leg.estimated_amount_out = current_amount;
            } else {
                leg.estimated_amount_out = 0;
            }
        }
        
        cycle.estimated_profit_lamports = current_amount.saturating_sub(initial_amount);
    }
}