use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WarcMessage {
    Metadata(Metadata),
    Header(Header),
    BlockChunk(BlockChunk),
    BlockEnd(BlockEnd),
    ExtractMetadata(ExtractMetadata),
    ExtractChunk(ExtractChunk),
    ExtractEnd(ExtractEnd),
    EndOfFile(EndOfFile),
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractMetadata {
    pub has_content: bool,
    pub file_path_components: Vec<String>,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractChunk {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractEnd {
    pub crc32: Option<u32>,
    pub crc32c: Option<u32>,
    pub xxh3: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EndOfFile {}
