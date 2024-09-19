use super::HeaderFields;

pub trait FieldsExt {
    fn get_comma_list<N: AsRef<str>>(&self, name: N, list: &mut Vec<String>);
}

impl FieldsExt for HeaderFields {
    fn get_comma_list<N: AsRef<str>>(&self, name: N, list: &mut Vec<String>) {
        for value in self.get_all(name.as_ref()) {
            if let Some(value) = value.as_text() {
                for item in value.split(",") {
                    let item = item.trim().to_ascii_lowercase();

                    if !list.contains(&item) {
                        list.push(item);
                    }
                }
            }
        }
    }
}
