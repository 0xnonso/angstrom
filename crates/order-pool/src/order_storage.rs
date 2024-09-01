use std::{
    fmt::Debug,
    sync::{Arc, Mutex}
};

use alloy_primitives::FixedBytes;
use angstrom_metrics::OrderStorageMetricsWrapper;
use angstrom_types::{
    orders::{OrderId, OrderSet},
    sol_bindings::{
        grouped_orders::{AllOrders, GroupedUserOrder, GroupedVanillaOrder, OrderWithStorageData},
        sol::TopOfBlockOrder
    }
};

use crate::{
    finalization_pool::FinalizationPool,
    limit::{LimitOrderPool, LimitPoolError},
    searcher::{SearcherPool, SearcherPoolError},
    PoolConfig
};

/// The Storage of all verified orders.
#[derive(Default, Clone)]
pub struct OrderStorage {
    pub limit_orders:                Arc<Mutex<LimitOrderPool>>,
    pub searcher_orders:             Arc<Mutex<SearcherPool>>,
    pub pending_finalization_orders: Arc<Mutex<FinalizationPool>>,
    pub metrics:                     OrderStorageMetricsWrapper
}

impl Debug for OrderStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Simplified implementation for the moment
        write!(f, "OrderStorage")
    }
}

impl OrderStorage {
    pub fn new(config: &PoolConfig) -> Self {
        let limit_orders = Arc::new(Mutex::new(LimitOrderPool::new(
            &config.ids,
            Some(config.lo_pending_limit.max_size)
        )));
        let searcher_orders = Arc::new(Mutex::new(SearcherPool::new(
            &config.ids,
            Some(config.s_pending_limit.max_size)
        )));
        let pending_finalization_orders = Arc::new(Mutex::new(FinalizationPool::new()));

        Self {
            limit_orders,
            searcher_orders,
            pending_finalization_orders,
            metrics: OrderStorageMetricsWrapper::default()
        }
    }

    pub fn add_new_limit_order(
        &self,
        order: OrderWithStorageData<GroupedUserOrder>
    ) -> Result<(), LimitPoolError> {
        if order.is_vanilla() {
            let mapped_order = order.try_map_inner(|this| {
                let GroupedUserOrder::Vanilla(order) = this else {
                    return Err(eyre::eyre!("unreachable"))
                };
                Ok(order)
            })?;

            self.limit_orders
                .lock()
                .expect("lock poisoned")
                .add_vanilla_order(mapped_order)?;
            self.metrics.incr_vanilla_limit_orders(1);
        } else {
            let mapped_order = order.try_map_inner(|this| {
                let GroupedUserOrder::Composable(order) = this else {
                    return Err(eyre::eyre!("unreachable"))
                };
                Ok(order)
            })?;

            self.limit_orders
                .lock()
                .expect("lock poisoned")
                .add_composable_order(mapped_order)?;
            self.metrics.incr_composable_limit_orders(1);
        }

        Ok(())
    }

    pub fn add_new_searcher_order(
        &self,
        order: OrderWithStorageData<TopOfBlockOrder>
    ) -> Result<(), SearcherPoolError> {
        self.searcher_orders
            .lock()
            .expect("lock poisoned")
            .add_searcher_order(order)?;

        self.metrics.incr_searcher_orders(1);

        Ok(())
    }

    pub fn add_filled_orders(
        &self,
        block_number: u64,
        orders: Vec<OrderWithStorageData<AllOrders>>
    ) {
        let num_orders = orders.len();
        self.pending_finalization_orders
            .lock()
            .expect("poisoned")
            .new_orders(block_number, orders);

        self.metrics.incr_pending_finalization_orders(num_orders);
    }

    pub fn finalized_block(&self, block_number: u64) {
        let orders = self
            .pending_finalization_orders
            .lock()
            .expect("poisoned")
            .finalized(block_number);

        self.metrics.decr_pending_finalization_orders(orders.len());
    }

    pub fn reorg(&self, order_hashes: Vec<FixedBytes<32>>) -> Vec<AllOrders> {
        let orders = self
            .pending_finalization_orders
            .lock()
            .expect("poisoned")
            .reorg(order_hashes)
            .collect::<Vec<_>>();

        self.metrics.decr_pending_finalization_orders(orders.len());
        orders
    }

    pub fn remove_searcher_order(&self, id: &OrderId) -> Option<OrderWithStorageData<AllOrders>> {
        let order = self
            .searcher_orders
            .lock()
            .expect("posioned")
            .remove_order(id)
            .map(|value| {
                value
                    .try_map_inner(|v| {
                        self.metrics.decr_searcher_orders(1);
                        Ok(AllOrders::TOB(v))
                    })
                    .unwrap()
            });

        order
    }

    pub fn remove_limit_order(&self, id: &OrderId) -> Option<OrderWithStorageData<AllOrders>> {
        self.limit_orders
            .lock()
            .expect("poisoned")
            .remove_order(id)
            .map(|order| {
                if order.is_vanilla() {
                    self.metrics.decr_vanilla_limit_orders(1);
                } else if order.is_composable() {
                    self.metrics.decr_composable_limit_orders(1);
                }

                order.try_map_inner(|inner| Ok(inner.into())).ok()
            })
            .flatten()
    }

    pub fn get_all_orders(&self) -> OrderSet<GroupedVanillaOrder, TopOfBlockOrder> {
        let limit = self.limit_orders.lock().expect("poisoned").get_all_orders();
        let searcher = self
            .searcher_orders
            .lock()
            .expect("poisoned")
            .get_all_orders();

        OrderSet { limit, searcher }
    }
}
