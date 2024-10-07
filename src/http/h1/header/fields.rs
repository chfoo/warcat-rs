use std::borrow::Cow;

use super::{HeaderFields, Hstring};

pub trait FieldsExt {
    fn get_comma_list<'a>(&'a self, name: &'a str) -> impl Iterator<Item = Cow<'a, str>>;

    fn get_u64_strict<N: AsRef<str>>(
        &self,
        name: N,
    ) -> Option<Result<u64, std::num::ParseIntError>>;
}

impl FieldsExt for HeaderFields {
    fn get_comma_list<'a>(&'a self, name: &'a str) -> impl Iterator<Item = Cow<'a, str>> {
        let mut list = Vec::new();

        for value in self.get_all(name) {
            if let Some(value) = value.as_text() {
                for item in value.split(",") {
                    let item = crate::util::to_ascii_lowercase_cow(item.trim());

                    if !list.contains(&item) {
                        list.push(item);
                    }
                }
            }
        }

        list.into_iter()
    }

    fn get_u64_strict<N: AsRef<str>>(
        &self,
        name: N,
    ) -> Option<Result<u64, std::num::ParseIntError>> {
        if let Some(Hstring::Text(ref value)) = self.get(name.as_ref()) {
            Some(crate::parse::parse_u64_strict(value))
        } else {
            None
        }
    }
}
