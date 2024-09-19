use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use crate::http::h1::error::{IoProtocolError, ProtocolError};

use super::Codec;

pub struct ChunkedEncoder {}

impl ChunkedEncoder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }
}

impl<W: Write> Codec<W> for ChunkedEncoder {
    fn transform(&mut self, input: &[u8], mut output: W) -> Result<(), IoProtocolError> {
        write!(output, "{:x}\r\n", input.len())?;
        output.write_all(input)?;
        output.write_all(b"\r\n")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkedDecoderState {
    SizeLine,
    ChunkData,
    Boundary,
    Done,
}

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

    fn process_size_line(&mut self) -> Result<bool, IoProtocolError> {
        let buf_len = self.buf.len();

        match parse::chunk(self.buf.make_contiguous()) {
            Ok((remain, len)) => {
                self.chunk_len = len;
                self.chunk_position = remain.len() as u64;
                self.state = ChunkedDecoderState::ChunkData;

                let consumed_len = buf_len - remain.len();

                self.buf.drain(..consumed_len);

                Ok(false)
            }
            Err(error) => match error {
                nom::Err::Incomplete(_needed) => {
                    self.state = ChunkedDecoderState::SizeLine;

                    Ok(true)
                }
                nom::Err::Error(_) => Err(ProtocolError::InvalidChunkedEncoding.into()),
                nom::Err::Failure(_) => Err(ProtocolError::InvalidChunkedEncoding.into()),
            },
        }
    }

    fn process_chunk<W: Write>(&mut self, output: &mut W) -> Result<bool, IoProtocolError> {
        debug_assert!(self.chunk_position <= self.chunk_len);

        let chunk_remain_len = self.chunk_len - self.chunk_position;

        let mut reader = (&mut self.buf).take(chunk_remain_len);
        let len = std::io::copy(&mut reader, output)?;

        self.chunk_position += len;

        if self.chunk_position == self.chunk_len {
            self.state = ChunkedDecoderState::Boundary;
        }

        Ok(false)
    }

    fn process_boundary(&mut self) -> Result<bool, IoProtocolError> {
        match parse::chunk_boundary(self.buf.make_contiguous()) {
            Ok((_remain, consumed)) => {
                let len = consumed.len();
                self.buf.drain(0..len);

                if self.chunk_len == 0 {
                    self.state = ChunkedDecoderState::Done;
                } else {
                    self.state = ChunkedDecoderState::SizeLine;
                }

                Ok(false)
            }
            Err(error) => match error {
                nom::Err::Incomplete(_needed) => Ok(true),
                nom::Err::Error(_) => Err(ProtocolError::InvalidChunkedEncoding.into()),
                nom::Err::Failure(_) => Err(ProtocolError::InvalidChunkedEncoding.into()),
            },
        }
    }
}

impl<W: Write> Codec<W> for ChunkedDecoder {
    fn transform(&mut self, input: &[u8], mut output: W) -> Result<(), IoProtocolError> {
        self.buf.write_all(input)?;

        // FIXME: handle trailer

        loop {
            let do_break = match self.state {
                ChunkedDecoderState::SizeLine => self.process_size_line()?,
                ChunkedDecoderState::ChunkData => self.process_chunk(&mut output)?,
                ChunkedDecoderState::Boundary => self.process_boundary()?,
                ChunkedDecoderState::Done => return Err(ProtocolError::LastChunk.into()),
            };

            if self.buf.is_empty() || do_break {
                break;
            }
        }

        Ok(())
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

    pub fn chunk(input: &[u8]) -> IResult<&[u8], u64> {
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

    #[test]
    fn test_encode() {
        let mut encoder = ChunkedEncoder::new();
        let mut output = Vec::new();

        encoder.transform(b"Hello world!", &mut output).unwrap();
        encoder.transform(b"", &mut output).unwrap();

        assert_eq!(
            output,
            b"c\r\n\
            Hello world!\
            \r\n\
            0\r\n\
            \r\n"
        );
    }

    #[test]
    fn test_decode() {
        let mut decoder = ChunkedDecoder::new();

        let mut output = Vec::new();

        decoder.transform(b"c", &mut output).unwrap();
        decoder.transform(b"\r\n", &mut output).unwrap();
        decoder.transform(b"Hello world!", &mut output).unwrap();
        decoder.transform(b"\r\n", &mut output).unwrap();
        decoder.transform(b"0\r\n", &mut output).unwrap();
        decoder.transform(b"\r\n", &mut output).unwrap();

        assert_eq!(output, b"Hello world!");
    }
}
