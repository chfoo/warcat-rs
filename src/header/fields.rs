use std::{collections::HashMap, net::IpAddr, str::FromStr};

use chrono::{DateTime, FixedOffset};
use url::Url;

use crate::error::ParseError;

use super::WarcFields;

pub trait FieldsExt {
    /// Returns the value if the name is present, otherwise empty string.
    fn get_or_default<N: AsRef<str>>(&self, name: N) -> &str;

    /// Parse a "content-type" field.
    fn get_media_type<N: AsRef<str>>(&self, name: N) -> Option<Result<MediaType, ParseError>>;

    /// Parse a ISO8601 field.
    fn get_date<N: AsRef<str>>(&self, name: N)
        -> Option<Result<DateTime<FixedOffset>, ParseError>>;

    /// Returns whether the value is delimitated by `<` and `>`.
    fn is_formatted_bad_spec_url<N: AsRef<str>>(&self, name: N) -> bool;

    /// Returns the value with the deliminator `<` and `>` removed.
    fn get_url_str<N: AsRef<str>>(&self, name: N) -> Option<&str>;

    /// Parse a URL (with the deliminator `<` and `>` removed).
    fn get_url<N: AsRef<str>>(&self, name: N) -> Option<Result<Url, ParseError>>;

    /// Parse an IP address.
    fn get_ip_addr<N: AsRef<str>>(&self, name: N) -> Option<Result<IpAddr, ParseError>>;
}

#[derive(Debug, Clone, Default)]
pub struct MediaType {
    pub type_: String,
    pub subtype: String,
    pub parameters: HashMap<String, String>,
}

impl MediaType {
    pub fn empty() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl FromStr for MediaType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_remain, output) = crate::parse::fields::media_type(s.as_bytes())?;

        Ok(Self {
            type_: String::from_utf8_lossy(output.type_).to_string(),
            subtype: String::from_utf8_lossy(output.subtype).to_string(),
            parameters: HashMap::from_iter(output.parameters.iter().map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).to_string(),
                    String::from_utf8_lossy(v).to_string(),
                )
            })),
        })
    }
}

impl FieldsExt for WarcFields {
    fn get_or_default<N: AsRef<str>>(&self, name: N) -> &str {
        self.get(name.as_ref())
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn get_media_type<N: AsRef<str>>(&self, name: N) -> Option<Result<MediaType, ParseError>> {
        self.get(name.as_ref())
            .map(|value| MediaType::from_str(value))
    }

    fn get_date<N: AsRef<str>>(
        &self,
        name: N,
    ) -> Option<Result<DateTime<FixedOffset>, ParseError>> {
        self.get(name.as_ref())
            .map(|value| DateTime::parse_from_rfc3339(value).map_err(|error| error.into()))
    }

    fn is_formatted_bad_spec_url<N: AsRef<str>>(&self, name: N) -> bool {
        if let Some(value) = self.get(name.as_ref()) {
            value.starts_with("<") && value.ends_with(">")
        } else {
            false
        }
    }

    fn get_url_str<N: AsRef<str>>(&self, name: N) -> Option<&str> {
        if let Some(value) = self.get(name.as_ref()) {
            if value.starts_with("<") && value.ends_with(">") {
                Some(value.trim_start_matches("<").trim_end_matches(">"))
            } else {
                Some(value)
            }
        } else {
            None
        }
    }

    fn get_url<N: AsRef<str>>(&self, name: N) -> Option<Result<Url, ParseError>> {
        if let Some(value) = self.get(name.as_ref()) {
            let value = if value.starts_with("<") && value.ends_with(">") {
                value.trim_start_matches("<").trim_end_matches(">")
            } else {
                value
            };

            Some(Url::parse(value).map_err(|error| error.into()))
        } else {
            None
        }
    }

    fn get_ip_addr<N: AsRef<str>>(&self, name: N) -> Option<Result<IpAddr, ParseError>> {
        self.get(name.as_ref())
            .map(|value| IpAddr::from_str(value).map_err(|error| error.into()))
    }
}
