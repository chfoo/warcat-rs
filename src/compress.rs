//! Reader and writer abstractions for compressed streams.
//!
//! Some codecs support concatenation which is files or segments of files
//! compressed individually and then joined directly together.
//! The purpose is to allow fast seeking within a file to a desired record.

use std::{
    fmt::{Debug, Display},
    io::{BufRead, Read, Write},
    str::FromStr,
};

use decode::{Decoder, PushDecoder};
use encode::Encoder;

mod decode;
mod encode;
pub mod zstd;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Level of compression.
pub enum Level {
    /// Compression level with balance between speed and resource consumption.
    Balanced,
    /// Better compression ratio at expensive of resource consumption.
    High,
    /// Faster compression speed at expense of worse compression ratio.
    Low,
}

impl Default for Level {
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Compression format.
pub enum Format {
    /// No codec. Leave data unchanged.
    Identity,

    /// Zlib file format with Deflate codec.
    Deflate,

    /// Gzip file format and codec.
    ///
    /// Supports concatenation.
    Gzip,

    /// Brotli raw codec.
    Brotli,

    /// Zstandard file format and codec.
    ///
    /// Supports concatenation.
    #[cfg(feature = "zstd")]
    Zstandard,
}

impl Format {
    /// Returns whether the codec supports concatenated members.
    pub fn supports_concatenation(&self) -> bool {
        match self {
            Self::Gzip => true,
            #[cfg(feature = "zstd")]
            Self::Zstandard => true,
            _ => false,
        }
    }
}

impl Default for Format {
    fn default() -> Self {
        Self::Identity
    }
}

impl FromStr for Format {
    type Err = FormatParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "identity" => Ok(Self::Identity),
            "deflate" => Ok(Self::Deflate),
            "gzip" | "x-gzip" | "gz" => Ok(Self::Gzip),
            "br" | "brotli" => Ok(Self::Brotli),
            #[cfg(feature = "zstd")]
            "zstd" | "zstandard" | "zst" => Ok(Self::Zstandard),
            _ => Err(FormatParseError),
        }
    }
}

impl Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity => write!(f, "identity"),
            Self::Deflate => write!(f, "deflate"),
            Self::Gzip => write!(f, "gzip"),
            Self::Brotli => write!(f, "br"),
            #[cfg(feature = "zstd")]
            Self::Zstandard => write!(f, "zstd"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
/// Error for `FromStr` of `Format`.
pub struct FormatParseError;

impl Display for FormatParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid compression format")
    }
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct CompressorConfig {
    pub format: Format,
    pub level: Level,
    pub dictionary: Dictionary,
}

/// Encoder for compressing streams.
#[derive(Debug)]
pub struct Compressor<W: Write> {
    encoder: Encoder<W>,
    config: CompressorConfig,
}

impl<W: Write> Compressor<W> {
    /// Create a compressor for writing compressed data to the given writer.
    pub fn new(dest: W, format: Format) -> Self {
        let config = CompressorConfig {
            format,
            ..Default::default()
        };
        Self::with_config(dest, config)
    }

    /// [Create](Self::new()) a compressor with the given configuration.
    pub fn with_config(dest: W, config: CompressorConfig) -> Self {
        let encoder = Encoder::new(dest, config.format, config.level, &config.dictionary);

        Self { encoder, config }
    }

    /// Return a reference of the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.encoder.get_ref()
    }

    /// Return a mutable reference of the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        self.encoder.get_mut()
    }

    /// Write ending encoder data, consume the compressor, and return the underlying writer.
    pub fn finish(self) -> std::io::Result<W> {
        self.encoder.finish()
    }

    /// Prepares the codec for writing a new stream.
    ///
    /// This function has effect for only codecs that support concatenation.
    /// If configured with a dictionary, it will be reused.
    pub fn start_new_segment(&mut self) -> std::io::Result<()> {
        match self.config.format {
            Format::Gzip => {
                let encoder = std::mem::replace(&mut self.encoder, Encoder::None);
                let dest = encoder.finish()?;
                self.encoder = Encoder::new(
                    dest,
                    self.config.format,
                    self.config.level,
                    &self.config.dictionary,
                );
            }
            #[cfg(feature = "zstd")]
            Format::Zstandard => {
                if let Encoder::Zstandard(encoder) = &mut self.encoder {
                    encoder.start_new_frame()?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl<W: Write> Write for Compressor<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.encoder.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.encoder.flush()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DecompressorConfig {
    pub format: Format,
    pub dictionary: Dictionary,
}

/// Decoder for decompressing streams.
#[derive(Debug)]
pub struct Decompressor<R: BufRead> {
    decoder: Decoder<R>,
    config: DecompressorConfig,
}

impl<R: BufRead> Decompressor<R> {
    /// Create a decompressor for reading compressed data from the given reader.
    pub fn new(source: R, format: Format) -> std::io::Result<Self> {
        let config = DecompressorConfig {
            format,
            ..Default::default()
        };
        Self::with_config(source, config)
    }

    /// [Create](Self::new()) a decompressor with a configuration.
    pub fn with_config(source: R, config: DecompressorConfig) -> std::io::Result<Self> {
        Ok(Self {
            decoder: Decoder::new(source, config.format, &config.dictionary)?,
            config,
        })
    }

    /// Return a reference of the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.decoder.get_ref()
    }

    /// Return a mutable reference of the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        self.decoder.get_mut()
    }

    /// Return the underlying reader.
    pub fn into_inner(self) -> R {
        self.decoder.into_inner()
    }

    /// Prepares the codec for reading a new stream.
    ///
    /// This function has effect for only codecs that support concatenation.
    /// If configured with a dictionary, it will be reused. It should only
    /// be called at the end of decompressing a segment. The end of a segment
    /// is indicated by 0 bytes returned when reading from this struct.
    pub fn start_next_segment(&mut self) -> std::io::Result<()> {
        match self.config.format {
            Format::Gzip => {
                let decoder = std::mem::replace(&mut self.decoder, Decoder::None);
                let source = decoder.into_inner();
                self.decoder = Decoder::new(source, self.config.format, &self.config.dictionary)?;
            }
            #[cfg(feature = "zstd")]
            Format::Zstandard => {
                if let Decoder::Zstandard(decoder) = &mut self.decoder {
                    decoder.start_next_frame()?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Returns if any data is left to be read.
    ///
    /// This function is intended for use with codecs that support
    /// concatenation. If there is nothing read but there is data remaining,
    /// then the next segment can be decompressed.
    pub fn has_data_left(&mut self) -> std::io::Result<bool> {
        let buf = self.decoder.get_mut().fill_buf()?;
        Ok(!buf.is_empty())
    }
}

impl<R: BufRead> Read for Decompressor<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.decoder.read(buf)
    }
}

/// Push-based decoder for decompressing streams.
///
/// This is similar to [`Decompressor`] except that the user is responsible for
/// moving data.
#[derive(Debug)]
pub struct PushDecompressor<W: Write> {
    decoder: PushDecoder<W>,
    config: DecompressorConfig,
}

impl<W: Write> PushDecompressor<W> {
    /// Create a decompressor for writing the decompressed data.
    pub fn new(output: W, format: Format) -> std::io::Result<Self> {
        let config = DecompressorConfig {
            format,
            ..Default::default()
        };
        Self::with_config(output, config)
    }

    /// [Create](Self::new()) a decompressor with the given configuration.
    pub fn with_config(output: W, config: DecompressorConfig) -> std::io::Result<Self> {
        Ok(Self {
            decoder: PushDecoder::new(output, config.format, &config.dictionary)?,
            config,
        })
    }

    /// Return a reference of the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.decoder.get_ref()
    }

    /// Return a mutable reference of the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        self.decoder.get_mut()
    }

    /// Return the underlying writer.
    pub fn into_inner(self) -> std::io::Result<W> {
        self.decoder.into_inner()
    }

    /// Prepares the codec for reading a new stream.
    ///
    /// This function has effect for only codecs that support concatenation.
    /// If configured with a dictionary, it will be reused. It should only
    /// be called at the end of decompressing a segment. The end of a segment
    /// is indicated by 0 bytes written when writing to this struct.
    pub fn start_next_segment(&mut self) -> std::io::Result<()> {
        match self.config.format {
            Format::Gzip => {
                let decoder = std::mem::replace(&mut self.decoder, PushDecoder::None);
                let dest = decoder.into_inner()?;
                self.decoder = PushDecoder::new(dest, self.config.format, &self.config.dictionary)?;
            }
            #[cfg(feature = "zstd")]
            Format::Zstandard => {
                if let PushDecoder::Zstandard(decoder) = &mut self.decoder {
                    decoder.start_next_frame()?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl<W: Write> Write for PushDecompressor<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.decoder.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.decoder.flush()
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Dictionary {
    /// No dictionary.
    None,
    /// Zstandard formatted dictionary supplied as a detached file.
    Zstd(Vec<u8>),
    /// Zstandard formatted dictionary embedded in a skippable frame.
    ///
    /// The dictionary is optionally compressed with Zstandard.
    ///
    /// For decompression, provide an empty Vec for the ".warc.zst"
    /// dictionary.
    WarcZstd(Vec<u8>),
}

impl Dictionary {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_zstd(&self) -> bool {
        matches!(self, Self::Zstd(..))
    }

    pub fn as_zstd(&self) -> Option<&Vec<u8>> {
        if let Self::Zstd(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn is_warc_zstd(&self) -> bool {
        matches!(self, Self::WarcZstd(..))
    }

    pub fn as_warc_zstd(&self) -> Option<&Vec<u8>> {
        if let Self::WarcZstd(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub(crate) fn as_warc_zstd_mut(&mut self) -> Option<&mut Vec<u8>> {
        if let Self::WarcZstd(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::*;

    #[test]
    fn test_compress_decompress() {
        let buf = Vec::new();
        let mut c = Compressor::new(buf, Format::Brotli);

        c.write_all(b"Hello world").unwrap();

        let buf = c.finish().unwrap();
        assert!(!buf.is_empty());

        let mut d = Decompressor::new(BufReader::new(Cursor::new(buf)), Format::Brotli).unwrap();

        let mut buf = Vec::new();
        d.read_to_end(&mut buf).unwrap();
        d.into_inner();

        assert_eq!(&buf, b"Hello world");
    }

    #[test]
    fn test_compress_push_decompress() {
        let buf = Vec::new();
        let mut c = Compressor::new(buf, Format::Brotli);

        c.write_all(b"Hello world").unwrap();

        let buf = c.finish().unwrap();
        assert!(!buf.is_empty());

        let mut d = PushDecompressor::new(Vec::new(), Format::Brotli).unwrap();

        d.write_all(&buf).unwrap();
        d.flush().unwrap();

        let buf = d.into_inner().unwrap();

        assert_eq!(&buf, b"Hello world");
    }

    #[test]
    fn test_compress_decompress_multistream() {
        let buf = Vec::new();
        let mut c = Compressor::new(buf, Format::Gzip);

        c.write_all(b"Hello").unwrap();
        c.start_new_segment().unwrap();
        c.write_all(b"world").unwrap();

        let buf = c.finish().unwrap();

        let mut d = Decompressor::new(BufReader::new(Cursor::new(buf)), Format::Gzip).unwrap();

        let mut buf = Vec::new();

        d.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, b"Hello");
        assert!(d.has_data_left().unwrap());

        buf.clear();
        d.start_next_segment().unwrap();
        d.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, b"world");
        assert!(!d.has_data_left().unwrap());

        d.into_inner();
    }
}
