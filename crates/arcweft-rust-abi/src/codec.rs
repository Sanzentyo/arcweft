//! Source codec attributes in the v1 Rust metadata transport. These contain
//! policy only; type identity and payload topology remain on their declarations.

use serde::{Deserialize, Serialize};

mod validation;
pub use validation::ArcweftRustCodecPolicyError;

/// JSON's portable integer range does not cover Rust's full i128 range. The
/// v1 ABI carries discriminants as canonical decimal text, including when an
/// enclosing tagged enum is buffered by serde.
pub(crate) mod discriminant {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[allow(clippy::ref_option)] // serde's `with` contract passes the field by reference.
    pub fn serialize<S: Serializer>(
        value: &Option<i128>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(|value| value.to_string()).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<i128>, D::Error> {
        let value = Option::<String>::deserialize(deserializer)?;
        value
            .map(|text| {
                let value = text.parse::<i128>().map_err(serde::de::Error::custom)?;
                if value.to_string() != text {
                    return Err(serde::de::Error::custom(
                        "non-canonical Rust enum discriminant",
                    ));
                }
                Ok(value)
            })
            .transpose()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcweftRustBytesFormat {
    Binary,
    Base64,
    Hex,
    Array,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcweftRustEnumRepr {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArcweftRustEnumTagStyle {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArcweftRustDataTypePolicy {
    pub name: String,
    pub deny_unknown_fields: bool,
    pub tag: ArcweftRustEnumTagStyle,
    pub repr: Option<ArcweftRustEnumRepr>,
}

impl ArcweftRustDataTypePolicy {
    /// Version-one canonical policy transcript used by source publication
    /// owners. Logical types and producer identities are encoded separately.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        fn string(bytes: &mut Vec<u8>, value: &str) {
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
        let mut bytes = Vec::new();
        string(&mut bytes, &self.name);
        bytes.push(u8::from(self.deny_unknown_fields));
        match &self.tag {
            ArcweftRustEnumTagStyle::External => bytes.push(0),
            ArcweftRustEnumTagStyle::Internal { tag } => {
                bytes.push(1);
                string(&mut bytes, tag);
            }
            ArcweftRustEnumTagStyle::Adjacent { tag, content } => {
                bytes.push(2);
                string(&mut bytes, tag);
                string(&mut bytes, content);
            }
        }
        bytes.push(self.repr.map_or(0, |repr| repr as u8 + 1));
        bytes
    }
    /// The standard policy of a Rust declaration with no data attributes.
    pub fn standard(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            deny_unknown_fields: true,
            tag: ArcweftRustEnumTagStyle::External,
            repr: None,
        }
    }
}
