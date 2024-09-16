//! Streams of serialized values.
use std::io::{BufRead, Write};

use serde::{de::DeserializeOwned, Serialize};

const RS: u8 = b'\x1e';
const RS_SEQ: &[u8] = b"\x1e";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqFormat {
    /// JSON sequences (RFC 7464)
    JsonSeq,
    /// CBOR sequences (RFC 8742)
    CborSeq,
}

pub struct SeqReader<R: BufRead> {
    buf: Vec<u8>,
    format: SeqFormat,
    input: R,
}

impl<R: BufRead> SeqReader<R> {
    pub fn new(input: R, format: SeqFormat) -> Self {
        Self {
            buf: Vec::new(),
            format,
            input,
        }
    }

    pub fn get_ref(&self) -> &R {
        &self.input
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.input
    }

    pub fn into_inner(self) -> R {
        self.input
    }

    pub fn get<T: DeserializeOwned>(&mut self) -> anyhow::Result<Option<T>> {
        // TODO: remove anyhow from public API
        match self.format {
            SeqFormat::JsonSeq => self.read_json(),
            SeqFormat::CborSeq => self.read_cbor(),
        }
    }

    fn read_json<T: DeserializeOwned>(&mut self) -> anyhow::Result<Option<T>> {
        loop {
            let read_len = self.input.read_until(RS, &mut self.buf)?;

            if read_len == 0 {
                return Ok(None);
            }

            if self.buf.ends_with(&[RS]) {
                self.buf.truncate(self.buf.len() - 1)
            }

            if self.buf.is_empty() {
                continue;
            }

            let message = serde_json::de::from_slice(&self.buf)?;

            self.buf.clear();

            return Ok(Some(message));
        }
    }

    fn read_cbor<T: DeserializeOwned>(&mut self) -> anyhow::Result<Option<T>> {
        if self.input.fill_buf()?.is_empty() {
            return Ok(None);
        }

        let message = ciborium::from_reader(&mut self.input)?;

        Ok(Some(message))
    }
}

pub struct SeqWriter<W: Write> {
    format: SeqFormat,
    pretty: bool,
    output: W,
}

impl<W: Write> SeqWriter<W> {
    pub fn new(output: W, format: SeqFormat) -> Self {
        Self {
            format,
            pretty: false,
            output,
        }
    }

    pub fn with_pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    pub fn get_ref(&self) -> &W {
        &self.output
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.output
    }

    pub fn into_inner(self) -> W {
        self.output
    }

    pub fn put<T: Serialize>(&mut self, value: T) -> std::io::Result<()> {
        match self.format {
            SeqFormat::JsonSeq => self.write_json(value),
            SeqFormat::CborSeq => self.write_cbor(value),
        }
    }

    fn write_json<T: Serialize>(&mut self, value: T) -> std::io::Result<()> {
        self.output.write_all(RS_SEQ)?;
        if self.pretty {
            serde_json::ser::to_writer_pretty(&mut self.output, &value)?;
        } else {
            serde_json::ser::to_writer(&mut self.output, &value)?;
        }
        self.output.write_all(b"\n")?;

        Ok(())
    }

    fn write_cbor<T: Serialize>(&mut self, value: T) -> std::io::Result<()> {
        ciborium::into_writer(&value, &mut self.output).map_err(|e| match e {
            ciborium::ser::Error::Io(e) => e,
            ciborium::ser::Error::Value(e) => std::io::Error::other(e),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::*;

    #[test]
    fn test_seq_reader_json() {
        let input = BufReader::new(Cursor::new(b"\x1e123\n\x1e456\n"));
        let mut reader = SeqReader::new(input, SeqFormat::JsonSeq);

        let item = reader.get::<i32>().unwrap();
        assert_eq!(item, Some(123));

        let item = reader.get::<i32>().unwrap();
        assert_eq!(item, Some(456));

        let item = reader.get::<i32>().unwrap();
        assert_eq!(item, None);
    }

    #[test]
    fn test_seq_reader_cbor() {
        let input = BufReader::new(Cursor::new(b"\x18\x7b\x19\x01\xC8"));
        let mut reader = SeqReader::new(input, SeqFormat::CborSeq);

        let item = reader.get::<i32>().unwrap();
        assert_eq!(item, Some(123));

        let item = reader.get::<i32>().unwrap();
        assert_eq!(item, Some(456));

        let item = reader.get::<i32>().unwrap();
        assert_eq!(item, None);
    }
}
