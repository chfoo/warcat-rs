use std::{fmt::Debug, io::Write};

use chunked::{ChunkedDecoder, ChunkedEncoder};
use compress::{CompressionDecoder, CompressionEncoder};

use crate::error::{GeneralError, ProtocolError, ProtocolErrorKind};

use super::header::{fields::FieldsExt, MessageHeader};

pub mod chunked;
pub mod compress;

pub type BoxedCodec = Box<dyn Codec>;

pub trait Codec: Debug {
    fn transform(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), GeneralError>;

    fn finish_input(&mut self, output: &mut Vec<u8>) -> Result<(), GeneralError> {
        let _ = output;

        Ok(())
    }

    /// Returns whether if there is buffered data containing
    /// the Trailer portion of Chunked-Transfer Encoding.
    ///
    /// (A out-of-band data function.)
    fn has_remaining_trailer(&self) -> bool {
        false
    }

    /// Writes buffered data containing the Trailer portion of Chunked-Transfer Encoding.
    ///
    /// (A out-of-band data function.)
    fn remaining_trailer(&mut self, trailer: &mut Vec<u8>) {
        let _ = trailer;
    }
}

#[derive(Debug, Default)]
pub struct IdentityCodec;

impl Codec for IdentityCodec {
    fn transform(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), GeneralError> {
        output.write_all(input)?;
        Ok(())
    }
}

pub fn build_decoders(
    header: &MessageHeader,
    codecs: &mut Vec<BoxedCodec>,
) -> Result<(), ProtocolError> {
    build_codecs(header, codecs, false)
}

pub fn build_encoders(
    header: &MessageHeader,
    codecs: &mut Vec<BoxedCodec>,
) -> Result<(), ProtocolError> {
    build_codecs(header, codecs, true)
}

fn build_codecs(
    header: &MessageHeader,
    codecs: &mut Vec<BoxedCodec>,
    encode: bool,
) -> Result<(), ProtocolError> {
    let mut te_names = header
        .fields
        .get_comma_list("transfer-encoding")
        .collect::<Vec<_>>();
    let ce_names = header.fields.get_comma_list("content-encoding");

    te_names.reverse();
    for name in te_names {
        if encode {
            if let Some(codec) = make_encoder(name.as_ref(), true) {
                codecs.push(codec);
                continue;
            }
        } else if let Some(codec) = make_decoder(name.as_ref(), true) {
            codecs.push(codec);
            continue;
        }
        return Err(ProtocolError::new(
            ProtocolErrorKind::UnsupportedTransferEncoding,
        ));
    }

    for name in ce_names {
        if name == "identity" {
            continue;
        }

        if encode {
            if let Some(codec) = make_encoder(name.as_ref(), false) {
                codecs.push(codec);
                continue;
            }
        } else if let Some(codec) = make_decoder(name.as_ref(), false) {
            codecs.push(codec);
            continue;
        }
        return Err(ProtocolError::new(
            ProtocolErrorKind::UnsupportedContentEncoding,
        ));
    }

    Ok(())
}

fn make_encoder(name: &str, transfer_encoding: bool) -> Option<BoxedCodec> {
    if let Ok(codec) = CompressionEncoder::try_of_name(name) {
        tracing::trace!(name, "built compression encoder");
        Some(Box::new(codec))
    } else if name == "chunked" && transfer_encoding {
        tracing::trace!(name, "built chunked encoder");
        Some(Box::new(ChunkedEncoder::new()))
    } else {
        None
    }
}

fn make_decoder(name: &str, transfer_encoding: bool) -> Option<BoxedCodec> {
    if let Ok(codec) = CompressionDecoder::try_of_name(name) {
        tracing::trace!(name, "built compression decoder");
        Some(Box::new(codec))
    } else if name == "chunked" && transfer_encoding {
        tracing::trace!(name, "built chunked decoder");
        Some(Box::new(ChunkedDecoder::new()))
    } else {
        None
    }
}

#[derive(Debug, Default)]
pub struct CodecPipeline {
    codecs: Vec<BoxedCodec>,
    buf_in: Vec<u8>,
    buf_out: Vec<u8>,
}

impl CodecPipeline {
    pub fn new(codecs: Vec<BoxedCodec>) -> Self {
        Self {
            codecs,
            buf_in: Vec::new(),
            buf_out: Vec::new(),
        }
    }

    pub fn transform<W: Write>(&mut self, input: &[u8], mut output: W) -> Result<(), GeneralError> {
        if self.codecs.is_empty() {
            output.write_all(input)?;
            return Ok(());
        }

        self.buf_in.extend_from_slice(input);

        for codec in &mut self.codecs {
            codec.transform(&self.buf_in, &mut self.buf_out)?;

            self.buf_in.clear();
            std::mem::swap(&mut self.buf_in, &mut self.buf_out);
        }

        output.write_all(&self.buf_in)?;

        self.buf_in.clear();

        Ok(())
    }

    pub fn finish_input<W: Write>(&mut self, mut output: W) -> Result<(), GeneralError> {
        if self.codecs.is_empty() {
            return Ok(());
        }

        for codec in &mut self.codecs {
            codec.finish_input(&mut self.buf_out)?;

            self.buf_in.clear();
            std::mem::swap(&mut self.buf_in, &mut self.buf_out);
        }

        output.write_all(&self.buf_in)?;
        self.buf_in.clear();

        Ok(())
    }

    pub fn has_remaining_trailer(&self) -> bool {
        self.codecs
            .iter()
            .any(|codec| codec.has_remaining_trailer())
    }

    pub fn remaining_trailer(&mut self, trailer: &mut Vec<u8>) {
        for codec in &mut self.codecs {
            codec.remaining_trailer(trailer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_pipeline() {
        let mut pipeline = CodecPipeline::new(vec![
            Box::new(IdentityCodec),
            Box::new(IdentityCodec),
            Box::new(IdentityCodec),
        ]);
        let mut output = Vec::new();

        pipeline.transform(b"a", &mut output).unwrap();
        pipeline.transform(b"b", &mut output).unwrap();
        pipeline.transform(b"c", &mut output).unwrap();

        assert_eq!(&output, b"abc");
    }
}
