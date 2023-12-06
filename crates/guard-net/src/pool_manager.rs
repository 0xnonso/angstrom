use std::{
    collections::HashMap,
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll}
};

use futures::{future::BoxFuture, stream::FuturesUnordered, Future, StreamExt};
use guard_eth::manager::EthEvent;
use guard_types::{
    orders::{
        GetPooledOrders, OrderConversion, OrderId, OrderLocation, OrderOrigin, OrderPriorityData,
        Orders, PoolOrder, PooledComposableOrder, PooledLimitOrder, PooledOrder,
        PooledSearcherOrder, SearcherPriorityData, ValidatedOrder, ValidationResults
    },
    primitive::PoolId,
    rpc::*
};
use order_pool::{AllOrders, OrderPoolHandle, OrderPoolInner, OrderSet, OrdersToPropagate};
use reth_primitives::{PeerId, TxHash, B256};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    oneshot
};
use tokio_stream::wrappers::{ReceiverStream, UnboundedReceiverStream};
use validation::order::OrderValidator;

use crate::{LruCache, NetworkOrderEvent, RequestResult, StromNetworkEvent, StromNetworkHandle};
/// Cache limit of transactions to keep track of for a single peer.
const PEER_ORDER_CACHE_LIMIT: usize = 1024 * 10;

/// Api to interact with [`PoolManager`] task.
#[derive(Debug, Clone)]
pub struct PoolHandle<L: PoolOrder, CL: PoolOrder, S: PoolOrder, CS: PoolOrder> {
    #[allow(dead_code)]
    /// Command channel to the [`TransactionsManager`]
    manager_tx: UnboundedSender<OrderCommand<L, CL, S, CS>>
}

#[derive(Debug)]
pub enum OrderCommand<L: PoolOrder, CL: PoolOrder, S: PoolOrder, CS: PoolOrder> {
    // new orders
    NewLimitOrder(OrderOrigin, <L as OrderConversion>::Order),
    NewSearcherOrder(OrderOrigin, <S as OrderConversion>::Order),
    NewComposableLimitOrder(OrderOrigin, <CL as OrderConversion>::Order),
    NewComposableSearcherOrder(OrderOrigin, <CS as OrderConversion>::Order),
    // fetch orders
    FetchAllVanillaOrders(
        oneshot::Sender<OrderSet<EcRecoveredLimitOrder, EcRecoveredSearcherOrder>>,
        Option<usize>
    ),
    FetchAllComposableOrders(
        oneshot::Sender<
            OrderSet<EcRecoveredComposableLimitOrder, EcRecoveredComposableSearcherOrder>
        >,
        Option<usize>
    ),
    FetchAllOrders(
        oneshot::Sender<
            AllOrders<
                EcRecoveredLimitOrder,
                EcRecoveredSearcherOrder,
                EcRecoveredComposableLimitOrder,
                EcRecoveredComposableSearcherOrder
            >
        >,
        Option<usize>
    )
}

impl<L: PoolOrder, CL: PoolOrder, S: PoolOrder, CS: PoolOrder> PoolHandle<L, CL, S, CS> {
    fn send(&self, cmd: OrderCommand<L, CL, S, CS>) {
        let _ = self.manager_tx.send(cmd);
    }

    async fn send_request<T>(
        &self,
        rx: oneshot::Receiver<T>,
        cmd: OrderCommand<L, CL, S, CS>
    ) -> T {
        self.send(cmd);
        rx.await.unwrap()
    }
}

impl<L: PoolOrder, CL: PoolOrder, S: PoolOrder, CS: PoolOrder> OrderPoolHandle
    for PoolHandle<L, CL, S, CS>
{
    type ComposableLimitOrder = CL;
    type ComposableSearcherOrder = CS;
    type LimitOrder = L;
    type SearcherOrder = S;

    fn new_limit_order(
        &self,
        origin: OrderOrigin,
        order: <Self::LimitOrder as OrderConversion>::Order
    ) {
        self.send(OrderCommand::NewLimitOrder(origin, order));
    }

    fn new_searcher_order(
        &self,
        origin: OrderOrigin,
        order: <Self::SearcherOrder as OrderConversion>::Order
    ) {
        self.send(OrderCommand::NewSearcherOrder(origin, order))
    }

    fn new_composable_limit_order(
        &self,
        origin: OrderOrigin,
        order: <Self::ComposableLimitOrder as OrderConversion>::Order
    ) {
        self.send(OrderCommand::NewComposableLimitOrder(origin, order))
    }

    fn new_composable_searcher_order(
        &self,
        origin: OrderOrigin,
        order: <Self::ComposableSearcherOrder as OrderConversion>::Order
    ) {
        self.send(OrderCommand::NewComposableSearcherOrder(origin, order))
    }

    fn get_all_vanilla_orders(&self) -> BoxFuture<OrderSet<Self::LimitOrder, Self::SearcherOrder>> {
        todo!()
    }

    // fn get_all_vanilla_orders_intersection(
    //     &self,
    //     buffer: usize
    // ) -> BoxFuture<
    //     OrderSet<
    //         <Self::LimitOrder as OrderConversion>::Order,
    //         <Self::SearcherOrder as OrderConversion>::Order
    //     >
    // > { Box::pin(async move { let (tx, rx) = oneshot::channel();
    // > self.send_request(rx, OrderCommand::FetchAllVanillaOrders(tx,
    // > Some(buffer))) .await })
    // }
    //
    // fn get_all_composable_orders(
    //     &self
    // ) -> BoxFuture<
    //     OrderSet<
    //         <Self::ComposableLimitOrder as OrderConversion>::Order,
    //         <Self::ComposableSearcherOrder as OrderConversion>::Order
    //     >
    // > { Box::pin(async move { let (tx, rx) = oneshot::channel();
    // > self.send_request(rx, OrderCommand::FetchAllComposableOrders(tx, None))
    // > .await })
    // }
    //
    // fn get_all_composable_orders_intersection(
    //     &self,
    //     buffer: usize
    // ) -> BoxFuture<
    //     OrderSet<
    //         <Self::ComposableLimitOrder as OrderConversion>::Order,
    //         <Self::ComposableSearcherOrder as OrderConversion>::Order
    //     >
    // > { Box::pin(async move { let (tx, rx) = oneshot::channel();
    // > self.send_request(rx, OrderCommand::FetchAllComposableOrders(tx,
    // > Some(buffer))) .await })
    // }
    //
    // fn get_all_orders(
    //     &self
    // ) -> BoxFuture<
    //     AllOrders<
    //         <Self::LimitOrder as OrderConversion>::Order,
    //         <Self::SearcherOrder as OrderConversion>::Order,
    //         <Self::ComposableLimitOrder as OrderConversion>::Order,
    //         <Self::ComposableSearcherOrder as OrderConversion>::Order
    //     >
    // > { Box::pin(async move { let (tx, rx) = oneshot::channel();
    // > self.send_request(rx, OrderCommand::FetchAllOrders(tx, None)) .await })
    // }
    //
    // fn get_all_orders_intersection(
    //     &self,
    //     buffer: usize
    // ) -> BoxFuture<
    //     AllOrders<
    //         <Self::LimitOrder as OrderConversion>::Order,
    //         <Self::SearcherOrder as OrderConversion>::Order,
    //         <Self::ComposableLimitOrder as OrderConversion>::Order,
    //         <Self::ComposableSearcherOrder as OrderConversion>::Order
    //     >
    // > { Box::pin(async move { let (tx, rx) = oneshot::channel();
    // > self.send_request(rx, OrderCommand::FetchAllOrders(tx, Some(buffer)))
    // > .await })
    // }
}

//TODO: Tmrw clean up + finish pool manager + pool inner
//TODO: Add metrics + events
pub struct PoolManager<L, CL, S, CS, V>
where
    L: PooledLimitOrder,
    CL: PooledComposableOrder + PooledLimitOrder,
    S: PooledSearcherOrder,
    CS: PooledComposableOrder + PooledSearcherOrder,
    V: OrderValidator
{
    /// The order pool. Streams up new transactions to be broadcasted
    pool:                 OrderPoolInner<L, CL, S, CS, V>,
    /// Network access.
    _network:             StromNetworkHandle,
    /// Subscriptions to all the strom-network related events.
    ///
    /// From which we get all new incoming order related messages.
    strom_network_events: UnboundedReceiverStream<StromNetworkEvent>,
    /// Ethereum updates stream that tells the pool manager about orders that
    /// have been filled  
    eth_network_events:   UnboundedReceiverStream<EthEvent>,
    /// Send half for the command channel. Used to generate new handles
    command_tx:           UnboundedSender<OrderCommand<L, CL, S, CS>>,
    /// receiver half of the commands to the pool manager
    command_rx:           UnboundedReceiverStream<OrderCommand<L, CL, S, CS>>,
    /// Order fetcher to handle inflight and missing order requests.
    _order_fetcher:       OrderFetcher,
    /// All currently pending orders grouped by peers.
    _orders_by_peers:     HashMap<TxHash, Vec<PeerId>>,
    /// Incoming events from the ProtocolManager.
    order_events:         UnboundedReceiverStream<NetworkOrderEvent>,
    /// All the connected peers.
    peers:                HashMap<PeerId, StromPeer>
}

impl<L, CL, S, CS, V> PoolManager<L, CL, S, CS, V>
where
    L: PooledLimitOrder<ValidationData = OrderPriorityData>,
    CL: PooledComposableOrder + PooledLimitOrder<ValidationData = OrderPriorityData>,
    S: PooledSearcherOrder<ValidationData = SearcherPriorityData>,
    CS: PooledComposableOrder + PooledSearcherOrder<ValidationData = SearcherPriorityData>,
    V: OrderValidator
{
    pub fn new(
        _pool: OrderPoolInner<L, CL, S, CS, V>,
        _network: StromNetworkHandle,
        _from_network: UnboundedReceiver<NetworkOrderEvent>
    ) {
        todo!()
    }
}

impl<L, CL, S, CS, V> PoolManager<L, CL, S, CS, V>
where
    L: PooledLimitOrder<ValidationData = OrderPriorityData, Order = SignedLimitOrder>,
    CL: PooledComposableOrder
        + PooledLimitOrder<ValidationData = OrderPriorityData, Order = SignedComposableLimitOrder>,
    S: PooledSearcherOrder<ValidationData = SearcherPriorityData, Order = SignedSearcherOrder>,
    CS: PooledComposableOrder
        + PooledSearcherOrder<
            ValidationData = SearcherPriorityData,
            Order = SignedComposableSearcherOrder
        >,
    V: OrderValidator<
        LimitOrder = L,
        SearcherOrder = S,
        ComposableLimitOrder = CL,
        ComposableSearcherOrder = CS
    >
{
    /// Returns a new handle that can send commands to this type.
    pub fn handle(&self) -> PoolHandle<L, CL, S, CS> {
        PoolHandle { manager_tx: self.command_tx.clone() }
    }

    fn on_command(&mut self, cmd: OrderCommand<L, CL, S, CS>) {
        match cmd {
            // new orders
            OrderCommand::NewLimitOrder(origin, order) => {
                if let Ok(order) = <L as OrderConversion>::try_from_order(order) {
                    self.pool.new_limit_order(origin, order);
                }
            }
            OrderCommand::NewSearcherOrder(origin, order) => {}
            OrderCommand::NewComposableLimitOrder(origin, order) => {}
            OrderCommand::NewComposableSearcherOrder(origin, order) => {}
            // fetch requests
            OrderCommand::FetchAllOrders(sender, is_intersection) => {}
            OrderCommand::FetchAllComposableOrders(sender, is_intersection) => {}
            OrderCommand::FetchAllVanillaOrders(sender, is_intersection) => {}
        }
    }

    fn on_eth_event(&mut self, eth: EthEvent) {
        match eth {
            EthEvent::FilledOrders(orders) => {
                let _orders = self.pool.filled_orders(&orders);
                todo!()
            }
            EthEvent::ReorgedOrders(_) => {
                todo!("add pending validation pool");
            }
            EthEvent::EOAStateChanges(state_changes) => {
                self.pool.eoa_state_change(state_changes);
            }
        }
    }

    //TODO
    fn on_network_order_event(&mut self, event: NetworkOrderEvent) {
        match event {
            NetworkOrderEvent::IncomingOrders { peer_id, orders } => {}
        }
    }

    fn on_network_event(&mut self, event: StromNetworkEvent) {
        match event {
            StromNetworkEvent::SessionEstablished { peer_id, client_version } => {
                // insert a new peer into the peerset
                self.peers.insert(
                    peer_id,
                    StromPeer {
                        orders: LruCache::new(NonZeroUsize::new(PEER_ORDER_CACHE_LIMIT).unwrap()),
                        //request_tx: messages,
                        client_version
                    }
                );
            }
            StromNetworkEvent::SessionClosed { peer_id, .. } => {
                // remove the peer
                self.peers.remove(&peer_id);
            }

            _ => {}
        }
    }

    fn on_propagate_orders(&mut self, orders: OrdersToPropagate<L, CL, S, CS>) {
        let order = match orders {
            OrdersToPropagate::Limit(limit) => PooledOrder::Limit(limit.to_signed()),
            OrdersToPropagate::Searcher(searcher) => PooledOrder::Searcher(searcher.to_signed()),
            OrdersToPropagate::LimitComposable(limit) => {
                PooledOrder::ComposableLimit(limit.to_signed())
            }
            OrdersToPropagate::SearcherCompsable(searcher) => {
                PooledOrder::ComposableSearcher(searcher.to_signed())
            }
        };

        self.peers
            .values_mut()
            .for_each(|peer| peer.propagate_order(vec![order.clone()]))
    }
}

impl<L, CL, S, CS, V> Future for PoolManager<L, CL, S, CS, V>
where
    L: PooledLimitOrder<ValidationData = OrderPriorityData, Order = SignedLimitOrder>,
    CL: PooledComposableOrder
        + PooledLimitOrder<ValidationData = OrderPriorityData, Order = SignedComposableLimitOrder>,
    S: PooledSearcherOrder<ValidationData = SearcherPriorityData, Order = SignedSearcherOrder>,
    CS: PooledComposableOrder
        + PooledSearcherOrder<
            ValidationData = SearcherPriorityData,
            Order = SignedComposableSearcherOrder
        >,
    V: OrderValidator<
            LimitOrder = L,
            SearcherOrder = S,
            ComposableLimitOrder = CL,
            ComposableSearcherOrder = CS
        > + Unpin
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
            this.on_command(cmd);
        }

        // drain incoming transaction events
        while let Poll::Ready(Some(event)) = this.order_events.poll_next_unpin(cx) {
            this.on_network_order_event(event);
        }

        //
        while let Poll::Ready(Some(orders)) = this.pool.poll_next_unpin(cx) {
            this.on_propagate_orders(orders);
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
    IncomingOrders { peer_id: PeerId, msg: Orders },
    /// Incoming `GetPooledOrders` request from a peer.
    GetPooledOrders {
        peer_id:  PeerId,
        request:  GetPooledOrders,
        response: oneshot::Sender<RequestResult<Orders>>
    }
}

/// Tracks a single peer
#[derive(Debug)]
struct StromPeer {
    /// Keeps track of transactions that we know the peer has seen.
    #[allow(dead_code)]
    orders:         LruCache<B256>,
    /// A communication channel directly to the peer's session task.
    //request_tx:     PeerRequestSender,
    /// negotiated version of the session.
    /// The peer's client version.
    #[allow(unused)]
    client_version: Arc<str>
}

impl StromPeer {
    pub fn propagate_order(&mut self, orders: Vec<PooledOrder>) {
        todo!()
    }
}

/// The type responsible for fetching missing orders from peers.
///
/// This will keep track of unique transaction hashes that are currently being
/// fetched and submits new requests on announced hashes.
#[derive(Debug, Default)]
struct OrderFetcher {
    /// All currently active requests for pooled transactions.
    _inflight_requests:               FuturesUnordered<GetPooledOrders>,
    /// Set that tracks all hashes that are currently being fetched.
    _inflight_hash_to_fallback_peers: HashMap<TxHash, Vec<PeerId>>
}
