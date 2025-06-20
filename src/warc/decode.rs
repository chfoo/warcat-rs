//! WARC file reading
use std::{
    collections::VecDeque,
    io::{Read, Seek, Write},
};

use crate::{
    compress::{DecompressorConfig, PushDecompressor},
    error::{GeneralError, ProtocolError, ProtocolErrorKind},
    header::WarcHeader,
    io::LogicalPosition,
};

const BUFFER_LENGTH: usize = crate::io::IO_BUFFER_LENGTH;
const MAX_HEADER_LENGTH: usize = 32768;

/// Configuration for a [`Decoder`]
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct DecoderConfig {
    /// Compression configuration of the file to be read
    pub decompressor: DecompressorConfig,
}

#[derive(Debug)]
pub struct DecStateHeader;
#[derive(Debug, Default)]
pub struct DecStateBlock {
    is_end: bool,
}

/// WARC format reader
#[derive(Debug)]
pub struct Decoder<S, R: Read> {
    state: S,
    input: R,
    push_decoder: PushDecoder,
    logical_position: u64,
    buf: Vec<u8>,
}

impl<S, R: Read> Decoder<S, R> {
    pub fn get_ref(&self) -> &R {
        &self.input
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.input
    }

    /// Returns the position of the beginning of a WARC record.
    ///
    /// This function is intended for indexing a WARC file.
    pub fn record_boundary_position(&self) -> u64 {
        self.push_decoder.record_boundary_position()
    }

    fn read_into_push_decoder(&mut self) -> std::io::Result<usize> {
        tracing::trace!("read into push decoder");

        self.buf.resize(BUFFER_LENGTH, 0);

        let read_length = self.input.read(&mut self.buf)?;

        self.buf.truncate(read_length);

        self.logical_position += read_length as u64;

        self.push_decoder.write_all(&self.buf)?;

        if read_length == 0 {
            self.push_decoder.write_eof()?;
        }

        tracing::trace!(read_length, "read into push decoder");

        Ok(read_length)
    }

    fn read_nonzero_into_push_decoder(&mut self) -> std::io::Result<()> {
        let read_length = self.read_into_push_decoder()?;

        if read_length == 0 {
            Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
        } else {
            Ok(())
        }
    }

    /// Resets the decoder state so that a new record can be decoded.
    ///
    /// Configuration is kept but any buffered data is discarded.
    ///
    /// This function may be used after file seeking or
    /// partially reading records.
    pub fn reset(mut self) -> std::io::Result<Decoder<DecStateHeader, R>> {
        self.push_decoder.reset()?;

        Ok(Decoder {
            state: DecStateHeader,
            input: self.input,
            push_decoder: self.push_decoder,
            logical_position: self.logical_position,
            buf: self.buf,
        })
    }
}

impl<R: Read> Decoder<DecStateHeader, R> {
    /// Creates a new decoder that reads from the given reader.
    pub fn new(input: R, config: DecoderConfig) -> std::io::Result<Self> {
        let push_decoder = PushDecoder::new(config)?;

        Ok(Self {
            state: DecStateHeader,
            input,
            push_decoder,
            logical_position: 0,
            buf: Vec::with_capacity(BUFFER_LENGTH),
        })
    }

    /// Returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.input
    }

    /// Returns whether it was detected that the file was compressed
    /// in a manner that makes random access to each record impossible.
    ///
    /// A false value is not guaranteed to be false unless the entire file has
    /// been read.
    pub fn has_record_at_time_compression_fault(&self) -> bool {
        self.push_decoder.has_record_at_time_compression_fault()
    }

    /// Returns whether there is another WARC record to be read.
    pub fn has_next_record(&mut self) -> std::io::Result<bool> {
        if self.push_decoder.is_finished() {
            return Ok(false);
        } else if self.push_decoder.is_ready() {
            self.read_into_push_decoder()?;
        }

        Ok(!self.push_decoder.is_ready())
    }

    /// Reads the header portion of a WARC record.
    ///
    /// This function consumes the reader and returns a typestate transitioned
    /// reader for reading the block portion of a WARC record.
    pub fn read_header(mut self) -> Result<(WarcHeader, Decoder<DecStateBlock, R>), GeneralError> {
        loop {
            match self.push_decoder.get_event()? {
                PushDecoderEvent::Ready | PushDecoderEvent::WantData => {
                    self.read_into_push_decoder()?;
                    continue;
                }
                PushDecoderEvent::Finished => {
                    return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into());
                }
                PushDecoderEvent::Continue => continue,
                PushDecoderEvent::Header { header } => {
                    return Ok((
                        header,
                        Decoder {
                            state: DecStateBlock::default(),
                            input: self.input,
                            push_decoder: self.push_decoder,
                            buf: self.buf,
                            logical_position: self.logical_position,
                        },
                    ));
                }
                PushDecoderEvent::BlockData { data: _ } => unreachable!(),
                PushDecoderEvent::EndRecord => unreachable!(),
            }
        }
    }
}

impl<R: Read + Seek> Decoder<DecStateHeader, R> {
    /// Prepare the internal decompressor to be ready for the source to be seeked.
    ///
    /// For Zstandard, this may load an embedded dictionary.
    /// For other compression formats, this has no effect.
    pub fn prepare_for_seek(&mut self) -> Result<(), GeneralError> {
        if self
            .push_decoder
            .config
            .decompressor
            .format
            .supports_concatenation()
        {
            loop {
                self.read_into_push_decoder()?;

                match self.push_decoder.get_event()? {
                    PushDecoderEvent::Ready
                    | PushDecoderEvent::Finished
                    | PushDecoderEvent::WantData
                    | PushDecoderEvent::Continue => {}
                    PushDecoderEvent::Header { .. }
                    | PushDecoderEvent::BlockData { .. }
                    | PushDecoderEvent::EndRecord => break,
                }
            }

            self.input.seek(std::io::SeekFrom::Start(0))?;
            self.push_decoder.reset()?;
        }

        Ok(())
    }
}

impl<R: Read> Decoder<DecStateBlock, R> {
    fn read_block_impl(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.state.is_end {
            return Ok(0);
        }

        if buf.is_empty() {
            return Ok(0);
        }

        self.push_decoder.set_max_buffer_len(buf.len());

        loop {
            match self
                .push_decoder
                .get_event()
                .map_err(std::io::Error::other)?
            {
                PushDecoderEvent::Ready => unreachable!(),
                PushDecoderEvent::Finished => unreachable!(),
                PushDecoderEvent::WantData => {
                    self.read_into_push_decoder()?;
                    continue;
                }
                PushDecoderEvent::Continue => continue,
                PushDecoderEvent::Header { header: _ } => unreachable!(),
                PushDecoderEvent::BlockData { data } => {
                    debug_assert!(data.len() <= buf.len());

                    let buf_upper = buf.len().min(data.len());
                    tracing::trace!(read_length = buf_upper, "read block");

                    buf[0..buf_upper].copy_from_slice(&data[0..buf_upper]);

                    return Ok(buf_upper);
                }
                PushDecoderEvent::EndRecord => {
                    self.state.is_end = true;
                    return Ok(0);
                }
            }
        }
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

        Ok(Decoder {
            state: DecStateHeader,
            input: self.input,
            push_decoder: self.push_decoder,
            logical_position: self.logical_position,
            buf: self.buf,
        })
    }

    fn read_remaining_block(&mut self) -> Result<(), GeneralError> {
        tracing::trace!("read remaining block");

        self.push_decoder.set_max_buffer_len(BUFFER_LENGTH);

        while !self.state.is_end {
            match self.push_decoder.get_event()? {
                PushDecoderEvent::Ready => unreachable!(),
                PushDecoderEvent::Finished => unreachable!(),
                PushDecoderEvent::WantData => {
                    self.read_nonzero_into_push_decoder()?;
                    continue;
                }
                PushDecoderEvent::Continue => continue,
                PushDecoderEvent::Header { header: _ } => unreachable!(),
                PushDecoderEvent::BlockData { data: _ } => continue,
                PushDecoderEvent::EndRecord => self.state.is_end = true,
            }
        }

        Ok(())
    }
}

impl<R: Read> Read for Decoder<DecStateBlock, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read_block_impl(buf)
    }
}

impl<R: Read, S> LogicalPosition for Decoder<S, R> {
    fn logical_position(&self) -> u64 {
        self.logical_position
    }
}

/// Events for [`PushDecoder`].
#[derive(Debug)]
pub enum PushDecoderEvent<'a> {
    /// No input data has been received yet.
    Ready,
    /// End-of-file has been reached and no more data can be decoded.
    Finished,
    /// Either more data or end-of-file (EOF) is needed.
    WantData,
    /// Internal processing was successful and the user should call again.
    Continue,
    /// Decoding a header was successful.
    Header { header: WarcHeader },
    /// A chunk of the decoded block data.
    BlockData { data: &'a [u8] },
    /// Finished processing a single record.
    EndRecord,
}

impl<'a> PushDecoderEvent<'a> {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished)
    }

    pub fn is_want_data(&self) -> bool {
        matches!(self, Self::WantData)
    }

    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue)
    }

    pub fn is_header(&self) -> bool {
        matches!(self, Self::Header { .. })
    }

    pub fn is_block_data(&self) -> bool {
        matches!(self, Self::BlockData { .. })
    }

    pub fn as_header(&self) -> Option<&WarcHeader> {
        if let Self::Header { header } = self {
            Some(header)
        } else {
            None
        }
    }

    pub fn as_block_data(&self) -> Option<&'a [u8]> {
        if let Self::BlockData { data } = self {
            Some(data)
        } else {
            None
        }
    }

    pub fn is_end_record(&self) -> bool {
        matches!(self, Self::EndRecord)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushDecoderState {
    PendingHeader,
    Header,
    Block,
    RecordBoundary,
    EndOfSegment,
    Finished,
}

/// WARC format decoder push-style.
///
/// This is similar to [`Decoder`] but input data is written to the struct
/// and events are gathered by the caller. This push-style method can be
/// use for sans-IO implementations.
#[derive(Debug)]
pub struct PushDecoder {
    config: DecoderConfig,
    state: PushDecoderState,
    decompressor: PushDecompressor<VecDeque<u8>>,
    decompressor_eof: bool,
    input_eof: bool,
    // Data that has not been decompresssed yet because it's for the next record.
    deferred_input_buf: VecDeque<u8>,
    // Total number of bytes written into the decoder.
    bytes_written_decoder: u64,
    // Total number of bytes read from the decoder (not including bytes for the next record).
    decoded_bytes_consumed: u64,
    // Index of record boundary in the raw file.
    record_boundary_position: u64,
    // Total number of bytes to be read from the record block.
    block_length: u64,
    // Number of bytes read so far from the record block.
    block_current_position: u64,
    // Maximum number of bytes that can be used for PushDecoderEvent::BlockData.
    buf_output_max_len: usize,
    // Number of bytes borrowed for PushDecoderEvent::BlockData.
    buf_output_reference_len: usize,
    // Detected a compressed file that can't be randomly accessed
    has_rat_comp_fault: bool,
}

impl PushDecoder {
    /// Creates a new decoder.
    pub fn new(config: DecoderConfig) -> std::io::Result<Self> {
        let decompressor =
            PushDecompressor::with_config(VecDeque::new(), config.decompressor.clone())?;

        Ok(Self {
            config,
            state: PushDecoderState::PendingHeader,
            decompressor,
            decompressor_eof: false,
            input_eof: false,
            deferred_input_buf: VecDeque::with_capacity(BUFFER_LENGTH),
            bytes_written_decoder: 0,
            decoded_bytes_consumed: 0,
            record_boundary_position: 0,
            block_length: 0,
            block_current_position: 0,
            buf_output_max_len: BUFFER_LENGTH,
            buf_output_reference_len: 0,
            has_rat_comp_fault: false,
        })
    }

    /// Returns the position of the beginning of a WARC record.
    ///
    /// This function is intended for indexing a WARC file.
    pub fn record_boundary_position(&self) -> u64 {
        self.record_boundary_position
    }

    /// Returns whether internal buffer contains unused bytes that can be
    /// used to decode the next record.
    pub fn has_next_record(&self) -> bool {
        !self.deferred_input_buf.is_empty()
    }

    /// Returns the maximum buffer length that can be used in [`PushDecoderEvent::BlockData`].
    pub fn max_buffer_len(&self) -> usize {
        self.buf_output_max_len
    }

    /// Sets the maximum buffer length that can be used in [`PushDecoderEvent::BlockData`].
    ///
    /// If the given value is 0, the value is set to a non-zero default.
    pub fn set_max_buffer_len(&mut self, value: usize) {
        if value != 0 {
            self.buf_output_max_len = value;
        } else {
            self.buf_output_max_len = BUFFER_LENGTH;
        }
    }

    /// Returns whether it was detected that the file was compressed
    /// in a manner that makes random access to each record impossible.
    ///
    /// A false value is not guaranteed to be false unless the entire file has
    /// been read.
    pub fn has_record_at_time_compression_fault(&self) -> bool {
        self.has_rat_comp_fault
    }

    /// Returns whether the next call to [`get_event()`](Self::get_event())
    /// will return [`PushDecoderEvent::Ready`].
    pub fn is_ready(&self) -> bool {
        matches!(self.state, PushDecoderState::PendingHeader)
    }

    /// Returns whether the next call to [`get_event()`](Self::get_event())
    /// will return [`PushDecoderEvent::Finished`].
    pub fn is_finished(&self) -> bool {
        matches!(self.state, PushDecoderState::Finished)
    }

    /// Returns a processed event.
    ///
    /// In order for this decoder to produce events, the caller must
    /// put input data using the [`Write`] trait.
    pub fn get_event(&mut self) -> Result<PushDecoderEvent, GeneralError> {
        self.decompressor
            .get_mut()
            .drain(0..self.buf_output_reference_len);
        self.buf_output_reference_len = 0;

        match self.state {
            PushDecoderState::PendingHeader => Ok(PushDecoderEvent::Ready),
            PushDecoderState::Header => self.process_header(),
            PushDecoderState::Block => self.process_block(),
            PushDecoderState::RecordBoundary => self.process_record_boundary(),
            PushDecoderState::EndOfSegment => self.process_end_of_segment(),
            PushDecoderState::Finished => Ok(PushDecoderEvent::Finished),
        }
    }

    /// Resets the decoder state so that a new record can be decoded.
    ///
    /// Configuration is kept but any buffered data is discarded.
    ///
    /// This function may be used after file seeking or
    /// partially reading records.
    pub fn reset(&mut self) -> std::io::Result<()> {
        self.state = PushDecoderState::PendingHeader;
        self.decompressor.get_mut().clear();
        self.deferred_input_buf.clear();
        self.decompressor.start_next_segment()?;
        Ok(())
    }

    fn process_header(&mut self) -> Result<PushDecoderEvent, GeneralError> {
        let buf = self.decompressor.get_mut().make_contiguous();

        if let Some(index) = crate::parse::scan_header_deliminator(buf) {
            let header = self.process_decodable_header(index)?;

            return Ok(PushDecoderEvent::Header { header });
        }

        self.precheck_header()?;
        self.check_max_header_length()?;

        Ok(PushDecoderEvent::WantData)
    }

    fn process_decodable_header(&mut self, index: usize) -> Result<WarcHeader, GeneralError> {
        // Okay to discard slice1 because we called make_contiguous() earlier.
        let (buf, _slice1) = self.decompressor.get_ref().as_slices();

        let header_bytes = &buf[0..index];
        let header = WarcHeader::parse(header_bytes)?;
        let length = header.content_length()?;
        let record_id = header.fields.get("WARC-Record-ID");
        let warc_type = header.fields.get("WARC-Type");

        self.decompressor.get_mut().drain(0..index);
        self.decoded_bytes_consumed += index as u64;

        tracing::trace!(
            record_id,
            warc_type,
            content_length = length,
            "process decodable header"
        );

        self.block_current_position = 0;
        self.block_length = length;

        tracing::trace!("Header -> Block");
        self.state = PushDecoderState::Block;

        Ok(header)
    }

    fn precheck_header(&self) -> Result<(), ProtocolError> {
        // Okay to discard slice1 because we called make_contiguous() earlier.
        let (buf, _slice1) = self.decompressor.get_ref().as_slices();

        match detect_header(buf) {
            HeaderDetectResult::Warc => Ok(()),
            HeaderDetectResult::Compression => {
                Err(ProtocolError::new(ProtocolErrorKind::UnexpectedCompression)
                    .with_position(self.bytes_written_decoder)
                    .with_snippet(buf[0..buf.len().min(16)].escape_ascii().to_string()))
            }
            HeaderDetectResult::NotWarc => {
                Err(ProtocolError::new(ProtocolErrorKind::UnknownHeader)
                    .with_position(self.bytes_written_decoder)
                    .with_snippet(buf[0..buf.len().min(16)].escape_ascii().to_string()))
            }
            HeaderDetectResult::NotSure => Ok(()),
        }
    }

    fn check_max_header_length(&self) -> Result<(), ProtocolError> {
        tracing::trace!("check max header length");

        if self.decompressor.get_ref().len() > MAX_HEADER_LENGTH {
            Err(ProtocolError::new(ProtocolErrorKind::HeaderTooBig))
        } else {
            Ok(())
        }
    }

    fn process_block(&mut self) -> Result<PushDecoderEvent, GeneralError> {
        tracing::trace!(
            self.block_length,
            self.block_current_position,
            "process block"
        );

        debug_assert!(self.block_length >= self.block_current_position);
        let remaining_bytes = self.block_length - self.block_current_position;

        if remaining_bytes == 0 {
            tracing::trace!("Block -> RecordBoundary");
            self.state = PushDecoderState::RecordBoundary;
            Ok(PushDecoderEvent::Continue)
        } else if self.decompressor.get_ref().is_empty() {
            Ok(PushDecoderEvent::WantData)
        } else {
            // Okay to discard slice1 because the caller will continually poll
            // until the buffer is empty.
            let (slice0, _slice1) = self.decompressor.get_ref().as_slices();

            let consume_len = self.buf_output_max_len.min(slice0.len());
            let consume_len = consume_len.min(remaining_bytes.try_into().unwrap_or(usize::MAX));

            self.block_current_position += consume_len as u64;
            self.buf_output_reference_len = consume_len;
            self.decoded_bytes_consumed += consume_len as u64;

            tracing::trace!(consume_len, "process block");

            Ok(PushDecoderEvent::BlockData {
                data: &slice0[0..consume_len],
            })
        }
    }

    fn process_record_boundary(&mut self) -> Result<PushDecoderEvent, GeneralError> {
        tracing::trace!(
            len = self.decompressor.get_ref().len(),
            "process record boundary"
        );

        if self.decompressor.get_ref().len() >= 4 {
            let mut buf = [0u8; 4];
            let mut iter = self.decompressor.get_ref().range(0..4).copied();
            buf[0] = iter.next().unwrap();
            buf[1] = iter.next().unwrap();
            buf[2] = iter.next().unwrap();
            buf[3] = iter.next().unwrap();

            if !buf.starts_with(b"\r\n\r\n") {
                Err(ProtocolError::new(ProtocolErrorKind::InvalidRecordBoundary).into())
            } else {
                self.decompressor.get_mut().drain(0..4);
                self.decoded_bytes_consumed += 4;

                self.state = PushDecoderState::EndOfSegment;
                Ok(PushDecoderEvent::Continue)
            }
        } else {
            Ok(PushDecoderEvent::WantData)
        }
    }

    fn process_end_of_segment(&mut self) -> Result<PushDecoderEvent, GeneralError> {
        tracing::trace!(self.decompressor_eof, "process end of segment");

        if self.config.decompressor.format.supports_concatenation()
            && self.decompressor.get_ref().is_empty()
            && !self.decompressor_eof
            && !self.input_eof
        {
            // Finish reading any end of compression member/frame checksums.
            Ok(PushDecoderEvent::WantData)
        } else {
            self.reset_for_next_record()?;

            Ok(PushDecoderEvent::EndRecord)
        }
    }

    fn reset_for_next_record(&mut self) -> Result<(), GeneralError> {
        tracing::trace!(
            remain_decomp_len = self.decompressor.get_ref().len(),
            "reset for next record"
        );
        // dbg!(String::from_utf8_lossy(self.decompressor.get_ref().as_slices().0));
        // dbg!(String::from_utf8_lossy(self.decompressor.get_ref().as_slices().1));

        if self.config.decompressor.format.is_identity() {
            self.record_boundary_position = self.decoded_bytes_consumed;
        } else {
            self.record_boundary_position = self.bytes_written_decoder;
        }

        if self.config.decompressor.format.supports_concatenation()
            && self.decompressor.get_ref().is_empty()
        {
            tracing::trace!("decompressor start next segment");
            self.decompressor.start_next_segment()?;
        } else if self.config.decompressor.format.supports_concatenation()
            && !self.has_rat_comp_fault
        {
            tracing::warn!("file is not using Record-at-time compression");
            self.has_rat_comp_fault = true;
        }

        self.consume_deferred_input_buf()?;

        self.decompressor_eof = false;

        if self.decompressor.get_ref().is_empty() {
            if self.input_eof {
                tracing::trace!("RecordBoundary -> Finished");
                self.state = PushDecoderState::Finished;
            } else {
                tracing::trace!("RecordBoundary -> PendingHeader");
                self.state = PushDecoderState::PendingHeader;
            }
        } else {
            tracing::trace!("RecordBoundary -> Header");
            self.state = PushDecoderState::Header;
        }

        Ok(())
    }

    fn consume_deferred_input_buf(&mut self) -> Result<(), GeneralError> {
        tracing::trace!(
            len = self.deferred_input_buf.len(),
            "consume deferred input buf"
        );

        while !self.deferred_input_buf.is_empty() {
            let (slice0, _slice1) = self.deferred_input_buf.as_slices();
            let write_len = self.decompressor.write(slice0)?;
            tracing::trace!(write_len, "consume deferred input buf");

            self.bytes_written_decoder += write_len as u64;

            if write_len == 0 {
                break;
            }

            self.deferred_input_buf.drain(..write_len);
        }
        Ok(())
    }

    /// Notify that there is no more input to be decoded.
    pub fn write_eof(&mut self) -> std::io::Result<()> {
        tracing::trace!("push decoder got write eof");
        self.input_eof = true;
        self.decompressor.write_eof()?;
        tracing::trace!(decoded_buf_len = self.decompressor.get_ref().len());

        Ok(())
    }
}

impl Write for PushDecoder {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.state == PushDecoderState::PendingHeader {
            tracing::trace!("PendingHeader -> Header");
            self.state = PushDecoderState::Header;
        }

        let write_len = self.decompressor.write(buf)?;
        self.bytes_written_decoder += write_len as u64;

        tracing::trace!(
            buf_len = buf.len(),
            write_len,
            decoded_buf_len = self.decompressor.get_ref().len(),
            "push decoder write"
        );

        if write_len != 0 {
            // FIXME: handle the case where a single record is compressed as
            // several zstd frames
            Ok(write_len)
        } else {
            self.decompressor_eof = true;
            self.deferred_input_buf.write_all(buf)?;
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.decompressor.flush()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HeaderDetectResult {
    Warc,
    NotWarc,
    Compression,
    NotSure,
}

fn detect_header(buf: &[u8]) -> HeaderDetectResult {
    if buf.starts_with(b"WARC/") {
        HeaderDetectResult::Warc
    } else if buf.starts_with(b"\x1f\x8b") || buf.starts_with(b"\x28\xb5\x2f\xfd") {
        HeaderDetectResult::Compression
    } else if buf.len() >= 5 {
        HeaderDetectResult::NotWarc
    } else {
        HeaderDetectResult::NotSure
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_detect_header() {
        assert_eq!(detect_header(b"WA"), HeaderDetectResult::NotSure);
        assert_eq!(detect_header(b"WARC"), HeaderDetectResult::NotSure);
        assert_eq!(detect_header(b"WARC/"), HeaderDetectResult::Warc);
        assert_eq!(detect_header(b"WARC/1"), HeaderDetectResult::Warc);
        assert_eq!(detect_header(b"AAAAA"), HeaderDetectResult::NotWarc);
        assert_eq!(detect_header(b"AAAAAA"), HeaderDetectResult::NotWarc);
        assert_eq!(detect_header(b"\x1f\x8b"), HeaderDetectResult::Compression);
        assert_eq!(detect_header(b"\x1f\x8b "), HeaderDetectResult::Compression);
        assert_eq!(
            detect_header(b"\x28\xb5\x2f\xfd"),
            HeaderDetectResult::Compression
        );
        assert_eq!(detect_header(b"\x28\xb5"), HeaderDetectResult::NotSure);
    }

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

    #[tracing_test::traced_test]
    #[test]
    fn test_push_reader() {
        let _data = b"WARC/1.1\r\n\
            Content-Length: 12\r\n\
            \r\n\
            Hello world!\
            \r\n\r\n\
            WARC/1.1\r\n\
            Content-Length: 0\r\n\
            \r\n\
            \r\n\r\n";

        let mut decoder = PushDecoder::new(DecoderConfig::default()).unwrap();

        let event = decoder.get_event().unwrap();
        assert!(event.is_ready());

        decoder.write_all(b"WARC/1.1\r\n").unwrap(); // len = 10

        let event = decoder.get_event().unwrap();
        assert!(event.is_want_data());

        decoder.write_all(b"Content-Length: 12\r\n").unwrap(); // len = 20
        decoder.write_all(b"\r\n").unwrap(); // len = 2
        decoder.write_all(b"Hello ").unwrap(); // len = 6

        let event = decoder.get_event().unwrap();
        assert!(event.is_header());
        assert_eq!(decoder.record_boundary_position(), 0);

        let event = decoder.get_event().unwrap();
        assert!(event.is_block_data());
        assert_eq!(event.as_block_data().unwrap(), b"Hello ");

        let event = decoder.get_event().unwrap();
        assert!(event.is_want_data());

        decoder.write_all(b"world!\r\n").unwrap(); // len = 8

        let event = decoder.get_event().unwrap();
        assert!(event.is_block_data());
        assert_eq!(event.as_block_data().unwrap(), b"world!");

        let event = decoder.get_event().unwrap();
        assert!(event.is_continue());

        let event = decoder.get_event().unwrap();
        assert!(event.is_want_data());

        decoder.write_all(b"\r\n").unwrap(); // len = 2
        decoder.write_all(b"WARC/1.1\r\n").unwrap();

        let event = decoder.get_event().unwrap();
        assert!(event.is_continue());

        let event = decoder.get_event().unwrap();
        assert!(event.is_end_record());

        let event = decoder.get_event().unwrap();
        assert!(event.is_want_data());

        decoder
            .write_all(
                b"Content-Length: 0\r\n\
                \r\n\
                \r\n\r\n",
            )
            .unwrap();

        decoder.write_eof().unwrap();

        let event = decoder.get_event().unwrap();
        assert!(event.is_header());
        assert_eq!(decoder.record_boundary_position(), 48);

        let event = decoder.get_event().unwrap();
        assert!(event.is_continue());

        let event = decoder.get_event().unwrap();
        assert!(event.is_continue());

        let event = decoder.get_event().unwrap();
        assert!(event.is_end_record());

        let event = decoder.get_event().unwrap();
        assert!(event.is_finished());
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_wrong_format() {
        let data = b"CDX\r\n";
        let reader = Decoder::new(Cursor::new(data), DecoderConfig::default()).unwrap();

        let error = reader.read_header().unwrap_err();
        dbg!(&error);
        assert_eq!(
            error.as_protocol().unwrap().kind(),
            ProtocolErrorKind::UnknownHeader
        );
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_unexpected_compression() {
        let data = b"\x1f\x8babc";
        let reader = Decoder::new(Cursor::new(data), DecoderConfig::default()).unwrap();

        let error = reader.read_header().unwrap_err();
        dbg!(&error);
        assert_eq!(
            error.as_protocol().unwrap().kind(),
            ProtocolErrorKind::UnexpectedCompression
        );
    }
}
