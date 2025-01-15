use std::{str::FromStr, sync::Arc, time::Duration};

use alloy::{
    network::EthereumWallet,
    node_bindings::AnvilInstance,
    primitives::{Address, U256},
    providers::{ext::AnvilApi, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner
};
use futures::Future;
use tracing::debug;

use super::anvil::WalletProviderRpc;
use crate::contracts::anvil::{spawn_anvil, LocalAnvilRpc};

pub mod angstrom;
pub mod mockreward;
pub mod uniswap;

pub const CONTROLLER_V1_ADDRESS: alloy_primitives::Address =
    alloy_primitives::address!("20Cc09ac7E8d13Dd39177786f4f4e9a802fe69a3");

pub const ANGSTROM_ADDRESS: alloy_primitives::Address =
    alloy_primitives::address!("9a59a6d48aae9B192ac58871e112D9e441f86A80");

pub const ANGSTROM_ADDRESS_SALT: u64 = 11412;

pub const POOL_GATE_ADDRESS: alloy_primitives::Address =
    alloy_primitives::address!("cD7AB7cFd92481C6AfF1E79F90A3Ac6056bd7A6e");

pub const POOL_MANAGER_ADDRESS: alloy_primitives::Address =
    alloy_primitives::address!("691841C2B3b60c309ad7D97813bE591412b87167");

#[allow(async_fn_in_trait)]
pub trait TestAnvilEnvironment: Clone {
    type P: alloy::providers::Provider + alloy::providers::WalletProvider;

    fn provider(&self) -> &Self::P;
    fn controller(&self) -> Address;

    async fn execute_then_mine<O>(&self, f: impl Future<Output = O> + Send) -> O {
        let mut fut = Box::pin(f);
        // poll for 500 ms. if  not resolves then we mine and join
        tokio::select! {
            o = &mut fut => {
                return o
            },
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
            }
        };

        let mine_one_fut = self.provider().anvil_mine(Some(U256::from(1)), None);
        let (res, _) = futures::join!(fut, mine_one_fut);
        res
    }

    async fn override_address(
        &self,
        from_addr: &mut Address,
        to_addr: Address
    ) -> eyre::Result<()> {
        let provider = self.provider();

        let code = provider.get_code_at(*from_addr).await?;
        provider.anvil_set_code(to_addr, code).await?;

        *from_addr = to_addr;

        //provider.anvil_mine(Some(U256::from(1)), None).await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct SpawnedAnvil {
    #[allow(dead_code)]
    anvil:      Arc<AnvilInstance>,
    provider:   WalletProviderRpc,
    controller: Address
}

impl SpawnedAnvil {
    pub async fn new() -> eyre::Result<Self> {
        debug!("Spawning Anvil...");
        let (anvil, provider) = spawn_anvil(7).await?;
        let controller = anvil.addresses()[7];
        debug!("Anvil spawned");
        Ok(Self { anvil: anvil.into(), provider, controller })
    }
}

impl TestAnvilEnvironment for SpawnedAnvil {
    type P = WalletProviderRpc;

    fn provider(&self) -> &Self::P {
        &self.provider
    }

    fn controller(&self) -> Address {
        self.controller
    }
}

#[derive(Clone)]
pub struct LocalAnvil {
    _url:     String,
    provider: LocalAnvilRpc
}

impl LocalAnvil {
    pub async fn new(url: String) -> eyre::Result<Self> {
        let sk: PrivateKeySigner = PrivateKeySigner::from_str(
            "4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356"
        )
        .unwrap();
        let wallet = EthereumWallet::new(sk);
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_builtin(&url)
            .await
            .unwrap();

        Ok(Self { _url: url, provider })
    }
}

impl TestAnvilEnvironment for LocalAnvil {
    type P = LocalAnvilRpc;

    fn provider(&self) -> &Self::P {
        &self.provider
    }

    fn controller(&self) -> Address {
        Address::from_str("14dC79964da2C08b23698B3D3cc7Ca32193d9955").unwrap()
    }
}
