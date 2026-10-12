//! One budgeted JSON grammar for raw decoding and typed-decoder preflight.

use std::collections::BTreeSet;
use std::marker::PhantomData;

use arcweft_data::{
    DataError, DataErrorKind, DecodeBudget, DecodeLimits, PathSegment, RawValue, Result,
};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

pub(super) fn decode_raw(input: &[u8], limits: &DecodeLimits) -> Result<RawValue> {
    parse::<RawProjection>(input, limits, !limits.allow_trailing_data)
}

pub(super) fn validate_document(input: &[u8], limits: &DecodeLimits) -> Result<()> {
    parse::<ValidationProjection>(input, limits, true)
}

fn parse<P: Projection>(input: &[u8], limits: &DecodeLimits, strict: bool) -> Result<P::Value> {
    let mut budget = DecodeContext {
        budget: DecodeBudget::new(input.len(), limits)?,
        failure: None,
        path: Vec::new(),
    };
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = JsonSeed::<P>::new(&mut budget)
        .deserialize(&mut deserializer)
        .map_err(|error| {
            budget
                .failure
                .take()
                .unwrap_or_else(|| super::json_error(&error))
        })??;
    if strict {
        deserializer
            .end()
            .map_err(|error| super::json_error(&error))?;
    }
    Ok(value)
}

/// Owns the parse budget and its first structured rejection. Serde can perform
/// delimiter checks after an early visitor return; retain the original failure
/// rather than replacing its kind/path with that secondary syntax error.
struct DecodeContext<'limits> {
    budget: DecodeBudget<'limits>,
    failure: Option<DataError>,
    path: Vec<PathSegment>,
}

impl DecodeContext<'_> {
    fn record<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result
            && self.failure.is_none()
        {
            let error = self
                .path
                .iter()
                .rev()
                .fold(error.clone(), |error, step| match step {
                    PathSegment::Field(name) => error.at_field(name.clone()),
                    PathSegment::Index(index) => error.at_index(*index),
                    PathSegment::Variant(name) => error.at_variant(name.clone()),
                });
            self.failure = Some(error);
        }
        result
    }
    fn enter_node(&mut self) -> Result<()> {
        let result = self.budget.enter_node();
        self.record(result)
    }
    fn exit_node(&mut self) {
        self.budget.exit_node();
    }
    fn sequence_item(&mut self, count: usize) -> Result<()> {
        let result = self.budget.sequence_item(count);
        self.record(result)
    }
    fn map_item(&mut self, count: usize) -> Result<()> {
        let result = self.budget.map_item(count);
        self.record(result)
    }
    fn string_len(&mut self, count: usize) -> Result<()> {
        let result = self.budget.string_len(count);
        self.record(result)
    }
}

trait Projection {
    type Value;
    type Sequence: Default;
    type Object: Default;

    fn scalar(value: RawValue) -> Self::Value;
    fn string(value: &str) -> Self::Value;
    fn push(sequence: &mut Self::Sequence, value: Self::Value);
    fn sequence(sequence: Self::Sequence) -> Self::Value;
    fn insert(object: &mut Self::Object, key: String, value: Self::Value);
    fn object(object: Self::Object) -> Self::Value;
}

struct RawProjection;

impl Projection for RawProjection {
    type Value = RawValue;
    type Sequence = Vec<RawValue>;
    type Object = Vec<(RawValue, RawValue)>;

    fn scalar(value: RawValue) -> Self::Value {
        value
    }
    fn string(value: &str) -> Self::Value {
        RawValue::String(value.to_owned())
    }
    fn push(sequence: &mut Self::Sequence, value: Self::Value) {
        sequence.push(value);
    }
    fn sequence(sequence: Self::Sequence) -> Self::Value {
        RawValue::Seq(sequence)
    }
    fn insert(object: &mut Self::Object, key: String, value: Self::Value) {
        object.push((RawValue::String(key), value));
    }
    fn object(object: Self::Object) -> Self::Value {
        RawValue::Map(object)
    }
}

struct ValidationProjection;

impl Projection for ValidationProjection {
    type Value = ();
    type Sequence = ();
    type Object = ();

    fn scalar(_: RawValue) {}
    fn string(_: &str) {}
    fn push((): &mut (), (): ()) {}
    fn sequence((): ()) {}
    fn insert((): &mut (), _: String, (): ()) {}
    fn object((): ()) {}
}

struct JsonSeed<'budget, 'limits, P> {
    budget: &'budget mut DecodeContext<'limits>,
    projection: PhantomData<P>,
    item: Option<CollectionItem>,
    field: Option<String>,
}

enum CollectionItem {
    Sequence(usize),
    Object(usize),
}

impl<'budget, 'limits, P> JsonSeed<'budget, 'limits, P> {
    fn new(budget: &'budget mut DecodeContext<'limits>) -> Self {
        Self {
            budget,
            projection: PhantomData,
            item: None,
            field: None,
        }
    }
    fn item(budget: &'budget mut DecodeContext<'limits>, item: CollectionItem) -> Self {
        Self {
            budget,
            projection: PhantomData,
            item: Some(item),
            field: None,
        }
    }
    fn field(budget: &'budget mut DecodeContext<'limits>, field: String) -> Self {
        Self {
            field: Some(field),
            ..Self::new(budget)
        }
    }
}

impl<'de, P: Projection> DeserializeSeed<'de> for JsonSeed<'_, '_, P> {
    type Value = Result<P::Value>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = match (&self.item, self.field) {
            (Some(CollectionItem::Sequence(count)), _) => Some(PathSegment::Index(count - 1)),
            (_, Some(field)) => Some(PathSegment::Field(field)),
            _ => None,
        };
        let has_path = path.is_some();
        if let Some(path) = path {
            self.budget.path.push(path);
        }
        let result = (|| {
            let admission = match self.item {
                Some(CollectionItem::Sequence(count)) => self.budget.sequence_item(count),
                Some(CollectionItem::Object(count)) => self.budget.map_item(count),
                None => Ok(()),
            };
            if let Err(error) = admission {
                return Ok(Err(error));
            }
            deserializer.deserialize_any(JsonVisitor::<P> {
                budget: self.budget,
                projection: PhantomData,
            })
        })();
        if has_path {
            self.budget.path.pop();
        }
        result
    }
}

struct JsonVisitor<'budget, 'limits, P> {
    budget: &'budget mut DecodeContext<'limits>,
    projection: PhantomData<P>,
}

impl<'de, P: Projection> Visitor<'de> for JsonVisitor<'_, '_, P> {
    type Value = Result<P::Value>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(self.scalar(RawValue::Null))
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(self.scalar(RawValue::Bool(value)))
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(self.scalar(RawValue::Signed(i128::from(value))))
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(self.scalar(RawValue::Unsigned(u128::from(value))))
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> std::result::Result<Self::Value, E> {
        Ok(self.scalar(RawValue::F64(value)))
    }
    fn visit_borrowed_str<E: serde::de::Error>(
        self,
        value: &'de str,
    ) -> std::result::Result<Self::Value, E> {
        Ok(self.string(value))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(self.string(value))
    }
    fn visit_string<E: serde::de::Error>(
        self,
        value: String,
    ) -> std::result::Result<Self::Value, E> {
        Ok(self.string(&value))
    }
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        if let Err(error) = self.budget.enter_node() {
            return Ok(Err(error));
        }
        let mut values = P::Sequence::default();
        let mut count = 0usize;
        while let Some(value) = seq.next_element_seed(JsonSeed::<P>::item(
            self.budget,
            CollectionItem::Sequence(count.saturating_add(1)),
        ))? {
            match value {
                Ok(value) => P::push(&mut values, value),
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error.at_index(count)));
                }
            }
            count += 1;
        }
        self.budget.exit_node();
        Ok(Ok(P::sequence(values)))
    }
    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        if let Err(error) = self.budget.enter_node() {
            return Ok(Err(error));
        }
        let mut entries = P::Object::default();
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key_seed(JsonSeed::<RawProjection>::item(
            self.budget,
            CollectionItem::Object(keys.len().saturating_add(1)),
        ))? {
            let key = match key {
                Ok(RawValue::String(key)) => key,
                Ok(_) => {
                    self.budget.exit_node();
                    return Ok(Err(DataError::invalid_type(
                        "JSON object key",
                        "non-string",
                    )));
                }
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error));
                }
            };
            if !keys.insert(key.clone()) {
                self.budget.exit_node();
                return Ok(self.budget.record(Err(DataError::new(
                    DataErrorKind::DuplicateField,
                    format!("duplicate field `{key}`"),
                )
                .at_field(key))));
            }
            let value = map.next_value_seed(JsonSeed::<P>::field(self.budget, key.clone()))?;
            match value {
                Ok(value) => P::insert(&mut entries, key, value),
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error.at_field(key)));
                }
            }
        }
        self.budget.exit_node();
        Ok(Ok(P::object(entries)))
    }
}

impl<P: Projection> JsonVisitor<'_, '_, P> {
    fn scalar(self, value: RawValue) -> Result<P::Value> {
        self.budget.enter_node()?;
        self.budget.exit_node();
        Ok(P::scalar(value))
    }
    fn string(self, value: &str) -> Result<P::Value> {
        self.budget.enter_node()?;
        let result = self
            .budget
            .string_len(value.len())
            .map(|()| P::string(value));
        self.budget.exit_node();
        result
    }
}
