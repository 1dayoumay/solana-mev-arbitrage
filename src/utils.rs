use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

pub type DirectRateLimiter = RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>;

pub fn create_rate_limiter(rps: u32) -> DirectRateLimiter {
    let quota = Quota::per_second(NonZeroU32::new(rps).unwrap());
    RateLimiter::direct(quota)
}

pub fn validate_pool_liquidity(liquidity: f64, threshold: f64) -> bool {
    liquidity >= threshold
}

pub fn calculate_price_from_tvl(token_a_liquidity: f64, token_b_liquidity: f64) -> f64 {
    if token_a_liquidity > 0.0 {
        token_b_liquidity / token_a_liquidity
    } else {
        0.0
    }
}