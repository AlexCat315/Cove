//! Swift `Codable` wire shapes, so documents persisted by either port decode
//! in the other.
//!
//! Swift's `JSONEncoder` writes a dictionary whose key is neither `String` nor
//! `Int` (nor `CodingKeyRepresentable`) as a flat `[key, value, key, value]`
//! array; the workspace ID types are such keys.
use serde::de::{Deserialize, Deserializer, Error, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use std::collections::BTreeMap;
use std::marker::PhantomData;

pub fn serialize_map<K, V, S>(map: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
where
    K: serde::Serialize,
    V: serde::Serialize,
    S: Serializer,
{
    let mut sequence = serializer.serialize_seq(Some(map.len() * 2))?;
    for (key, value) in map {
        sequence.serialize_element(key)?;
        sequence.serialize_element(value)?;
    }
    sequence.end()
}

pub fn deserialize_map<'de, K, V, D>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
    D: Deserializer<'de>,
{
    struct Pairs<K, V>(PhantomData<(K, V)>);

    impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>> Visitor<'de> for Pairs<K, V> {
        type Value = BTreeMap<K, V>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a flat [key, value, ...] array")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut map = BTreeMap::new();
            while let Some(key) = sequence.next_element::<K>()? {
                let value = sequence
                    .next_element::<V>()?
                    .ok_or_else(|| A::Error::custom("a dictionary key has no value"))?;
                map.insert(key, value);
            }
            Ok(map)
        }
    }

    deserializer.deserialize_seq(Pairs(PhantomData))
}

pub fn yes() -> bool {
    true
}

/// One associated value of a Swift enum case: `{"_0": value}`.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Unlabelled<T> {
    #[serde(rename = "_0")]
    pub value: T,
}
