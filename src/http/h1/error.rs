#[derive(Debug, thiserror::Error)]
pub enum IoProtocolError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("unsupported compression format")]
    UnsupportedCompressionFormat,

    #[error("invalid chunked encoding")]
    InvalidChunkedEncoding,

    #[error("last chunk")]
    LastChunk,
}
