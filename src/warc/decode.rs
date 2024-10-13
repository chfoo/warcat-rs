//! WARC file reading
use std::io::{BufRead, Read};

use crate::{
    compress::{Decompressor, Format},
    error::{GeneralError, ProtocolError, ProtocolErrorKind},
    header::WarcHeader,
    io::{BufferReader, LogicalPosition},
};

const BUFFER_LENGTH: usize = 4096;
const MAX_HEADER_LENGTH: usize = 32768;

/// Configuration for a [`Decoder`]
#[derive(Debug, Clone, Default)]
pub struct DecoderConfig {
    /// Compression format of the file to be read
    pub compression_format: Format,
}

#[derive(Debug)]
pub struct DecStateHeader;
#[derive(Debug, Default)]
pub struct DecStateBlock {
    length: u64,
    read: u64,
    boundary: [u8; 4],
    boundary_len: usize,
}

/// WARC format reader
///
/// This struct is able keep its state consistent when the underlying reader
/// returns an error such as [`std::io::ErrorKind::WouldBlock`]. It is safe
/// for the caller to retry.
/// This property allows the decoder to be used in a push-based fashion
/// in alignment to the sans-IO philosophy.
#[derive(Debug)]
pub struct Decoder<S, R: Read> {
    state: S,
    input: BufferReader<Decompressor<BufferReader<R>>>,
    config: DecoderConfig,
    record_boundary_position: u64,
}

impl<S, R: Read> Decoder<S, R> {
    pub fn get_ref(&self) -> &R {
        self.input.get_ref().get_ref().get_ref()
    }

    pub fn get_mut(&mut self) -> &mut R {
        self.input.get_mut().get_mut().get_mut()
    }

    /// Returns the position of the beginning of a WARC record.
    ///
    /// This function is intended for indexing a WARC file.
    pub fn record_boundary_position(&self) -> u64 {
        self.record_boundary_position
    }
}

impl<R: Read> Decoder<DecStateHeader, R> {
    /// Creates a new decoder that reads from the given reader.
    pub fn new(input: R, config: DecoderConfig) -> std::io::Result<Self> {
        let input = BufferReader::new(Decompressor::new(
            BufferReader::new(input),
            config.compression_format,
        )?);

        Ok(Self {
            state: DecStateHeader,
            input,
            config,
            record_boundary_position: 0,
        })
    }

    /// Returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.input.into_inner().into_inner().into_inner()
    }

    /// Returns whether there is another WARC record to be read.
    pub fn has_next_record(&mut self) -> std::io::Result<bool> {
        self.input.fill_buffer_if_empty()?;

        Ok(!self.input.buffer().is_empty() || self.input.get_mut().has_data_left()?)
    }

    /// Reads the header portion of a WARC record.
    ///
    /// This function consumes the reader and returns a typestate transitioned
    /// reader for reading the block portion of a WARC record.
    pub fn read_header(mut self) -> Result<(WarcHeader, Decoder<DecStateBlock, R>), GeneralError> {
        loop {
            if let Some(index) = crate::parse::scan_header_deliminator(self.input.buffer()) {
                let header_bytes = &self.input.buffer()[0..index];
                let header = WarcHeader::parse(header_bytes)?;
                let length = header.content_length()?;
                let record_id = header.fields.get("WARC-Record-ID");
                let warc_type = header.fields.get("WARC-Type");
                self.input.consume(index);

                tracing::trace!(record_id, warc_type, content_length = length, "read record");

                return Ok((
                    header,
                    Decoder {
                        state: DecStateBlock {
                            length,
                            ..Default::default()
                        },
                        input: self.input,
                        config: self.config,
                        record_boundary_position: self.record_boundary_position,
                    },
                ));
            }

            self.check_max_header_length()?;
            self.fill_buf_from_source()?;
        }
    }

    fn check_max_header_length(&self) -> Result<(), ProtocolError> {
        tracing::trace!("check max header length");

        if self.input.buffer().len() > MAX_HEADER_LENGTH {
            Err(ProtocolError::new(ProtocolErrorKind::HeaderTooBig))
        } else {
            Ok(())
        }
    }

    fn fill_buf_from_source(&mut self) -> std::io::Result<()> {
        tracing::trace!("fill buf");

        let read_length = self.input.fill_buffer()?;

        if read_length == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        }

        tracing::trace!(read_length, buf_len = self.input.buffer().len(), "fill buf");

        Ok(())
    }
}

impl<R: Read> Decoder<DecStateBlock, R> {
    fn read_block_impl(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        assert!(self.state.length >= self.state.read);
        let remain_length = self.state.length - self.state.read;
        let buf_upper = buf
            .len()
            .min(usize::try_from(remain_length).unwrap_or(usize::MAX));
        let buf = &mut buf[0..buf_upper];

        let read_length = self.input.read(buf)?;

        self.state.read += read_length as u64;
        tracing::trace!(read_length, remain_length, "read block");

        Ok(read_length)
    }

    /// Indicate that reading the block portion of WARC record has completed.
    ///
    /// It's not necessary for the user to read the entire block or at all;
    /// this function will continue to the end of the record automatically.
    ///
    /// Consumes the writer and returns a typestate transitioned writer that
    /// can read the next WARC record.
    pub fn finish_block(mut self) -> Result<Decoder<DecStateHeader, R>, GeneralError> {
        tracing::trace!("finish block");
        self.read_remaining_block()?;
        self.read_record_boundary()?;

        self.input.fill_buffer_if_empty()?;

        if self.config.compression_format.is_multistream() && !self.input.buffer().is_empty() {
            tracing::warn!("file not using 'Record-at-time compression'");
        }

        self.record_boundary_position = self.logical_position();

        if self.input.buffer().is_empty() && self.input.get_mut().has_data_left()? {
            self.input.get_mut().restart_stream()?;
        }

        Ok(Decoder {
            state: DecStateHeader,
            input: self.input,
            config: self.config,
            record_boundary_position: self.record_boundary_position,
        })
    }

    fn read_remaining_block(&mut self) -> std::io::Result<()> {
        tracing::trace!("read remaining block");
        let mut buf: Vec<u8> = Vec::with_capacity(BUFFER_LENGTH);

        loop {
            assert!(self.state.length >= self.state.read);
            let remaining_bytes = self.state.length - self.state.read;

            if remaining_bytes == 0 {
                break;
            }

            let buf_length = BUFFER_LENGTH.min(remaining_bytes as usize);
            buf.resize(buf_length, 0);
            let read_length = self.input.read(&mut buf)?;
            self.state.read += read_length as u64;
            tracing::trace!(read_length, remaining_bytes, "read remaining block");
        }

        Ok(())
    }

    fn read_record_boundary(&mut self) -> Result<(), GeneralError> {
        tracing::trace!("read record boundary");

        loop {
            let remain_len = 4 - self.state.boundary_len;

            if remain_len == 0 {
                break;
            }

            let buf = &mut self.state.boundary[self.state.boundary_len..];
            let read_len = self.input.read(buf)?;

            self.state.boundary_len += read_len;
        }

        if &self.state.boundary != b"\r\n\r\n" {
            Err(ProtocolError::new(ProtocolErrorKind::InvalidRecordBoundary).into())
        } else {
            Ok(())
        }
    }
}

impl<R: Read> Read for Decoder<DecStateBlock, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read_block_impl(buf)
    }
}

impl<R: Read, S> LogicalPosition for Decoder<S, R> {
    fn logical_position(&self) -> u64 {
        if self.config.compression_format == Format::Identity {
            self.input.logical_position()
        } else {
            self.input.get_ref().get_ref().logical_position()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[tracing_test::traced_test]
    #[test]
    fn test_reader() {
        let data = b"WARC/1.1\r\n\
            Content-Length: 12\r\n\
            \r\n\
            Hello world!\
            \r\n\r\n\
            WARC/1.1\r\n\
            Content-Length: 0\r\n\
            \r\n\
            \r\n\r\n";

        let reader = Decoder::new(Cursor::new(data), DecoderConfig::default()).unwrap();

        let (_header, mut reader) = reader.read_header().unwrap();
        let mut block = Vec::new();
        reader.read_to_end(&mut block).unwrap();
        let mut reader = reader.finish_block().unwrap();

        assert!(reader.has_next_record().unwrap());

        let (_header, mut reader) = reader.read_header().unwrap();
        let mut block = Vec::new();
        reader.read_to_end(&mut block).unwrap();
        let mut reader = reader.finish_block().unwrap();

        assert!(!reader.has_next_record().unwrap());

        reader.into_inner();
    }
}
