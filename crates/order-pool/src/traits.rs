use std::fmt;

use alloy_primitives::{Address, Bytes, TxHash, U128, U256};
use guard_types::{
    primitive::{ComposableOrder, Order, PoolKey},
    rpc::{
        EcRecoveredComposableLimitOrder, EcRecoveredComposableSearcherOrder, EcRecoveredLimitOrder,
        EcRecoveredSearcherOrder, SignedComposableLimitOrder
    }
};

#[async_trait::async_trait]
#[auto_impl::auto_impl(Arc)]
pub trait OrderPool: Send + Sync + Clone {
    /// The transaction type of the limit order pool
    type LimitOrder: PooledLimitOrder;

    /// The transaction type of the searcher order pool
    type SearcherOrder: PooledSearcherOrder;

    /// The transaction type of the composable limit order pool
    type ComposableLimitOrder: PooledComposableOrder + PooledLimitOrder;

    /// The transaction type of the composable searcher order pool
    type ComposableSearcherOrder: PooledComposableOrder + PooledSearcherOrder;
}

pub trait PooledOrder: fmt::Debug + Send + Sync {
    /// Hash of the order
    fn hash(&self) -> TxHash;

    /// The order signer
    fn from(&self) -> Address;

    /// Transaction nonce
    fn nonce(&self) -> U256;

    /// Amount of tokens to sell
    fn amount_in(&self) -> u128;

    /// Min amount of tokens to buy
    fn amount_out_min(&self) -> u128;

    /// Limit Price
    fn limit_price(&self) -> u128;

    /// Order deadline
    fn deadline(&self) -> U256;

    /// Returns a measurement of the heap usage of this type and all its
    /// internals.
    fn size(&self) -> usize;

    /// Returns the length of the rlp encoded transaction object
    ///
    /// Note: Implementations should cache this value.
    fn encoded_length(&self) -> usize;

    /// Returns chain_id
    fn chain_id(&self) -> Option<u64>;
}

pub trait PooledLimitOrder: PooledOrder {
    /// The liquidity pool this order trades in
    fn pool_and_direction(&self) -> (u8, bool);
}

pub trait PooledSearcherOrder: PooledOrder {
    /// The liquidity pool this order trades in
    fn pool(&self) -> u8;
    /// donate value
    fn donate(&self) -> (U128, U128);
}

trait PooledComposableOrder: PooledOrder {
    fn pre_hook(&self) -> Option<Bytes>;

    fn post_hook(&self) -> Option<Bytes>;
}

impl PooledOrder for EcRecoveredLimitOrder {
    fn hash(&self) -> TxHash {
        self.signed_order.hash
    }

    fn from(&self) -> Address {
        self.signer
    }

    fn nonce(&self) -> U256 {
        self.order.nonce
    }

    fn amount_in(&self) -> u128 {
        self.signed_order.order.amountIn
    }

    fn amount_out_min(&self) -> u128 {
        self.signed_order.order.amountOutMin
    }

    fn limit_price(&self) -> u128 {
        self.amount_out_min() / self.amount_in()
    }

    fn deadline(&self) -> U256 {
        self.signed_order.order.deadline
    }

    fn size(&self) -> usize {
        unreachable!()
    }

    fn encoded_length(&self) -> usize {
        unreachable!()
    }

    fn chain_id(&self) -> Option<u64> {
        unreachable!()
    }
}

impl PooledLimitOrder for EcRecoveredLimitOrder {
    fn pool_and_direction(&self) -> (u8, bool) {
        //(self.signed_order.order.pool, self.signed_order.order.direction)
        todo!()
    }
}
