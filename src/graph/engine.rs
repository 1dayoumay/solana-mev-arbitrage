use super::GraphEngine;
use crate::types::{PoolInfo, TokenMint, PathEdge, DexType};
use crate::error::{BotError, Result};
use petgraph::graphmap::DiGraphMap;
use std::collections::HashMap;
use solana_sdk::pubkey::Pubkey;

pub struct PetgraphEngine {
    graph: DiGraphMap<TokenMint, PathEdge>,
    pool_index: HashMap<Pubkey, (TokenMint, TokenMint)>,
    pools: HashMap<Pubkey, PoolInfo>,
}

impl PetgraphEngine {
    pub fn new() -> Self {
        Self {
            graph: DiGraphMap::new(),
            pool_index: HashMap::new(),
            pools: HashMap::new(),
        }
    }
}

impl GraphEngine for PetgraphEngine {
    fn add_or_update_pool(&mut self, pool: PoolInfo) -> Result<()> {
        let token_a = pool.token_a;
        let token_b = pool.token_b;
        let address = pool.address;
        
        let edge = PathEdge {
            pool_address: address,
            dex: pool.dex,
            price: pool.price,
            liquidity_usd: pool.liquidity_usd,
            fee_bps: pool.fee_bps,
        };
        
        self.graph.add_node(token_a);
        self.graph.add_node(token_b);
        self.graph.add_edge(token_a, token_b, edge.clone());
        
        let reverse_edge = PathEdge {
            price: if pool.price > 0.0 { 1.0 / pool.price } else { 0.0 },
            ..edge
        };
        self.graph.add_edge(token_b, token_a, reverse_edge);
        
        self.pool_index.insert(address, (pool.token_a, pool.token_b));
        self.pools.insert(address, pool);
        
        Ok(())
    }
    
    fn get_pool(&self, address: &Pubkey) -> Option<&PoolInfo> {
        self.pools.get(address)
    }
    
    fn remove_pool(&mut self, address: &Pubkey) -> Result<()> {
        if let Some((token_a, token_b)) = self.pool_index.remove(address) {
            self.graph.remove_edge(token_a, token_b);
            self.graph.remove_edge(token_b, token_a);
            self.pools.remove(address);
        }
        Ok(())
    }
    
    fn get_all_pools(&self) -> Vec<&PoolInfo> {
        self.pools.values().collect()
    }
    
    fn clear(&mut self) {
        self.graph.clear();
        self.pool_index.clear();
        self.pools.clear();
    }
}