use super::request_response::RequestResponse;
use super::wallet::ReceiptWithSignatures;
use ethers::types::{Address, U256};
use libp2p::core::ProtocolName;
use serde::{Deserialize, Serialize};
use std::vec::Vec;

pub const FILE_EXCHANGE_PROTOCOL_ID: &[u8] = b"/dse/file-exchange/0.1";

#[derive(Clone)]
pub struct FileExchangeProtocol;

impl ProtocolName for FileExchangeProtocol {
    fn protocol_name(&self) -> &[u8] {
        FILE_EXCHANGE_PROTOCOL_ID
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum FileExchangeRequest {
    /// Requester wants the file
    ///
    /// TODO: Probably add merkle proof here
    IWant { self_address: Address },
    IWillSeed {
        process_id: u32,
        self_address: Address,
        // chunk_price: U256
    },
    DataChunk {
        process_id: u32,
        sequence_no: usize,
        rfp_sequence_no: usize,
        chunks: Vec<u8>,
    },
    Rfp {
        process_id: u32,
        receipt: ReceiptWithSignatures,
    },
    RfpC {
        process_id: u32,
        receipt: ReceiptWithSignatures,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum FileExchangeResponse {
    /// Acknowledges the request
    Ack,
    /// Bad request
    Bad,
}

pub type FileExchangeCodec =
    RequestResponse<FileExchangeProtocol, FileExchangeRequest, FileExchangeResponse>;
