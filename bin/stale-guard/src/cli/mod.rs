//! CLI definition and entrypoint to executable
use std::path::Path;

use clap::Parser;
use guard_network::{NetworkBuilder, StatusState, StromNetworkHandle, VerificationSidecar};
use guard_rpc::{
    api::{ConsensusApiServer, OrderApiServer, QuotingApiServer},
    ConsensusApi, OrderApi, QuotesApi
};
use reth::{
    args::get_secret_key,
    cli::{
        components::{RethNodeComponents, RethRpcComponents},
        config::{RethNetworkConfig, RethRpcConfig},
        ext::{RethCliExt, RethNodeCommandConfig},
        Cli
    },
    primitives::{Chain, PeerId}
};

/// Convenience function for parsing CLI options, set up logging and run the
/// chosen command.
#[inline]
pub fn run() -> eyre::Result<()> {
    Cli::<StromRethExt>::parse()
        .with_node_extension(StaleGuardConfig::default())
        .run()
}

struct StromRethExt;

impl RethCliExt for StromRethExt {
    type Node = StaleGuardConfig;
}

#[derive(Debug, Clone, Default, clap::Args)]
struct StaleGuardConfig {
    #[clap(long)]
    pub mev_guard: bool,
    #[clap(skip)]
    /// init state
    state:         GuardInitState
}

/// This holds all the handles that are started with the network that our rpc
/// modules will need.
#[derive(Debug, Clone, Default)]
struct GuardInitState {
    network_handle: Option<StromNetworkHandle>
}

impl RethNodeCommandConfig for StaleGuardConfig {
    fn configure_network<Conf, Reth>(
        &mut self,
        config: &mut Conf,
        components: &Reth
    ) -> eyre::Result<()>
    where
        Conf: RethNetworkConfig,
        Reth: RethNodeComponents
    {
        let path = Path::new("");
        let secret_key = get_secret_key(&path).unwrap();

        let state = StatusState {
            version:   0,
            chain:     Chain::mainnet(),
            peer:      PeerId::default(),
            timestamp: 0
        };

        let verification =
            VerificationSidecar { status: state, has_sent: false, has_received: false, secret_key };

        let (pool_tx, pool_rx) =
            reth_metrics::common::mpsc::metered_unbounded_channel("order pool");

        let (consensus_tx, consensus_rx) =
            reth_metrics::common::mpsc::metered_unbounded_channel("consensus");

        let (protocol, network_handle) = NetworkBuilder::new(components.provider(), verification)
            .with_pool_manager(pool_tx)
            .with_consensus_manager(consensus_tx)
            .build(components.task_executor());

        config.add_rlpx_sub_protocol(protocol);

        //config.add_rlpx_sub_protocol();
        Ok(())
    }

    fn extend_rpc_modules<Conf, Reth>(
        &mut self,
        _config: &Conf,
        _components: &Reth,
        rpc_components: RethRpcComponents<'_, Reth>
    ) -> eyre::Result<()>
    where
        Conf: RethRpcConfig,
        Reth: RethNodeComponents
    {
        //TODO: Add the handle to the order pool & consensus module
        let pool = rpc_components.registry.pool();
        let consensus = _components.network();

        let order_api = OrderApi { pool: pool.clone() };
        let quotes_api = QuotesApi { pool: pool.clone() };
        let consensus_api = ConsensusApi { consensus };
        rpc_components
            .modules
            .merge_configured(order_api.into_rpc())?;
        rpc_components
            .modules
            .merge_configured(quotes_api.into_rpc())?;
        rpc_components
            .modules
            .merge_configured(consensus_api.into_rpc())?;

        //_components.task_executor().spawn_critical();

        Ok(())
    }
}
