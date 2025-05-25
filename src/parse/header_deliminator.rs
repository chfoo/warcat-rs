use nom::{
    bytes::complete::take_till1, character::complete::line_ending, combinator::recognize,
    multi::many0_count, sequence::terminated, IResult, Parser,
};

fn field_line(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_till1(|b| b == b'\r' || b == b'\n'), line_ending).parse(input)
}

pub fn field_lines(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(terminated(many0_count(field_line), line_ending)).parse(input)
}
