use std::fmt::Debug;
use std::io::Write;

use chunked::{ChunkedDecoder, ChunkedEncoder};
use compress::{CompressionDecoder, CompressionEncoder};

use crate::error::GeneralError;

use super::header::{fields::FieldsExt, MessageHeader};

pub mod chunked;
pub mod compress;

pub type BoxedCodec<W> = Box<dyn Codec<W>>;

pub trait Codec<W: Write>: Debug {
    fn transform(&mut self, input: &[u8], output: W) -> Result<(), GeneralError>;
}

pub fn build_decoders<W: Write>(header: &MessageHeader, codecs: &mut Vec<BoxedCodec<W>>) {
    build_codecs(header, codecs, false);
}

pub fn build_encoders<W: Write>(header: &MessageHeader, codecs: &mut Vec<BoxedCodec<W>>) {
    build_codecs(header, codecs, true);
}

fn build_codecs<W: Write>(header: &MessageHeader, codecs: &mut Vec<BoxedCodec<W>>, encode: bool) {
    let names_1 = header.fields.get_comma_list("transfer-encoding");
    let names_2 = header.fields.get_comma_list("content-encoding");

    for name in names_1.chain(names_2) {
        if name == "identity" || name == "*" {
            continue;
        }
        #[allow(clippy::collapsible_else_if)]
        if encode {
            if let Ok(codec) = CompressionEncoder::try_of_name(&name) {
                codecs.push(Box::new(codec));
            } else if name == "chunked" {
                codecs.push(Box::new(ChunkedEncoder::new()))
            }
        } else {
            if let Ok(codec) = CompressionDecoder::try_of_name(&name) {
                codecs.push(Box::new(codec));
            } else if name == "chunked" {
                codecs.push(Box::new(ChunkedDecoder::new()))
            }
        }
    }
}
