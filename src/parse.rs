//! Parsing utilities.
use std::{borrow::Cow, cell::LazyCell};

use regex::bytes::Regex;

use crate::error::ParseError;

pub(crate) mod fields;
pub(crate) mod header_deliminator;
pub(crate) mod warc;

/// Get the index (inclusive) of the header deliminator (an empty line).
pub fn scan_header_deliminator(data: &[u8]) -> Option<usize> {
    match header_deliminator::field_lines(data) {
        Ok((_input, output)) => Some(output.len()),
        Err(_) => None,
    }
}

/// Parse a HTTP-like fields of name-value pairs.
pub fn parse_name_value_fields(value: &[u8]) -> Result<Vec<fields::FieldPairRef>, ParseError> {
    match fields::field_pairs(value) {
        Ok((_input, output)) => Ok(output),
        Err(error) => Err(error.into()),
    }
}

/// Returns whether the value is a valid name in a HTTP-like field.
pub fn validate_field_name(value: &[u8]) -> Result<(), ParseError> {
    match nom::combinator::all_consuming(fields::field_name)(value) {
        Ok((_input, _output)) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Returns whether the value is a valid value in a HTTP-like field.
///
/// When `multiline` is `true`, obsolete line folding is permitted.
pub fn validate_field_value(value: &[u8], multiline: bool) -> Result<(), ParseError> {
    let f = if multiline {
        fields::field_value
    } else {
        fields::field_value_no_multline
    };
    match nom::combinator::all_consuming(f)(value) {
        Ok((_input, _output)) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Parse a value into a `u64`.
///
/// Unlike [`u64::try_from()`], only ASCII digits are permitted. Use of std
/// library parsing functions may lead to security issues.
pub fn parse_u64_strict(value: &str) -> Result<u64, std::num::ParseIntError> {
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return "?".parse();
    }

    value.parse()
}

/// Remove line folding from a HTTP-like field value.
pub fn remove_line_folding(value: &[u8]) -> Cow<'_, [u8]> {
    let re = LazyCell::new(|| Regex::new(r"(?:\r\n|\n)[ \t]+").unwrap());
    re.replace_all(value, b" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_header_none() {
        assert_eq!(scan_header_deliminator(b""), None);
        assert_eq!(scan_header_deliminator(b"a"), None);
    }

    #[test]
    fn test_scan_header() {
        assert_eq!(scan_header_deliminator(b"\r\nz"), Some(2));
        assert_eq!(scan_header_deliminator(b"a\r\n\r\nz"), Some(5));
        assert_eq!(scan_header_deliminator(b"a\r\nb\r\n\r\nz"), Some(8));
        assert_eq!(scan_header_deliminator(b"a\nb\n\nz"), Some(5));
    }

    #[test]
    fn test_remove_line_folding() {
        assert_eq!(*remove_line_folding(b"abc"), *b"abc");
        assert_eq!(*remove_line_folding(b"abc\r\n  def"), *b"abc def");
        assert_eq!(
            *remove_line_folding(b"abc\r\n  def\r\n\t123"),
            *b"abc def 123"
        );
        assert_eq!(*remove_line_folding(b"abc\n  def"), *b"abc def");
    }
}
