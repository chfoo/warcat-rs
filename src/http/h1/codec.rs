use std::io::Write;

use chunked::{ChunkedDecoder, ChunkedEncoder};
use compress::{CompressionDecoder, CompressionEncoder};

use super::{
    error::IoProtocolError,
    header::{fields::FieldsExt, MessageHeader},
};

pub mod chunked;
pub mod compress;

pub type BoxedCodec<W> = Box<dyn Codec<W>>;

pub trait Codec<W: Write> {
    fn transform(&mut self, input: &[u8], output: W) -> Result<(), IoProtocolError>;
}

pub fn build_decoders<W: Write>(header: &MessageHeader, codecs: &mut Vec<BoxedCodec<W>>) {
    let mut names = Vec::new();

    header
        .fields
        .get_comma_list("transfer-encoding", &mut names);
    header.fields.get_comma_list("content-encoding", &mut names);

    for name in &names {
        if name == "identity" || name == "*" {
            continue;
        }

        if let Ok(codec) = CompressionDecoder::try_of_name(name) {
            codecs.push(Box::new(codec));
        } else if name == "chunked" {
            codecs.push(Box::new(ChunkedDecoder::new()))
        }
    }
}

pub fn build_encoders<W: Write>(header: &MessageHeader, codecs: &mut Vec<BoxedCodec<W>>) {
    let mut names = Vec::new();

    header
        .fields
        .get_comma_list("transfer-encoding", &mut names);
    header.fields.get_comma_list("content-encoding", &mut names);

    for name in &names {
        if name == "identity" || name == "*" {
            continue;
        }

        if let Ok(codec) = CompressionEncoder::try_of_name(name) {
            codecs.push(Box::new(codec));
        } else if name == "chunked" {
            codecs.push(Box::new(ChunkedEncoder::new()))
        }
    }
}
