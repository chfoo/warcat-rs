//! Error representations
use std::{
    backtrace::Backtrace,
    fmt::Display,
    path::{Path, PathBuf},
    str::Utf8Error,
    string::FromUtf8Error,
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GeneralError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl GeneralError {
    pub fn is_parse(&self) -> bool {
        matches!(self, Self::Parse(..))
    }

    pub fn as_parse(&self) -> Option<&ParseError> {
        if let Self::Parse(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_parse(self) -> Result<ParseError, Self> {
        if let Self::Parse(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn is_protocol(&self) -> bool {
        matches!(self, Self::Protocol(..))
    }

    pub fn as_protocol(&self) -> Option<&ProtocolError> {
        if let Self::Protocol(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_protocol(self) -> Result<ProtocolError, Self> {
        if let Self::Protocol(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn is_storage(&self) -> bool {
        matches!(self, Self::Storage(..))
    }

    pub fn as_storage(&self) -> Option<&StorageError> {
        if let Self::Storage(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_storage(self) -> Result<StorageError, Self> {
        if let Self::Storage(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn is_io(&self) -> bool {
        matches!(self, Self::Io(..))
    }

    pub fn as_io(&self) -> Option<&std::io::Error> {
        if let Self::Io(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_io(self) -> Result<std::io::Error, Self> {
        if let Self::Io(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }
}

/// Error for parsing.
#[derive(Debug, thiserror::Error)]
pub struct ParseError {
    kind: ParseErrorKind,
    context: Box<ParseContext>,
    backtrace: Option<Box<Backtrace>>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ParseError {
    pub fn new(kind: ParseErrorKind) -> Self {
        Self {
            kind,
            context: Default::default(),
            backtrace: Some(Box::new(std::backtrace::Backtrace::capture())),
            source: None,
        }
    }

    pub fn other(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::new(ParseErrorKind::Other).with_source(error)
    }

    pub fn with_file<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.context.file = Some(path.into());
        self
    }

    pub fn with_position(mut self, value: u64) -> Self {
        self.context.position = Some(value);
        self
    }

    pub fn with_snippet<S: Into<String>>(mut self, value: S) -> Self {
        self.context.snippet = Some(value.into());
        self
    }

    pub fn with_id<S: Into<String>>(mut self, value: S) -> Self {
        self.context.id = Some(value.into());
        self
    }

    pub fn with_backtrace(mut self, backtrace: Backtrace) -> Self {
        self.backtrace = Some(Box::new(backtrace));
        self
    }

    pub fn with_source<T: Into<Box<dyn std::error::Error + Send + Sync>>>(
        mut self,
        source: T,
    ) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn append_from(&mut self, other: &Self) {
        if let Some(other_position) = other.position() {
            if let Some(position) = &mut self.context.position {
                *position += other_position;
            }
        }
    }

    pub fn file(&self) -> Option<&Path> {
        self.context.file.as_deref()
    }

    pub fn position(&self) -> Option<u64> {
        self.context.position
    }

    pub fn snippet(&self) -> Option<&String> {
        self.context.snippet.as_ref()
    }

    pub fn id(&self) -> Option<&str> {
        self.context.id.as_deref()
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error: {}", self.kind)?;

        if let Some(file) = self.file() {
            write!(f, " file '{:?}'", file)?;
        }

        if let Some(position) = self.position() {
            write!(f, " position {}", position)?;
        }

        if let Some(snippet) = self.snippet() {
            write!(f, " near '{}'", snippet)?;
        }

        if let Some(id) = self.id() {
            write!(f, " ID {}", id)?;
        }

        Ok(())
    }
}

impl<T: std::fmt::Debug> From<nom::Err<nom::error::Error<&[T]>>> for ParseError {
    fn from(value: nom::Err<nom::error::Error<&[T]>>) -> Self {
        match value {
            nom::Err::Incomplete(_needed) => ParseError::new(ParseErrorKind::IncompleteInput),
            nom::Err::Error(error) | nom::Err::Failure(error) => {
                ParseError::new(ParseErrorKind::Syntax)
                    .with_position(error.input.len() as u64)
                    .with_snippet(format!(
                        "{:?}",
                        &error.input[error.input.len().saturating_sub(10)..]
                    ))
                    .with_source(nom::error::Error::new(error.input.len(), error.code))
            }
        }
    }
}

impl From<FromUtf8Error> for ParseError {
    fn from(value: FromUtf8Error) -> Self {
        ParseError::new(ParseErrorKind::InvalidUtf8)
            .with_position(value.utf8_error().valid_up_to() as u64)
    }
}

impl From<Utf8Error> for ParseError {
    fn from(value: Utf8Error) -> Self {
        ParseError::new(ParseErrorKind::InvalidUtf8).with_position(value.valid_up_to() as u64)
    }
}

impl From<chrono::ParseError> for ParseError {
    fn from(value: chrono::ParseError) -> Self {
        ParseError::new(ParseErrorKind::Syntax).with_source(value)
    }
}

impl From<url::ParseError> for ParseError {
    fn from(value: url::ParseError) -> Self {
        ParseError::new(ParseErrorKind::Syntax).with_source(value)
    }
}

impl From<std::net::AddrParseError> for ParseError {
    fn from(value: std::net::AddrParseError) -> Self {
        ParseError::new(ParseErrorKind::Syntax).with_source(value)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ParseErrorKind {
    IncompleteInput,
    Syntax,
    InvalidUtf8,
    InputTooLong,
    Other,
}

impl Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompleteInput => write!(f, "incomplete input"),
            Self::Syntax => write!(f, "syntax error"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8"),
            Self::InputTooLong => write!(f, "input too long"),
            Self::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Default)]
struct ParseContext {
    file: Option<PathBuf>,
    position: Option<u64>,
    snippet: Option<String>,
    id: Option<String>,
}

/// Error for protocols.
#[derive(Debug, thiserror::Error)]
#[error("protocol error: {kind}")]
pub struct ProtocolError {
    kind: ProtocolErrorKind,
    backtrace: Option<Box<Backtrace>>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ProtocolError {
    pub fn new(kind: ProtocolErrorKind) -> Self {
        Self {
            kind,
            backtrace: Some(Box::new(std::backtrace::Backtrace::capture())),
            source: None,
        }
    }

    pub fn other(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::new(ProtocolErrorKind::Other).with_source(error)
    }

    pub fn with_source<T: Into<Box<dyn std::error::Error + Send + Sync>>>(
        mut self,
        source: T,
    ) -> Self {
        self.source = Some(source.into());
        self
    }
}

impl From<ProtocolErrorKind> for ProtocolError {
    fn from(value: ProtocolErrorKind) -> Self {
        Self::new(value)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ProtocolErrorKind {
    HeaderTooBig,
    MissingContentLength,
    ContentLengthMismatch,
    InvalidContentLength,
    InvalidRecordBoundary,
    InvalidMessageBoundary,
    UnsupportedTransferEncoding,
    UnsupportedContentEncoding,
    UnsupportedCompressionFormat,
    InvalidChunkedEncoding,
    UnsupportedDigest,
    InvalidBaseEncodedValue,
    UnsupportedSegmentedRecord,
    AmbiguousSpecification,
    Other,
}

impl Display for ProtocolErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::HeaderTooBig => "header too big",
            Self::MissingContentLength => "missing content length",
            Self::ContentLengthMismatch => "content length mismatch",
            Self::InvalidContentLength => "invalid content length",
            Self::InvalidRecordBoundary => "invalid record boundary",
            Self::InvalidMessageBoundary => "invalid message boundary",
            Self::UnsupportedTransferEncoding => "unsupported transfer encoding",
            Self::UnsupportedContentEncoding => "unsupported content encoding",
            Self::UnsupportedCompressionFormat => "unsupported compression format",
            Self::InvalidChunkedEncoding => "invalid chunked encoding",
            Self::UnsupportedDigest => "unsupported digest",
            Self::InvalidBaseEncodedValue => "invalid base encoded value",
            Self::UnsupportedSegmentedRecord => "unsupported segmented record",
            Self::AmbiguousSpecification => "ambiguous specification",
            Self::Other => "other",
        };

        f.write_str(value)
    }
}

/// Error for internal storage error such as databases.
#[derive(Debug, thiserror::Error)]
pub struct StorageError {
    context: String,
    backtrace: Option<Box<Backtrace>>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl StorageError {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            context: String::new(),
            backtrace: None,
            source: None,
        }
    }

    pub fn with_context<S: AsRef<str>>(mut self, value: S) -> Self {
        self.context = value.as_ref().to_string();
        self
    }

    pub fn with_backtrace(mut self, backtrace: Backtrace) -> Self {
        self.backtrace = Some(Box::new(backtrace));
        self
    }

    pub fn with_source<T: Into<Box<dyn std::error::Error + Send + Sync>>>(
        mut self,
        source: T,
    ) -> Self {
        self.source = Some(source.into());
        self
    }
}

impl Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("storage error")?;

        if !self.context.is_empty() {
            f.write_str(": ")?;
            f.write_str(&self.context)?;
        }

        Ok(())
    }
}

impl From<redb::DatabaseError> for StorageError {
    fn from(value: redb::DatabaseError) -> Self {
        Self::new().with_source(value)
    }
}

impl From<redb::TransactionError> for StorageError {
    fn from(value: redb::TransactionError) -> Self {
        Self::new().with_source(value)
    }
}

impl From<redb::StorageError> for StorageError {
    fn from(value: redb::StorageError) -> Self {
        Self::new().with_source(value)
    }
}

impl From<redb::TableError> for StorageError {
    fn from(value: redb::TableError) -> Self {
        Self::new().with_source(value)
    }
}

impl From<redb::CommitError> for StorageError {
    fn from(value: redb::CommitError) -> Self {
        Self::new().with_source(value)
    }
}
