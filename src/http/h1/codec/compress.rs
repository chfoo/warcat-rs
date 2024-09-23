use std::io::Write;
use std::str::FromStr;

use crate::{
    compress::{Compressor, Format as CompressionFormat, PushDecompressor},
    error::{GeneralError, ProtocolError, ProtocolErrorKind},
};

use super::Codec;

#[derive(Debug)]
pub struct CompressionEncoder {
    compressor: Option<Compressor<Vec<u8>>>,
}

impl CompressionEncoder {
    pub fn new(compressor: Compressor<Vec<u8>>) -> Self {
        Self {
            compressor: Some(compressor),
        }
    }

    pub fn try_of_name(name: &str) -> Result<Self, ProtocolError> {
        let format = CompressionFormat::from_str(name)
            .map_err(|_| ProtocolError::new(ProtocolErrorKind::UnsupportedCompressionFormat))?;

        Ok(Self::new(Compressor::new(Vec::new(), format)))
    }
}

impl Codec for CompressionEncoder {
    fn transform(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), GeneralError> {
        if let Some(compressor) = &mut self.compressor {
            compressor.write_all(input)?;

            output.extend_from_slice(compressor.get_ref());
            compressor.get_mut().clear();
        }

        Ok(())
    }

    fn finish_input(&mut self, output: &mut Vec<u8>) -> Result<(), GeneralError> {
        if let Some(mut compressor) = self.compressor.take() {
            compressor.flush()?;

            let buf = compressor.finish()?;

            output.extend_from_slice(&buf);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct CompressionDecoder {
    decompressor: PushDecompressor<Vec<u8>>,
}

impl CompressionDecoder {
    pub fn new(decompressor: PushDecompressor<Vec<u8>>) -> Self {
        Self { decompressor }
    }

    pub fn try_of_name(name: &str) -> Result<Self, GeneralError> {
        let format = CompressionFormat::from_str(name)
            .map_err(|_| ProtocolError::new(ProtocolErrorKind::UnsupportedCompressionFormat))?;

        Ok(Self::new(PushDecompressor::new(Vec::new(), format)?))
    }
}

impl Codec for CompressionDecoder {
    fn transform(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), GeneralError> {
        self.decompressor.write_all(input)?;
        self.decompressor.flush()?;

        output.extend_from_slice(self.decompressor.get_ref());
        self.decompressor.get_mut().clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression() {
        let mut encoder = CompressionEncoder::try_of_name("gzip").unwrap();
        let mut buf = Vec::new();

        encoder.transform(b"Hello world!", &mut buf).unwrap();
        encoder.finish_input(&mut buf).unwrap();

        let mut output = Vec::new();

        let mut decoder = CompressionDecoder::try_of_name("gzip").unwrap();
        decoder.transform(&buf, &mut output).unwrap();

        assert_eq!(&output, b"Hello world!");
    }
}
