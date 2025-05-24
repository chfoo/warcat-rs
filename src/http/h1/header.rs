//! HTTP headers

use core::str;
use std::{borrow::Cow, io::Write};

use crate::{error::ParseError, fields::FieldMap};

pub mod fields;
mod parse;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hstring {
    Text(String),
    Opaque(Vec<u8>),
}

impl Hstring {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Opaque(vec) => vec,
        }
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        match self {
            Hstring::Text(text) => text.into(),
            Hstring::Opaque(vec) => String::from_utf8_lossy(vec),
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(..))
    }

    pub fn as_text(&self) -> Option<&str> {
        if let Self::Text(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_text(self) -> Result<String, Self> {
        if let Self::Text(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn is_opaque(&self) -> bool {
        matches!(self, Self::Opaque(..))
    }

    pub fn as_opaque(&self) -> Option<&[u8]> {
        if let Self::Opaque(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_opaque(self) -> Result<Vec<u8>, Self> {
        if let Self::Opaque(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }
}

impl Default for Hstring {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl From<String> for Hstring {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&str> for Hstring {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

impl From<Vec<u8>> for Hstring {
    fn from(v: Vec<u8>) -> Self {
        match String::from_utf8(v) {
            Ok(v) => Self::Text(v),
            Err(e) => Self::Opaque(e.into_bytes()),
        }
    }
}

impl From<&[u8]> for Hstring {
    fn from(v: &[u8]) -> Self {
        match String::from_utf8(v.to_vec()) {
            Ok(v) => Self::Text(v),
            Err(e) => Self::Opaque(e.into_bytes()),
        }
    }
}

pub type HeaderFields = FieldMap<String, Hstring>;
pub type TrailerFields = FieldMap<String, Hstring>;

impl HeaderFields {
    pub fn serialize<W: Write>(&self, mut buf: W) -> std::io::Result<()> {
        for (name, value) in self {
            buf.write_all(name.as_bytes())?;
            buf.write_all(b": ")?;
            buf.write_all(value.as_bytes())?;
            buf.write_all(b"\r\n")?;
        }

        buf.write_all(b"\r\n")?;

        Ok(())
    }

    pub fn parse(&mut self, input: &[u8]) -> Result<(), ParseError> {
        let (_remain, pairs) = crate::parse::fields::field_pairs(input)?;

        for pair in pairs {
            let name = String::from_utf8(pair.name.to_vec())?;
            let value = crate::parse::remove_line_folding(pair.value)
                .into_owned()
                .into();

            self.insert(name, value);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RequestLine {
    pub method: String,
    pub request_target: String,
    pub http_version: String,
}

#[derive(Debug, Clone)]
pub struct StatusLine {
    pub http_version: String,
    pub status_code: u16,
    pub reason_phrase: Hstring,
}

#[derive(Debug, Clone)]
pub enum StartLine {
    Request(RequestLine),
    Status(StatusLine),
}

impl StartLine {
    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request(..))
    }

    pub fn as_request(&self) -> Option<&RequestLine> {
        if let Self::Request(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_request(self) -> Result<RequestLine, Self> {
        if let Self::Request(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn is_status(&self) -> bool {
        matches!(self, Self::Status(..))
    }

    pub fn as_status(&self) -> Option<&StatusLine> {
        if let Self::Status(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_status(self) -> Result<StatusLine, Self> {
        if let Self::Status(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub start_line: StartLine,
    pub fields: HeaderFields,
}

impl MessageHeader {
    pub fn empty() -> Self {
        Self {
            start_line: StartLine::Request(RequestLine {
                method: String::new(),
                request_target: String::new(),
                http_version: String::new(),
            }),
            fields: HeaderFields::new(),
        }
    }

    pub fn new_request<S1: Into<String>, S2: Into<String>>(method: S1, target: S2) -> Self {
        Self {
            start_line: StartLine::Request(RequestLine {
                method: method.into(),
                request_target: target.into(),
                http_version: "HTTP/1.1".to_string(),
            }),
            fields: HeaderFields::new(),
        }
    }

    pub fn new_response<S2: Into<String>>(status_code: u16, reason_phrase: S2) -> Self {
        Self {
            start_line: StartLine::Status(StatusLine {
                http_version: "HTTP/1.1".to_string(),
                status_code,
                reason_phrase: reason_phrase.into().into(),
            }),
            fields: HeaderFields::new(),
        }
    }

    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        let mut header = Self::empty();

        let (remain, start_line) = self::parse::start_line(input)?;

        match start_line {
            parse::StartLine::RequestLine(request_line) => {
                header.start_line = StartLine::Request(RequestLine {
                    method: String::from_utf8(request_line.method.to_vec()).unwrap(),
                    request_target: String::from_utf8(request_line.request_target.to_vec())
                        .unwrap(),
                    http_version: String::from_utf8(request_line.http_version.to_vec()).unwrap(),
                })
            }
            parse::StartLine::StatusLine(status_line) => {
                header.start_line = StartLine::Status(StatusLine {
                    http_version: String::from_utf8(status_line.http_version.to_vec()).unwrap(),
                    status_code: str::from_utf8(status_line.status_code)
                        .unwrap()
                        .parse()
                        .unwrap(),
                    reason_phrase: status_line.reason_phrase.into(),
                });
            }
        }

        header.fields.parse(remain)?;

        Ok(header)
    }

    pub fn serialize<W: Write>(&self, mut buf: W) -> std::io::Result<()> {
        self.serialize_start_line(&mut buf)?;
        self.fields.serialize(&mut buf)?;
        Ok(())
    }

    fn serialize_start_line<W: Write>(&self, mut buf: W) -> std::io::Result<()> {
        match &self.start_line {
            StartLine::Request(request_line) => {
                buf.write_all(request_line.method.as_bytes())?;
                buf.write_all(b" ")?;
                buf.write_all(request_line.request_target.as_bytes())?;
                buf.write_all(b" ")?;
                buf.write_all(request_line.http_version.as_bytes())?;
            }
            StartLine::Status(status_line) => {
                buf.write_all(status_line.http_version.as_bytes())?;
                write!(buf, " {:03} ", status_line.status_code)?;
                buf.write_all(status_line.reason_phrase.as_bytes())?;
            }
        }

        buf.write_all(b"\r\n")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_parse_request() {
        let data = "GET /index.html HTTP/1.1\r\n\
            User-Agent: example\r\n\
            Host: example.com\r\n\
            \r\n";

        let header = MessageHeader::parse(data.as_bytes()).unwrap();

        let request_line = header.start_line.as_request().unwrap();

        assert_eq!(request_line.method, "GET");
        assert_eq!(request_line.request_target, "/index.html");
        assert_eq!(request_line.http_version, "HTTP/1.1");
        assert_eq!(header.fields.len(), 2);
        assert_eq!(header.fields.get("Host"), Some(&"example.com".into()));

        let mut buf = Vec::new();
        header.serialize(&mut buf).unwrap();
        assert_eq!(buf, data.as_bytes());
    }

    #[test]
    fn test_header_parse_response() {
        let data = "HTTP/1.1 200 OK\r\n\
            Server: example.com\r\n\
            Content-Length: 123\r\n\
            \r\n";

        let header = MessageHeader::parse(data.as_bytes()).unwrap();

        let status_line = header.start_line.as_status().unwrap();

        assert_eq!(status_line.http_version, "HTTP/1.1");
        assert_eq!(status_line.status_code, 200);
        assert_eq!(status_line.reason_phrase.as_text(), Some("OK"));
        assert_eq!(header.fields.len(), 2);
        assert_eq!(header.fields.get("Server"), Some(&"example.com".into()));

        let mut buf = Vec::new();
        header.serialize(&mut buf).unwrap();
        assert_eq!(buf, data.as_bytes());
    }

    #[test]
    fn test_header_parse_response_empty_reason_phrase() {
        let data = "HTTP/1.1 200 \r\n\
            Server: example.com\r\n\
            \r\n";

        let header = MessageHeader::parse(data.as_bytes()).unwrap();

        let status_line = header.start_line.as_status().unwrap();

        assert_eq!(status_line.http_version, "HTTP/1.1");
        assert_eq!(status_line.status_code, 200);
        assert_eq!(status_line.reason_phrase.as_text(), Some(""));
        assert_eq!(header.fields.len(), 1);
        assert_eq!(header.fields.get("Server"), Some(&"example.com".into()));

        let mut buf = Vec::new();
        header.serialize(&mut buf).unwrap();
        assert_eq!(buf, data.as_bytes());
    }

    #[test]
    fn test_header_parse_response_missing_mandatory_space() {
        let data = "HTTP/1.1 200\r\n\
            Server: example.com\r\n\
            \r\n";

        let header = MessageHeader::parse(data.as_bytes()).unwrap();

        let status_line = header.start_line.as_status().unwrap();

        assert_eq!(status_line.http_version, "HTTP/1.1");
        assert_eq!(status_line.status_code, 200);
        assert_eq!(status_line.reason_phrase.as_text(), Some(""));
        assert_eq!(header.fields.len(), 1);
        assert_eq!(header.fields.get("Server"), Some(&"example.com".into()));
    }

    #[test]
    fn test_header_parse_other_names() {
        let data = "http/1.1 200 OK\r\n\
            Server: example.com\r\n\r\n";
        let header = MessageHeader::parse(data.as_bytes()).unwrap();

        let status_line = header.start_line.as_status().unwrap();
        assert_eq!(status_line.status_code, 200);

        let data = "ICY 200 OK\r\n\
        abc: 123\r\n\r\n";
        let result = MessageHeader::parse(data.as_bytes());
        assert!(result.is_err());
    }
}
