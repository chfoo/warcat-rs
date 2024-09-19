//! WARC file writing
use std::io::{BufWriter, Write};

use crate::{
    compress::{Compressor, Format, Level},
    error::ParseIoError,
    header::WarcHeader,
};

/// Configuration for a [`Encoder`].
#[derive(Debug, Clone, Default)]
pub struct EncoderConfig {
    /// Format for compressing the written file
    pub compression: Format,
    /// Compression level
    pub compression_level: Level,
}

pub struct EncStateHeader;
pub struct EncStateBlock {
    length: u64,
    written: u64,
}

/// WARC format writer
pub struct Encoder<S, W: Write> {
    state: S,
    output: BufWriter<Compressor<W>>,
    config: EncoderConfig,
}

impl<W: Write> Encoder<EncStateHeader, W> {
    /// Create a new encoder.
    ///
    /// The destination writer should not be a compression stream. To enable
    /// compression, you must configure it with [`EncoderConfig`].
    pub fn new(dest: W, config: EncoderConfig) -> Self {
        let output = Compressor::with_level(dest, config.compression, config.compression_level);

        Self {
            state: EncStateHeader,
            output: BufWriter::new(output),
            config,
        }
    }

    /// Start a new WARC record with a given header.
    ///
    /// The validation function will be called on the header before
    /// writing it to the stream.
    ///
    /// Consumes the writer and returns a writer that has typestate
    /// transitioned to writing the WARC block portion of the record.
    pub fn write_header(
        mut self,
        header: &WarcHeader,
    ) -> Result<Encoder<EncStateBlock, W>, ParseIoError> {
        header.validate()?;
        header.serialize(&mut self.output)?;

        let length = header.content_length()?;

        Ok(Encoder {
            state: EncStateBlock { length, written: 0 },
            output: self.output,
            config: self.config,
        })
    }

    /// Flushes any buffered data and returns the underlying stream.
    ///
    /// You must call this function before dropping the struct in order
    /// to have a valid WARC file.
    pub fn finish(self) -> std::io::Result<W> {
        self.output.into_inner()?.finish()
    }
}

impl<W: Write> Encoder<EncStateBlock, W> {
    fn write_block_impl(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let remain_length = self.state.length - self.state.written;
        let buf_upper = buf
            .len()
            .min(usize::try_from(remain_length).unwrap_or(usize::MAX));
        let buf = &buf[0..buf_upper];

        let write_length = self.output.write(buf)?;
        self.state.written += write_length as u64;

        debug_assert!(self.state.length >= self.state.written);

        if self.state.length == self.state.written {
            self.write_finish_block()?;
        }

        Ok(write_length)
    }

    fn write_finish_block(&mut self) -> std::io::Result<()> {
        self.output.write_all(b"\r\n\r\n")?;
        self.output.flush()?;
        self.output.get_mut().restart_stream()?;
        Ok(())
    }

    /// Indicate writing the block portion of a WARC record has completed.
    ///
    /// Consumes the writer and returns a typestate transitioned
    /// writer for writing a new record.
    pub fn finish_block(self) -> std::io::Result<Encoder<EncStateHeader, W>> {
        if self.state.length != self.state.written {
            return Err(std::io::Error::other(ContentLengthMismatch::new(
                self.state.length,
                self.state.written,
            )));
        }

        Ok(Encoder {
            state: EncStateHeader,
            output: self.output,
            config: self.config,
        })
    }
}

impl<W: Write> Write for Encoder<EncStateBlock, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_block_impl(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}

/// Error for a block size mismatch in a WARC record.
#[derive(Debug, Default, thiserror::Error)]
#[error("content length mismatch: expected {expected}, got {expected}")]
pub struct ContentLengthMismatch {
    expected: u64,
    actual: u64,
}

impl ContentLengthMismatch {
    pub fn new(expected: u64, actual: u64) -> Self {
        Self { expected, actual }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writer() {
        let buf = Vec::new();
        let writer = Encoder::new(buf, EncoderConfig::default());

        let header = WarcHeader::new(12, "a");
        let mut writer = writer.write_header(&header).unwrap();
        writer.write_all(b"Hello world!").unwrap();
        let writer = writer.finish_block().unwrap();

        let header = WarcHeader::new(0, "a");
        let mut writer = writer.write_header(&header).unwrap();
        writer.write_all(b"").unwrap();
        let writer = writer.finish_block().unwrap();

        let buf = writer.finish().unwrap();

        assert!(buf.starts_with(b"WARC/1.1\r\n"));
    }
}
