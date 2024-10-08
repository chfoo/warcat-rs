use std::{collections::VecDeque, io::Read};

use crate::error::GeneralError;

use super::{
    codec::CodecPipeline,
    header::{MessageHeader, TrailerFields},
};

/// Encodes a HTTP request/response message.
///
/// Important: This struct makes no semantic validation! It simply outputs
/// what you call.
pub struct Sender {
    codec_pipeline: CodecPipeline,
    output_buf: VecDeque<u8>,
}

impl Sender {
    pub fn new() -> Self {
        Self {
            codec_pipeline: CodecPipeline::default(),
            output_buf: VecDeque::new(),
        }
    }

    /// Send the header.
    pub fn send_header(&mut self, header: &MessageHeader) -> Result<(), GeneralError> {
        let mut codecs = Vec::new();
        super::codec::build_encoders(header, &mut codecs)?;

        self.codec_pipeline = CodecPipeline::new(codecs);

        header.serialize(&mut self.output_buf).unwrap();

        Ok(())
    }

    /// Send body data.
    pub fn send_body(&mut self, data: &[u8]) -> Result<(), GeneralError> {
        self.codec_pipeline.transform(data, &mut self.output_buf)?;

        Ok(())
    }

    /// Ends the message with a chunked-transfer encoding.
    ///
    /// Flushes any buffered output and outputs the trailer.
    pub fn send_trailer(&mut self, fields: &TrailerFields) -> Result<(), GeneralError> {
        self.codec_pipeline.finish_input(&mut self.output_buf)?;

        fields.serialize(&mut self.output_buf).unwrap();

        Ok(())
    }

    /// Ends the message, flushing any buffered output.
    pub fn end_message(&mut self) -> Result<(), GeneralError> {
        self.codec_pipeline.finish_input(&mut self.output_buf)?;

        Ok(())
    }

    /// At the end of the message, reset the internal state for a new message.
    pub fn reset(&mut self) {
        self.codec_pipeline = CodecPipeline::default();
    }

    /// Writes the output data into the given buffer and returns the amount written.
    pub fn read_output(&mut self, buf: &mut [u8]) -> usize {
        self.output_buf.read(buf).unwrap()
    }
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tracing_test::traced_test]
    #[test]
    fn test_send() {
        let mut output = Vec::new();
        let mut sender = Sender::new();

        let header = MessageHeader::new_request("GET", "/index.html");
        sender.send_header(&header).unwrap();
        sender.send_body(b"Hello world!").unwrap();
        sender.end_message().unwrap();

        loop {
            let mut buf = [0u8; 1024];
            let len = sender.read_output(&mut buf);

            if len == 0 {
                break;
            }

            output.extend_from_slice(&buf[0..len]);
        }

        assert_eq!(
            output,
            b"GET /index.html HTTP/1.1\r\n\
            \r\n\
            Hello world!"
        );
    }
}
