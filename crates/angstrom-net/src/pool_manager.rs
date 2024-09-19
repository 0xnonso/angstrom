use std::{
    collections::HashMap,
    future::IntoFuture,
    marker::PhantomData,
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll}
};

use angstrom_eth::manager::EthEvent;
use angstrom_types::{
    contract_bindings::poolmanager::PoolManager::{syncCall, PoolManagerCalls::updateDynamicLPFee},
    orders::{OrderOrigin, OrderPriorityData, OrderSet},
    primitive::Order,
    sol_bindings::{
        grouped_orders::{
            AllOrders, FlashVariants, GroupedVanillaOrder, OrderWithStorageData, StandingVariants
        },
        sol::TopOfBlockOrder
    }
};
use futures::{
    future::BoxFuture,
    poll,
    stream::{BoxStream, FuturesUnordered},
    Future, FutureExt, Stream, StreamExt
};
use order_pool::{
    order_storage::OrderStorage, OrderIndexer, OrderPoolHandle, PoolConfig, PoolInnerEvent,
    PoolManagerUpdate
};
use reth_metrics::common::mpsc::UnboundedMeteredReceiver;
use reth_network::transactions::ValidationOutcome;
use reth_network_peers::PeerId;
use reth_primitives::{TxHash, B256};
use reth_rpc_types::txpool::TxpoolStatus;
use reth_tasks::TaskSpawner;
use tokio::sync::{
    broadcast,
    broadcast::{Receiver, Sender},
    mpsc,
    mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender},
    oneshot
};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream, UnboundedReceiverStream};
use validation::{
    order::{
        self, order_validator::OrderValidator, OrderValidationRequest, OrderValidationResults,
        OrderValidatorHandle, ValidationFuture
    },
    validator::ValidationRequest
};

use crate::{
    LruCache, NetworkOrderEvent, ReputationChangeKind, StromMessage, StromNetworkEvent,
    StromNetworkHandle
};

/// Cache limit of transactions to keep track of for a single peer.
const PEER_ORDER_CACHE_LIMIT: usize = 1024 * 10;

/// Api to interact with [`PoolManager`] task.
#[derive(Debug, Clone)]
pub struct PoolHandle {
    pub manager_tx:      UnboundedSender<OrderCommand>,
    pub pool_manager_tx: tokio::sync::broadcast::Sender<PoolManagerUpdate>,
    pub validator_tx:    UnboundedSender<ValidationRequest>
}

#[derive(Debug)]
pub enum OrderCommand {
    // new orders
    NewOrder(OrderOrigin, AllOrders, tokio::sync::oneshot::Sender<OrderValidationResults>)
}

impl PoolHandle {
    fn send(&self, cmd: OrderCommand) -> Result<(), SendError<OrderCommand>> {
        self.manager_tx.send(cmd)
    }

    async fn send_request<T>(&self, rx: oneshot::Receiver<T>, cmd: OrderCommand) -> T {
        self.send(cmd);
        rx.await.unwrap()
    }

    async fn send_validation_request<T>(
        &self,
        rx: oneshot::Receiver<T>,
        cmd: ValidationRequest
    ) -> T {
        self.validator_tx.send(cmd);
        rx.await.unwrap()
    }
}

impl OrderPoolHandle for PoolHandle {
    fn new_order(
        &self,
        origin: OrderOrigin,
        order: AllOrders
    ) -> impl Future<Output = bool> + Send {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send(OrderCommand::NewOrder(origin, order, tx)).is_ok();
        rx.map(|result| match result {
            Ok(OrderValidationResults::Valid(_)) => true,
            Ok(OrderValidationResults::Invalid(_)) => false,
            Ok(OrderValidationResults::TransitionedToBlock) => false,
            Err(_) => false
        })
    }

    fn subscribe_orders(&self) -> Receiver<PoolManagerUpdate> {
        self.pool_manager_tx.subscribe()
    }

    fn validate_order(
        &self,
        order_origin: OrderOrigin,
        order: AllOrders
    ) -> impl Future<Output = bool> + Send {
        let (tx, rx) = oneshot::channel::<OrderValidationResults>();
        self.send_validation_request(
            rx,
            ValidationRequest::Order(OrderValidationRequest::ValidateOrder(
                tx,
                order,
                order_origin
            ))
        )
        .map(|result| match result {
            OrderValidationResults::Valid(_) => true,
            OrderValidationResults::Invalid(_) => false,
            OrderValidationResults::TransitionedToBlock => false
        })
    }
}

pub struct PoolManagerBuilder<V>
where
    V: OrderValidatorHandle
{
    validator:            V,
    order_storage:        Option<Arc<OrderStorage>>,
    network_handle:       StromNetworkHandle,
    strom_network_events: UnboundedReceiverStream<StromNetworkEvent>,
    eth_network_events:   UnboundedReceiverStream<EthEvent>,
    order_events:         UnboundedMeteredReceiver<NetworkOrderEvent>,
    config:               PoolConfig
}

impl<V> PoolManagerBuilder<V>
where
    V: OrderValidatorHandle<Order = AllOrders> + Unpin
{
    pub fn new(
        validator: V,
        order_storage: Option<Arc<OrderStorage>>,
        network_handle: StromNetworkHandle,
        eth_network_events: UnboundedReceiverStream<EthEvent>,
        order_events: UnboundedMeteredReceiver<NetworkOrderEvent>
    ) -> Self {
        Self {
            order_events,
            eth_network_events,
            strom_network_events: network_handle.subscribe_network_events(),
            network_handle,
            validator,
            order_storage,
            config: Default::default()
        }
    }

    pub fn with_config(mut self, config: PoolConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_storage(mut self, order_storage: Arc<OrderStorage>) -> Self {
        self.order_storage.insert(order_storage);
        self
    }

    pub fn build_with_channels<TP: TaskSpawner>(
        self,
        task_spawner: TP,
        tx: UnboundedSender<OrderCommand>,
        rx: UnboundedReceiver<OrderCommand>,
        validator_tx: UnboundedSender<ValidationRequest>,
        pool_manager_tx: tokio::sync::broadcast::Sender<PoolManagerUpdate>
    ) -> PoolHandle {
        let rx = UnboundedReceiverStream::new(rx);
        let order_storage = self
            .order_storage
            .unwrap_or_else(|| Arc::new(OrderStorage::new(&self.config)));
        let handle = PoolHandle {
            manager_tx:      tx.clone(),
            pool_manager_tx: pool_manager_tx.clone(),
            validator_tx:    validator_tx.clone()
        };
        let inner = OrderIndexer::new(
            self.validator.clone(),
            order_storage.clone(),
            0,
            pool_manager_tx.clone()
        );

        task_spawner.spawn_critical(
            "transaction manager",
            Box::pin(PoolManager {
                eth_network_events: self.eth_network_events,
                strom_network_events: self.strom_network_events,
                order_events: self.order_events,
                peers: HashMap::default(),
                order_indexer: inner,
                network: self.network_handle,
                command_rx: rx,
                pool_manager_tx
            })
        );

        handle
    }

    pub fn build<TP: TaskSpawner>(self, task_spawner: TP) -> PoolHandle {
        let (tx, rx) = unbounded_channel();
        // TODO: Fix me
        let (validator_tx, validator_rx) = unbounded_channel();
        let rx = UnboundedReceiverStream::new(rx);
        let order_storage = self
            .order_storage
            .unwrap_or_else(|| Arc::new(OrderStorage::new(&self.config)));
        let (pool_manager_tx, _) = broadcast::channel(100);
        let handle = PoolHandle {
            manager_tx:      tx.clone(),
            pool_manager_tx: pool_manager_tx.clone(),
            validator_tx:    validator_tx.clone()
        };
        let inner = OrderIndexer::new(
            self.validator.clone(),
            order_storage.clone(),
            0,
            pool_manager_tx.clone()
        );

        task_spawner.spawn_critical(
            "transaction manager",
            Box::pin(PoolManager {
                eth_network_events: self.eth_network_events,
                strom_network_events: self.strom_network_events,
                order_events: self.order_events,
                peers: HashMap::default(),
                order_indexer: inner,
                network: self.network_handle,
                command_rx: rx,
                pool_manager_tx
            })
        );

        handle
    }
}

pub struct PoolManager<V>
where
    V: OrderValidatorHandle
{
    /// access to validation and sorted storage of orders.
    order_indexer:        OrderIndexer<V>,
    /// Network access.
    network:              StromNetworkHandle,
    /// Subscriptions to all the strom-network related events.
    ///
    /// From which we get all new incoming order related messages.
    strom_network_events: UnboundedReceiverStream<StromNetworkEvent>,
    /// Ethereum updates stream that tells the pool manager about orders that
    /// have been filled  
    eth_network_events:   UnboundedReceiverStream<EthEvent>,
    /// receiver half of the commands to the pool manager
    command_rx:           UnboundedReceiverStream<OrderCommand>,
    /// Incoming events from the ProtocolManager.
    order_events:         UnboundedMeteredReceiver<NetworkOrderEvent>,
    /// All the connected peers.
    peers:                HashMap<PeerId, StromPeer>,
    /// Broadcast channel for orders.
    pool_manager_tx:      broadcast::Sender<PoolManagerUpdate>
}

impl<V> PoolManager<V>
where
    V: OrderValidatorHandle<Order = AllOrders>
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        order_indexer: OrderIndexer<V>,
        network: StromNetworkHandle,
        strom_network_events: UnboundedReceiverStream<StromNetworkEvent>,
        eth_network_events: UnboundedReceiverStream<EthEvent>,
        _command_tx: UnboundedSender<OrderCommand>,
        command_rx: UnboundedReceiverStream<OrderCommand>,
        order_events: UnboundedMeteredReceiver<NetworkOrderEvent>,
        pool_manager_tx: tokio::sync::broadcast::Sender<PoolManagerUpdate>
    ) -> Self {
        Self {
            strom_network_events,
            network,
            order_indexer,
            peers: HashMap::new(),
            order_events,
            command_rx,
            eth_network_events,
            pool_manager_tx
        }
    }

    fn on_command(&mut self, cmd: OrderCommand) {
        match cmd {
            OrderCommand::NewOrder(origin, order, validation_response) => self
                .order_indexer
                .new_rpc_order(OrderOrigin::External, order, validation_response)
        }
    }

    fn on_eth_event(&mut self, eth: EthEvent) {
        match eth {
            EthEvent::NewBlockTransitions { block_number, filled_orders, address_changeset } => {
                self.order_indexer.start_new_block_processing(
                    block_number,
                    filled_orders,
                    address_changeset
                );
            }
            EthEvent::ReorgedOrders(orders) => {
                self.order_indexer.reorg(orders);
            }
            EthEvent::FinalizedBlock(block) => {
                self.order_indexer.finalized_block(block);
            }
            EthEvent::NewBlock(block) => {}
        }
    }

    fn on_network_order_event(&mut self, event: NetworkOrderEvent) {
        match event {
            NetworkOrderEvent::IncomingOrders { peer_id, orders } => {
                orders.into_iter().for_each(|order| {
                    self.peers
                        .get_mut(&peer_id)
                        .map(|peer| peer.orders.insert(order.order_hash()));

                    self.order_indexer.new_network_order(
                        peer_id,
                        OrderOrigin::External,
                        order.clone()
                    );
                    // TODO: add an "await" for the new_order() to complete
                    // if !self.order_sorter.is_valid_order(&order) {
                    //     self.network
                    //         .peer_reputation_change(peer_id,
                    // ReputationChangeKind::BadOrder); }
                });
            }
        }
    }

    fn on_network_event(&mut self, event: StromNetworkEvent) {
        match event {
            StromNetworkEvent::SessionEstablished { peer_id } => {
                // insert a new peer into the peerset
                self.peers.insert(
                    peer_id,
                    StromPeer {
                        orders: LruCache::new(NonZeroUsize::new(PEER_ORDER_CACHE_LIMIT).unwrap())
                    }
                );
            }
            StromNetworkEvent::SessionClosed { peer_id, .. } => {
                // remove the peer
                self.peers.remove(&peer_id);
            }
            StromNetworkEvent::PeerRemoved(peer_id) => {
                self.peers.remove(&peer_id);
            }
            StromNetworkEvent::PeerAdded(peer_id) => {
                self.peers.insert(
                    peer_id,
                    StromPeer {
                        orders: LruCache::new(NonZeroUsize::new(PEER_ORDER_CACHE_LIMIT).unwrap())
                    }
                );
            }
        }
    }

    fn on_pool_events(&mut self, orders: Vec<PoolInnerEvent>) {
        let broadcast_orders = orders
            .into_iter()
            .filter_map(|order| match order {
                PoolInnerEvent::Propagation(p) => Some(p),
                PoolInnerEvent::BadOrderMessages(o) => {
                    o.into_iter().for_each(|peer| {
                        self.network.peer_reputation_change(
                            peer,
                            crate::ReputationChangeKind::InvalidOrder
                        );
                    });
                    None
                }
                PoolInnerEvent::None => None
            })
            .collect::<Vec<_>>();

        broadcast_orders.iter().for_each(|order| {
            self.pool_manager_tx
                .send(PoolManagerUpdate::NewOrder(order.clone()));
        });
        // need to update network types for this
        self.network
            .broadcast_tx(StromMessage::PropagatePooledOrders(broadcast_orders));
    }
}

impl<V> Future for PoolManager<V>
where
    V: OrderValidatorHandle<Order = AllOrders> + Unpin
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // pull all eth events
        while let Poll::Ready(Some(eth)) = this.eth_network_events.poll_next_unpin(cx) {
            this.on_eth_event(eth);
        }

        // drain network/peer related events
        while let Poll::Ready(Some(event)) = this.strom_network_events.poll_next_unpin(cx) {
            this.on_network_event(event);
        }

        // drain commands
        while let Poll::Ready(Some(cmd)) = this.command_rx.poll_next_unpin(cx) {
            tracing::debug!(?cmd, "that was a command");
            this.on_command(cmd);
        }

        // drain incoming transaction events
        while let Poll::Ready(Some(event)) = this.order_events.poll_next_unpin(cx) {
            this.on_network_order_event(event);
        }

        // poll underlying pool. This is the validation process that's being polled
        while let Poll::Ready(Some(orders)) = this.order_indexer.poll_next_unpin(cx) {
            this.on_pool_events(orders);
        }

        Poll::Pending
    }
}

/// All events related to orders emitted by the network.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum NetworkTransactionEvent {
    /// Received list of transactions from the given peer.
    ///
    /// This represents transactions that were broadcasted to use from the peer.
    IncomingOrders { peer_id: PeerId, msg: Vec<AllOrders> }
}

/// Tracks a single peer
#[derive(Debug)]
struct StromPeer {
    /// Keeps track of transactions that we know the peer has seen.
    #[allow(dead_code)]
    orders: LruCache<B256>
}
