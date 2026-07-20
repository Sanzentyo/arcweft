//! Serde deserialization directly over nodes from the one accepted Taplo tree.

use serde::{
    de::{
        self, DeserializeOwned, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
        Visitor,
    },
    forward_to_deserialize_any,
};
use std::{fmt, vec};
use taplo::dom::{Node, node::IntegerValue};
use thiserror::Error;

/// Failure to deserialize a typed manifest value from its Taplo node.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message}")]
pub(crate) struct ManifestTreeError {
    message: String,
}

impl de::Error for ManifestTreeError {
    fn custom<T>(message: T) -> Self
    where
        T: fmt::Display,
    {
        Self {
            message: message.to_string(),
        }
    }
}

/// Deserializes one typed value without reparsing or converting the Taplo tree.
pub(crate) fn deserialize_node<T>(node: Node) -> Result<T, ManifestTreeError>
where
    T: DeserializeOwned,
{
    T::deserialize(ManifestTreeDeserializer { node })
}

struct ManifestTreeDeserializer {
    node: Node,
}

impl<'de> de::Deserializer<'de> for ManifestTreeDeserializer {
    type Error = ManifestTreeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Table(table) => {
                let entries = table
                    .entries()
                    .read()
                    .iter()
                    .map(|(key, value)| (key.value().to_owned(), value.clone()))
                    .collect::<Vec<_>>();
                visitor.visit_map(ManifestMapAccess {
                    entries: entries.into_iter(),
                    value: None,
                })
            }
            Node::Array(array) => {
                let values = array.items().read().iter().cloned().collect::<Vec<_>>();
                visitor.visit_seq(ManifestSeqAccess {
                    values: values.into_iter(),
                })
            }
            Node::Bool(value) => visitor.visit_bool(value.value()),
            Node::Str(value) => visitor.visit_string(value.value().to_owned()),
            Node::Integer(value) => match value.value() {
                IntegerValue::Negative(value) => visitor.visit_i64(value),
                IntegerValue::Positive(value) => visitor.visit_u64(value),
            },
            Node::Float(value) => visitor.visit_f64(value.value()),
            Node::Date(_) => Err(de::Error::custom(
                "TOML date and datetime values are not manifest strings",
            )),
            Node::Invalid(_) => Err(de::Error::custom("invalid Taplo value node")),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Str(value) => visitor.visit_enum(value.value().to_owned().into_deserializer()),
            Node::Table(table) => {
                let mut entries = table
                    .entries()
                    .read()
                    .iter()
                    .map(|(key, value)| (key.value().to_owned(), value.clone()))
                    .collect::<Vec<_>>();
                if entries.len() != 1 {
                    return Err(de::Error::custom(
                        "externally tagged manifest enum must contain exactly one variant",
                    ));
                }
                let (variant, value) = entries
                    .pop()
                    .ok_or_else(|| de::Error::custom("manifest enum variant is missing"))?;
                visitor.visit_enum(ManifestEnumAccess {
                    variant,
                    value: Some(value),
                })
            }
            _ => Err(de::Error::custom(
                "manifest enum value must be a string or one-entry table",
            )),
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Table(table) if table.entries().read().is_empty() => visitor.visit_unit(),
            _ => Err(de::Error::custom("manifest value must be an empty table")),
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf seq tuple tuple_struct map struct identifier
    }
}

struct ManifestEnumAccess {
    variant: String,
    value: Option<Node>,
}

impl<'de> EnumAccess<'de> for ManifestEnumAccess {
    type Error = ManifestTreeError;
    type Variant = ManifestVariantAccess;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let Self { variant, value } = self;
        let variant = seed.deserialize(variant.into_deserializer())?;
        Ok((variant, ManifestVariantAccess { value }))
    }
}

struct ManifestVariantAccess {
    value: Option<Node>,
}

impl<'de> VariantAccess<'de> for ManifestVariantAccess {
    type Error = ManifestTreeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            None => Ok(()),
            Some(node) => {
                let _: () = de::Deserialize::deserialize(ManifestTreeDeserializer { node })?;
                Ok(())
            }
        }
    }

    fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        let node = self
            .value
            .take()
            .ok_or_else(|| de::Error::custom("manifest enum payload is missing"))?;
        seed.deserialize(ManifestTreeDeserializer { node })
    }

    fn tuple_variant<V>(mut self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let node = self
            .value
            .take()
            .ok_or_else(|| de::Error::custom("manifest enum payload is missing"))?;
        de::Deserializer::deserialize_seq(ManifestTreeDeserializer { node }, visitor)
    }

    fn struct_variant<V>(
        mut self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let node = self
            .value
            .take()
            .ok_or_else(|| de::Error::custom("manifest enum payload is missing"))?;
        de::Deserializer::deserialize_map(ManifestTreeDeserializer { node }, visitor)
    }
}

struct ManifestSeqAccess {
    values: vec::IntoIter<Node>,
}

impl<'de> SeqAccess<'de> for ManifestSeqAccess {
    type Error = ManifestTreeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.values
            .next()
            .map(|node| seed.deserialize(ManifestTreeDeserializer { node }))
            .transpose()
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.values.len())
    }
}

struct ManifestMapAccess {
    entries: vec::IntoIter<(String, Node)>,
    value: Option<Node>,
}

impl<'de> MapAccess<'de> for ManifestMapAccess {
    type Error = ManifestTreeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        let Some((key, value)) = self.entries.next() else {
            return Ok(None);
        };
        self.value = Some(value);
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = self
            .value
            .take()
            .ok_or_else(|| de::Error::custom("manifest map value is missing"))?;
        seed.deserialize(ManifestTreeDeserializer { node: value })
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::deserialize_node;
    use serde::Deserialize;
    use taplo::{dom::FromSyntax, parser};

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    #[serde(deny_unknown_fields, rename_all = "kebab-case")]
    struct Fixture {
        mode: Mode,
        values: Vec<u32>,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    #[serde(rename_all = "snake_case")]
    enum Mode {
        ExactValue,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    enum Payload {
        Record(Vec<u32>),
        Named { value: u32 },
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct TaggedFixture {
        payload: Payload,
    }

    fn root(source: &str) -> taplo::dom::Node {
        let parsed = parser::parse(source);
        assert!(parsed.errors.is_empty());
        taplo::dom::Node::from_syntax(parsed.into_syntax().into())
    }

    #[test]
    fn typed_deserialization_walks_the_existing_tree() {
        assert_eq!(
            deserialize_node::<Fixture>(root("mode = \"exact_value\"\nvalues = [1, 2, 3]\n"))
                .expect("typed value"),
            Fixture {
                mode: Mode::ExactValue,
                values: vec![1, 2, 3],
            }
        );
    }

    #[test]
    fn typed_deserialization_retains_strict_record_rules() {
        assert!(
            deserialize_node::<Fixture>(root(
                "mode = \"exact_value\"\nvalues = [1]\nunknown = true\n"
            ))
            .is_err()
        );
    }

    #[test]
    fn typed_deserialization_supports_one_entry_externally_tagged_enums() {
        assert_eq!(
            deserialize_node::<TaggedFixture>(root("payload = { Record = [1, 2] }\n"))
                .expect("newtype variant"),
            TaggedFixture {
                payload: Payload::Record(vec![1, 2]),
            }
        );
        assert_eq!(
            deserialize_node::<TaggedFixture>(root("payload = { Named = { value = 3 } }\n"))
                .expect("struct variant"),
            TaggedFixture {
                payload: Payload::Named { value: 3 },
            }
        );
    }

    #[test]
    fn externally_tagged_enum_rejects_ambiguous_tables() {
        assert!(
            deserialize_node::<TaggedFixture>(root(
                "payload = { Record = [], Named = { value = 3 } }\n"
            ))
            .is_err()
        );
    }
}
