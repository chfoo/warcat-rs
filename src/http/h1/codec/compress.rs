use std::io::Write;
use std::str::FromStr;

use crate::{
    compress::{Compressor, Format as CompressionFormat, PushDecompressor},
    error::{GeneralError, ProtocolError, ProtocolErrorKind},
};

use super::Codec;

#[derive(Debug)]
pub struct CompressionEncoder {
    compressor: Compressor<Vec<u8>>,
}

impl CompressionEncoder {
    pub fn new(compressor: Compressor<Vec<u8>>) -> Self {
        Self { compressor }
    }

    pub fn try_of_name(name: &str) -> Result<Self, ProtocolError> {
        let format = CompressionFormat::from_str(name)
            .map_err(|_| ProtocolError::new(ProtocolErrorKind::UnsupportedCompressionFormat))?;

        Ok(Self::new(Compressor::new(Vec::new(), format)))
    }
}

impl<W: Write> Codec<W> for CompressionEncoder {
    fn transform(&mut self, input: &[u8], mut output: W) -> Result<(), GeneralError> {
        self.compressor.write_all(input)?;

        output.write_all(self.compressor.get_ref())?;
        self.compressor.get_mut().clear();

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

impl<W: Write> Codec<W> for CompressionDecoder {
    fn transform(&mut self, input: &[u8], mut output: W) -> Result<(), GeneralError> {
        self.decompressor.write_all(input)?;

        output.write_all(self.decompressor.get_ref())?;
        self.decompressor.get_mut().clear();

        Ok(())
    }
}
