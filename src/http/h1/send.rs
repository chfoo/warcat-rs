use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use crate::error::GeneralError;

use super::{
    codec::BoxedCodec,
    header::{HeaderFields, MessageHeader},
};

/// Encodes a HTTP request/response session
///
/// Important: This struct makes no semantic validation!
pub struct Sender {
    codecs: Vec<BoxedCodec<Vec<u8>>>,
    codec_buf: Vec<u8>,
    codec_buf2: Vec<u8>,
    output_buf: VecDeque<u8>,
}

impl Sender {
    pub fn new() -> Self {
        Self {
            codecs: Vec::new(),
            codec_buf: Vec::new(),
            codec_buf2: Vec::new(),
            output_buf: VecDeque::new(),
        }
    }

    pub fn send_header(&mut self, header: &MessageHeader) {
        super::codec::build_encoders(header, &mut self.codecs);

        header.serialize(&mut self.output_buf).unwrap();
    }

    pub fn send_body(&mut self, data: &[u8]) -> Result<(), GeneralError> {
        // TODO: transform the data
        for codec in &mut self.codecs {
            // codec.transform(&self.codec_buf, &mut self.codec_buf2);
        }

        self.output_buf.write_all(data).unwrap();

        Ok(())
    }

    pub fn send_trailer(&mut self, fields: &HeaderFields) {
        fields.serialize(&mut self.output_buf).unwrap();
    }

    pub fn end_message(&mut self) {
        todo!()
    }

    pub fn reset(&mut self) {
        self.codecs.clear();
    }

    pub fn read_output(&mut self, buf: &mut [u8]) -> usize {
        self.output_buf.read(buf).unwrap()
    }
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}
