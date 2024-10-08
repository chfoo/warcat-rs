use std::collections::HashMap;

use crate::error::ParseError;

use super::WarcFields;

pub trait FieldsExt {
    /// Returns the value if the name is present, otherwise empty string.
    fn get_or_default<N: AsRef<str>>(&self, name: N) -> &str;

    /// Parse a "content-type" field.
    fn get_media_type<N: AsRef<str>>(&self, name: N) -> Result<Option<MediaType>, ParseError>;

    /// Returns whether the value is delimitated by `<` and `>`.
    fn is_formatted_bad_spec_url<N: AsRef<str>>(&self, name: N) -> bool;

    /// Returns the value with the deliminator `<` and `>` removed.
    fn get_bad_spec_url<N: AsRef<str>>(&self, name: N) -> Option<&str>;
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

impl FieldsExt for WarcFields {
    fn get_or_default<N: AsRef<str>>(&self, name: N) -> &str {
        self.get(name.as_ref())
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn get_media_type<N: AsRef<str>>(&self, name: N) -> Result<Option<MediaType>, ParseError> {
        if let Some(value) = self.get(name.as_ref()) {
            let (_remain, output) = crate::parse::fields::media_type(value.as_bytes())?;

            Ok(Some(MediaType {
                type_: String::from_utf8_lossy(output.type_).to_string(),
                subtype: String::from_utf8_lossy(output.subtype).to_string(),
                parameters: HashMap::from_iter(output.parameters.iter().map(|(k, v)| {
                    (
                        String::from_utf8_lossy(k).to_string(),
                        String::from_utf8_lossy(v).to_string(),
                    )
                })),
            }))
        } else {
            Ok(None)
        }
    }

    fn is_formatted_bad_spec_url<N: AsRef<str>>(&self, name: N) -> bool {
        if let Some(value) = self.get(name.as_ref()) {
            value.starts_with("<") && value.ends_with(">")
        } else {
            false
        }
    }

    fn get_bad_spec_url<N: AsRef<str>>(&self, name: N) -> Option<&str> {
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
}
