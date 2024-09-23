//! Content extraction from WARC files
//!
//! This module provides methods for extracting content from WARC files
//! for casual viewing.

use std::{borrow::Cow, io::Write};

use crate::error::GeneralError;
use crate::http::h1::recv::{Receiver as HttpDecoder, ReceiverEvent};
use crate::{
    error::ParseError,
    header::{fields::FieldsExt, WarcHeader},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    None,
    HttpResponse,
    Resource,
}

#[derive(Debug)]
enum Decoder {
    None,
    Identity,
    Http(HttpDecoder),
}

/// Extracts content from a WARC record
#[derive(Debug)]
pub struct WarcExtractor {
    state: State,
    decoder: Decoder,
    output_path: Vec<String>,
}

impl WarcExtractor {
    pub fn new() -> Self {
        Self {
            state: State::None,
            decoder: Decoder::None,
            output_path: Vec::new(),
        }
    }

    pub fn read_header(&mut self, header: &WarcHeader) -> Result<(), ParseError> {
        let warc_type = header.fields.get_or_default("WARC-Type");
        let media_type = header.fields.get_media_type("Content-Type")?;
        let mut is_http_response = false;

        if let Some(media_type) = media_type {
            is_http_response = media_type.type_ == "application"
                && media_type.subtype == "http"
                && media_type
                    .parameters
                    .get("msgtype")
                    .map(String::as_str)
                    .unwrap_or_default()
                    == "response";
        }
        let url = header
            .fields
            .get_bad_spec_url("WARC-Target-URI")
            .unwrap_or_default();

        if warc_type == "response" && is_http_response && !url.is_empty() {
            self.state = State::HttpResponse;
            self.decoder = Decoder::Http(HttpDecoder::new());
            self.output_path = url_to_path_components(url);
        } else if warc_type == "resource" && !url.is_empty() {
            self.state = State::Resource;
            self.decoder = Decoder::Identity;
        } else {
            self.state = State::None;
        }

        Ok(())
    }

    pub fn has_content(&self) -> bool {
        self.state != State::None
    }

    pub fn file_path_components(&self) -> Vec<String> {
        self.output_path.clone()
    }

    pub fn extract_data<W: Write>(
        &mut self,
        block_data: &[u8],
        mut output: W,
    ) -> Result<(), GeneralError> {
        match &mut self.decoder {
            Decoder::None => Ok(()),
            Decoder::Identity => Ok(output.write_all(block_data)?),
            Decoder::Http(decoder) => {
                decoder.recv_data(block_data);

                loop {
                    match decoder.get_event()? {
                        ReceiverEvent::WantData => break,
                        ReceiverEvent::Header(_header) => {}
                        ReceiverEvent::Body(data) => {
                            output.write_all(data)?;
                        }
                        ReceiverEvent::Trailer(_trailer) => {}
                        ReceiverEvent::End => break,
                    }
                }

                Ok(())
            }
        }
    }
}

impl Default for WarcExtractor {
    fn default() -> Self {
        Self::new()
    }
}

const MAX_COMPONENT_LEN: usize = 200;

pub fn url_to_path_components(url: &str) -> Vec<String> {
    let mut components = Vec::new();

    match url::Url::parse(url) {
        Ok(url) => {
            components.push(url.scheme().to_string());

            if url.has_authority() {
                components.push(escape_authority(url.authority()).to_string());
            }

            if let Some(segments) = url.path_segments() {
                for segment in segments {
                    if !segment.is_empty() {
                        components.push(escape_component(segment).to_string());
                    }
                }
            } else {
                components.push(escape_component(url.path()).to_string());
            }

            if let Some(query) = url.query() {
                components.push(escape_component(query).to_string());
            }
        }
        Err(_) => components.push(escape_component(url).to_string()),
    }

    components
}

const ESCAPE_SET: percent_encoding::AsciiSet = percent_encoding::CONTROLS
    .add(b'/')
    .add(b'\\')
    .add(b':')
    .add(b'*')
    .add(b'?')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'|');

fn escape_authority(authority: &str) -> Cow<'_, str> {
    let mut authority = Cow::Borrowed(authority);

    authority = escape_directory_reference(authority);
    authority = escape_windows_device_reserved_name(authority);
    authority = escape_unsupported_windows_shell_filename(authority);

    if authority.len() > MAX_COMPONENT_LEN {
        authority.to_mut().drain(MAX_COMPONENT_LEN..);
    }

    authority
}

fn escape_component(component: &str) -> Cow<'_, str> {
    let mut component = percent_encoding::percent_decode_str(component).decode_utf8_lossy();

    component = escape_directory_reference(component);
    component = escape_windows_device_reserved_name(component);
    component = escape_unsupported_windows_shell_filename(component);

    let output: Cow<'_, str> =
        percent_encoding::percent_encode(component.as_bytes(), &ESCAPE_SET).into();

    component = match output {
        Cow::Borrowed(value) => {
            // Library has a single byte optimization which makes lifetime tricky
            if value != component.as_ref() {
                Cow::Owned(value.to_string())
            } else {
                component
            }
        }
        Cow::Owned(value) => Cow::Owned(value),
    };

    if component.len() > MAX_COMPONENT_LEN {
        component.to_mut().drain(MAX_COMPONENT_LEN..);
    }

    component
}

fn escape_directory_reference(mut component: Cow<'_, str>) -> Cow<'_, str> {
    if component == "." {
        component.to_mut().replace_range(.., "_");
    } else if component == ".." {
        component.to_mut().replace_range(.., "__");
    }

    component
}

const RESERVED_WINDOWS_FILENAMES: [&str; 30] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
];

fn escape_windows_device_reserved_name(mut component: Cow<'_, str>) -> Cow<'_, str> {
    if let Some(first) = component.split('.').next() {
        if RESERVED_WINDOWS_FILENAMES
            .iter()
            .any(|item| item.eq_ignore_ascii_case(first))
        {
            component.to_mut().insert(0, '_');
        }
    }

    component
}

fn escape_unsupported_windows_shell_filename(mut component: Cow<'_, str>) -> Cow<'_, str> {
    if component.ends_with(".") || component.ends_with(" ") {
        component.to_mut().pop();
        component.to_mut().push('_');
    }

    component
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_component() {
        assert_eq!(escape_component(""), "");
        assert_eq!(escape_component(" "), "_");
        assert_eq!(escape_component("."), "_");
        assert_eq!(escape_component(".."), "__");
        assert_eq!(escape_component("/"), "%2F");
        assert_eq!(escape_component("nul"), "_nul");
        assert_eq!(escape_component("nul.tar.gz"), "_nul.tar.gz");
        assert_eq!(escape_component("?"), "%3F");
        assert_eq!(escape_component("\u{00ff}"), "%C3%BF");
        assert_eq!(escape_component(&"a".repeat(300)), "a".repeat(200));
    }
}
