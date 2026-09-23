#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    Bytes, BytesFormat, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions,
    EncodeOptions, EnumRepr, EnumTagStyle, FieldShape, FormatId, RawValue, Result, ShapeAccess,
    ShapeRef, TypeShape, Value, VariantShape, decode_with_shape_ref, encode_with_shape_ref,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number as JsonNumber, Value as JsonValue};

#[derive(Clone, Copy, Debug, Default)]
pub struct JsonCodec;

impl Codec for JsonCodec {
    fn id(&self) -> FormatId {
        FormatId::new("json")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/json", "text/json"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape_ref(value, shape, access)?.into_tagged_options();
        let json = raw_to_json_value(&raw, shape, access)?;
        let bytes = if options.pretty {
            serde_json::to_vec_pretty(&json)
        } else {
            serde_json::to_vec(&json)
        }
        .map_err(|error| json_error(&error))?;
        Ok(bytes)
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let mut deserializer = serde_json::Deserializer::from_slice(input);
        let dynamic_raw = BudgetedJsonRawSeed {
            budget: &mut budget,
        }
        .deserialize(&mut deserializer)
        .map_err(|error| json_error(&error))??;
        if !options.limits.allow_trailing_data {
            deserializer.end().map_err(|error| json_error(&error))?;
        }
        let json = raw_dynamic_to_json(&dynamic_raw)?;
        let raw = json_to_raw_value(&json, shape, access)?;
        let value = decode_with_shape_ref(&raw, shape, access)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

struct BudgetedJsonRawSeed<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
}

impl<'de> DeserializeSeed<'de> for BudgetedJsonRawSeed<'_, '_> {
    type Value = Result<RawValue>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(BudgetedJsonRawVisitor {
            budget: self.budget,
        })
    }
}

struct BudgetedJsonRawVisitor<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
}

impl<'de> Visitor<'de> for BudgetedJsonRawVisitor<'_, '_> {
    type Value = Result<RawValue>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Null))
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Signed(i128::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Unsigned(u128::from(value))))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::F64(value)))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.string(value))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.string(value))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.string(value.as_str()))
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if let Err(error) = self.budget.enter_node() {
            return Ok(Err(error));
        }
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(BudgetedJsonRawSeed {
            budget: self.budget,
        })? {
            if let Err(error) = self.budget.sequence_item(values.len().saturating_add(1)) {
                self.budget.exit_node();
                return Ok(Err(error));
            }
            match value {
                Ok(value) => values.push(value),
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error.at_index(values.len())));
                }
            }
        }
        self.budget.exit_node();
        Ok(Ok(RawValue::Seq(values)))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        if let Err(error) = self.budget.enter_node() {
            return Ok(Err(error));
        }
        let mut entries = Vec::new();
        while let Some(key) = map.next_key_seed(BudgetedJsonRawSeed {
            budget: self.budget,
        })? {
            if let Err(error) = self.budget.map_item(entries.len().saturating_add(1)) {
                self.budget.exit_node();
                return Ok(Err(error));
            }
            let key = match key {
                Ok(key) => key,
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error));
                }
            };
            let value = map.next_value_seed(BudgetedJsonRawSeed {
                budget: self.budget,
            })?;
            let value = match value {
                Ok(value) => value,
                Err(error) => {
                    self.budget.exit_node();
                    return Ok(Err(error));
                }
            };
            entries.push((key, value));
        }
        self.budget.exit_node();
        Ok(Ok(RawValue::Map(entries)))
    }
}

impl BudgetedJsonRawVisitor<'_, '_> {
    fn scalar(self, raw: RawValue) -> Result<RawValue> {
        self.budget.enter_node()?;
        self.budget.exit_node();
        Ok(raw)
    }

    fn string(self, value: &str) -> Result<RawValue> {
        self.budget.enter_node()?;
        if let Err(error) = self.budget.string_len(value.len()) {
            self.budget.exit_node();
            return Err(error);
        }
        self.budget.exit_node();
        Ok(RawValue::String(value.to_owned()))
    }
}

const OPTION_TAG_KEY: &str = "$arcweft";
const OPTION_TAG_VALUE: &str = "option";
const OPTION_PRESENT_KEY: &str = "present";
const OPTION_VALUE_KEY: &str = "value";

fn raw_to_json_value(
    raw: &RawValue,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    let shape = shape_ref.resolve(access)?;
    match (shape.as_ref(), raw) {
        (TypeShape::Ref(id), raw) => raw_to_json_value(raw, ShapeRef::Id(*id), access),
        (TypeShape::Unit, RawValue::Null) => Ok(JsonValue::Null),
        (TypeShape::Bool, RawValue::Bool(value)) => Ok(JsonValue::Bool(*value)),
        (
            TypeShape::I8
            | TypeShape::I16
            | TypeShape::I32
            | TypeShape::I64
            | TypeShape::I128
            | TypeShape::Isize,
            RawValue::Signed(value),
        ) => signed_to_json(*value),
        (
            TypeShape::U8
            | TypeShape::U16
            | TypeShape::U32
            | TypeShape::U64
            | TypeShape::U128
            | TypeShape::Usize,
            RawValue::Unsigned(value),
        ) => unsigned_to_json(*value),
        (TypeShape::F32, RawValue::F32(value)) => float_to_json(f64::from(*value), "f32"),
        (TypeShape::F64, RawValue::F64(value)) => float_to_json(*value, "f64"),
        (TypeShape::String | TypeShape::Char, RawValue::String(value)) => {
            Ok(JsonValue::String(value.clone()))
        }
        (TypeShape::Bytes { format }, RawValue::Bytes(bytes)) => {
            bytes_to_json(&Bytes::new(bytes.clone()), *format)
        }
        (TypeShape::Option(inner), raw) => raw_tagged_option_to_json(raw, inner, access),
        (TypeShape::Seq(inner), RawValue::Seq(values)) => {
            raw_seq_to_json(values, ShapeRef::Inline(inner), access)
        }
        (TypeShape::Tuple(items), RawValue::Seq(values)) if items.len() == values.len() => items
            .iter()
            .zip(values)
            .enumerate()
            .map(|(index, (item_shape, value))| {
                raw_to_json_value(value, ShapeRef::Inline(item_shape), access)
                    .map_err(|error| error.at_index(index))
            })
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        (TypeShape::Tuple(items), RawValue::Seq(values)) => Err(DataError::invalid_type(
            format!("tuple with {} items", items.len()),
            format!("tuple with {} items", values.len()),
        )),
        (TypeShape::Map { key, value, .. }, RawValue::Map(entries)) => {
            let key_shape = ShapeRef::Inline(key).resolve(access)?;
            if matches!(key_shape.as_ref(), TypeShape::String) {
                raw_string_map_to_json(entries, value, access)
            } else {
                raw_map_pairs_to_json(entries, key, value, access)
            }
        }
        (TypeShape::Record { fields, .. }, RawValue::Map(entries)) => {
            raw_record_to_json(entries, fields, access)
        }
        (
            TypeShape::Enum {
                variants,
                tag,
                repr,
                ..
            },
            raw,
        ) => raw_enum_to_json(raw, variants, tag, *repr, access),
        (shape, raw) => Err(DataError::invalid_type(shape.type_name(), raw.type_name())),
    }
}

fn raw_enum_to_json(
    raw: &RawValue,
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<EnumRepr>,
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    if repr.is_some() {
        return raw_dynamic_to_json(raw);
    }
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type("enum map", raw.type_name()));
    };
    let fields = raw_enum_fields(entries)?;
    let tag_key = match tag {
        EnumTagStyle::External => "variant",
        EnumTagStyle::Internal { tag } | EnumTagStyle::Adjacent { tag, .. } => tag,
    };
    let variant = match fields.get(tag_key) {
        Some(RawValue::String(variant)) => variant.as_str(),
        Some(other) => {
            return Err(DataError::invalid_type(
                "enum tag string",
                other.type_name(),
            ));
        }
        None => {
            return Err(DataError::new(
                DataErrorKind::MissingField,
                format!("missing enum tag field `{tag_key}`"),
            )
            .at_field(tag_key.to_owned()));
        }
    };
    let case = json_enum_case(variants, variant)?;
    match tag {
        EnumTagStyle::External => {
            let mut object = Map::new();
            object.insert("variant".to_owned(), JsonValue::String(variant.to_owned()));
            match (&case.payload, fields.get("payload")) {
                (Some(shape), Some(raw)) => {
                    object.insert(
                        "payload".to_owned(),
                        raw_to_json_value(raw, ShapeRef::Inline(shape), access)
                            .map_err(|error| error.at_variant(variant))?,
                    );
                }
                (None, None) => {}
                (Some(_), None) => {
                    return Err(DataError::new(
                        DataErrorKind::MissingField,
                        format!("missing payload for enum variant `{variant}`"),
                    )
                    .at_variant(variant));
                }
                (None, Some(_)) => {
                    return Err(
                        DataError::invalid_type("unit enum variant", "payload").at_variant(variant)
                    );
                }
            }
            Ok(JsonValue::Object(object))
        }
        EnumTagStyle::Adjacent { tag, content } => {
            let mut object = Map::new();
            object.insert(tag.clone(), JsonValue::String(variant.to_owned()));
            match (&case.payload, fields.get(content.as_str())) {
                (Some(shape), Some(raw)) => {
                    object.insert(
                        content.clone(),
                        raw_to_json_value(raw, ShapeRef::Inline(shape), access)
                            .map_err(|error| error.at_variant(variant))?,
                    );
                }
                (None, None) => {}
                (Some(_), None) => {
                    return Err(DataError::new(
                        DataErrorKind::MissingField,
                        format!("missing enum content field `{content}`"),
                    )
                    .at_variant(variant)
                    .at_field(content.clone()));
                }
                (None, Some(_)) => {
                    return Err(
                        DataError::invalid_type("unit enum variant", "payload").at_variant(variant)
                    );
                }
            }
            Ok(JsonValue::Object(object))
        }
        EnumTagStyle::Internal { tag } => {
            let payload = RawValue::Map(
                entries
                    .iter()
                    .filter(|(key, _)| !matches!(key, RawValue::String(key) if key == tag))
                    .cloned()
                    .collect(),
            );
            let mut object = Map::new();
            object.insert(tag.clone(), JsonValue::String(variant.to_owned()));
            match &case.payload {
                Some(shape) => {
                    let JsonValue::Object(payload) =
                        raw_to_json_value(&payload, ShapeRef::Inline(shape), access)
                            .map_err(|error| error.at_variant(variant))?
                    else {
                        return Err(DataError::unsupported(
                            "internally tagged enum payload must encode as a record",
                        )
                        .at_variant(variant));
                    };
                    for (key, value) in payload {
                        if object.insert(key.clone(), value).is_some() {
                            return Err(DataError::new(
                                DataErrorKind::DuplicateField,
                                format!("internal enum payload duplicates tag field `{key}`"),
                            )
                            .at_variant(variant)
                            .at_field(key));
                        }
                    }
                }
                None if fields.len() == 1 => {}
                None => {
                    for (key, value) in fields {
                        if key != tag.as_str() {
                            object.insert(key.to_owned(), raw_dynamic_to_json(value)?);
                        }
                    }
                }
            }
            Ok(JsonValue::Object(object))
        }
    }
}

fn json_enum_to_raw(
    value: &JsonValue,
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<EnumRepr>,
    access: &dyn ShapeAccess,
) -> Result<RawValue> {
    if repr.is_some() {
        return json_integer_to_raw(value);
    }
    let JsonValue::Object(fields) = value else {
        return Err(DataError::invalid_type(
            "enum object",
            json_type_name(value),
        ));
    };
    let tag_key = match tag {
        EnumTagStyle::External => "variant",
        EnumTagStyle::Internal { tag } | EnumTagStyle::Adjacent { tag, .. } => tag,
    };
    let variant = match fields.get(tag_key) {
        Some(JsonValue::String(variant)) => variant.as_str(),
        Some(other) => {
            return Err(DataError::invalid_type(
                "enum tag string",
                json_type_name(other),
            ));
        }
        None => {
            return Err(DataError::new(
                DataErrorKind::MissingField,
                format!("missing enum tag field `{tag_key}`"),
            )
            .at_field(tag_key.to_owned()));
        }
    };
    let case = json_enum_case(variants, variant)?;
    let mut entries = vec![(
        RawValue::String(tag_key.to_owned()),
        RawValue::String(variant.to_owned()),
    )];
    match tag {
        EnumTagStyle::External => {
            match (&case.payload, fields.get("payload")) {
                (Some(shape), Some(payload)) => entries.push((
                    RawValue::String("payload".to_owned()),
                    json_to_raw_value(payload, ShapeRef::Inline(shape), access)
                        .map_err(|error| error.at_variant(variant))?,
                )),
                (None, Some(payload)) => entries.push((
                    RawValue::String("payload".to_owned()),
                    json_dynamic_to_raw(payload)?,
                )),
                _ => {}
            }
            for (key, value) in fields {
                if key != tag_key && key != "payload" {
                    entries.push((RawValue::String(key.clone()), json_dynamic_to_raw(value)?));
                }
            }
        }
        EnumTagStyle::Adjacent { content, .. } => {
            match (&case.payload, fields.get(content)) {
                (Some(shape), Some(payload)) => entries.push((
                    RawValue::String(content.clone()),
                    json_to_raw_value(payload, ShapeRef::Inline(shape), access)
                        .map_err(|error| error.at_variant(variant))?,
                )),
                (None, Some(payload)) => entries.push((
                    RawValue::String(content.clone()),
                    json_dynamic_to_raw(payload)?,
                )),
                _ => {}
            }
            for (key, value) in fields {
                if key != tag_key && key != content {
                    entries.push((RawValue::String(key.clone()), json_dynamic_to_raw(value)?));
                }
            }
        }
        EnumTagStyle::Internal { .. } => {
            let mut payload_fields = fields.clone();
            payload_fields.remove(tag_key);
            if let Some(shape) = &case.payload {
                let payload = json_to_raw_value(
                    &JsonValue::Object(payload_fields),
                    ShapeRef::Inline(shape),
                    access,
                )
                .map_err(|error| error.at_variant(variant))?;
                let RawValue::Map(payload_entries) = payload else {
                    return Err(DataError::unsupported(
                        "internally tagged enum payload must decode as a record",
                    )
                    .at_variant(variant));
                };
                entries.extend(payload_entries);
            } else {
                for (key, value) in payload_fields {
                    entries.push((RawValue::String(key), json_dynamic_to_raw(&value)?));
                }
            }
        }
    }
    Ok(RawValue::Map(entries))
}

fn raw_enum_fields(entries: &[(RawValue, RawValue)]) -> Result<BTreeMap<&str, &RawValue>> {
    let mut fields = BTreeMap::new();
    for (key, value) in entries {
        let RawValue::String(key) = key else {
            return Err(DataError::invalid_type(
                "string enum field",
                key.type_name(),
            ));
        };
        if fields.insert(key.as_str(), value).is_some() {
            return Err(DataError::new(
                DataErrorKind::DuplicateField,
                format!("duplicate enum field `{key}`"),
            )
            .at_field(key.clone()));
        }
    }
    Ok(fields)
}

fn json_enum_case<'a>(variants: &'a [VariantShape], name: &str) -> Result<&'a VariantShape> {
    variants
        .iter()
        .find(|case| case.wire_name == name)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("unknown enum variant `{name}`"),
            )
        })
}

fn raw_tagged_option_to_json(
    raw: &RawValue,
    inner: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    let payload = tagged_raw_option_payload(raw)?;
    let mut object = Map::new();
    object.insert(
        OPTION_TAG_KEY.to_owned(),
        JsonValue::String(OPTION_TAG_VALUE.to_owned()),
    );
    object.insert(
        OPTION_PRESENT_KEY.to_owned(),
        JsonValue::Bool(payload.is_some()),
    );
    if let Some(payload) = payload {
        object.insert(
            OPTION_VALUE_KEY.to_owned(),
            raw_to_json_value(payload, ShapeRef::Inline(inner), access)?,
        );
    }
    Ok(JsonValue::Object(object))
}

fn tagged_raw_option_payload(raw: &RawValue) -> Result<Option<&RawValue>> {
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type(
            "tagged option map",
            raw.type_name(),
        ));
    };
    let mut fields = BTreeMap::<&str, &RawValue>::new();
    for (key, value) in entries {
        let RawValue::String(key) = key else {
            return Err(DataError::invalid_type(
                "string option marker key",
                key.type_name(),
            ));
        };
        if fields.insert(key, value).is_some() {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "tagged option contains duplicate fields",
            ));
        }
    }
    if !matches!(fields.get(OPTION_TAG_KEY), Some(RawValue::String(tag)) if tag == OPTION_TAG_VALUE)
    {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "option value is missing its Arcweft marker",
        ));
    }
    match fields.get(OPTION_PRESENT_KEY) {
        Some(RawValue::Bool(false)) if fields.len() == 2 => Ok(None),
        Some(RawValue::Bool(true)) if fields.len() == 3 => fields
            .get(OPTION_VALUE_KEY)
            .copied()
            .map(Some)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::MissingField,
                    "tagged present option is missing its value",
                )
                .at_field(OPTION_VALUE_KEY)
            }),
        _ => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "tagged option fields do not match its presence marker",
        )),
    }
}

fn signed_to_json(value: i128) -> Result<JsonValue> {
    i64::try_from(value)
        .map(JsonNumber::from)
        .map(JsonValue::Number)
        .map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "signed integer cannot be represented as JSON number",
            )
        })
}

fn unsigned_to_json(value: u128) -> Result<JsonValue> {
    u64::try_from(value)
        .map(JsonNumber::from)
        .map(JsonValue::Number)
        .map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "unsigned integer cannot be represented as JSON number",
            )
        })
}

fn float_to_json(value: f64, label: &'static str) -> Result<JsonValue> {
    JsonNumber::from_f64(value)
        .map(JsonValue::Number)
        .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, format!("invalid {label}")))
}

fn raw_seq_to_json(
    values: &[RawValue],
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            raw_to_json_value(value, shape, access).map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(JsonValue::Array)
}

fn raw_string_map_to_json(
    entries: &[(RawValue, RawValue)],
    shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("string map key", key.type_name()));
            };
            raw_to_json_value(raw_value, ShapeRef::Inline(shape), access)
                .map(|json| (key.clone(), json))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<Map<_, _>>>()
        .map(JsonValue::Object)
}

fn raw_map_pairs_to_json(
    entries: &[(RawValue, RawValue)],
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    entries
        .iter()
        .enumerate()
        .map(|(index, (key, value))| {
            let key = raw_to_json_value(key, ShapeRef::Inline(key_shape), access)
                .map_err(|error| error.at_index(index))?;
            let value = raw_to_json_value(value, ShapeRef::Inline(value_shape), access)
                .map_err(|error| error.at_index(index))?;
            Ok(JsonValue::Array(vec![key, value]))
        })
        .collect::<Result<Vec<_>>>()
        .map(JsonValue::Array)
}

fn raw_record_to_json(
    entries: &[(RawValue, RawValue)],
    fields: &[FieldShape],
    access: &dyn ShapeAccess,
) -> Result<JsonValue> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("record field key", key.type_name()));
            };
            let value = match fields.iter().find(|field| field.wire_name == *key) {
                Some(field) => {
                    let shape = field
                        .resolve_value_shape(access)
                        .map_err(|error| error.at_field(key.clone()))?;
                    raw_to_json_value(raw_value, ShapeRef::Inline(shape.as_ref()), access)
                }
                None => raw_to_json_value(raw_value, ShapeRef::Inline(&TypeShape::Unit), access),
            }
            .map_err(|error| error.at_field(key.clone()))?;
            Ok((key.clone(), value))
        })
        .collect::<Result<Map<_, _>>>()
        .map(JsonValue::Object)
}

fn json_to_raw_value(
    value: &JsonValue,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<RawValue> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => json_to_raw_value(value, ShapeRef::Id(*id), access),
        TypeShape::Unit => match value {
            JsonValue::Null => Ok(RawValue::Null),
            other => Err(DataError::invalid_type("null", json_type_name(other))),
        },
        TypeShape::Bool => match value {
            JsonValue::Bool(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", json_type_name(other))),
        },
        TypeShape::String | TypeShape::Char => match value {
            JsonValue::String(value) => Ok(RawValue::String(value.clone())),
            other => Err(DataError::invalid_type("string", json_type_name(other))),
        },
        TypeShape::Bytes { format } => json_to_bytes(value, *format).map(RawValue::Bytes),
        TypeShape::Option(inner) => tagged_json_option_payload(value)?
            .map(|payload| {
                json_to_raw_value(payload, ShapeRef::Inline(inner), access).map(Box::new)
            })
            .transpose()
            .map(|payload| RawValue::Option(payload)),
        TypeShape::Seq(inner) => match value {
            JsonValue::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    json_to_raw_value(value, ShapeRef::Inline(inner), access)
                        .map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("array", json_type_name(other))),
        },
        TypeShape::Tuple(items) => match value {
            JsonValue::Array(values) if values.len() == items.len() => items
                .iter()
                .zip(values)
                .enumerate()
                .map(|(index, (item_shape, value))| {
                    json_to_raw_value(value, ShapeRef::Inline(item_shape), access)
                        .map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            JsonValue::Array(values) => Err(DataError::invalid_type(
                format!("tuple with {} items", items.len()),
                format!("tuple with {} items", values.len()),
            )),
            other => Err(DataError::invalid_type(
                "tuple array",
                json_type_name(other),
            )),
        },
        TypeShape::Map {
            key, value: inner, ..
        } => {
            let key_shape = ShapeRef::Inline(key).resolve(access)?;
            if matches!(key_shape.as_ref(), TypeShape::String) {
                json_object_entries(value, inner, access).map(RawValue::Map)
            } else {
                json_pair_entries(value, key, inner, access).map(RawValue::Map)
            }
        }
        TypeShape::Record { fields, .. } => match value {
            JsonValue::Object(entries) => entries
                .iter()
                .map(|(key, value)| {
                    let raw = match fields.iter().find(|field| field.wire_name == *key) {
                        Some(field) => {
                            let shape = field
                                .resolve_value_shape(access)
                                .map_err(|error| error.at_field(key.clone()))?;
                            json_to_raw_value(value, ShapeRef::Inline(shape.as_ref()), access)
                                .map_err(|error| error.at_field(key.clone()))
                        }
                        None => json_dynamic_to_raw(value),
                    }?;
                    Ok((RawValue::String(key.clone()), raw))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Map),
            other => Err(DataError::invalid_type("object", json_type_name(other))),
        },
        TypeShape::Enum {
            variants,
            tag,
            repr,
            ..
        } => json_enum_to_raw(value, variants, tag, *repr, access),
        TypeShape::F32 | TypeShape::F64 => json_float_to_raw(value, shape.as_ref()),
        TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::I128
        | TypeShape::Isize
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::U128
        | TypeShape::Usize => json_integer_to_raw(value),
    }
}

fn tagged_json_option_payload(value: &JsonValue) -> Result<Option<&JsonValue>> {
    let JsonValue::Object(fields) = value else {
        return Err(DataError::invalid_type(
            "tagged option object",
            json_type_name(value),
        ));
    };
    if !matches!(fields.get(OPTION_TAG_KEY), Some(JsonValue::String(tag)) if tag == OPTION_TAG_VALUE)
    {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "option value is missing its Arcweft marker",
        ));
    }
    match fields.get(OPTION_PRESENT_KEY) {
        Some(JsonValue::Bool(false)) if fields.len() == 2 => Ok(None),
        Some(JsonValue::Bool(true)) if fields.len() == 3 => {
            fields.get(OPTION_VALUE_KEY).map(Some).ok_or_else(|| {
                DataError::new(
                    DataErrorKind::MissingField,
                    "tagged present option is missing its value",
                )
                .at_field(OPTION_VALUE_KEY)
            })
        }
        _ => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "tagged option fields do not match its presence marker",
        )),
    }
}

fn json_object_entries(
    value: &JsonValue,
    shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Vec<(RawValue, RawValue)>> {
    match value {
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(key, value)| {
                json_to_raw_value(value, ShapeRef::Inline(shape), access)
                    .map(|raw| (RawValue::String(key.clone()), raw))
                    .map_err(|error| error.at_field(key.clone()))
            })
            .collect(),
        other => Err(DataError::invalid_type("object", json_type_name(other))),
    }
}

fn json_pair_entries(
    value: &JsonValue,
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Vec<(RawValue, RawValue)>> {
    let JsonValue::Array(entries) = value else {
        return Err(DataError::invalid_type(
            "map pair array",
            json_type_name(value),
        ));
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let JsonValue::Array(pair) = entry else {
                return Err(
                    DataError::invalid_type("map entry pair", json_type_name(entry))
                        .at_index(index),
                );
            };
            let [key, value] = pair.as_slice() else {
                return Err(
                    DataError::invalid_type("map entry pair of length 2", "other length")
                        .at_index(index),
                );
            };
            let key = json_to_raw_value(key, ShapeRef::Inline(key_shape), access)
                .map_err(|error| error.at_index(index))?;
            let value = json_to_raw_value(value, ShapeRef::Inline(value_shape), access)
                .map_err(|error| error.at_index(index))?;
            Ok((key, value))
        })
        .collect()
}

fn raw_map_to_json(entries: &[(RawValue, RawValue)]) -> Result<JsonValue> {
    let mut object = Map::new();
    for (key, value) in entries {
        let RawValue::String(key) = key else {
            return Err(DataError::invalid_type("object key", key.type_name()));
        };
        if object.contains_key(key) {
            return Err(DataError::new(
                DataErrorKind::DuplicateField,
                format!("duplicate JSON object key `{key}`"),
            )
            .at_field(key.clone()));
        }
        object.insert(key.clone(), raw_dynamic_to_json(value)?);
    }
    Ok(JsonValue::Object(object))
}

fn json_integer_to_raw(value: &JsonValue) -> Result<RawValue> {
    let JsonValue::Number(number) = value else {
        return Err(DataError::invalid_type("number", json_type_name(value)));
    };
    if let Some(value) = number.as_i64() {
        return Ok(RawValue::Signed(i128::from(value)));
    }
    if let Some(value) = number.as_u64() {
        return Ok(RawValue::Unsigned(u128::from(value)));
    }
    Err(DataError::new(
        DataErrorKind::InvalidEncoding,
        "floating-point JSON number cannot decode as integer",
    ))
}

fn json_float_to_raw(value: &JsonValue, shape: &TypeShape) -> Result<RawValue> {
    let JsonValue::Number(number) = value else {
        return Err(DataError::invalid_type("number", json_type_name(value)));
    };
    let Some(value) = number.as_f64() else {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "invalid JSON float",
        ));
    };
    match shape {
        TypeShape::F32 => parse_json_f32(value).map(RawValue::F32),
        TypeShape::F64 => Ok(RawValue::F64(value)),
        _ => unreachable!("caller passes float shape"),
    }
}

fn parse_json_f32(value: f64) -> Result<f32> {
    let decoded = value.to_string().parse::<f32>().map_err(|error| {
        DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("cannot decode f32 from JSON number: {error}"),
        )
    })?;
    if decoded.is_finite() {
        Ok(decoded)
    } else {
        Err(DataError::new(
            DataErrorKind::NumberOutOfRange,
            "JSON number is out of range for f32",
        ))
    }
}

fn json_dynamic_to_raw(value: &JsonValue) -> Result<RawValue> {
    match value {
        JsonValue::Null => Ok(RawValue::Null),
        JsonValue::Bool(value) => Ok(RawValue::Bool(*value)),
        JsonValue::Number(number) => {
            if let Some(value) = number.as_i64() {
                return Ok(RawValue::Signed(i128::from(value)));
            }
            if let Some(value) = number.as_u64() {
                return Ok(RawValue::Unsigned(u128::from(value)));
            }
            number.as_f64().map(RawValue::F64).ok_or_else(|| {
                DataError::new(DataErrorKind::InvalidEncoding, "invalid JSON number")
            })
        }
        JsonValue::String(value) => Ok(RawValue::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(json_dynamic_to_raw)
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Seq),
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(key, value)| {
                json_dynamic_to_raw(value).map(|raw| (RawValue::String(key.clone()), raw))
            })
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Map),
    }
}

fn raw_dynamic_to_json(raw: &RawValue) -> Result<JsonValue> {
    match raw {
        RawValue::Null => Ok(JsonValue::Null),
        RawValue::Bool(value) => Ok(JsonValue::Bool(*value)),
        RawValue::Signed(value) => i64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(DataErrorKind::NumberOutOfRange, "i128 outside JSON range")
            }),
        RawValue::Unsigned(value) => u64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(DataErrorKind::NumberOutOfRange, "u128 outside JSON range")
            }),
        RawValue::F32(value) => JsonNumber::from_f64(f64::from(*value))
            .map(JsonValue::Number)
            .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, "invalid f32")),
        RawValue::F64(value) => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, "invalid f64")),
        RawValue::String(value) => Ok(JsonValue::String(value.clone())),
        RawValue::Bytes(value) => bytes_to_json(&Bytes::new(value.clone()), BytesFormat::Base64),
        RawValue::Seq(values) => values
            .iter()
            .map(raw_dynamic_to_json)
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        RawValue::Map(entries) => raw_map_to_json(entries),
        RawValue::Option(_) => Err(DataError::unsupported(
            "raw option requires shape-aware JSON encoding",
        )),
    }
}

fn json_to_bytes(value: &JsonValue, bytes_format: BytesFormat) -> Result<Vec<u8>> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let JsonValue::String(value) = value else {
                return Err(DataError::invalid_type(
                    "base64 string",
                    json_type_name(value),
                ));
            };
            BASE64_STANDARD
                .decode(value.as_bytes())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        BytesFormat::Hex => {
            let JsonValue::String(value) = value else {
                return Err(DataError::invalid_type("hex string", json_type_name(value)));
            };
            decode_hex(value)
        }
        BytesFormat::Array => {
            let JsonValue::Array(values) = value else {
                return Err(DataError::invalid_type("byte array", json_type_name(value)));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let JsonValue::Number(number) = value else {
                        return Err(
                            DataError::invalid_type("byte", json_type_name(value)).at_index(index)
                        );
                    };
                    let Some(value) = number.as_u64() else {
                        return Err(
                            DataError::invalid_type("byte", "negative or float").at_index(index)
                        );
                    };
                    u8::try_from(value)
                        .map_err(|_| {
                            DataError::new(
                                DataErrorKind::NumberOutOfRange,
                                format!("byte value {value} exceeds 255"),
                            )
                        })
                        .map_err(|error| error.at_index(index))
                })
                .collect()
        }
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    let chunks = value.as_bytes().chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "hex byte string has odd length",
        ));
    }
    chunks
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            u8::from_str_radix(text, 16)
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        })
        .collect()
}

const fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

pub fn to_json_value(value: &Value, bytes_format: BytesFormat) -> Result<JsonValue> {
    match value {
        Value::Unit => Ok(JsonValue::Null),
        Value::Bool(value) => Ok(JsonValue::Bool(*value)),
        Value::Number(arcweft_data::Number::I(value)) => i64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "i128 cannot be represented as JSON number",
                )
            }),
        Value::Number(arcweft_data::Number::U(value)) => u64::try_from(*value)
            .map(JsonNumber::from)
            .map(JsonValue::Number)
            .map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "u128 cannot be represented as JSON number",
                )
            }),
        Value::Number(arcweft_data::Number::F32(value)) => JsonNumber::from_f64(f64::from(*value))
            .map(JsonValue::Number)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "non-finite f32 cannot be represented as JSON number",
                )
            }),
        Value::Number(arcweft_data::Number::F64(value)) => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "non-finite f64 cannot be represented as JSON number",
                )
            }),
        Value::String(value) => Ok(JsonValue::String(value.clone())),
        Value::Char(value) => Ok(JsonValue::String(value.to_string())),
        Value::Bytes(bytes) => bytes_to_json(bytes, bytes_format),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                to_json_value(value, bytes_format).map_err(|err| err.at_index(index))
            })
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        Value::Tuple(values) => values
            .iter()
            .map(|value| to_json_value(value, bytes_format))
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        Value::Option(value) => {
            let mut object = Map::new();
            object.insert(
                OPTION_TAG_KEY.to_owned(),
                JsonValue::String(OPTION_TAG_VALUE.to_owned()),
            );
            object.insert(
                OPTION_PRESENT_KEY.to_owned(),
                JsonValue::Bool(value.is_some()),
            );
            if let Some(value) = value {
                object.insert(
                    OPTION_VALUE_KEY.to_owned(),
                    to_json_value(value, bytes_format)?,
                );
            }
            Ok(JsonValue::Object(object))
        }
        Value::Map { entries, .. } => {
            if entries
                .iter()
                .all(|(key, _)| matches!(key, Value::String(_)))
            {
                let mut object = Map::new();
                for (key, value) in entries {
                    let Value::String(key) = key else {
                        unreachable!()
                    };
                    if object.contains_key(key) {
                        return Err(DataError::new(
                            DataErrorKind::DuplicateField,
                            format!("duplicate JSON map key `{key}`"),
                        )
                        .at_field(key.clone()));
                    }
                    object.insert(
                        key.clone(),
                        to_json_value(value, bytes_format)
                            .map_err(|error| error.at_field(key.clone()))?,
                    );
                }
                Ok(JsonValue::Object(object))
            } else {
                entries
                    .iter()
                    .enumerate()
                    .map(|(index, (key, value))| {
                        let key = to_json_value(key, bytes_format)
                            .map_err(|error| error.at_index(index))?;
                        let value = to_json_value(value, bytes_format)
                            .map_err(|error| error.at_index(index))?;
                        Ok(JsonValue::Array(vec![key, value]))
                    })
                    .collect::<Result<Vec<_>>>()
                    .map(JsonValue::Array)
            }
        }
        Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_json_value(value, bytes_format)
                    .map(|json| (key.clone(), json))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<Map<_, _>>>()
            .map(JsonValue::Object),
        Value::Enum { variant, payload } => {
            let mut object = Map::new();
            object.insert("variant".to_owned(), JsonValue::String(variant.clone()));
            if let Some(payload) = payload {
                object.insert("payload".to_owned(), to_json_value(payload, bytes_format)?);
            }
            Ok(JsonValue::Object(object))
        }
    }
}

pub fn from_json_value(value: &JsonValue) -> Result<Value> {
    match value {
        JsonValue::Null => Ok(Value::Unit),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::Number(number) => json_number(number),
        JsonValue::String(value) => Ok(Value::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_json_value(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        JsonValue::Object(values) => values
            .iter()
            .map(|(key, value)| {
                from_json_value(value)
                    .map(|decoded| (key.clone(), decoded))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
    }
}

fn bytes_to_json(bytes: &Bytes, bytes_format: BytesFormat) -> Result<JsonValue> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            Ok(JsonValue::String(BASE64_STANDARD.encode(bytes.as_slice())))
        }
        BytesFormat::Array => bytes
            .as_slice()
            .iter()
            .map(|byte| Ok(JsonValue::Number(JsonNumber::from(*byte))))
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.as_slice().len().saturating_mul(2));
            bytes
                .as_slice()
                .iter()
                .try_for_each(|byte| write!(&mut encoded, "{byte:02x}"))
                .map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })?;
            Ok(JsonValue::String(encoded))
        }
    }
}

fn json_number(number: &JsonNumber) -> Result<Value> {
    if let Some(value) = number.as_i64() {
        return Ok(Value::Number(arcweft_data::Number::I(i128::from(value))));
    }
    if let Some(value) = number.as_u64() {
        return Ok(Value::Number(arcweft_data::Number::U(u128::from(value))));
    }
    if let Some(value) = number.as_f64() {
        return Ok(Value::Number(arcweft_data::Number::F64(value)));
    }
    Err(DataError::new(
        DataErrorKind::InvalidEncoding,
        "invalid JSON number",
    ))
}

fn json_error(error: &serde_json::Error) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
}
