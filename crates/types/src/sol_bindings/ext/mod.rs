//! extension functionality to sol types
use std::fmt;

use alloy_primitives::{Address, TxHash, U256};
use serde::{Deserialize, Serialize};

pub mod contract_bundle_ext;
pub mod grouped_orders;
pub mod top_of_block_ext;

pub trait FetchAssetIndexes {
    fn get_token_in(&self) -> u16;
    fn get_token_out(&self) -> u16;
}

/// The capability of all default orders.
pub trait RawPoolOrder: fmt::Debug + Send + Sync + Clone + Unpin + 'static {
    /// defines  
    /// Hash of the order
    fn order_hash(&self) -> TxHash;

    /// The order signer
    fn from(&self) -> Address;

    /// Amount of tokens to sell
    fn amount_in(&self) -> u128;

    /// Min amount of tokens to buy
    fn amount_out_min(&self) -> u128;

    /// Limit Price
    fn limit_price(&self) -> U256;

    /// Order deadline
    fn deadline(&self) -> Option<U256>;

    /// the way in which we avoid a respend attack
    fn respend_avoidance_strategy(&self) -> RespendAvoidanceMethod;

    /// token in
    fn token_in(&self) -> Address;
    /// token out
    fn token_out(&self) -> Address;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum RespendAvoidanceMethod {
    Nonce(u64),
    Block(u64)
}
