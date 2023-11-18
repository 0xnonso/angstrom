use std::{
    collections::HashMap,
    ops::{Add, Deref, DerefMut}
};

use alloy_primitives::{Address, TxHash, U256};
use alloy_rlp::{Decodable, Encodable, Error};
use alloy_rlp_derive::{RlpDecodable, RlpEncodable};
use alloy_sol_types::SolStruct;
use derive_more::{AsRef, Deref};
use reth_primitives::{recover_signer, Signature as ESignature};
use secp256k1::Error as SigError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::primitive::{ComposableOrder, Order, Signature, ANGSTROM_DOMAIN};

/// Submitted order pre-processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, RlpEncodable, RlpDecodable)]
pub struct SignedLimitOrder {
    /// Order hash
    pub hash:      TxHash,
    /// The original order from the user.
    pub order:     Order,
    /// The user's EIP-712 signature of the Order.
    pub signature: Signature
}

impl SignedLimitOrder {
    pub fn recover_signer(&self) -> Option<Address> {
        let hash = self.order.eip712_signing_hash(&ANGSTROM_DOMAIN);
        self.signature.0.recover_signer(hash)
    }
}

impl TryInto<EcRecoveredLimitOrder> for SignedLimitOrder {
    type Error = SigError;

    fn try_into(self) -> Result<EcRecoveredLimitOrder, Self::Error> {
        let sig = self
            .recover_signer()
            .ok_or_else(|| SigError::IncorrectSignature)?;

        Ok(EcRecoveredLimitOrder { signer: sig, signed_order: self })
    }
}

/// Signed transaction with recovered signer.
#[derive(Debug, Clone, PartialEq, Hash, Eq, AsRef, Deref)]
pub struct EcRecoveredLimitOrder {
    /// Signer of the transaction
    pub signer:       Address,
    /// Signed transaction
    #[deref]
    #[as_ref]
    pub signed_order: SignedLimitOrder
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, RlpEncodable, RlpDecodable)]
pub struct SignedComposableLimitOrder {
    /// Order hash
    pub hash:      TxHash,
    /// The original order from the user.
    pub order:     ComposableOrder,
    /// The user's EIP-712 signature of the Order.
    pub signature: Signature
}

impl SignedComposableLimitOrder {
    pub fn recover_signer(&self) -> Option<Address> {
        let hash = self.order.eip712_signing_hash(&ANGSTROM_DOMAIN);
        self.signature.0.recover_signer(hash)
    }
}

impl TryInto<EcRecoveredComposableLimitOrder> for SignedComposableLimitOrder {
    type Error = SigError;

    fn try_into(self) -> Result<EcRecoveredComposableLimitOrder, Self::Error> {
        let sig = self
            .recover_signer()
            .ok_or_else(|| SigError::IncorrectSignature)?;

        Ok(EcRecoveredComposableLimitOrder { signer: sig, signed_order: self })
    }
}

/// Signed transaction with recovered signer.
#[derive(Debug, Clone, PartialEq, Hash, Eq, AsRef, Deref)]
pub struct EcRecoveredComposableLimitOrder {
    /// Signer of the transaction
    pub signer:       Address,
    /// Signed transaction
    #[deref]
    #[as_ref]
    pub signed_order: SignedComposableLimitOrder
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CallerInfo {
    pub address:   Address,
    pub nonce:     u64,
    pub overrides: HashMap<Address, HashMap<U256, U256>>
}
