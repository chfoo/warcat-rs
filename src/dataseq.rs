//! Streams of serialized values.
use std::io::{BufRead, Write};

use serde::{de::DeserializeOwned, Serialize};

const RS: u8 = b'\x1e';
const RS_SEQ: &[u8] = b"\x1e";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeqFormat {
    /// JSON sequences (RFC 7464)
    JsonSeq,
    /// JSON lines
    JsonL,
    /// CBOR sequences (RFC 8742)
    CborSeq,
    /// Comma separated value
    Csv,
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

    pub fn get<T: DeserializeOwned>(&mut self) -> Result<Option<T>, SeqError> {
        match self.format {
            SeqFormat::JsonSeq => self.read_json(),
            SeqFormat::JsonL => self.read_json_lines(),
            SeqFormat::CborSeq => self.read_cbor(),
            SeqFormat::Csv => todo!(),
        }
    }

    fn read_json<T: DeserializeOwned>(&mut self) -> Result<Option<T>, SeqError> {
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

    fn read_json_lines<T: DeserializeOwned>(&mut self) -> Result<Option<T>, SeqError> {
        let read_len = self.input.read_until(b'\n', &mut self.buf)?;

        if read_len == 0 {
            return Ok(None);
        }

        let message = serde_json::de::from_slice(&self.buf)?;

        self.buf.clear();

        Ok(Some(message))
    }

    fn read_cbor<T: DeserializeOwned>(&mut self) -> Result<Option<T>, SeqError> {
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

    pub fn put<T: Serialize>(&mut self, value: T) -> Result<(), SeqError> {
        match self.format {
            SeqFormat::JsonSeq => self.write_json(value),
            SeqFormat::JsonL => self.write_json_lines(value),
            SeqFormat::CborSeq => self.write_cbor(value),
            SeqFormat::Csv => self.write_csv(value),
        }
    }

    fn write_json<T: Serialize>(&mut self, value: T) -> Result<(), SeqError> {
        self.output.write_all(RS_SEQ)?;

        if self.pretty {
            serde_json::to_writer_pretty(&mut self.output, &value)?;
        } else {
            serde_json::to_writer(&mut self.output, &value)?;
        }

        self.output.write_all(b"\n")?;

        Ok(())
    }

    fn write_json_lines<T: Serialize>(&mut self, value: T) -> Result<(), SeqError> {
        serde_json::to_writer(&mut self.output, &value)?;

        self.output.write_all(b"\n")?;

        Ok(())
    }

    fn write_cbor<T: Serialize>(&mut self, value: T) -> Result<(), SeqError> {
        ciborium::into_writer(&value, &mut self.output)?;
        Ok(())
    }

    fn write_csv<T: Serialize>(&mut self, value: T) -> Result<(), SeqError> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_writer(&mut self.output);

        writer.serialize(value)?;
        writer.flush()?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SeqError {
    #[error("sequence serde error: {0}")]
    Serde(Box<dyn std::error::Error + Send + Sync>),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<serde_json::Error> for SeqError {
    fn from(value: serde_json::Error) -> Self {
        if let Some(error) = value.io_error_kind() {
            Self::Io(error.into())
        } else {
            Self::Serde(Box::new(value))
        }
    }
}

impl From<ciborium::de::Error<std::io::Error>> for SeqError {
    fn from(value: ciborium::de::Error<std::io::Error>) -> Self {
        if let ciborium::de::Error::Io(error) = value {
            Self::Io(error)
        } else {
            Self::Serde(Box::new(value))
        }
    }
}

impl From<ciborium::ser::Error<std::io::Error>> for SeqError {
    fn from(value: ciborium::ser::Error<std::io::Error>) -> Self {
        if let ciborium::ser::Error::Io(error) = value {
            Self::Io(error)
        } else {
            Self::Serde(Box::new(value))
        }
    }
}

impl From<csv::Error> for SeqError {
    fn from(value: csv::Error) -> Self {
        if value.is_io_error() {
            if let csv::ErrorKind::Io(error) = value.into_kind() {
                Self::Io(error)
            } else {
                unreachable!()
            }
        } else {
            Self::Serde(Box::new(value))
        }
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
