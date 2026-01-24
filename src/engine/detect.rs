use crate::engine::graph::PriceGraph;
use crate::engine::types::*;
use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info};

pub struct CycleDetector;

impl CycleDetector {
    pub fn find_negative_cycles(
        graph: &PriceGraph,
        start_mint: Pubkey,
        min_hops: usize,
        max_hops: usize,
        min_profit_bps: i64,
    ) -> Vec<ArbitrageCycle> {
        let mut cycles = Vec::new();
        let mut distances: HashMap<Pubkey, f64> = HashMap::new();
        let mut predecessors: HashMap<Pubkey, (Pubkey, PoolEdge)> = HashMap::new();
        
        distances.insert(start_mint, 1.0);
        
        for _ in 0..max_hops {
            let mut updated = false;
            
            // <-- FIXED: Added type annotation
            for entry in graph.edges.iter() {
                let from_mint: Pubkey = *entry.key();
                for edge in entry.value() {
                    let current_dist = distances.get(&from_mint).copied().unwrap_or(f64::MAX);
                    let new_dist = current_dist * edge.price;
                    
                    if new_dist < distances.get(&edge.pool_pubkey).copied().unwrap_or(f64::MAX) {
                        distances.insert(edge.pool_pubkey, new_dist);
                        predecessors.insert(edge.pool_pubkey, (from_mint, edge.clone()));
                        updated = true;
                    }
                }
            }
            
            if !updated {
                break;
            }
        }
        
        // Check for negative cycles (profit opportunities)
        // <-- FIXED: Added type annotation
        for entry in graph.edges.iter() {
            let from_mint: Pubkey = *entry.key();
            for edge in entry.value() {
                if let Some(&start_dist) = distances.get(&from_mint) {
                    let new_dist = start_dist * edge.price;
                    
                    if new_dist < distances.get(&edge.pool_pubkey).copied().unwrap_or(f64::MAX) {
                        // <-- FIXED: Changed self.reconstruct_cycle to Self::reconstruct_cycle
                        if let Some(cycle) = Self::reconstruct_cycle(
                            &predecessors,
                            from_mint,
                            edge.pool_pubkey,
                            min_hops,
                            max_hops,
                        ) {
                            if cycle.total_profit_bps > min_profit_bps {
                                cycles.push(cycle);
                            }
                        }
                    }
                }
            }
        }
        
        cycles.sort_by(|a, b| b.total_profit_bps.cmp(&a.total_profit_bps));
        cycles
    }

    fn reconstruct_cycle(
        predecessors: &HashMap<Pubkey, (Pubkey, PoolEdge)>,
        start: Pubkey,
        end: Pubkey,
        min_hops: usize,
        max_hops: usize,
    ) -> Option<ArbitrageCycle> {
        let mut path = Vec::new();
        let mut current = end;
        let mut visited = HashMap::new();
        
        while let Some((prev, edge)) = predecessors.get(&current) {
            if visited.contains_key(&current) {
                break;
            }
            visited.insert(current, true);
            path.push((prev, edge.clone()));
            current = *prev;
            
            if current == start && path.len() >= min_hops {
                break;
            }
            
            if path.len() > max_hops {
                return None;
            }
        }
        
        if path.len() < min_hops || path.len() > max_hops {
            return None;
        }
        
        let mut total_price = 1.0;
        let mut legs = Vec::new();
        
        for (_, edge) in path.iter() {
            total_price *= edge.price;
            legs.push(SwapLeg {
                from_mint: edge.pool_pubkey, // This will be corrected in optimization
                to_mint: edge.pool_pubkey,
                pool_pubkey: edge.pool_pubkey,
                dex_type: edge.dex_type,
                amount_in: 0,
                estimated_amount_out: 0,
            });
        }
        
        let profit_bps = ((total_price - 1.0) * 10_000.0) as i64;
        
        Some(ArbitrageCycle {
            legs,
            total_profit_bps: profit_bps,
            estimated_profit_lamports: 0,
            total_hops: path.len(),
        })
    }
}