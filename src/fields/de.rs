use std::marker::PhantomData;

use serde::{de::Visitor, Deserialize, Deserializer};

use super::FieldMap;

struct FieldMapVisitor<N, V> {
    _n: PhantomData<N>,
    _v: PhantomData<V>,
}

impl<N, V> FieldMapVisitor<N, V> {
    fn new() -> Self {
        Self {
            _n: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<'de, N, V> Visitor<'de> for FieldMapVisitor<N, V>
where
    N: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = FieldMap<N, V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("sequence of two-item tuples")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut items = Vec::new();

        while let Some(item) = seq.next_element()? {
            items.push(item);
        }

        Ok(FieldMap { fields: items })
    }
}

impl<'de, N, V> Deserialize<'de> for FieldMap<N, V>
where
    N: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<FieldMap<N, V>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(FieldMapVisitor::new())
    }
}
