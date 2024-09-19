//! WARC headers
use std::io::Write;

use chrono::Utc;

use crate::{
    error::{ParseError, ParseErrorKind},
    fields::FieldMap,
};

/// Data structure for representing a WARC header.
#[derive(Debug, Clone)]
pub struct WarcHeader {
    /// The version string such as "WARC/1.1".
    pub version: String,
    /// The name-value fields of the header.
    pub fields: FieldMap<String, String>,
}

impl WarcHeader {
    /// Create a new empty header.
    ///
    /// The version and fields will be empty.
    pub fn empty() -> Self {
        Self {
            version: String::new(),
            fields: FieldMap::new(),
        }
    }

    /// Create a new header with the bare minimum values.
    ///
    /// The user supplies the `Content-Length` and `WARC-Type`.
    /// `WARC-Record-ID` and `WARC-Date` is automatically generated.
    pub fn new<WT>(content_length: u64, warc_type: WT) -> Self
    where
        WT: Into<String>,
    {
        let mut header = WarcHeader::empty();
        header.version = "WARC/1.1".to_string();
        let uuid = uuid::Uuid::now_v7();
        let date_now = Utc::now();

        header
            .fields
            .insert("WARC-Record-ID".to_string(), format!("<{}>", uuid.urn()));
        header
            .fields
            .insert("WARC-Type".to_string(), warc_type.into());
        header
            .fields
            .insert("WARC-Date".to_string(), date_now.to_rfc3339());
        header.set_content_length(content_length);

        header
    }

    /// Parses a WARC header from the given bytes.
    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        let (remain, version) = crate::parse::warc::version_line(input)?;

        let mut header = Self::empty();
        header.version = String::from_utf8(version.to_vec())?;

        let (_remain, pairs) = crate::parse::fields::field_pairs(remain)?;

        for pair in pairs {
            let name = String::from_utf8(pair.name.to_vec())?;
            let value = String::from_utf8(crate::parse::remove_line_folding(pair.value).to_vec())?;

            header.fields.insert(name, value);
        }

        Ok(header)
    }

    /// Returns the value of `Content-Length` as an integer.
    pub fn content_length(&self) -> Result<u64, ParseError> {
        if let Some(value) = self.fields.get_u64_strict("Content-Length") {
            Ok(value.map_err(|e| {
                ParseError::new(ParseErrorKind::InvalidContentLength).with_source(e)
            })?)
        } else {
            Err(ParseError::new(ParseErrorKind::InvalidContentLength))
        }
    }

    /// Sets the value of `Content-Length` as an integer.
    pub fn set_content_length(&mut self, value: u64) {
        self.fields
            .insert("Content-Length".to_string(), value.to_string());
    }

    /// Returns whether the header is a valid WARC formatted header.
    ///
    /// **Important:** This function does not validate whether the *contents* of
    /// the header conforms to the WARC specification!
    pub fn validate(&self) -> Result<(), ParseError> {
        crate::parse::warc::version(self.version.as_bytes())?;

        for (name, value) in &self.fields {
            crate::parse::validate_field_name(name.as_bytes())?;
            crate::parse::validate_field_value(value.as_bytes(), false)?;
        }

        Ok(())
    }

    /// Write the WARC header as serialized bytes.
    pub fn serialize<W: Write>(&self, mut buf: W) -> std::io::Result<()> {
        buf.write_all(self.version.as_bytes())?;
        buf.write_all(b"\r\n")?;

        for (name, value) in &self.fields {
            buf.write_all(name.as_bytes())?;
            buf.write_all(b": ")?;
            buf.write_all(value.as_bytes())?;
            buf.write_all(b"\r\n")?;
        }

        buf.write_all(b"\r\n")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_parse_serialize() {
        let data = "WARC/1.1\r\n\
            WARC-Record-ID: <example:123456>\r\n\
            Content-Length: 0\r\n\
            \r\n";
        let header = WarcHeader::parse(data.as_bytes()).unwrap();

        assert_eq!(&header.version, "WARC/1.1");
        assert_eq!(header.fields.len(), 2);

        let mut buf = Vec::new();

        header.serialize(&mut buf).unwrap();

        assert_eq!(&buf, data.as_bytes());
    }
}
