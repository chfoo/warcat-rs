use std::{
    fmt::Debug,
    io::{BufRead, Read, Write},
};

use brotli::{writer::DecompressorWriter as BrPushDecoder, Decompressor as BrDecoder};
use flate2::{
    bufread::{GzDecoder, ZlibDecoder},
    write::{GzDecoder as GzPushDecoder, ZlibDecoder as ZlibPushDecoder},
};

#[cfg(feature = "zstd")]
use super::zstd::{ZstdDecoder, ZstdPushDecoder};
use super::{Dictionary, Format};

pub enum Decoder<R: BufRead> {
    Identity(R),
    Deflate(ZlibDecoder<R>),
    Gzip(GzDecoder<R>),
    Brotli(Box<BrDecoder<R>>),
    #[cfg(feature = "zstd")]
    Zstandard(ZstdDecoder<R>),
    None,
}

impl<R: BufRead> Decoder<R> {
    pub fn new(source: R, format: Format, dictionary: &Dictionary) -> std::io::Result<Decoder<R>> {
        match format {
            Format::Identity => Ok(Decoder::Identity(source)),
            Format::Deflate => Ok(Decoder::Deflate(ZlibDecoder::new(source))),
            Format::Gzip => Ok(Decoder::Gzip(GzDecoder::new(source))),
            Format::Brotli => Ok(Decoder::Brotli(Box::new(BrDecoder::new(source, 4096)))),
            #[cfg(feature = "zstd")]
            Format::Zstandard => Ok(Decoder::Zstandard(ZstdDecoder::new(
                source,
                dictionary.clone(),
            )?)),
        }
    }
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
    pub fn get_ref(&self) -> &R {
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

    pub fn get_mut(&mut self) -> &mut R {
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

    pub fn into_inner(self) -> R {
        match self {
            Self::Identity(r) => r,
            Self::Deflate(codec) => codec.into_inner(),
            Self::Gzip(codec) => codec.into_inner(),
            Self::Brotli(codec) => codec.into_inner(),
            #[cfg(feature = "zstd")]
            Self::Zstandard(codec) => codec.into_inner(),
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

pub enum PushDecoder<W: Write> {
    Identity(W),
    Deflate(ZlibPushDecoder<W>),
    Gzip(GzPushDecoder<W>),
    Brotli(Box<BrPushDecoder<W>>),
    #[cfg(feature = "zstd")]
    Zstandard(ZstdPushDecoder<W>),
    None,
}

impl<W: Write> PushDecoder<W> {
    pub fn new(
        dest: W,
        format: Format,
        dictionary: &Dictionary,
    ) -> std::io::Result<PushDecoder<W>> {
        match format {
            Format::Identity => Ok(PushDecoder::Identity(dest)),
            Format::Deflate => Ok(PushDecoder::Deflate(ZlibPushDecoder::new(dest))),
            Format::Gzip => Ok(PushDecoder::Gzip(GzPushDecoder::new(dest))),
            Format::Brotli => Ok(PushDecoder::Brotli(Box::new(BrPushDecoder::new(
                dest, 4096,
            )))),
            #[cfg(feature = "zstd")]
            Format::Zstandard => Ok(PushDecoder::Zstandard(ZstdPushDecoder::new(
                dest,
                dictionary.clone(),
            )?)),
        }
    }
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
    pub fn get_ref(&self) -> &W {
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

    pub fn get_mut(&mut self) -> &mut W {
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

    pub fn into_inner(self) -> std::io::Result<W> {
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
