use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::{digit1, line_ending},
    combinator::{map, recognize, verify},
    sequence::terminated,
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

    terminated(alt((status_line, request_line)), line_ending).parse(input)
}

pub fn request_line(input: &[u8]) -> IResult<&[u8], RequestLine<'_>> {
    let parts = (method, tag(" "), request_target, tag(" "), http_version);

    #[allow(clippy::type_complexity)]
    map(parts, |output: (&[u8], &[u8], &[u8], &[u8], &[u8])| {
        RequestLine {
            method: output.0,
            request_target: output.2,
            http_version: output.4,
        }
    })
    .parse(input)
}

pub fn status_line(input: &[u8]) -> IResult<&[u8], StatusLine<'_>> {
    alt((status_line_strict, status_line_non_strict)).parse(input)
}

fn status_line_strict(input: &[u8]) -> IResult<&[u8], StatusLine<'_>> {
    let parts = (http_version, tag(" "), status_code, tag(" "), reason_phrase);

    #[allow(clippy::type_complexity)]
    map(parts, |output: (&[u8], &[u8], &[u8], &[u8], &[u8])| {
        StatusLine {
            http_version: output.0,
            status_code: output.2,
            reason_phrase: output.4,
        }
    })
    .parse(input)
}

fn status_line_non_strict(input: &[u8]) -> IResult<&[u8], StatusLine<'_>> {
    // https://mailman.nginx.org/pipermail/nginx/2013-June/039186.html
    let parts = (http_version, tag(" "), status_code);

    map(parts, |output: (&[u8], &[u8], &[u8])| StatusLine {
        http_version: output.0,
        status_code: output.2,
        reason_phrase: b"",
    })
    .parse(input)
}

fn method(input: &[u8]) -> IResult<&[u8], &[u8]> {
    crate::parse::fields::token(input)
}

fn request_target(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(|c: u8| c.is_ascii_graphic())(input)
}

fn http_version(input: &[u8]) -> IResult<&[u8], &[u8]> {
    // Newer HTTP specifications requires the http-name to be case-sensitive,
    // but we should be lenient instead.
    recognize((
        tag_no_case("HTTP"),
        tag("/"),
        one_digit,
        tag("."),
        one_digit,
    ))
    .parse(input)
}

fn one_digit(input: &[u8]) -> IResult<&[u8], &[u8]> {
    verify(digit1, |i: &[u8]| i.len() == 1).parse(input)
}

fn status_code(input: &[u8]) -> IResult<&[u8], &[u8]> {
    verify(digit1, |i: &[u8]| i.len() == 3).parse(input)
}

fn reason_phrase(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(|b: u8| {
        b.is_ascii_graphic() || b == b' ' || b == b'\t' || crate::parse::fields::is_obs_text(b)
    })(input)
}
