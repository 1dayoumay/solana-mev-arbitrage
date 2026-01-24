# Cross-Dex Arbitrage System: Progressive Complexity Roadmap

## Architecture Foundation (Current State)

**Present**: Pool discovery via `initialize_pools_from_markets()`, real-time state refresh (`PoolDataRefresher`), multi-base token support, account derivation cache.

**Missing**: Price calculation engine, graph structure, cycle detection, simulation layer, instruction builders, Jito integration.

---

## Phase 1: Off-Chain Logic & Simulation Engine (Zero Transaction Cost)

### 1.1 Real-Time Graph Construction
**Methodology**: 
- **Nodes**: Token mints (SOL, USDC, TOKEN_A).
- **Edges**: Bidirectional per pool. Store `price`, `liquidity_depth`, `fee_tier`, `DexType`, and **SDK pool struct pointer**.
- **Price Sources**: 
  - CLMM: `price = sqrt_price_x64² / 2^128`
  - AMM: `price = reserve_out / reserve_in`
  - and so on...

### 1.2 Negative Cycle Detection
**Algorithm**: Tarjan's SCC. Filter cycles by `min_hops` and `max_hops` (configurable, default 2-5). Exclude 2-hop unless explicitly enabled.
**Pre-Filter**: `Π(price_i) > 1.005` (0.5% profit after fees) before simulation.

### 1.3 Amount Optimization & Slippage Modeling
**Binary Search**: Find `max_amount_in` where profit > 0, bounded by `min(capital, liquidity/10)`.
**Slippage**: `slippage_bp = 10 + (trade_size / pool_liquidity) * 10000`. Applied per leg to `minimum_out`.

### 1.4 Simulation Engine (Local)
**SDK Integration**: Call `sol-trade-sdk`'s `simulate` flag in `TradeBuyParams`. Parse logs for exact output amounts and CU consumption.
**Validation**: 
- Verify `simulated_profit > jito_tip + priority_fee + 0.01 SOL` buffer.
- Check `simulation_result.value.err.is_none()`.
- Measure `latency_ms` from detection to simulate finish.

---

## Phase 2: Transaction Building & Validation (Local Execution)

### 2.1 SDK Instruction Composition
**Mapping**: Convert `SwapLeg` to SDK `TradeBuyParams`:
- `DexType` → SDK enum variant.
- `MintPoolData` → SDK `*Params` struct (e.g., `PumpFunParams` from `PumpPool`).
- Set `create_input_mint_ata: true`, `close_input_mint_ata: true` for WSOL legs.

### 2.2 Multi-Leg Transaction Assembly
**Pattern**: 
```rust
let mut instructions = vec![compute_budget_ix];
for leg in route.legs {
    let mut leg_ixs = sdk_builder.build_buy_instructions(&params).await?;
    instructions.append(&mut leg_ixs);
}
```
**Size Check**: If `instructions.len() > 20`, generate ALT via SDK's `address_lookup` module.

### 2.3 Local Validation
**Dry Run**: Execute `rpc.simulate_transaction()` on fully assembled transaction (without sending).
**Assert**:
- `simulation_value.err.is_none()`
- `simulation_value.logs.contains("Profit: X SOL")`
- Transaction size < 1232 bytes (or ALT populated correctly).

---

## Phase 3: On-Chain Execution via Jito Bundles

### 3.1 Jito Integration
**Bundle Construction**: 
- Add tip instruction: `ComputeBudgetInstruction::transfer(min(simulated_profit * 0.15, 0.05 SOL))` to Jito tip account.
- Use `VersionedTransaction` with ALT. **Single bundle per cycle** (atomic execution).
- **Timeout**: 300ms. Retry 3x with 20% tip increase each attempt.

### 3.2 Execution Modes (Configurable)
- **Sequential**: For 2-hop cycles, single transaction. No executor program needed.
- **Bundled**: For 3+ hop cycles, Jito bundle with all legs. **No executor program**—relies on bundle atomicity.
- **Executor Program** (Optional): Deploy Anchor CPI program for true atomic multi-hop if bundle reliability is insufficient.

### 3.3 Landing Verification
**Callback**: Parse `bundle_result` for transaction signatures.
**Metrics**: Track `bundle_landing_rate` and `actual_profit` vs `simulated_profit`.

---

## Phase 4: Capital Efficiency & Advanced Features

### 4.1 Flash Loan Integration (Toggleable)
**Trigger Condition**: Enable when `trade_size > 6000 * flash_loan_fee` (e.g., 0.09% Kamino fee → trades >$6k).
**Flow**: 
1. Flash loan borrow → 2. Execute cycle → 3. Repay loan + fee → 4. Profit remains.
**Implementation**: Wrap SDK builders in loan/repay instructions via `solend_sdk`.

### 4.2 Multi-Threaded Detection
**Parallelization**: Use `rayon` crate for graph traversal. Shard graph by token mint prefix (e.g., 16 shards on `mint[0] & 0xF`).

### 4.3 Dynamic Configuration Reload
**Hot Reload**: Watch `config.toml` for changes. Update `min_hops`, `max_hops`, `profit_threshold` without restart.

---

## Implementation Checklist (Present vs. Required)

| Component | Status | Action |
|-----------|--------|--------|
| **Pool Discovery** | ✅ Present | Keep `initialize_pools_from_markets()` |
| **State Refresh** | ✅ Present | Keep `PoolDataRefresher` |
| **Account Derivation** | ✅ Present | Keep `MintPoolData` structure |
| **Graph Engine** | ❌ Missing | Implement `src/engine/graph.rs` with `dashmap` |
| **Cycle Detection** | ❌ Missing | Implement `src/engine/detect.rs` with Tarjan's |
| **Amount Optimization** | ❌ Missing | Implement `src/engine/optimize.rs` binary search |
| **SDK Integration** | ❌ Missing | Add `sol-trade-sdk` dependency |
| **Simulation Layer** | ❌ Missing | Wrap SDK's `simulate` in `src/engine/simulate.rs` |
| **Instruction Builder** | ❌ Missing | Build `src/execution/builder.rs` to map legs to SDK params |
| **Jito Bundles** | ❌ Missing | Integrate SDK's bundle middleware in `src/execution/bundle.rs` |
| **Flash Loans** | ❌ Missing | Add `src/execution/flashloan.rs` with toggle |

---

## Configuration Schema

```toml
[arbitrage.engine]
min_hops = 2
max_hops = 5
min_profit_basis_points = 50
capital_per_cycle_percent = 20

[arbitrage.simulation]
slippage_buffer_bp = 20
enabled = true

[arbitrage.execution]
mode = "bundled"  # "sequential", "bundled", "executor"
jito_tip_dynamic = true
jito_tip_min_lamports = 10000000
jito_tip_max_lamports = 50000000
bundle_timeout_ms = 300

[arbitrage.flashloan]
enabled = false
min_trade_size_usd = 6000
provider = "kamino"

[monitoring]
log_format = "json"
metrics_enabled = true
daily_stop_loss_usd = 100
circuit_breaker_failures = 5
circuit_breaker_window_minutes = 10
```

---
Proceed sequentially. Do not advance until current phase gates are met.