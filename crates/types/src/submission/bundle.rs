use alloy_rlp_derive::{RlpDecodable, RlpEncodable};
use revm::primitives::TxEnv;
use serde::{Deserialize, Serialize};

use crate::{
    primitive::{Bundle, LowerBound},
    Signature
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionBundle {
    Vanilla(SignedVanillaBundle),
    Composable(ComposableBundle)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedLowerBound {
    pub lower_bound: LowerBound,
    pub signatures:  Vec<Signature>
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposableBundle {
    pub bundle:             Bundle,
    pub signed_lower_bound: SignedLowerBound
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RlpEncodable, RlpDecodable)]
pub struct SignedVanillaBundle {
    pub bundle:     Bundle,
    pub signatures: Signature
}

impl From<Bundle> for TxEnv {
    fn from(_value: Bundle) -> Self {
        todo!()
    }
}
