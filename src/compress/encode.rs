use std::{fmt::Debug, io::Write};

#[cfg(feature = "zstd")]
use super::zstd::ZstdEncoder;
use brotli::CompressorWriter as BrEncoder;
use flate2::write::{GzEncoder, ZlibEncoder};

use super::{Dictionary, Format, Level};

pub enum Encoder<W: Write> {
    Identity(W),
    Deflate(ZlibEncoder<W>),
    Gzip(GzEncoder<W>),
    Brotli(Box<BrEncoder<W>>),
    #[cfg(feature = "zstd")]
    Zstandard(ZstdEncoder<W>),
    None,
}

impl<W: Write> Encoder<W> {
    pub fn new(dest: W, format: Format, level: Level, dictionary: &Dictionary) -> Encoder<W> {
        let level = get_encoder_level(format, level);

        match format {
            Format::Identity => Encoder::Identity(dest),
            Format::Deflate => Encoder::Deflate(ZlibEncoder::new(
                dest,
                flate2::Compression::new(level as u32),
            )),
            Format::Gzip => {
                Encoder::Gzip(GzEncoder::new(dest, flate2::Compression::new(level as u32)))
            }
            Format::Brotli => {
                Encoder::Brotli(Box::new(BrEncoder::new(dest, 4096, level as u32, 22)))
            }
            #[cfg(feature = "zstd")]
            Format::Zstandard => {
                Encoder::Zstandard(ZstdEncoder::new(dest, level, dictionary.clone()).unwrap())
            }
        }
    }

    pub fn get_ref(&self) -> &W {
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

    pub fn get_mut(&mut self) -> &mut W {
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

    pub fn finish(self) -> std::io::Result<W> {
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
