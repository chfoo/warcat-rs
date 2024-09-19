use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{digit1, line_ending},
    combinator::{map, recognize, verify},
    sequence::{terminated, tuple},
    IResult,
};

pub enum StartLine<'a> {
    RequestLine(RequestLine<'a>),
    StatusLine(StatusLine<'a>),
}

pub struct RequestLine<'a> {
    pub method: &'a [u8],
    pub request_target: &'a [u8],
    pub http_version: &'a [u8],
}

pub struct StatusLine<'a> {
    pub http_version: &'a [u8],
    pub status_code: &'a [u8],
    pub reason_phrase: &'a [u8],
}

pub fn start_line(input: &[u8]) -> IResult<&[u8], StartLine<'_>> {
    let status_line = map(status_line, StartLine::StatusLine);
    let request_line = map(request_line, StartLine::RequestLine);

    terminated(alt((status_line, request_line)), line_ending)(input)
}

pub fn request_line(input: &[u8]) -> IResult<&[u8], RequestLine<'_>> {
    let parts = tuple((method, tag(b" "), request_target, tag(b" "), http_version));

    map(parts, |output: (&[u8], &[u8], &[u8], &[u8], &[u8])| {
        RequestLine {
            method: output.0,
            request_target: output.2,
            http_version: output.4,
        }
    })(input)
}

pub fn status_line(input: &[u8]) -> IResult<&[u8], StatusLine<'_>> {
    let parts = tuple((
        http_version,
        tag(b" "),
        status_code,
        tag(b" "),
        reason_phrase,
    ));

    map(parts, |output: (&[u8], &[u8], &[u8], &[u8], &[u8])| {
        StatusLine {
            http_version: output.0,
            status_code: output.2,
            reason_phrase: output.4,
        }
    })(input)
}

fn method(input: &[u8]) -> IResult<&[u8], &[u8]> {
    crate::parse::fields::token(input)
}

fn request_target(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(|c: u8| c.is_ascii_graphic())(input)
}

fn http_version(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(tuple((
        tag(b"HTTP"),
        tag(b"/"),
        one_digit,
        tag(b"."),
        one_digit,
    )))(input)
}

fn one_digit(input: &[u8]) -> IResult<&[u8], &[u8]> {
    verify(digit1, |i: &[u8]| i.len() == 1)(input)
}

fn status_code(input: &[u8]) -> IResult<&[u8], &[u8]> {
    verify(digit1, |i: &[u8]| i.len() == 3)(input)
}

fn reason_phrase(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(|b: u8| {
        b.is_ascii_graphic() || b == b' ' || b == b'\t' || crate::parse::fields::is_obs_text(b)
    })(input)
}
