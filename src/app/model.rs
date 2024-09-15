use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WarcMessage {
    ExportMetadata(ExportMetadata),
    Header(Header),
    BlockChunk(BlockChunk),
    BlockEnd(BlockEnd),
    EndOfFile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExportMetadata {
    pub file: PathBuf,
    pub position: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Header {
    pub version: String,
    pub fields: Vec<(String, String)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockChunk {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockEnd {
    pub crc32c: u32,
}

// #[derive(Debug, Clone, Deserialize, Serialize)]
// pub enum Response {
//     Ok,
//     Error(String),
// }
