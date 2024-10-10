//! Reader and writer abstractions for compressed streams.
//!
//! Some codecs support multistreams which are compressed files concatenated
//! together. The purpose is to allow fast seeking within a file to a desired
//! record.
//!
//! Zstandard only has minimal support.

// FIXME: For zstd, stopping properly at a frame does not work:
// https://github.com/gyscos/zstd-rs/issues/15
// FIXME: Dictionary in zstd requires skippable frames but the library does
// not support it
use std::{
    fmt::{Debug, Display},
    io::{BufRead, Read, Write},
    str::FromStr,
};

use brotli::{
    writer::DecompressorWriter as BrPushDecoder, CompressorWriter as BrEncoder,
    Decompressor as BrDecoder,
};
use flate2::{
    bufread::{GzDecoder, ZlibDecoder},
    write::{GzDecoder as GzPushDecoder, GzEncoder, ZlibDecoder as ZlibPushDecoder, ZlibEncoder},
};

#[cfg(feature = "zstd")]
use zstd::stream::{
    read::Decoder as ZstdDecoder, write::Decoder as ZstdPushDecoder, write::Encoder as ZstdEncoder,
};

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
    /// Supports multiple streams.
    Gzip,

    /// Brotli raw codec.
    Brotli,

    /// Zstandard file format and codec.
    ///
    /// Supports multiple streams.
    #[cfg(feature = "zstd")]
    Zstandard,
}

impl Format {
    /// Returns whether the codec supports concatenated members.
    pub fn is_multistream(&self) -> bool {
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

enum Encoder<W: Write> {
    Identity(W),
    Deflate(ZlibEncoder<W>),
    Gzip(GzEncoder<W>),
    Brotli(Box<BrEncoder<W>>),
    #[cfg(feature = "zstd")]
    Zstandard(ZstdEncoder<'static, W>),
    None,
}

impl<W: Write> Encoder<W> {
    fn get_ref(&self) -> &W {
        match self {
            Self::Identity(w) => w,
            Self::Deflate(codec) => codec.get_ref(),
            Self::Gzip(codec) => codec.get_ref(),
            Self::Brotli(codec) => codec.get_ref(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.get_ref(),
            Self::None => unreachable!(),
        }
    }

    fn get_mut(&mut self) -> &mut W {
        match self {
            Self::Identity(w) => w,
            Self::Deflate(codec) => codec.get_mut(),
            Self::Gzip(codec) => codec.get_mut(),
            Self::Brotli(codec) => codec.get_mut(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.get_mut(),
            Self::None => unreachable!(),
        }
    }

    fn finish(self) -> std::io::Result<W> {
        match self {
            Self::Identity(w) => Ok(w),
            Self::Deflate(codec) => codec.finish(),
            Self::Gzip(codec) => codec.finish(),
            Self::Brotli(codec) => Ok(codec.into_inner()),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.finish(),
            Self::None => unreachable!(),
        }
    }
}

impl<W: Write> Write for Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Identity(w) => w.write(buf),
            Self::Deflate(w) => w.write(buf),
            Self::Gzip(w) => w.write(buf),
            Self::Brotli(w) => w.write(buf),
            #[cfg(feature = "zstd")]
            Self::Zstandard(w) => w.write(buf),
            Self::None => unreachable!(),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Identity(w) => w.flush(),
            Self::Deflate(w) => w.flush(),
            Self::Gzip(w) => w.flush(),
            Self::Brotli(w) => w.flush(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(w) => w.flush(),
            Self::None => unreachable!(),
        }
    }
}

impl<W: Write> Debug for Encoder<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(_arg0) => f.debug_tuple("Identity").finish(),
            Self::Deflate(_arg0) => f.debug_tuple("Deflate").finish(),
            Self::Gzip(_arg0) => f.debug_tuple("Gzip").finish(),
            Self::Brotli(_arg0) => f.debug_tuple("Brotli").finish(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(_arg0) => f.debug_tuple("Zstandard").finish(),
            Self::None => write!(f, "None"),
        }
    }
}

/// Encoder for compressing streams.
#[derive(Debug)]
pub struct Compressor<W: Write> {
    encoder: Encoder<W>,
    format: Format,
    level: Level,
}

impl<W: Write> Compressor<W> {
    /// Create a compressor for writing compressed data to the given writer.
    pub fn new(dest: W, format: Format) -> Self {
        let level = Level::default();
        let encoder = create_encoder(dest, format, level);

        Self {
            encoder,
            format,
            level,
        }
    }

    /// Create a compressor like [`Self::new()`] with a compression level.
    pub fn with_level(dest: W, format: Format, level: Level) -> Self {
        let encoder = create_encoder(dest, format, level);

        Self {
            encoder,
            format,
            level,
        }
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
    /// This function has effect for only multistream codecs.
    pub fn restart_stream(&mut self) -> std::io::Result<()> {
        if self.format.is_multistream() {
            let encoder = std::mem::replace(&mut self.encoder, Encoder::None);
            let dest = encoder.finish()?;
            self.encoder = create_encoder(dest, self.format, self.level);
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

enum Decoder<R: BufRead> {
    Identity(R),
    Deflate(ZlibDecoder<R>),
    Gzip(GzDecoder<R>),
    Brotli(Box<BrDecoder<R>>),
    #[cfg(feature = "zstd")]
    Zstandard(ZstdDecoder<'static, R>),
    None,
}

impl<R: BufRead> Debug for Decoder<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(_arg0) => f.debug_tuple("Identity").finish(),
            Self::Deflate(_arg0) => f.debug_tuple("Deflate").finish(),
            Self::Gzip(_arg0) => f.debug_tuple("Gzip").finish(),
            Self::Brotli(_arg0) => f.debug_tuple("Brotli").finish(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(_arg0) => f.debug_tuple("Zstandard").finish(),
            Self::None => write!(f, "None"),
        }
    }
}

impl<R: BufRead> Decoder<R> {
    fn get_ref(&self) -> &R {
        match self {
            Self::Identity(r) => r,
            Self::Deflate(codec) => codec.get_ref(),
            Self::Gzip(codec) => codec.get_ref(),
            Self::Brotli(codec) => codec.get_ref(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.get_ref(),
            Self::None => unreachable!(),
        }
    }

    fn get_mut(&mut self) -> &mut R {
        match self {
            Self::Identity(r) => r,
            Self::Deflate(codec) => codec.get_mut(),
            Self::Gzip(codec) => codec.get_mut(),
            Self::Brotli(codec) => codec.get_mut(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.get_mut(),
            Self::None => unreachable!(),
        }
    }

    fn into_inner(self) -> R {
        match self {
            Self::Identity(r) => r,
            Self::Deflate(codec) => codec.into_inner(),
            Self::Gzip(codec) => codec.into_inner(),
            Self::Brotli(codec) => codec.into_inner(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.finish(),
            Self::None => unreachable!(),
        }
    }
}

impl<R: BufRead> Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Decoder::Identity(r) => r.read(buf),
            Decoder::Deflate(codec) => codec.read(buf),
            Decoder::Gzip(codec) => codec.read(buf),
            Decoder::Brotli(codec) => codec.read(buf),
            #[cfg(feature = "zstd")]
            Decoder::Zstandard(codec) => codec.read(buf),
            Decoder::None => unreachable!(),
        }
    }
}

/// Decoder for decompressing streams.
#[derive(Debug)]
pub struct Decompressor<R: BufRead> {
    decoder: Decoder<R>,
    format: Format,
}

impl<R: BufRead> Decompressor<R> {
    /// Create a decompressor.for reading compressed data from the given reader.
    pub fn new(source: R, format: Format) -> std::io::Result<Self> {
        Ok(Self {
            decoder: create_decoder(source, format)?,
            format,
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
    /// This function is only to be used for multistream codecs.
    pub fn restart_stream(&mut self) -> std::io::Result<()> {
        let decoder = std::mem::replace(&mut self.decoder, Decoder::None);
        let source = decoder.into_inner();
        self.decoder = create_decoder(source, self.format)?;

        Ok(())
    }

    /// Returns if any data is left to be read.
    ///
    /// This function is intended for use with multistream codecs.
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

enum PushDecoder<W: Write> {
    Identity(W),
    Deflate(ZlibPushDecoder<W>),
    Gzip(GzPushDecoder<W>),
    Brotli(Box<BrPushDecoder<W>>),
    #[cfg(feature = "zstd")]
    Zstandard(ZstdPushDecoder<'static, W>),
    None,
}

impl<W: Write> Debug for PushDecoder<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(_arg0) => f.debug_tuple("Identity").finish(),
            Self::Deflate(_arg0) => f.debug_tuple("Deflate").finish(),
            Self::Gzip(_arg0) => f.debug_tuple("Gzip").finish(),
            Self::Brotli(_arg0) => f.debug_tuple("Brotli").finish(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(_arg0) => f.debug_tuple("Zstandard").finish(),
            Self::None => write!(f, "None"),
        }
    }
}

impl<W: Write> PushDecoder<W> {
    fn get_ref(&self) -> &W {
        match self {
            Self::Identity(v) => v,
            Self::Deflate(codec) => codec.get_ref(),
            Self::Gzip(codec) => codec.get_ref(),
            Self::Brotli(codec) => codec.get_ref(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.get_ref(),
            Self::None => unreachable!(),
        }
    }

    fn get_mut(&mut self) -> &mut W {
        match self {
            Self::Identity(v) => v,
            Self::Deflate(codec) => codec.get_mut(),
            Self::Gzip(codec) => codec.get_mut(),
            Self::Brotli(codec) => codec.get_mut(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.get_mut(),
            Self::None => unreachable!(),
        }
    }

    fn into_inner(self) -> std::io::Result<W> {
        match self {
            Self::Identity(v) => Ok(v),
            Self::Deflate(codec) => codec.finish(),
            Self::Gzip(codec) => codec.finish(),
            Self::Brotli(mut codec) => {
                codec.close()?;
                match codec.into_inner() {
                    Ok(v) => Ok(v),
                    Err(v) => Ok(v),
                }
            }
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => Ok(codec.into_inner()),
            Self::None => unreachable!(),
        }
    }
}

impl<W: Write> Write for PushDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Identity(w) => w.write(buf),
            Self::Deflate(w) => w.write(buf),
            Self::Gzip(w) => w.write(buf),
            Self::Brotli(w) => w.write(buf),
            #[cfg(feature = "zstd")]
            Self::Zstandard(w) => w.write(buf),
            Self::None => unreachable!(),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Identity(w) => w.flush(),
            Self::Deflate(w) => w.flush(),
            Self::Gzip(w) => w.flush(),
            Self::Brotli(w) => w.flush(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(w) => w.flush(),
            Self::None => unreachable!(),
        }
    }
}

/// Push-based decoder for decompressing streams.
///
/// This is similar to [`Decompressor`] except that the user is responsible for
/// moving data.
#[derive(Debug)]
pub struct PushDecompressor<W: Write> {
    decoder: PushDecoder<W>,
    format: Format,
}

impl<W: Write> PushDecompressor<W> {
    /// Create a decompressor.for writing the decompressed data.
    pub fn new(output: W, format: Format) -> std::io::Result<Self> {
        Ok(Self {
            decoder: create_push_decoder(output, format)?,
            format,
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
    /// This function is only to be used for multistream codecs.
    pub fn restart_stream(&mut self) -> std::io::Result<()> {
        let decoder = std::mem::replace(&mut self.decoder, PushDecoder::None);
        let dest = decoder.into_inner()?;
        self.decoder = create_push_decoder(dest, self.format)?;

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

fn get_encoder_level(format: Format, level: Level) -> i32 {
    match format {
        Format::Identity => match level {
            Level::Balanced => 0,
            Level::High => 0,
            Level::Low => 0,
        },
        Format::Deflate | Format::Gzip => match level {
            Level::Balanced => 6,
            Level::High => 9,
            Level::Low => 1,
        },

        Format::Brotli => match level {
            Level::Balanced => 4,
            Level::High => 7,
            Level::Low => 0,
        },
        #[cfg(feature = "zstd")]
        Format::Zstandard => match level {
            Level::Balanced => 3,
            Level::High => 9,
            Level::Low => 1,
        },
    }
}

fn create_encoder<W: Write>(dest: W, format: Format, level: Level) -> Encoder<W> {
    let level = get_encoder_level(format, level);

    match format {
        Format::Identity => Encoder::Identity(dest),
        Format::Deflate => Encoder::Deflate(ZlibEncoder::new(
            dest,
            flate2::Compression::new(level as u32),
        )),
        Format::Gzip => Encoder::Gzip(GzEncoder::new(dest, flate2::Compression::new(level as u32))),
        Format::Brotli => Encoder::Brotli(Box::new(BrEncoder::new(dest, 4096, level as u32, 22))),
        #[cfg(feature = "zstd")]
        Format::Zstandard => Encoder::Zstandard(ZstdEncoder::new(dest, level).unwrap()),
    }
}

fn create_decoder<R: BufRead>(source: R, format: Format) -> std::io::Result<Decoder<R>> {
    match format {
        Format::Identity => Ok(Decoder::Identity(source)),
        Format::Deflate => Ok(Decoder::Deflate(ZlibDecoder::new(source))),
        Format::Gzip => Ok(Decoder::Gzip(GzDecoder::new(source))),
        Format::Brotli => Ok(Decoder::Brotli(Box::new(BrDecoder::new(source, 4096)))),
        #[cfg(feature = "zstd")]
        Format::Zstandard => Ok(Decoder::Zstandard(
            ZstdDecoder::with_buffer(source)?.single_frame(),
        )),
    }
}

fn create_push_decoder<W: Write>(dest: W, format: Format) -> std::io::Result<PushDecoder<W>> {
    match format {
        Format::Identity => Ok(PushDecoder::Identity(dest)),
        Format::Deflate => Ok(PushDecoder::Deflate(ZlibPushDecoder::new(dest))),
        Format::Gzip => Ok(PushDecoder::Gzip(GzPushDecoder::new(dest))),
        Format::Brotli => Ok(PushDecoder::Brotli(Box::new(BrPushDecoder::new(
            dest, 4096,
        )))),
        // FIXME: no single frame option
        #[cfg(feature = "zstd")]
        Format::Zstandard => Ok(PushDecoder::Zstandard(ZstdPushDecoder::new(dest)?)),
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
        c.restart_stream().unwrap();
        c.write_all(b"world").unwrap();

        let buf = c.finish().unwrap();

        let mut d = Decompressor::new(BufReader::new(Cursor::new(buf)), Format::Gzip).unwrap();

        let mut buf = Vec::new();

        d.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, b"Hello");
        assert!(d.has_data_left().unwrap());

        buf.clear();
        d.restart_stream().unwrap();
        d.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, b"world");
        assert!(!d.has_data_left().unwrap());

        d.into_inner();
    }
}
