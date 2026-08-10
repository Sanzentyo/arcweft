use crate::{
    diagnostic::ResourceManifestDiagnosticCode,
    limits::ResourceManifestDecodeLimits,
    source_map::{JsonPath, JsonTokenRange, ResourceManifestSourceMap},
};
use arcweft_source::SourceRange;
use json_spanned_value::Spanned;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Number, Value};
use std::{collections::BTreeMap, fmt, ops::Range};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StrictJsonError {
    pub(crate) code: ResourceManifestDiagnosticCode,
    pub(crate) message: String,
    pub(crate) primary: Range<usize>,
    pub(crate) related: Option<Range<usize>>,
}

pub(crate) fn parse_strict_json(
    source: &str,
    limits: ResourceManifestDecodeLimits,
) -> Result<(Value, ResourceManifestSourceMap), StrictJsonError> {
    check_lexical_limits(source, limits)?;
    let root = json_spanned_value::from_str::<Spanned<JsonValue>>(source).map_err(|error| {
        StrictJsonError {
            code: ResourceManifestDiagnosticCode::InvalidJson,
            message: format!("invalid resource manifest JSON: {error}"),
            primary: 0..source.len(),
            related: None,
        }
    })?;
    let mut source_map = ResourceManifestSourceMap::default();
    let root = lower_value(&root, &JsonPath::default(), None, &mut source_map, limits)?;
    Ok((root, source_map))
}

fn check_lexical_limits(
    source: &str,
    limits: ResourceManifestDecodeLimits,
) -> Result<(), StrictJsonError> {
    if source.starts_with('\u{feff}') {
        return Err(limit_error(
            ResourceManifestDiagnosticCode::BomNotAllowed,
            "UTF-8 BOM is not admitted",
            0..3,
        ));
    }
    if source.len() > limits.bytes() {
        return Err(limit_error(
            ResourceManifestDiagnosticCode::ByteLimit,
            format!(
                "manifest has {} bytes; maximum is {}",
                source.len(),
                limits.bytes()
            ),
            limits.bytes().min(source.len())..source.len(),
        ));
    }

    let mut depth = 0usize;
    let mut nodes = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut scalar_start = None;
    for (offset, byte) in source.bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if let Some(start) = scalar_start {
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b']' | b'}') {
                check_number_token(source, start..offset)?;
                scalar_start = None;
            } else {
                continue;
            }
        }
        match byte {
            b'"' => {
                charge_node(&mut nodes, limits.lexical_nodes(), offset)?;
                in_string = true;
            }
            b'[' | b'{' => {
                charge_node(&mut nodes, limits.lexical_nodes(), offset)?;
                depth += 1;
                if depth > limits.nesting_depth() {
                    return Err(limit_error(
                        ResourceManifestDiagnosticCode::DepthLimit,
                        format!(
                            "manifest nesting depth {depth} exceeds {}",
                            limits.nesting_depth()
                        ),
                        offset..offset + 1,
                    ));
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            b'-' | b'0'..=b'9' | b't' | b'f' | b'n' => {
                charge_node(&mut nodes, limits.lexical_nodes(), offset)?;
                scalar_start = Some(offset);
            }
            _ => {}
        }
    }
    if let Some(start) = scalar_start {
        check_number_token(source, start..source.len())?;
    }
    Ok(())
}

fn check_number_token(source: &str, range: Range<usize>) -> Result<(), StrictJsonError> {
    let token = &source[range.clone()];
    if !token
        .as_bytes()
        .first()
        .is_some_and(|byte| *byte == b'-' || byte.is_ascii_digit())
    {
        return Ok(());
    }
    if token.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
        return Err(limit_error(
            ResourceManifestDiagnosticCode::InvalidInteger,
            "JSON fractions and exponents are not admitted",
            range,
        ));
    }
    let valid = if token.starts_with('-') {
        token.parse::<i64>().is_ok()
    } else {
        token.parse::<u64>().is_ok()
    };
    if !valid {
        return Err(limit_error(
            ResourceManifestDiagnosticCode::IntegerOverflow,
            "JSON integer exceeds the admitted 64-bit range",
            range,
        ));
    }
    Ok(())
}

fn charge_node(observed: &mut usize, maximum: usize, offset: usize) -> Result<(), StrictJsonError> {
    *observed += 1;
    if *observed > maximum {
        return Err(limit_error(
            ResourceManifestDiagnosticCode::NodeLimit,
            format!("manifest lexical node count exceeds {maximum}"),
            offset..offset + 1,
        ));
    }
    Ok(())
}

fn limit_error(
    code: ResourceManifestDiagnosticCode,
    message: impl Into<String>,
    primary: Range<usize>,
) -> StrictJsonError {
    StrictJsonError {
        code,
        message: message.into(),
        primary,
        related: None,
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
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }
    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(JsonValue::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(JsonValue::Number(value.into()))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(JsonValue::Number(value.into()))
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
        Ok(JsonValue::String(value.into()))
    }
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value))
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
    key_range: Option<Range<usize>>,
    source_map: &mut ResourceManifestSourceMap,
    limits: ResourceManifestDecodeLimits,
) -> Result<Value, StrictJsonError> {
    let range = value.range();
    source_map.insert(
        path.clone(),
        JsonTokenRange::new(
            key_range.clone().map(source_range),
            source_range(range.clone()),
        ),
    );
    match value.get_ref() {
        JsonValue::Null => Err(StrictJsonError {
            code: ResourceManifestDiagnosticCode::NullNotAllowed,
            message: "explicit null is not admitted".into(),
            primary: range,
            related: key_range,
        }),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::Number(value) if value.is_f64() => Err(StrictJsonError {
            code: ResourceManifestDiagnosticCode::InvalidInteger,
            message: "JSON fractions and exponents are not admitted".into(),
            primary: range,
            related: key_range,
        }),
        JsonValue::Number(value) => Ok(Value::Number(value.clone())),
        JsonValue::String(value) => {
            if value.len() > limits.string_bytes() {
                return Err(limit_error(
                    ResourceManifestDiagnosticCode::StringLimit,
                    format!(
                        "decoded string has {} bytes; maximum is {}",
                        value.len(),
                        limits.string_bytes()
                    ),
                    range,
                ));
            }
            Ok(Value::String(value.clone()))
        }
        JsonValue::Array(values) => {
            if values.len() > limits.collection_items() {
                return Err(limit_error(
                    ResourceManifestDiagnosticCode::CollectionLimit,
                    format!(
                        "array has {} items; maximum is {}",
                        values.len(),
                        limits.collection_items()
                    ),
                    range,
                ));
            }
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    lower_value(value, &path.index(index), None, source_map, limits)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        JsonValue::Object(values) => {
            lower_object(values, path, range, source_map, limits).map(Value::Object)
        }
    }
}

fn lower_object(
    values: &[(Spanned<String>, Spanned<JsonValue>)],
    path: &JsonPath,
    range: Range<usize>,
    source_map: &mut ResourceManifestSourceMap,
    limits: ResourceManifestDecodeLimits,
) -> Result<serde_json::Map<String, Value>, StrictJsonError> {
    if values.len() > limits.object_members() {
        return Err(limit_error(
            ResourceManifestDiagnosticCode::RecordLimit,
            format!(
                "object has {} members; maximum is {}",
                values.len(),
                limits.object_members()
            ),
            range,
        ));
    }
    let mut first = BTreeMap::<&str, Range<usize>>::new();
    let mut object = serde_json::Map::new();
    for (key, value) in values {
        let key_text = key.get_ref().as_str();
        let key_span = key.range();
        if key_text.len() > limits.string_bytes() {
            return Err(limit_error(
                ResourceManifestDiagnosticCode::StringLimit,
                format!(
                    "decoded object key has {} bytes; maximum is {}",
                    key_text.len(),
                    limits.string_bytes()
                ),
                key_span,
            ));
        }
        if let Some(first_span) = first.insert(key_text, key_span.clone()) {
            return Err(StrictJsonError {
                code: ResourceManifestDiagnosticCode::DuplicateKey,
                message: format!("duplicate JSON key `{key_text}`"),
                primary: key_span,
                related: Some(first_span),
            });
        }
        object.insert(
            key_text.to_owned(),
            lower_value(
                value,
                &path.field(key_text),
                Some(key_span),
                source_map,
                limits,
            )?,
        );
    }
    Ok(object)
}

fn source_range(range: Range<usize>) -> SourceRange {
    SourceRange::new(range.start, range.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_byte_limit_is_inclusive() {
        let input = include_str!("../tests/fixtures/minimal.input.json");
        let maximum = ResourceManifestDecodeLimits::PRODUCTION.bytes();
        let mut exact = String::with_capacity(maximum + 1);
        exact.push_str(input);
        exact.extend(std::iter::repeat_n(' ', maximum - input.len()));
        assert_eq!(exact.len(), maximum);
        parse_strict_json(&exact, ResourceManifestDecodeLimits::PRODUCTION).unwrap();

        exact.push(' ');
        let error = parse_strict_json(&exact, ResourceManifestDecodeLimits::PRODUCTION)
            .expect_err("one byte over the inclusive production limit must fail");
        assert_eq!(error.code, ResourceManifestDiagnosticCode::ByteLimit);
    }
}
