use alloy_primitives::{
    aliases::{I24, U24},
    Address
};
use angstrom_types::primitive::PoolId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub tokens:   Vec<TokenContext>,
    pub universe: Vec<PoolContext>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenContext {
    pub symbol:   String,
    pub address:  Address,
    pub decimals: u8
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolContext {
    pub pool_address: Address,
    pub pool_ticker:  String,
    pub pool_id:      PoolId,
    pub token0:       Address,
    pub token1:       Address,
    pub fee:          U24,
    pub tick_spacing: I24,
    pub storage_idx:  u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerContext {
    pub pool:  PoolContext,
    pub stats: PoolStats
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCandle {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CandleTimeframe {
    OneMinute,
    FifteenMinutes,
    OneHour,
    OneDay
}
