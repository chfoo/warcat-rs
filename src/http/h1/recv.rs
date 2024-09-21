use crate::error::GeneralError;

use super::{
    codec::BoxedCodec,
    header::{fields::FieldsExt, MessageHeader},
};

#[derive(Debug)]
pub enum ReceiverEvent {
    WantData,
    Header(MessageHeader),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Header,
    Body,
}

/// Decodes a HTTP request/response session
#[derive(Debug)]
pub struct Receiver {
    state: State,
    buf: Vec<u8>,
    content_length: Option<u64>,
    codecs: Vec<BoxedCodec<Vec<u8>>>,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            state: State::Header,
            buf: Vec::new(),
            content_length: None,
            codecs: Vec::new(),
        }
    }

    pub fn recv_data(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn get_event(&mut self) -> Result<ReceiverEvent, GeneralError> {
        match self.state {
            State::Header => self.process_header(),
            State::Body => todo!(),
        }
    }

    fn process_header(&mut self) -> Result<ReceiverEvent, GeneralError> {
        if let Some(index) = crate::parse::scan_header_deliminator(&self.buf) {
            let header_bytes = &self.buf[0..index];
            let header = MessageHeader::parse(header_bytes)?;
            self.buf.drain(0..index);
            self.state = State::Body;
            self.process_content_length(&header);
            self.process_codecs(&header);

            Ok(ReceiverEvent::Header(header))
        } else {
            Ok(ReceiverEvent::WantData)
        }
    }

    fn process_content_length(&mut self, header: &MessageHeader) {
        self.content_length = None;

        if let Some(Ok(len)) = header.fields.get_u64_strict("Content-Length") {
            if !header
                .fields
                .get_comma_list("Transfer-Encoding")
                .any(|name| name == "chunked")
            {
                self.content_length = Some(len);
            }
        }
    }

    fn process_codecs(&mut self, header: &MessageHeader) {
        self.codecs.clear();

        super::codec::build_decoders(header, &mut self.codecs);
    }
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}
