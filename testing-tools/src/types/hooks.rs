use std::{future::Future, pin::Pin};

use crate::testnet_controllers::AngstromTestnet;

pub enum StateMachineHook<'a, C> {
    Action(StateMachineActionHookFn<'a, C>),
    Check(StateMachineCheckHookFn<C>),
    CheckedAction(StateMachineCheckedActionHookFn<'a, C>)
}

/// execute an action on the testnet
pub type StateMachineActionHookFn<'a, C> = Box<
    dyn FnOnce(
        &'a mut AngstromTestnet<C>
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + 'a>>
>;

/// check something on the testnet
pub type StateMachineCheckHookFn<C> =
    Box<dyn FnOnce(&mut AngstromTestnet<C>) -> eyre::Result<bool>>;

/// execute an action and check something on the testnet
pub type StateMachineCheckedActionHookFn<'a, C> = Box<
    dyn FnOnce(
        &'a mut AngstromTestnet<C>
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + Sync + 'a>>
>;

pub(crate) trait HookResult: Sized {
    fn error(&self) -> Option<&eyre::ErrReport>;

    fn is_pass(&self) -> bool;

    fn fmt_result(self, i: usize, name: &'static str) {
        if let Some(e) = self.error() {
            tracing::error!(target: "testnet::state-machine", hook = i, name, "{:?}", e);
            panic!();
        }

        if self.is_pass() {
            tracing::info!(target: "testnet::state-machine", hook = i, name, "hook PASSED");
        } else {
            tracing::warn!(target: "testnet::state-machine", hook = i, name, "hook FAILED");
        }
    }
}

impl HookResult for eyre::Result<()> {
    fn is_pass(&self) -> bool {
        self.is_ok()
    }

    fn error(&self) -> Option<&eyre::ErrReport> {
        self.as_ref().err()
    }
}

impl HookResult for eyre::Result<bool> {
    fn is_pass(&self) -> bool {
        match self.as_ref() {
            Ok(true) => true,
            _ => false
        }
    }

    fn error(&self) -> Option<&eyre::ErrReport> {
        self.as_ref().err()
    }
}
