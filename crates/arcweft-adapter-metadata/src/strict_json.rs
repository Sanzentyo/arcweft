use json_spanned_value::Spanned;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Number, Value};
use std::{collections::BTreeMap, fmt, ops::Range};
use thiserror::Error;

/// Explicit resource limits for one generated adapter-metadata decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterMetadataDecodeLimits {
    bytes: usize,
    nesting_depth: usize,
    nodes: usize,
}

/// Counter whose inclusive adapter-metadata decode limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdapterMetadataDecodeLimitKind {
    Bytes,
    NestingDepth,
    Nodes,
}

/// Typed path into a generated metadata JSON document.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct JsonPath(Vec<JsonPathSegment>);

/// One field or array index in a generated metadata path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum JsonPathSegment {
    Field(Box<str>),
    Index(usize),
}

/// Source ranges for one JSON value and its optional object key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonToken {
    pub key_span: Option<Range<usize>>,
    pub value_span: Range<usize>,
}

/// Source ranges retained from the sole strict JSON parse.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterMetadataSourceMap {
    entries: BTreeMap<JsonPath, JsonToken>,
}

/// Strict generated JSON failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StrictJsonError {
    #[error("invalid generated metadata JSON: {message}")]
    Syntax { message: String },
    #[error(
        "generated metadata JSON exceeded its {kind:?} limit: observed {observed}, maximum {maximum}"
    )]
    Limit {
        kind: AdapterMetadataDecodeLimitKind,
        observed: usize,
        maximum: usize,
        span: Option<Range<usize>>,
    },
    #[error("duplicate JSON key `{key}` at {path:?}")]
    DuplicateKey {
        path: JsonPath,
        key: Box<str>,
        first: Range<usize>,
        duplicate: Range<usize>,
    },
    #[error("explicit null is not admitted at {path:?}")]
    Null { path: JsonPath, span: Range<usize> },
    #[error("floating-point values are not admitted at {path:?}")]
    Float { path: JsonPath, span: Range<usize> },
}

impl AdapterMetadataDecodeLimits {
    /// Production envelope for one generated metadata document.
    ///
    /// The byte ceiling matches the accepted source-document boundary. The
    /// smaller structural ceilings bound source-map allocation and recursive
    /// lowering independently of source length.
    pub const PRODUCTION: Self = Self::new(8_388_608, 64, 65_536);

    /// Creates inclusive byte, nesting, and node ceilings.
    ///
    /// The node ceiling is charged lexically before deserialization, including
    /// containers, scalar values, string values, and object keys. Semantic
    /// values are charged again while the source map is built.
    pub const fn new(bytes: usize, nesting_depth: usize, nodes: usize) -> Self {
        Self {
            bytes,
            nesting_depth,
            nodes,
        }
    }

    pub const fn bytes(self) -> usize {
        self.bytes
    }

    pub const fn nesting_depth(self) -> usize {
        self.nesting_depth
    }

    pub const fn nodes(self) -> usize {
        self.nodes
    }
}

impl JsonPath {
    pub fn segments(&self) -> &[JsonPathSegment] {
        &self.0
    }

    fn field(&self, field: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(JsonPathSegment::Field(field.into()));
        Self(segments)
    }

    fn index(&self, index: usize) -> Self {
        let mut segments = self.0.clone();
        segments.push(JsonPathSegment::Index(index));
        Self(segments)
    }
}

impl AdapterMetadataSourceMap {
    pub fn token(&self, path: &JsonPath) -> Option<&JsonToken> {
        self.entries.get(path)
    }

    pub fn entries(&self) -> &BTreeMap<JsonPath, JsonToken> {
        &self.entries
    }
}

pub(crate) fn parse_strict_json(
    source: &str,
    limits: AdapterMetadataDecodeLimits,
) -> Result<(Value, AdapterMetadataSourceMap), StrictJsonError> {
    check_lexical_limits(source, limits)?;
    let root = json_spanned_value::from_str::<Spanned<JsonValue>>(source).map_err(|error| {
        StrictJsonError::Syntax {
            message: error.to_string(),
        }
    })?;
    let mut source_map = AdapterMetadataSourceMap::default();
    let mut budget = JsonValueBudget::new(limits.nodes());
    let value = lower_value(
        &root,
        &JsonPath::default(),
        None,
        &mut source_map,
        &mut budget,
    )?;
    Ok((value, source_map))
}

fn check_lexical_limits(
    source: &str,
    limits: AdapterMetadataDecodeLimits,
) -> Result<(), StrictJsonError> {
    if source.len() > limits.bytes() {
        return Err(StrictJsonError::Limit {
            kind: AdapterMetadataDecodeLimitKind::Bytes,
            observed: source.len(),
            maximum: limits.bytes(),
            span: None,
        });
    }

    let mut depth = 0usize;
    let mut nodes = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_scalar = false;
    for (offset, byte) in source.bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
            }
            continue;
        }
        if in_scalar {
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b']' | b'}') {
                in_scalar = false;
            } else {
                continue;
            }
        }
        match byte {
            b'"' => {
                charge_lexical_node(&mut nodes, limits.nodes(), offset)?;
                in_string = true;
            }
            b'[' | b'{' => {
                charge_lexical_node(&mut nodes, limits.nodes(), offset)?;
                depth = depth.saturating_add(1);
                if depth > limits.nesting_depth() {
                    return Err(StrictJsonError::Limit {
                        kind: AdapterMetadataDecodeLimitKind::NestingDepth,
                        observed: depth,
                        maximum: limits.nesting_depth(),
                        span: Some(offset..offset + 1),
                    });
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            b'-' | b'0'..=b'9' | b't' | b'f' | b'n' => {
                charge_lexical_node(&mut nodes, limits.nodes(), offset)?;
                in_scalar = true;
            }
            _ => {}
        }
    }
    Ok(())
}

fn charge_lexical_node(
    observed: &mut usize,
    maximum: usize,
    offset: usize,
) -> Result<(), StrictJsonError> {
    *observed = (*observed).saturating_add(1);
    if *observed > maximum {
        return Err(StrictJsonError::Limit {
            kind: AdapterMetadataDecodeLimitKind::Nodes,
            observed: *observed,
            maximum,
            span: Some(offset..offset + 1),
        });
    }
    Ok(())
}

struct JsonValueBudget {
    maximum: usize,
    observed: usize,
}

impl JsonValueBudget {
    const fn new(maximum: usize) -> Self {
        Self {
            maximum,
            observed: 0,
        }
    }

    fn charge(&mut self, span: Range<usize>) -> Result<(), StrictJsonError> {
        self.observed = self.observed.saturating_add(1);
        if self.observed > self.maximum {
            return Err(StrictJsonError::Limit {
                kind: AdapterMetadataDecodeLimitKind::Nodes,
                observed: self.observed,
                maximum: self.maximum,
                span: Some(span),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Spanned<JsonValue>>),
    Object(Vec<(Spanned<String>, Spanned<JsonValue>)>),
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonValueVisitor)
    }
}

struct JsonValueVisitor;

impl<'de> Visitor<'de> for JsonValueVisitor {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(JsonValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(JsonValue::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(JsonValue::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(JsonValue::Number)
            .ok_or_else(|| de::Error::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<Spanned<JsonValue>>()? {
            values.push(value);
        }
        Ok(JsonValue::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(key) = map.next_key::<Spanned<String>>()? {
            values.push((key, map.next_value::<Spanned<JsonValue>>()?));
        }
        Ok(JsonValue::Object(values))
    }
}

fn lower_value(
    value: &Spanned<JsonValue>,
    path: &JsonPath,
    key_span: Option<Range<usize>>,
    source_map: &mut AdapterMetadataSourceMap,
    budget: &mut JsonValueBudget,
) -> Result<Value, StrictJsonError> {
    let span = value.range();
    budget.charge(span.clone())?;
    source_map.entries.insert(
        path.clone(),
        JsonToken {
            key_span,
            value_span: span.clone(),
        },
    );
    match value.get_ref() {
        JsonValue::Null => Err(StrictJsonError::Null {
            path: path.clone(),
            span,
        }),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::Number(value) if value.is_f64() => Err(StrictJsonError::Float {
            path: path.clone(),
            span,
        }),
        JsonValue::Number(value) => Ok(Value::Number(value.clone())),
        JsonValue::String(value) => Ok(Value::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| lower_value(value, &path.index(index), None, source_map, budget))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        JsonValue::Object(values) => {
            let mut first_spans = BTreeMap::<&str, Range<usize>>::new();
            let mut object = serde_json::Map::new();
            for (key, value) in values {
                let key_text = key.get_ref().as_str();
                let key_range = key.range();
                if let Some(first) = first_spans.insert(key_text, key_range.clone()) {
                    return Err(StrictJsonError::DuplicateKey {
                        path: path.clone(),
                        key: key_text.into(),
                        first,
                        duplicate: key_range,
                    });
                }
                let child_path = path.field(key_text);
                object.insert(
                    key_text.to_owned(),
                    lower_value(value, &child_path, Some(key_range), source_map, budget)?,
                );
            }
            Ok(Value::Object(object))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterMetadataDecodeLimitKind, AdapterMetadataDecodeLimits, JsonPathSegment,
        StrictJsonError, parse_strict_json,
    };

    #[test]
    fn rejects_duplicate_keys_with_both_ranges() {
        let source = r#"{"package":1,"package":2}"#;
        let error = parse_strict_json(source, AdapterMetadataDecodeLimits::PRODUCTION).unwrap_err();
        let StrictJsonError::DuplicateKey {
            first, duplicate, ..
        } = error
        else {
            panic!("unexpected error")
        };
        assert_eq!(&source[first], "\"package\"");
        assert_eq!(&source[duplicate], "\"package\"");
    }

    #[test]
    fn retains_array_element_value_spans() {
        let source = r#"{"values":[10,20]}"#;
        let (_, map) = parse_strict_json(source, AdapterMetadataDecodeLimits::PRODUCTION).unwrap();
        let (path, token) = map
            .entries()
            .iter()
            .find(|(path, _)| {
                path.segments()
                    == [
                        JsonPathSegment::Field("values".into()),
                        JsonPathSegment::Index(1),
                    ]
            })
            .unwrap();
        assert_eq!(path.segments().len(), 2);
        assert_eq!(&source[token.value_span.clone()], "20");
    }

    #[test]
    fn enforces_byte_depth_and_node_limits_before_unbounded_lowering() {
        let byte_error = parse_strict_json(
            "{}",
            AdapterMetadataDecodeLimits::new(1, usize::MAX, usize::MAX),
        )
        .unwrap_err();
        assert!(matches!(
            byte_error,
            StrictJsonError::Limit {
                kind: AdapterMetadataDecodeLimitKind::Bytes,
                observed: 2,
                maximum: 1,
                span: None,
            }
        ));

        let nested = r#"{"brackets":"[[[","value":[[0]]]}"#;
        let depth_error = parse_strict_json(
            nested,
            AdapterMetadataDecodeLimits::new(usize::MAX, 2, usize::MAX),
        )
        .unwrap_err();
        assert!(matches!(
            depth_error,
            StrictJsonError::Limit {
                kind: AdapterMetadataDecodeLimitKind::NestingDepth,
                observed: 3,
                maximum: 2,
                span: Some(_),
            }
        ));

        let node_error = parse_strict_json(
            "[0,1]",
            AdapterMetadataDecodeLimits::new(usize::MAX, usize::MAX, 2),
        )
        .unwrap_err();
        assert!(matches!(
            node_error,
            StrictJsonError::Limit {
                kind: AdapterMetadataDecodeLimitKind::Nodes,
                observed: 3,
                maximum: 2,
                span: Some(_),
            }
        ));
    }
}
