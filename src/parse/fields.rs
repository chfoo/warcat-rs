use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1, take_while_m_n},
    character::complete::{line_ending, space0, space1},
    combinator::{map, recognize},
    multi::{many0, many0_count},
    sequence::{delimited, pair, separated_pair, terminated},
    IResult,
};

pub struct FieldPairRef<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
}

impl<'a> From<(&'a [u8], &'a [u8])> for FieldPairRef<'a> {
    fn from(value: (&'a [u8], &'a [u8])) -> Self {
        Self {
            name: value.0,
            value: value.1,
        }
    }
}

pub fn field_pairs(input: &[u8]) -> IResult<&[u8], Vec<FieldPairRef<'_>>> {
    many0(terminated(field_pair, line_ending))(input)
}

fn field_pair(input: &[u8]) -> IResult<&[u8], FieldPairRef<'_>> {
    let val = delimited(space0, field_value, space0);
    let pair = separated_pair(field_name, tag(":"), val);

    map(pair, |p| p.into())(input)
}

pub fn field_name(input: &[u8]) -> IResult<&[u8], &[u8]> {
    token(input)
}

fn token(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_tchar)(input)
}

pub fn field_value(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let a = alt((field_content, obs_fold));
    recognize(many0_count(a))(input)
}

pub fn field_value_no_multline(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(many0_count(field_content))(input)
}

fn field_content(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(pair(
        take_while_m_n(1, 1, is_field_vchar),
        take_while(is_field_char),
    ))(input)
}

fn is_field_vchar(b: u8) -> bool {
    b.is_ascii_graphic() || is_obs_text(b)
}

fn is_field_char(b: u8) -> bool {
    is_field_vchar(b) || b == b' ' || b == b'\t'
}

fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn is_obs_text(b: u8) -> bool {
    b >= 0x80
}

fn obs_fold(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(pair(line_ending, space1))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_pairs_empty() {
        let (_remain, output) = field_pairs(b"").unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_field_pairs_1() {
        let (_remain, output) = field_pairs(b"n1:\r\n").unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(output[0].name, b"n1");
        assert_eq!(output[0].value, b"");

        let (_remain, output) = field_pairs(b"n1:v1\r\n").unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(output[0].name, b"n1");
        assert_eq!(output[0].value, b"v1");
    }

    #[test]
    fn test_field_pairs_many() {
        let (_remain, output) = field_pairs(b"n1:v1\r\nn2:\r\nn3:v3\r\n").unwrap();

        assert_eq!(output.len(), 3);
        assert_eq!(output[0].name, b"n1");
        assert_eq!(output[0].value, b"v1");
        assert_eq!(output[1].name, b"n2");
        assert_eq!(output[1].value, b"");
        assert_eq!(output[2].name, b"n3");
        assert_eq!(output[2].value, b"v3");
    }

    #[test]
    fn test_field_pairs_line_folding() {
        let (_remain, output) = field_pairs(b"n1:v1\r\n  1\r\nn2:v2\r\n").unwrap();

        assert_eq!(output.len(), 2);
        assert_eq!(output[0].name, b"n1");
        assert_eq!(output[0].value, b"v1\r\n  1");
        assert_eq!(output[1].name, b"n2");
        assert_eq!(output[1].value, b"v2");
    }
}
