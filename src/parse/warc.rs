use nom::{
    bytes::complete::{tag, take_while},
    character::complete::line_ending,
    combinator::recognize,
    sequence::{pair, terminated},
    IResult,
};

pub fn version(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let tag = tag("WARC/");
    let digits = take_while(|c: u8| c.is_ascii_digit() || c == b'.');

    recognize(pair(tag, digits))(input)
}

pub fn version_line(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(version, line_ending)(input)
}
