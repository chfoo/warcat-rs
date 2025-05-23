//! HTTP-style name-value fields
use std::{borrow::Borrow, fmt::Display};

mod de;
mod ser;

/// Trait for names.
pub trait EqIcase<Rhs: ?Sized = Self> {
    /// Returns whether the values are equal without ASCII case-sensitivity.
    fn eq_ignore_ascii_case(&self, other: &Rhs) -> bool;
}

impl EqIcase for String {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        str::eq_ignore_ascii_case(self, other)
    }
}

impl EqIcase for str {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        str::eq_ignore_ascii_case(self, other)
    }
}

impl EqIcase for &str {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        str::eq_ignore_ascii_case(self, other)
    }
}

impl EqIcase for Vec<u8> {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        <[u8]>::eq_ignore_ascii_case(self, other)
    }
}

impl EqIcase for [u8] {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        <[u8]>::eq_ignore_ascii_case(self, other)
    }
}

impl EqIcase for &[u8] {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        <[u8]>::eq_ignore_ascii_case(self, other)
    }
}

/// Data structure for HTTP-style name-value fields.
///
/// This is a multimap where keys are case-insensitive.
///
/// No validation is performed on whether the names or values are valid HTTP
/// values.
#[derive(Debug, Clone)]
pub struct FieldMap<N, V> {
    fields: Vec<(N, V)>,
}

impl<N: EqIcase, V> FieldMap<N, V> {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            fields: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.fields.clear()
    }

    pub fn insert(&mut self, name: N, value: V) {
        self.remove(&name);
        self.fields.push((name, value));
    }

    pub fn append(&mut self, name: N, value: V) {
        self.fields.push((name, value))
    }

    pub fn remove<Q>(&mut self, name: &Q)
    where
        Q: EqIcase + ?Sized,
        N: Borrow<Q>,
    {
        self.fields
            .retain(|(n, _v)| !n.borrow().eq_ignore_ascii_case(name));
    }

    pub fn contains_name<Q>(&self, name: &Q) -> bool
    where
        Q: EqIcase + ?Sized,
        N: Borrow<Q>,
    {
        self.fields
            .iter()
            .any(|(n, _v)| n.borrow().eq_ignore_ascii_case(name))
    }

    pub fn get<Q>(&self, name: &Q) -> Option<&V>
    where
        Q: EqIcase + ?Sized,
        N: Borrow<Q>,
    {
        self.fields
            .iter()
            .find(|(n, _v)| n.borrow().eq_ignore_ascii_case(name))
            .map(|(_n, v)| v)
    }

    pub fn get_all<'a, Q>(&'a self, name: &'a Q) -> impl Iterator<Item = &'a V> + 'a
    where
        Q: EqIcase + ?Sized,
        N: Borrow<Q>,
    {
        self.fields.iter().filter_map(move |(n, v)| {
            if n.borrow().eq_ignore_ascii_case(name) {
                Some(v)
            } else {
                None
            }
        })
    }

    pub fn iter(&self) -> FieldMapIter<'_, N, V> {
        FieldMapIter::new(&self.fields)
    }
}

impl<N: EqIcase> FieldMap<N, String> {
    pub fn get_u64_strict<Q>(&self, name: &Q) -> Option<Result<u64, std::num::ParseIntError>>
    where
        Q: EqIcase + ?Sized,
        N: Borrow<Q>,
    {
        self.get(name)
            .map(|value| crate::parse::parse_u64_strict(value))
    }
}

impl<N: EqIcase, V> Default for FieldMap<N, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: EqIcase, V> IntoIterator for FieldMap<N, V> {
    type Item = (N, V);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a, N: EqIcase, V> IntoIterator for &'a FieldMap<N, V> {
    type Item = (&'a N, &'a V);
    type IntoIter = FieldMapIter<'a, N, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<N: EqIcase, V> Extend<(N, V)> for FieldMap<N, V> {
    fn extend<T: IntoIterator<Item = (N, V)>>(&mut self, iter: T) {
        self.fields.extend(iter)
    }
}

impl<N: EqIcase, V> FromIterator<(N, V)> for FieldMap<N, V> {
    fn from_iter<T: IntoIterator<Item = (N, V)>>(iter: T) -> Self {
        Self {
            fields: Vec::from_iter(iter),
        }
    }
}

impl<N: EqIcase + Display, V: Display> Display for FieldMap<N, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, value) in &self.fields {
            write!(f, "{}: {}\r\n", name, value)?;
        }

        Ok(())
    }
}

pub struct FieldMapIter<'a, N, V> {
    fields: std::slice::Iter<'a, (N, V)>,
}

impl<'a, N, V> FieldMapIter<'a, N, V> {
    fn new(fields: &'a [(N, V)]) -> Self {
        Self {
            fields: fields.iter(),
        }
    }
}

impl<'a, N, V> Iterator for FieldMapIter<'a, N, V> {
    type Item = (&'a N, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.fields.next().map(|(n, v)| (n, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fields_generics() {
        let mut f = FieldMap::<String, String>::new();
        f.insert("a".to_string(), "a".to_string());
        f.get("a");

        let mut f = FieldMap::<&'static str, &'static str>::new();
        f.insert("a", "a");
        f.get("a");

        let mut f = FieldMap::<Vec<u8>, Vec<u8>>::new();
        f.insert(b"a".to_vec(), b"a".to_vec());
        f.get(b"a".as_slice());

        let mut f = FieldMap::<&'static [u8], &'static [u8]>::new();
        f.insert(b"a", b"a");
        f.get(b"a".as_slice());
    }

    #[test]
    fn test_fields_create() {
        let mut f = FieldMap::from_iter([("n1", "v1")]);

        assert!(!f.is_empty());
        assert_eq!(f.len(), 1);
        assert!(f.contains_name("n1"));
        assert_eq!(f.get("n1"), Some(&"v1"));

        f.clear();

        assert!(f.is_empty());
        assert_eq!(f.len(), 0);
        assert!(!f.contains_name("n1"));
        assert_eq!(f.get("n1"), None);
    }

    #[test]
    fn test_fields_insert_remove() {
        let mut f = FieldMap::new();

        f.insert("n1", "v1-0");
        f.insert("n1", "v1-1");

        f.append("n2", "v2-0");
        f.append("n2", "v2-1");

        assert_eq!(f.len(), 3);
        assert!(f.contains_name("n1"));
        assert!(f.contains_name("n2"));

        assert_eq!(f.get("n1"), Some(&"v1-1"));
        assert_eq!(f.get("n2"), Some(&"v2-0"));
        assert_eq!(f.get_all("n2").collect::<Vec<_>>(), vec![&"v2-0", &"v2-1"]);

        f.remove("n2");

        assert_eq!(f.len(), 1);
        assert!(f.contains_name("n1"));
        assert!(!f.contains_name("n2"));
    }

    #[test]
    fn test_fields_iterator() {
        let f = FieldMap::from_iter([("n1", "v1"), ("n2", "v2-0"), ("n2", "v2-1")]);

        assert_eq!(
            f.iter().collect::<Vec<_>>(),
            vec![(&"n1", &"v1"), (&"n2", &"v2-0"), (&"n2", &"v2-1")]
        );
        assert_eq!(
            f.into_iter().collect::<Vec<_>>(),
            vec![("n1", "v1"), ("n2", "v2-0"), ("n2", "v2-1")]
        );
    }

    #[test]
    fn test_fields_case_insensitive() {
        let mut f = FieldMap::from_iter([
            ("n1", "v1-0"),
            ("N1", "v1-1"),
            ("n2", "v2-0"),
            ("N2", "v2-1"),
        ]);

        assert_eq!(f.len(), 4);

        assert!(f.contains_name("N1"));
        assert!(f.contains_name("N2"));
        assert_eq!(f.get("N1"), Some(&"v1-0"));
        assert_eq!(f.get("N2"), Some(&"v2-0"));

        f.insert("N1", "v1-2");
        f.remove("N2");

        assert_eq!(f.len(), 1);
        assert!(f.contains_name("N1"));
        assert!(!f.contains_name("N2"));
    }
}
