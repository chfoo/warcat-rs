use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use super::format::filename_compression_format;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    #[clap(long, short)]
    pub quiet: bool,

    #[clap(long, default_value = "warn")]
    pub log_level: super::logging::Level,

    #[clap(long)]
    pub log_file: Option<PathBuf>,

    #[clap(long)]
    pub log_json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Export(ExportCommand),
    Import(ImportCommand),
    List(ListCommand),
}

#[derive(Parser, Debug)]
pub struct ExportCommand {
    #[clap(long, default_value = "-")]
    pub input: PathBuf,

    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    #[clap(long, default_value = "json-seq")]
    pub format: SerializationFormat,
}

#[derive(Parser, Debug)]
pub struct ImportCommand {
    #[clap(long, default_value = "-")]
    pub input: PathBuf,

    #[clap(long, default_value = "json-seq")]
    pub format: SerializationFormat,

    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    #[clap(long, default_value = "high")]
    pub compression_level: CompressionLevel,
}

#[derive(Parser, Debug)]
pub struct ListCommand {
    #[clap(long, default_value = "-")]
    pub input: PathBuf,

    #[clap(long, default_value = "auto")]
    pub compression: CompressionFormat,

    #[clap(long, default_value = "-")]
    pub output: PathBuf,

    #[clap(long, default_value = "json-seq")]
    pub format: ListSerializationFormat,

    #[clap(
        long, value_delimiter = ',',
        default_value = ":position,WARC-Record-ID,WARC-Type,Content-Type,WARC-Target-URI"
    )]
    pub field: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompressionFormat {
    Auto,
    None,
    Gzip,
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
            CompressionFormat::Zstandard => Ok(Self::Zstandard),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompressionLevel {
    Balanced,
    High,
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
    JsonSeq,
    CborSeq,
}

impl From<SerializationFormat> for crate::dataseq::SeqFormat {
    fn from(value: SerializationFormat) -> Self {
        match value {
            SerializationFormat::JsonSeq => Self::JsonSeq,
            SerializationFormat::CborSeq => Self::CborSeq,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ListSerializationFormat {
    JsonSeq,
    CborSeq,
    Csv,
}

impl From<ListSerializationFormat> for crate::dataseq::SeqFormat {
    fn from(value: ListSerializationFormat) -> Self {
        match value {
            ListSerializationFormat::JsonSeq => Self::JsonSeq,
            ListSerializationFormat::CborSeq => Self::CborSeq,
            ListSerializationFormat::Csv => Self::Csv,
        }
    }
}
