use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WarcMessage {
    Metadata(Metadata),
    Header(Header),
    BlockChunk(BlockChunk),
    BlockEnd(BlockEnd),
    EndOfFile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Metadata {
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
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockEnd {
    pub crc32: Option<u32>,
    pub crc32c: Option<u32>,
    pub xxh3: Option<u64>,
}
