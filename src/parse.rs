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

pub fn parse_name_value_fields(value: &[u8]) -> Result<Vec<fields::FieldPairRef>, ParseError> {
    match fields::field_pairs(value) {
        Ok((_input, output)) => Ok(output),
        Err(error) => Err(error.into()),
    }
}

pub fn validate_field_name(value: &[u8]) -> Result<(), ParseError> {
    match nom::combinator::all_consuming(fields::field_name)(value) {
        Ok((_input, _output)) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

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

pub fn parse_u64_strict(value: &str) -> Result<u64, std::num::ParseIntError> {
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return "?".parse();
    }

    value.parse()
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
}
