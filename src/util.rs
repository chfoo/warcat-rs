use std::borrow::Cow;

pub fn to_ascii_uppercase_cow(text: &str) -> Cow<'_, str> {
    if text.chars().any(|c| c.is_ascii_lowercase()) {
        Cow::Owned(text.to_ascii_uppercase())
    } else {
        Cow::Borrowed(text)
    }
}

pub fn to_ascii_lowercase_cow(text: &str) -> Cow<'_, str> {
    if text.chars().any(|c| c.is_ascii_uppercase()) {
        Cow::Owned(text.to_ascii_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}
