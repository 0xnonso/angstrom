use bytes::BytesMut;
use reth_codecs::{main_codec, Compact};
use reth_primitives::H512;
use reth_rlp::{Decodable, DecodeError, Encodable, RlpDecodable, RlpEncodable};
use secp256k1::PublicKey;
use serde::{Deserialize, Serialize};

use super::header::BlockId;
use crate::{consensus::Time, on_chain::Signature};

#[main_codec]
#[derive(Debug, Clone, RlpDecodable, RlpEncodable, PartialEq, Eq, Hash)]
pub struct BlockCommit {
    pub height:     u64,
    pub round:      u64,
    pub block_id:   BlockId,
    pub signatures: Vec<BlockCommitSignature>
}

#[main_codec]
#[derive(Debug, Clone, RlpDecodable, RlpEncodable, PartialEq, Eq, Hash)]
pub struct BlockCommitSignature {
    pub leader_address: H512,
    pub timestamp:      Time,
    pub signature:      Signature
}
