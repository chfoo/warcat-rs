use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use crate::error::{GeneralError, ProtocolError, ProtocolErrorKind};

use super::Codec;

#[derive(Debug)]
pub struct ChunkedEncoder {}

impl ChunkedEncoder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }
}

impl Codec for ChunkedEncoder {
    fn transform(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), GeneralError> {
        if !input.is_empty() {
            write!(output, "{:x}\r\n", input.len())?;
            output.write_all(input)?;
            output.write_all(b"\r\n")?;
        }

        Ok(())
    }

    fn finish_input(&mut self, output: &mut Vec<u8>) -> Result<(), GeneralError> {
        output.extend_from_slice("0\r\n\r\n".as_bytes());

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopState {
    Continue,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkedDecoderState {
    SizeLine,
    ChunkData,
    Boundary,
    Done,
}

#[derive(Debug)]
pub struct ChunkedDecoder {
    state: ChunkedDecoderState,
    buf: VecDeque<u8>,
    chunk_len: u64,
    chunk_position: u64,
}

impl ChunkedDecoder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            state: ChunkedDecoderState::SizeLine,
            buf: VecDeque::new(),
            chunk_len: 0,
            chunk_position: 0,
        }
    }

    fn process_size_line(&mut self) -> Result<LoopState, GeneralError> {
        let buf_len = self.buf.len();

        match parse::chunk_size_line(self.buf.make_contiguous()) {
            Ok((remain, len)) => {
                self.chunk_len = len;
                self.chunk_position = 0;
                self.state = ChunkedDecoderState::ChunkData;

                let consumed_len = buf_len - remain.len();

                self.buf.drain(..consumed_len);
                tracing::trace!(len, consumed_len, "parsed chunk line");

                if self.chunk_len == 0 {
                    self.state = ChunkedDecoderState::Done;
                }

                Ok(LoopState::Continue)
            }
            Err(error) => match error {
                nom::Err::Incomplete(_needed) => {
                    self.state = ChunkedDecoderState::SizeLine;

                    Ok(LoopState::Break)
                }
                nom::Err::Error(_) => {
                    Err(ProtocolError::new(ProtocolErrorKind::InvalidChunkedEncoding).into())
                }
                nom::Err::Failure(_) => {
                    Err(ProtocolError::new(ProtocolErrorKind::InvalidChunkedEncoding).into())
                }
            },
        }
    }

    fn process_chunk(&mut self, output: &mut Vec<u8>) -> Result<LoopState, GeneralError> {
        debug_assert!(self.chunk_position <= self.chunk_len);

        let chunk_remain_len = self.chunk_len - self.chunk_position;

        let mut reader = (&mut self.buf).take(chunk_remain_len);
        let len = std::io::copy(&mut reader, output)?;

        self.chunk_position += len;

        tracing::trace!(self.chunk_position, self.chunk_len, "process chunk data");

        if self.chunk_position == self.chunk_len {
            self.state = ChunkedDecoderState::Boundary;
        }

        Ok(LoopState::Continue)
    }

    fn process_boundary(&mut self) -> Result<LoopState, GeneralError> {
        match parse::chunk_boundary(self.buf.make_contiguous()) {
            Ok((_remain, consumed)) => {
                let len = consumed.len();
                self.buf.drain(0..len);

                self.state = ChunkedDecoderState::SizeLine;

                tracing::trace!("chunk boundary");

                Ok(LoopState::Continue)
            }
            Err(error) => match error {
                nom::Err::Incomplete(_needed) => Ok(LoopState::Break),
                nom::Err::Error(_) => {
                    Err(ProtocolError::new(ProtocolErrorKind::InvalidChunkedEncoding).into())
                }
                nom::Err::Failure(_) => {
                    Err(ProtocolError::new(ProtocolErrorKind::InvalidChunkedEncoding).into())
                }
            },
        }
    }
}

impl Codec for ChunkedDecoder {
    fn transform(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), GeneralError> {
        self.buf.write_all(input)?;

        loop {
            let loop_state = match self.state {
                ChunkedDecoderState::SizeLine => self.process_size_line()?,
                ChunkedDecoderState::ChunkData => self.process_chunk(output)?,
                ChunkedDecoderState::Boundary => self.process_boundary()?,
                ChunkedDecoderState::Done => LoopState::Break,
            };

            if self.buf.is_empty() || loop_state == LoopState::Break {
                break;
            }
        }

        Ok(())
    }

    fn has_remaining_trailer(&self) -> bool {
        !self.buf.is_empty()
    }

    fn remaining_trailer(&mut self, trailer: &mut Vec<u8>) {
        tracing::trace!(len = self.buf.len(), "remaining trailer");

        std::io::copy(&mut self.buf, trailer).unwrap();
    }
}

mod parse {
    use core::str;

    use nom::{
        bytes::streaming::{tag, take_while},
        character::streaming::{hex_digit1, line_ending},
        combinator::map,
        sequence::{pair, terminated},
        IResult,
    };

    pub fn chunk_size_line(input: &[u8]) -> IResult<&[u8], u64> {
        terminated(map(pair(chunk_size, chunk_ext), |p| p.0), tag(b"\r\n"))(input)
    }

    fn chunk_size(input: &[u8]) -> IResult<&[u8], u64> {
        map(hex_digit1, |b: &[u8]| {
            u64::from_str_radix(str::from_utf8(b).unwrap(), 16).unwrap()
        })(input)
    }

    fn chunk_ext(input: &[u8]) -> IResult<&[u8], &[u8]> {
        take_while(|b: u8| b.is_ascii_graphic() || b == b' ' || b == b'\t')(input)
    }

    pub fn chunk_boundary(input: &[u8]) -> IResult<&[u8], &[u8]> {
        line_ending(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tracing_test::traced_test]
    #[test]
    fn test_encode() {
        let mut encoder = ChunkedEncoder::new();
        let mut output = Vec::new();

        encoder.transform(b"Hello world!", &mut output).unwrap();
        encoder.finish_input(&mut output).unwrap();

        assert_eq!(
            output,
            b"c\r\n\
            Hello world!\
            \r\n\
            0\r\n\
            \r\n"
        );
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_decode() {
        let mut decoder = ChunkedDecoder::new();

        let mut output = Vec::new();

        decoder.transform(b"6\r\n", &mut output).unwrap();
        decoder.transform(b"Hello ", &mut output).unwrap();
        decoder.transform(b"\r\n", &mut output).unwrap();
        decoder.transform(b"6\r\n", &mut output).unwrap();
        decoder.transform(b"world!", &mut output).unwrap();
        decoder.transform(b"\r\n", &mut output).unwrap();
        decoder.transform(b"0\r\n", &mut output).unwrap();
        decoder.transform(b"a: b\r\n", &mut output).unwrap();
        decoder.transform(b"\r\n", &mut output).unwrap();

        assert_eq!(output, b"Hello world!");

        assert!(decoder.has_remaining_trailer());
        let mut trailer = Vec::new();
        decoder.remaining_trailer(&mut trailer);

        assert_eq!(trailer, b"a: b\r\n\r\n");
        assert!(!decoder.has_remaining_trailer());
    }
}
