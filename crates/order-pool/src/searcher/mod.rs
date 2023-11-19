use std::collections::{BTreeMap, HashMap};

use alloy_primitives::{Address, B256};
use composable::ComposableSearcherPool;
use guard_types::orders::{OrderId, PooledComposableOrder, PooledSearcherOrder};

use self::searcher::VanillaSearcherPool;
use crate::common::SizeTracker;

mod composable;
mod searcher;

pub struct SearcherPool<T: PooledSearcherOrder, C: PooledComposableOrder + PooledSearcherOrder> {
    /// Holds all non composable searcher order pools
    searcher_orders: VanillaSearcherPool<T>,
    /// Holds all composable searcher order pools
    composable_searcher_orders: ComposableSearcherPool<C>,
    /// The size of the current transactions.
    size: SizeTracker
}
