use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use crate::verify::Check;

use super::format::filename_compression_format;

/// WARC archive tool
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// Specifies the operation to perform.
    #[command(subcommand)]
    pub command: Command,

    /// Disable any progress messages.
    ///
    /// Does not affect logging.
    #[clap(long, short)]
    pub quiet: bool,

    /// Filter log messages by level.
    #[clap(long, default_value = "off")]
    pub log_level: super::logging::Level,

    /// Write log messages to the given file instead of standard error.
    #[clap(long)]
    pub log_file: Option<PathBuf>,

    /// Write log messages as JSON sequences instead of a console logging format.
    #[clap(long)]
    pub log_json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Export(ExportCommand),
    Import(ImportCommand),
    List(ListCommand),
    Get(GetCommand),
    Extract(ExtractCommand),
    Verify(VerifyCommand),
    Self_(SelfCommand),
    #[command(hide(true))]
    DumpHelp,
}

/// Decodes a WARC file to messages in a easier-to-process format such as JSON.
#[derive(Parser, Debug)]
pub struct ExportCommand {
    /// Path to a WARC file.
    #[clap(long, default_value = "-")]
    pub input: Vec<PathBuf>,

    /// Specify the compression format of the input WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Path for the output messages.
    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    /// Format for the output messages.
    #[clap(long, default_value = "json-seq")]
    pub format: SerializationFormat,

    /// Do not output block messages.
    #[clap(long)]
    pub no_block: bool,

    /// Output extract messages.
    #[clap(long)]
    pub extract: bool,
}

/// Encodes a WARC file from messages in a format of the `export` subcommand.
#[derive(Parser, Debug)]
pub struct ImportCommand {
    /// Path to the input messages.
    #[clap(long, default_value = "-")]
    pub input: Vec<PathBuf>,

    /// Format for the input messages.
    #[clap(long, default_value = "json-seq")]
    pub format: SerializationFormat,

    /// Path of the output WARC file.
    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    /// Compression format of the output WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Level of compression for the output.
    #[clap(long, default_value = "high")]
    pub compression_level: CompressionLevel,
}

/// Provides a listing of the WARC records.
#[derive(Parser, Debug)]
pub struct ListCommand {
    /// Path of the WARC file.
    #[clap(long, default_value = "-")]
    pub input: Vec<PathBuf>,

    /// Compression format of the input WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Path to output listings.
    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    /// Format of the output.
    #[clap(long, default_value = "json-seq")]
    pub format: ListSerializationFormat,

    /// Fields to include in the listing.
    ///
    /// The option accepts names of fields that occur in a WARC header.
    ///
    /// The pseudo-name `:position` represents the position in the file.
    /// `:file` represents the path of the file.
    #[clap(
        long,
        value_delimiter = ',',
        default_value = ":position,WARC-Record-ID,WARC-Type,Content-Type,WARC-Target-URI"
    )]
    pub field: Vec<String>,
}

/// Returns a single WARC record.
#[derive(Parser, Debug)]
pub struct GetCommand {
    #[command(subcommand)]
    pub subcommand: GetSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GetSubcommand {
    Export(GetExportSubcommand),
    Extract(GetExtractSubcommand),
}

/// Output export messages.
#[derive(Parser, Debug)]
pub struct GetExportSubcommand {
    /// Path of the WARC file.
    #[clap(long, default_value = "-")]
    pub input: PathBuf,

    /// Compression format of the input WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Position where the record is located in the input WARC file.
    #[clap(long, required = true)]
    pub position: u64,

    /// The ID of the record to extract.
    #[clap(long, required = true)]
    pub id: String,

    /// Path for the output messages.
    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    /// Format for the output messages.
    #[clap(long, default_value = "json-seq")]
    pub format: SerializationFormat,

    /// Do not output block messages.
    #[clap(long)]
    pub no_block: bool,

    /// Output extract messages.
    #[clap(long)]
    pub extract: bool,
}

/// Extract a resource.
#[derive(Parser, Debug)]
pub struct GetExtractSubcommand {
    // Path of the WARC file.
    #[clap(long, default_value = "-")]
    pub input: PathBuf,

    /// Compression format of the input WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Position where the record is located in the input WARC file.
    #[clap(long, required = true)]
    pub position: u64,

    /// The ID of the record to extract.
    #[clap(long, required = true)]
    pub id: String,

    /// Path for the output file.
    #[clap(long, default_value = "-")]
    pub output: PathBuf,
}

/// Extracts resources for casual viewing of the WARC contents.
///
/// Files are extracted to a directory structure similar to the archived
/// URL.
///
/// This operation does not automatically permit offline viewing of archived
/// websites; no content conversion or link-rewriting is performed.
#[derive(Parser, Debug)]
pub struct ExtractCommand {
    /// Path to the WARC file.
    #[clap(long, default_value = "-")]
    pub input: Vec<PathBuf>,

    /// Compression format of the input WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Path to the output directory.
    #[clap(long, default_value = "./")]
    pub output: PathBuf,

    /// Whether to ignore errors.
    #[clap(long)]
    pub continue_on_error: bool,

    /// Select only records with a field.
    ///
    /// Rule format is "NAME" or "NAME:VALUE".
    #[clap(long)]
    pub include: Vec<String>,

    /// Select only records matching a regular expression.
    ///
    /// Rule format is "NAME:VALUEPATTERN".
    #[clap(long)]
    pub include_pattern: Vec<String>,

    /// Do not select records with a field.
    ///
    /// Rule format is "NAME" or "NAME:VALUE".
    #[clap(long)]
    pub exclude: Vec<String>,

    /// Do not select records matching a regular expression.
    ///
    /// Rule format is "NAME:VALUEPATTERN".
    #[clap(long)]
    pub exclude_pattern: Vec<String>,
}

/// Perform specification and integrity checks on WARC files.
#[derive(Parser, Debug)]
pub struct VerifyCommand {
    /// Path to the WARC file.
    #[clap(long, default_value = "-")]
    pub input: Vec<PathBuf>,

    /// Compression format of the input WARC file.
    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    /// Path to output problems.
    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    /// Format of the output.
    #[clap(long, default_value = "json-seq")]
    pub format: ListSerializationFormat,

    /// Do not perform check.
    #[clap(long, value_delimiter = ',')]
    pub exclude_check: Vec<VerifyCheck>,

    /// Database filename for storing temporary intermediate data.
    pub database: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum VerifyCheck {
    MandatoryFields,
    KnownRecordType,
    ContentType,
    ConcurrentTo,
    BlockDigest,
    PayloadDigest,
    IpAddress,
    RefersTo,
    RefersToTargetUri,
    RefersToDate,
    TargetUri,
    Truncated,
    WarcinfoId,
    Filename,
    Profile,
    // IdentifiedPayloadType,
    Segment,
    RecordAtTimeCompression,
}

impl From<VerifyCheck> for Check {
    fn from(value: VerifyCheck) -> Self {
        match value {
            VerifyCheck::MandatoryFields => Self::MandatoryFields,
            VerifyCheck::KnownRecordType => Self::KnownRecordType,
            VerifyCheck::ContentType => Self::ContentType,
            VerifyCheck::ConcurrentTo => Self::ConcurrentTo,
            VerifyCheck::BlockDigest => Self::BlockDigest,
            VerifyCheck::PayloadDigest => Self::PayloadDigest,
            VerifyCheck::IpAddress => Self::IpAddress,
            VerifyCheck::RefersTo => Self::RefersTo,
            VerifyCheck::RefersToTargetUri => Self::RefersToTargetUri,
            VerifyCheck::RefersToDate => Self::RefersToDate,
            VerifyCheck::TargetUri => Self::TargetUri,
            VerifyCheck::WarcinfoId => Self::WarcinfoId,
            VerifyCheck::Truncated => Self::Truncated,
            VerifyCheck::Filename => Self::Filename,
            VerifyCheck::Profile => Self::Profile,
            // VerifyCheck::IdentifiedPayloadType => Self::IdentifiedPayloadType,
            VerifyCheck::Segment => Self::Segment,
            VerifyCheck::RecordAtTimeCompression => Self::RecordAtTimeCompression,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompressionFormat {
    /// Automatically detect the format by the filename extension.
    Auto,
    /// No compression.
    None,
    /// Gzip format.
    Gzip,
    /// Zstandard format.
    #[cfg(feature = "zstd")]
    Zstandard,
}

impl CompressionFormat {
    pub fn try_into_native(&self, path: &Path) -> anyhow::Result<crate::compress::Format> {
        if *self == Self::Auto {
            Ok(filename_compression_format(path)
                .ok_or_else(|| anyhow::anyhow!("unsupported compression or file format"))?)
        } else {
            Ok((*self)
                .try_into()
                .map_err(|_| anyhow::anyhow!("unsupported compression or file format"))?)
        }
    }
}

impl TryFrom<CompressionFormat> for crate::compress::Format {
    type Error = ();

    fn try_from(value: CompressionFormat) -> Result<Self, Self::Error> {
        match value {
            CompressionFormat::Auto => Err(()),
            CompressionFormat::None => Ok(Self::Identity),
            CompressionFormat::Gzip => Ok(Self::Gzip),
            #[cfg(feature = "zstd")]
            CompressionFormat::Zstandard => Ok(Self::Zstandard),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompressionLevel {
    /// A balance between compression ratio and resource consumption.
    Balanced,
    /// Use a high level of resources to achieve a better compression ratio.
    ///
    /// This is slower and may use more memory.
    ///
    /// For older algorithms, this is usually the highest configuration
    /// possible.
    /// For modern algorithms, this uses a high, but reasonably
    /// practical configuration.
    High,
    /// Fast and low resource usage, but lower compression ratio.
    Low,
}

impl From<CompressionLevel> for crate::compress::Level {
    fn from(value: CompressionLevel) -> Self {
        match value {
            CompressionLevel::Balanced => Self::Balanced,
            CompressionLevel::High => Self::High,
            CompressionLevel::Low => Self::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SerializationFormat {
    /// JSON sequences (RFC 7464)
    ///
    /// Each message is a JSON object delimitated by a Record Separator (U+001E)
    /// and a Line Feed (U+000A).
    JsonSeq,
    /// JSON Lines
    ///
    /// Each message is a JSON object terminated by a Line Feed (U+000A).
    Jsonl,
    /// CBOR sequences (RFC 8742).
    ///
    /// Messages are a series of consecutive CBOR data items.
    CborSeq,
}

impl From<SerializationFormat> for crate::dataseq::SeqFormat {
    fn from(value: SerializationFormat) -> Self {
        match value {
            SerializationFormat::JsonSeq => Self::JsonSeq,
            SerializationFormat::Jsonl => Self::JsonL,
            SerializationFormat::CborSeq => Self::CborSeq,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ListSerializationFormat {
    /// JSON sequences (RFC 7464)
    ///
    /// Each message is a JSON object delimitated by a Record Separator (U+001E)
    /// and a Line Feed (U+000A).
    JsonSeq,
    /// JSON Lines
    ///
    /// Each message is a JSON object terminated by a Line Feed (U+000A).
    Jsonl,
    /// CBOR sequences (RFC 8742).
    ///
    /// Messages are a series of consecutive CBOR data items.
    CborSeq,
    /// Comma separated values.
    Csv,
}

impl From<ListSerializationFormat> for crate::dataseq::SeqFormat {
    fn from(value: ListSerializationFormat) -> Self {
        match value {
            ListSerializationFormat::JsonSeq => Self::JsonSeq,
            ListSerializationFormat::Jsonl => Self::JsonL,
            ListSerializationFormat::CborSeq => Self::CborSeq,
            ListSerializationFormat::Csv => Self::Csv,
        }
    }
}

/// Self-installer and uninstaller.
#[derive(Debug, Parser)]
pub struct SelfCommand {
    #[command(subcommand)]
    pub command: SelfSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SelfSubcommand {
    /// Launch the interactive self-installer.
    Install {
        /// Install automatically without user interaction.
        #[arg(long)]
        quiet: bool,
    },

    /// Launch the interactive uninstaller.
    Uninstall {
        /// Uninstall automatically without user interaction.
        #[arg(long)]
        quiet: bool,
    },
}
