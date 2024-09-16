//! WARC file writing
use std::io::{BufWriter, Write};

use crate::{
    compress::{Compressor, CompressorConfig},
    error::ParseIoError,
    header::WarcHeader,
};

/// Configuration for a [`Writer`].
#[derive(Debug, Clone, Default)]
pub struct WriterConfig {
    /// Configuration for compressing the written file
    pub compression: CompressorConfig,
}

pub struct StateHeader;
pub struct StateBlock {
    length: u64,
    written: u64,
}

/// WARC format writer
pub struct Writer<S, W: Write> {
    state: S,
    output: BufWriter<Compressor<W>>,
    config: WriterConfig,
}

impl<W: Write> Writer<StateHeader, W> {
    /// Create a new writer.
    ///
    /// The destination writer should not be a compression stream. To enable
    /// compression, you must configure it with [`WriterConfig`].
    pub fn new(dest: W, config: WriterConfig) -> Self {
        let output = Compressor::new(dest, config.compression.clone());

        Self {
            state: StateHeader,
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
    ) -> Result<Writer<StateBlock, W>, ParseIoError> {
        header.validate()?;
        header.serialize(&mut self.output)?;

        let length = header.content_length()?;

        Ok(Writer {
            state: StateBlock { length, written: 0 },
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

impl<W: Write> Writer<StateBlock, W> {
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
    pub fn finish_block(self) -> std::io::Result<Writer<StateHeader, W>> {
        if self.state.length != self.state.written {
            return Err(std::io::Error::other(ContentLengthMismatch::new(
                self.state.length,
                self.state.written,
            )));
        }

        Ok(Writer {
            state: StateHeader,
            output: self.output,
            config: self.config,
        })
    }
}

impl<W: Write> Write for Writer<StateBlock, W> {
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
        let writer = Writer::new(buf, WriterConfig::default());

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
