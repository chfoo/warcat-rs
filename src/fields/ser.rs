use serde::{ser::SerializeSeq, Serialize, Serializer};

use super::FieldMap;

impl<N, V> Serialize for FieldMap<N, V>
where
    N: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.fields.len()))?;

        for item in &self.fields {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}
