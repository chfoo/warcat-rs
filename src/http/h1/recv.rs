use crate::error::{GeneralError, ProtocolError, ProtocolErrorKind};

use super::{
    codec::CodecPipeline,
    header::{fields::FieldsExt, MessageHeader, StartLine, TrailerFields},
};

const MAX_HEADER_LENGTH: usize = 32768;

#[derive(Debug)]
pub enum ReceiverEvent<'a> {
    WantData,
    Header(MessageHeader),
    Body(&'a [u8]),
    Trailer(TrailerFields),
    End,
}

#[derive(Debug)]
enum ContentLength {
    None,
    Yes(u64),
    ChunkedBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Header,
    Body,
    Trailer,
    End,
}

/// Decodes a HTTP request/response message.
#[derive(Debug)]
pub struct Receiver {
    state: State,
    input_buf: Vec<u8>,
    output_buf: Vec<u8>,
    content_length: ContentLength,
    current_body: u64,
    has_trailer: bool,
    trailer_buf: Vec<u8>,
    codec_pipeline: CodecPipeline,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            state: State::Header,
            input_buf: Vec::new(),
            output_buf: Vec::new(),
            content_length: ContentLength::None,
            current_body: 0,
            has_trailer: false,
            trailer_buf: Vec::new(),
            codec_pipeline: CodecPipeline::default(),
        }
    }

    /// Put input data.
    pub fn recv_data(&mut self, data: &[u8]) {
        self.input_buf.extend_from_slice(data);
    }

    /// Process the input data and return an output.
    pub fn get_event(&mut self) -> Result<ReceiverEvent, GeneralError> {
        match self.state {
            State::Header => self.process_header(),
            State::Body => self.process_body(),
            State::Trailer => self.process_trailer(),
            State::End => Ok(ReceiverEvent::End),
        }
    }

    /// If at the end of message, reset this struct for a new message.
    pub fn next_message(&mut self) -> Result<(), ProtocolError> {
        if self.state != State::End {
            return Err(ProtocolError::new(
                ProtocolErrorKind::InvalidMessageBoundary,
            ));
        }

        tracing::trace!("next message");
        self.state = State::Header;
        Ok(())
    }

    fn process_header(&mut self) -> Result<ReceiverEvent, GeneralError> {
        if let Some(index) = crate::parse::scan_header_deliminator(&self.input_buf) {
            let header_bytes = &self.input_buf[0..index];
            let header = MessageHeader::parse(header_bytes)?;
            self.input_buf.drain(0..index);

            tracing::trace!(len = index, "process header");

            self.config_codecs(&header)?;
            self.config_content_length(&header)?;

            self.state = State::Body;

            Ok(ReceiverEvent::Header(header))
        } else if self.input_buf.len() > MAX_HEADER_LENGTH {
            Err(ProtocolError::new(ProtocolErrorKind::HeaderTooBig).into())
        } else {
            Ok(ReceiverEvent::WantData)
        }
    }

    fn config_content_length(&mut self, header: &MessageHeader) -> Result<(), ProtocolError> {
        self.current_body = 0;
        self.content_length = ContentLength::None;

        if let StartLine::Status(status) = &header.start_line {
            if status.status_code / 100 == 1
                || status.status_code == 204
                || status.status_code == 304
            {
                tracing::trace!("content length set to 0 by status code");
                self.content_length = ContentLength::Yes(0);
                return Ok(());
            }
        }

        let te_names = header
            .fields
            .get_comma_list("Transfer-Encoding")
            .collect::<Vec<_>>();
        self.has_trailer = te_names.contains(&"chunked".into());

        // Transfer-Encoding is higher priority than Content-Length.
        // Presence of both may indicate "request smuggling".
        if header.fields.contains_name("Transfer-Encoding") {
            // If transfer encoding and the last is not "chunked", the chunked encoding
            // cannot indicate an end of message boundary.
            if let Some(name) = &te_names.last() {
                if name.as_ref() == "chunked" {
                    self.content_length = ContentLength::ChunkedBoundary;
                    tracing::trace!("message has framing via chunked encoding");
                } else if header.start_line.is_request() {
                    // Only the server can close the connection when there is no message
                    // length framing.
                    return Err(ProtocolError::new(ProtocolErrorKind::MissingContentLength));
                } else {
                    tracing::trace!("message has no framing (mixed transfer encoding)");
                }
            }
        } else if let Some(result) = header.fields.get_u64_strict("Content-Length") {
            let len = result.map_err(|error| {
                ProtocolError::new(ProtocolErrorKind::InvalidContentLength).with_source(error)
            })?;

            self.content_length = ContentLength::Yes(len);
            tracing::trace!("message has content length");
        }

        // If a request does not satisfy the above conditions, then the body length is 0.
        if matches!(self.content_length, ContentLength::None) && header.start_line.is_request() {
            self.content_length = ContentLength::Yes(0);
            tracing::trace!("content length set to 0");
        }

        Ok(())
    }

    fn config_codecs(&mut self, header: &MessageHeader) -> Result<(), GeneralError> {
        let mut codecs = Vec::new();

        super::codec::build_decoders(header, &mut codecs)?;

        self.codec_pipeline = CodecPipeline::new(codecs);

        Ok(())
    }

    fn process_body(&mut self) -> Result<ReceiverEvent, GeneralError> {
        match &self.content_length {
            ContentLength::Yes(content_length) => self.process_body_content_length(*content_length),
            ContentLength::None => self.process_body_no_length(),
            ContentLength::ChunkedBoundary => self.process_body_chunked_boundary(),
        }
    }

    fn process_body_content_length(
        &mut self,
        content_length: u64,
    ) -> Result<ReceiverEvent, GeneralError> {
        self.output_buf.clear();

        let remain_len = self.input_buf.len().min(
            (content_length - self.current_body)
                .try_into()
                .unwrap_or(usize::MAX),
        );

        if remain_len > 0 {
            self.codec_pipeline
                .transform(&self.input_buf[0..remain_len], &mut self.output_buf)?;
            self.input_buf.drain(0..remain_len);

            self.current_body += remain_len as u64;
            tracing::trace!(
                position = self.current_body,
                "process body data (has content length)"
            );

            Ok(ReceiverEvent::Body(&self.output_buf))
        } else if self.current_body >= content_length {
            self.state = State::End;
            Ok(ReceiverEvent::End)
        } else {
            Ok(ReceiverEvent::WantData)
        }
    }

    fn process_body_no_length(&mut self) -> Result<ReceiverEvent, GeneralError> {
        self.output_buf.clear();

        self.codec_pipeline
            .transform(&self.input_buf, &mut self.output_buf)?;
        self.input_buf.clear();

        tracing::trace!(
            len = self.output_buf.len(),
            "process body data (no content length)"
        );

        if !self.output_buf.is_empty() {
            Ok(ReceiverEvent::Body(&self.output_buf))
        } else if self.has_trailer && self.codec_pipeline.has_remaining_trailer() {
            self.state = State::Trailer;
            self.process_trailer()
        } else {
            Ok(ReceiverEvent::WantData)
        }
    }

    fn process_body_chunked_boundary(&mut self) -> Result<ReceiverEvent, GeneralError> {
        self.output_buf.clear();

        self.codec_pipeline
            .transform(&self.input_buf, &mut self.output_buf)?;
        self.input_buf.clear();

        tracing::trace!(
            len = self.output_buf.len(),
            "process body data (chunked encoding)"
        );

        if !self.output_buf.is_empty() {
            Ok(ReceiverEvent::Body(&self.output_buf))
        } else if self.codec_pipeline.has_remaining_trailer() {
            self.state = State::Trailer;
            self.process_trailer()
        } else {
            Ok(ReceiverEvent::WantData)
        }
    }

    fn process_trailer(&mut self) -> Result<ReceiverEvent, GeneralError> {
        self.codec_pipeline.remaining_trailer(&mut self.trailer_buf);

        if let Some(index) = crate::parse::scan_header_deliminator(&self.trailer_buf) {
            let trailer_bytes = &self.trailer_buf[0..index];
            let mut trailer = TrailerFields::new();
            trailer.parse(trailer_bytes)?;

            tracing::trace!(len = trailer_bytes.len(), "process trailer");
            self.trailer_buf.drain(0..index);
            self.input_buf.clear();
            self.input_buf.extend_from_slice(&self.trailer_buf);
            self.trailer_buf.clear();

            self.state = State::End;

            Ok(ReceiverEvent::Trailer(trailer))
        } else if self.input_buf.len() > MAX_HEADER_LENGTH {
            Err(ProtocolError::new(ProtocolErrorKind::HeaderTooBig).into())
        } else {
            Ok(ReceiverEvent::WantData)
        }
    }
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
    };

    use crate::compress::{Compressor, Format};

    use super::*;

    #[tracing_test::traced_test]
    #[test]
    fn test_receiver_content_length() {
        let mut receiver = Receiver::new();
        receiver.recv_data(
            b"HTTP/1.1 200 OK\r\n\
            Content-Length: 10000\r\n\
            \r\n",
        );

        let mut remain_bytes = 10000;
        let mut output = Vec::new();

        loop {
            let event = receiver.get_event().unwrap();

            match event {
                ReceiverEvent::WantData => {
                    if remain_bytes > 0 {
                        receiver.recv_data(b"aaaaa");
                        remain_bytes -= 5;
                    } else {
                        break;
                    }
                }
                ReceiverEvent::Header(message_header) => {
                    let line = message_header.start_line.as_status().unwrap();
                    assert_eq!(line.status_code, 200);
                }
                ReceiverEvent::Body(data) => {
                    output.extend_from_slice(data);
                }
                ReceiverEvent::Trailer(_field_map) => unreachable!(),
                ReceiverEvent::End => break,
            }
        }

        assert_eq!(&output, &[b'a';10000]);
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_receiver_content_length_multi_message() {
        let mut receiver = Receiver::new();
        receiver.recv_data(
            b"HTTP/1.1 200 OK\r\n\
            Content-Length: 12\r\n\
            \r\n\
            Hello world!\
            HTTP/1.1 404 Not Found\r\n\
            \r\n\
            Not found.",
        );

        let mut output = Vec::new();

        loop {
            let event = receiver.get_event().unwrap();

            match event {
                ReceiverEvent::WantData => unreachable!(),
                ReceiverEvent::Header(message_header) => {
                    let line = message_header.start_line.as_status().unwrap();
                    assert_eq!(line.status_code, 200);
                }
                ReceiverEvent::Body(data) => {
                    output.extend_from_slice(data);
                }
                ReceiverEvent::Trailer(_field_map) => unreachable!(),
                ReceiverEvent::End => break,
            }
        }
        assert_eq!(output, b"Hello world!");

        output.clear();
        receiver.next_message().unwrap();

        loop {
            let event = receiver.get_event().unwrap();

            match event {
                ReceiverEvent::WantData => break,
                ReceiverEvent::Header(message_header) => {
                    let line = message_header.start_line.as_status().unwrap();
                    assert_eq!(line.status_code, 404)
                }
                ReceiverEvent::Body(data) => {
                    output.extend_from_slice(data);
                }
                ReceiverEvent::Trailer(_field_map) => unreachable!(),
                ReceiverEvent::End => break,
            }
        }

        assert_eq!(output, b"Not found.");
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_receiver_no_length() {
        let mut receiver = Receiver::new();
        receiver.recv_data(
            b"HTTP/1.1 200 OK\r\n\
            Content-Type: text/plain\r\n\
            \r\n\
            Hello world!",
        );

        let mut output = Vec::new();

        loop {
            let event = receiver.get_event().unwrap();

            match event {
                ReceiverEvent::WantData => break,
                ReceiverEvent::Header(message_header) => {
                    let line = message_header.start_line.as_status().unwrap();
                    assert_eq!(line.status_code, 200);
                }
                ReceiverEvent::Body(data) => {
                    output.extend_from_slice(data);
                }
                ReceiverEvent::Trailer(_field_map) => unreachable!(),
                ReceiverEvent::End => break,
            }
        }

        assert_eq!(output, b"Hello world!");
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_receiver_chunked_compression() {
        let mut content = VecDeque::new();
        let mut compressor = Compressor::new(&mut content, Format::Gzip);
        compressor.write_all(b"Hello world!").unwrap();
        compressor.finish().unwrap();

        let mut input = Vec::new();
        input.extend_from_slice(
            b"HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            Content-Encoding: gzip\r\n\
            \r\n",
        );

        loop {
            let mut buf = [0u8; 2];
            let len = content.read(&mut buf).unwrap();

            write!(input, "{:x}\r\n", len).unwrap();

            if len == 0 {
                break;
            }

            input.write_all(&buf[0..len]).unwrap();
            input.write_all(b"\r\n").unwrap();
        }

        input.extend_from_slice(
            b"my-field: 12345\r\n\
            \r\n",
        );

        input.extend_from_slice(
            b"HTTP/1.1 404 Not found\r\n\
            \r\n\
            Not found.",
        );

        dbg!(String::from_utf8_lossy(&input));

        let mut receiver = Receiver::new();
        receiver.recv_data(&input);

        let mut output = Vec::new();

        loop {
            let event = receiver.get_event().unwrap();

            match event {
                ReceiverEvent::WantData => unreachable!(),
                ReceiverEvent::Header(message_header) => {
                    let line = message_header.start_line.as_status().unwrap();
                    assert_eq!(line.status_code, 200);
                }
                ReceiverEvent::Body(data) => {
                    output.extend_from_slice(data);
                }
                ReceiverEvent::Trailer(trailer) => {
                    assert!(trailer.contains_name("my-field"));
                }
                ReceiverEvent::End => break,
            }
        }
        assert_eq!(output, b"Hello world!");

        output.clear();
        receiver.next_message().unwrap();

        loop {
            let event = receiver.get_event().unwrap();

            match event {
                ReceiverEvent::WantData => break,
                ReceiverEvent::Header(message_header) => {
                    let line = message_header.start_line.as_status().unwrap();
                    assert_eq!(line.status_code, 404)
                }
                ReceiverEvent::Body(data) => {
                    output.extend_from_slice(data);
                }
                ReceiverEvent::Trailer(_field_map) => unreachable!(),
                ReceiverEvent::End => break,
            }
        }

        assert_eq!(output, b"Not found.");
    }
}
