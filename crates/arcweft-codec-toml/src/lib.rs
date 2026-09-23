#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    BytesFormat, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions,
    EnumRepr, EnumTagStyle, FormatId, Number, RawValue, Result, ShapeAccess, ShapeRef, TypeShape,
    Value, VariantShape, decode_with_shape_ref, encode_with_shape_ref,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use source_preflight::preflight_toml_source_budget;
use toml::Value as TomlValue;

mod source_preflight;

#[derive(Clone, Copy, Debug, Default)]
pub struct TomlCodec;

impl Codec for TomlCodec {
    fn id(&self) -> FormatId {
        FormatId::new("toml")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/toml", "application/x-toml"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape_ref(value, shape, access)?.into_tagged_options();
        let toml = raw_to_toml_value(&raw, shape, access)?;
        toml::to_string_pretty(&toml)
            .map(String::into_bytes)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let source = std::str::from_utf8(input)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        preflight_toml_source_budget(source, &options.limits)?;
        let deserializer = toml::Deserializer::parse(source)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let dynamic_raw = BudgetedTomlRawSeed {
            budget: &mut budget,
        }
        .deserialize(deserializer)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))??;
        let toml = raw_dynamic_to_toml(&dynamic_raw)?;
        let raw = toml_to_raw_value(&toml, shape, access)?;
        let value = decode_with_shape_ref(&raw, shape, access)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

struct BudgetedTomlRawSeed<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
}

impl<'de> DeserializeSeed<'de> for BudgetedTomlRawSeed<'_, '_> {
    type Value = Result<RawValue>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(BudgetedTomlRawVisitor {
            budget: self.budget,
        })
    }
}

struct BudgetedTomlRawVisitor<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
}

impl<'de> Visitor<'de> for BudgetedTomlRawVisitor<'_, '_> {
    type Value = Result<RawValue>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a TOML value")
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

    fn visit_i128<E>(self, value: i128) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Signed(value)))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Unsigned(u128::from(value))))
    }

    fn visit_u128<E>(self, value: u128) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.scalar(RawValue::Unsigned(value)))
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
        while let Some(value) = seq.next_element_seed(BudgetedTomlRawSeed {
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
        while let Some(key) = map.next_key_seed(BudgetedTomlRawSeed {
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
            let value = map.next_value_seed(BudgetedTomlRawSeed {
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

impl BudgetedTomlRawVisitor<'_, '_> {
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

fn raw_to_toml_value(
    raw: &RawValue,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
    let shape = shape_ref.resolve(access)?;
    match (shape.as_ref(), raw) {
        (TypeShape::Ref(id), raw) => raw_to_toml_value(raw, ShapeRef::Id(*id), access),
        (TypeShape::Bool, RawValue::Bool(value)) => Ok(TomlValue::Boolean(*value)),
        (
            TypeShape::I8
            | TypeShape::I16
            | TypeShape::I32
            | TypeShape::I64
            | TypeShape::I128
            | TypeShape::Isize,
            RawValue::Signed(value),
        ) => signed_to_toml(*value),
        (
            TypeShape::U8
            | TypeShape::U16
            | TypeShape::U32
            | TypeShape::U64
            | TypeShape::U128
            | TypeShape::Usize,
            RawValue::Unsigned(value),
        ) => unsigned_to_toml(*value),
        (TypeShape::F32, RawValue::F32(value)) => Ok(TomlValue::Float(f64::from(*value))),
        (TypeShape::F64, RawValue::F64(value)) => Ok(TomlValue::Float(*value)),
        (TypeShape::String | TypeShape::Char, RawValue::String(value)) => {
            Ok(TomlValue::String(value.clone()))
        }
        (TypeShape::Bytes { format }, RawValue::Bytes(bytes)) => bytes_to_toml(bytes, *format),
        (TypeShape::Unit, RawValue::Null) => Err(toml_null_error()),
        (TypeShape::Option(inner), raw) => raw_tagged_option_to_toml(raw, inner, access),
        (TypeShape::Seq(inner), RawValue::Seq(values)) => {
            raw_seq_to_toml(values, ShapeRef::Inline(inner), access)
        }
        (TypeShape::Tuple(items), RawValue::Seq(values)) if items.len() == values.len() => items
            .iter()
            .zip(values)
            .enumerate()
            .map(|(index, (item_shape, value))| {
                raw_to_toml_value(value, ShapeRef::Inline(item_shape), access)
                    .map_err(|error| error.at_index(index))
            })
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        (TypeShape::Tuple(items), RawValue::Seq(values)) => Err(DataError::invalid_type(
            format!("tuple with {} items", items.len()),
            format!("tuple with {} items", values.len()),
        )),
        (TypeShape::Map { key, value, .. }, RawValue::Map(entries)) => {
            let key_shape = ShapeRef::Inline(key).resolve(access)?;
            if matches!(key_shape.as_ref(), TypeShape::String) {
                raw_string_map_to_toml(entries, value, access)
            } else {
                raw_map_pairs_to_toml(entries, key, value, access)
            }
        }
        (TypeShape::Record { fields, .. }, RawValue::Map(entries)) => {
            raw_record_to_toml(entries, fields, access)
        }
        (
            TypeShape::Enum {
                variants,
                tag,
                repr,
                ..
            },
            raw,
        ) => raw_enum_to_toml(raw, variants, tag, *repr, access),
        (shape, raw) => Err(DataError::invalid_type(shape.type_name(), raw.type_name())),
    }
}

fn raw_enum_to_toml(
    raw: &RawValue,
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<EnumRepr>,
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
    if repr.is_some() {
        return raw_dynamic_to_toml(raw);
    }
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type("enum table", raw.type_name()));
    };
    let fields = raw_enum_fields(entries)?;
    let (tag_key, content_key) = match tag {
        EnumTagStyle::External => ("variant", Some("payload")),
        EnumTagStyle::Internal { tag } => (tag.as_str(), None),
        EnumTagStyle::Adjacent { tag, content } => (tag.as_str(), Some(content.as_str())),
    };
    let variant = match fields.get(tag_key).copied() {
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
    let case = toml_enum_case(variants, variant)?;
    let mut table = toml::Table::new();
    table.insert(tag_key.to_owned(), TomlValue::String(variant.to_owned()));
    if let Some(content_key) = content_key {
        match (&case.payload, fields.get(content_key).copied()) {
            (Some(shape), Some(raw)) => {
                table.insert(
                    content_key.to_owned(),
                    raw_to_toml_value(raw, ShapeRef::Inline(shape), access)
                        .map_err(|error| error.at_variant(variant))?,
                );
            }
            (None, None) => {}
            (Some(_), None) => {
                return Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing enum content field `{content_key}`"),
                )
                .at_variant(variant)
                .at_field(content_key.to_owned()));
            }
            (None, Some(_)) => {
                return Err(
                    DataError::invalid_type("unit enum variant", "payload").at_variant(variant)
                );
            }
        }
        for (key, value) in fields {
            if key != tag_key && key != content_key {
                table.insert(key.to_owned(), raw_dynamic_to_toml(value)?);
            }
        }
    } else {
        let payload = RawValue::Map(
            entries
                .iter()
                .filter(|(key, _)| !matches!(key, RawValue::String(key) if key == tag_key))
                .cloned()
                .collect(),
        );
        match &case.payload {
            Some(shape) => {
                let TomlValue::Table(payload) =
                    raw_to_toml_value(&payload, ShapeRef::Inline(shape), access)
                        .map_err(|error| error.at_variant(variant))?
                else {
                    return Err(DataError::unsupported(
                        "internally tagged enum payload must encode as a table",
                    )
                    .at_variant(variant));
                };
                for (key, value) in payload {
                    if table.insert(key.clone(), value).is_some() {
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
                    if key != tag_key {
                        table.insert(key.to_owned(), raw_dynamic_to_toml(value)?);
                    }
                }
            }
        }
    }
    Ok(TomlValue::Table(table))
}

fn toml_enum_to_raw(
    value: &TomlValue,
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<EnumRepr>,
    access: &dyn ShapeAccess,
) -> Result<RawValue> {
    if let Some(repr) = repr {
        return toml_integer_to_raw(value, &repr.type_shape());
    }
    let TomlValue::Table(fields) = value else {
        return Err(DataError::invalid_type("enum table", toml_type_name(value)));
    };
    let (tag_key, content_key) = match tag {
        EnumTagStyle::External => ("variant", Some("payload")),
        EnumTagStyle::Internal { tag } => (tag.as_str(), None),
        EnumTagStyle::Adjacent { tag, content } => (tag.as_str(), Some(content.as_str())),
    };
    let variant = match fields.get(tag_key) {
        Some(TomlValue::String(variant)) => variant.as_str(),
        Some(other) => {
            return Err(DataError::invalid_type(
                "enum tag string",
                toml_type_name(other),
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
    let case = toml_enum_case(variants, variant)?;
    let mut entries = vec![(
        RawValue::String(tag_key.to_owned()),
        RawValue::String(variant.to_owned()),
    )];
    if let Some(content_key) = content_key {
        for (key, value) in fields {
            if key == tag_key {
                continue;
            }
            let raw = if key == content_key {
                match &case.payload {
                    Some(shape) => toml_to_raw_value(value, ShapeRef::Inline(shape), access)
                        .map_err(|error| error.at_variant(variant))?,
                    None => toml_dynamic_to_raw(value)?,
                }
            } else {
                toml_dynamic_to_raw(value)?
            };
            entries.push((RawValue::String(key.clone()), raw));
        }
    } else {
        let mut payload = fields.clone();
        payload.remove(tag_key);
        if let Some(shape) = &case.payload {
            let raw =
                toml_to_raw_value(&TomlValue::Table(payload), ShapeRef::Inline(shape), access)
                    .map_err(|error| error.at_variant(variant))?;
            let RawValue::Map(payload_entries) = raw else {
                return Err(DataError::unsupported(
                    "internally tagged enum payload must decode as a record",
                )
                .at_variant(variant));
            };
            entries.extend(payload_entries);
        } else {
            for (key, value) in payload {
                entries.push((RawValue::String(key), toml_dynamic_to_raw(&value)?));
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

fn toml_enum_case<'a>(variants: &'a [VariantShape], name: &str) -> Result<&'a VariantShape> {
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

const OPTION_TAG_KEY: &str = "$arcweft";
const OPTION_TAG_VALUE: &str = "option";
const OPTION_PRESENT_KEY: &str = "present";
const OPTION_VALUE_KEY: &str = "value";

fn raw_tagged_option_to_toml(
    raw: &RawValue,
    inner: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
    let payload = tagged_raw_option_payload(raw)?;
    let mut table = toml::Table::new();
    table.insert(
        OPTION_TAG_KEY.to_owned(),
        TomlValue::String(OPTION_TAG_VALUE.to_owned()),
    );
    table.insert(
        OPTION_PRESENT_KEY.to_owned(),
        TomlValue::Boolean(payload.is_some()),
    );
    if let Some(payload) = payload {
        let value = if matches!(inner, TypeShape::Unit) && matches!(payload, RawValue::Null) {
            TomlValue::String(String::new())
        } else {
            raw_to_toml_value(payload, ShapeRef::Inline(inner), access)?
        };
        table.insert(OPTION_VALUE_KEY.to_owned(), value);
    }
    Ok(TomlValue::Table(table))
}

fn tagged_raw_option_payload(raw: &RawValue) -> Result<Option<&RawValue>> {
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type(
            "tagged option table",
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

fn signed_to_toml(value: i128) -> Result<TomlValue> {
    i64::try_from(value)
        .map(TomlValue::Integer)
        .map_err(|_| toml_integer_range_error())
}

fn unsigned_to_toml(value: u128) -> Result<TomlValue> {
    i64::try_from(value)
        .map(TomlValue::Integer)
        .map_err(|_| toml_integer_range_error())
}

fn raw_seq_to_toml(
    values: &[RawValue],
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            raw_to_toml_value(value, shape, access).map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(TomlValue::Array)
}

fn raw_string_map_to_toml(
    entries: &[(RawValue, RawValue)],
    shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("string map key", key.type_name()));
            };
            raw_to_toml_value(raw_value, ShapeRef::Inline(shape), access)
                .map(|toml| (key.clone(), toml))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<toml::Table>>()
        .map(TomlValue::Table)
}

fn raw_map_pairs_to_toml(
    entries: &[(RawValue, RawValue)],
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
    entries
        .iter()
        .enumerate()
        .map(|(index, (key, value))| {
            let key = raw_to_toml_value(key, ShapeRef::Inline(key_shape), access)
                .map_err(|error| error.at_index(index))?;
            let value = raw_to_toml_value(value, ShapeRef::Inline(value_shape), access)
                .map_err(|error| error.at_index(index))?;
            Ok(TomlValue::Array(vec![key, value]))
        })
        .collect::<Result<Vec<_>>>()
        .map(TomlValue::Array)
}

fn raw_record_to_toml(
    entries: &[(RawValue, RawValue)],
    fields: &[arcweft_data::FieldShape],
    access: &dyn ShapeAccess,
) -> Result<TomlValue> {
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
                    raw_to_toml_value(raw_value, ShapeRef::Inline(shape.as_ref()), access)
                }
                None => raw_to_toml_value(raw_value, ShapeRef::Inline(&TypeShape::Unit), access),
            }
            .map_err(|error| error.at_field(key.clone()))?;
            Ok((key.clone(), value))
        })
        .collect::<Result<toml::Table>>()
        .map(TomlValue::Table)
}

fn raw_map_to_toml(entries: &[(RawValue, RawValue)]) -> Result<TomlValue> {
    entries
        .iter()
        .map(|(key, value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("table key", key.type_name()));
            };
            raw_dynamic_to_toml(value).map(|toml| (key.clone(), toml))
        })
        .collect::<Result<toml::Table>>()
        .map(TomlValue::Table)
}

fn toml_to_raw_value(
    value: &TomlValue,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<RawValue> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => toml_to_raw_value(value, ShapeRef::Id(*id), access),
        TypeShape::Unit => Err(DataError::invalid_type("null", toml_type_name(value))),
        TypeShape::Bool => match value {
            TomlValue::Boolean(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", toml_type_name(other))),
        },
        TypeShape::String | TypeShape::Char => match value {
            TomlValue::String(value) => Ok(RawValue::String(value.clone())),
            TomlValue::Datetime(value) => Ok(RawValue::String(value.to_string())),
            other => Err(DataError::invalid_type("string", toml_type_name(other))),
        },
        TypeShape::Bytes { format } => toml_to_bytes(value, *format).map(RawValue::Bytes),
        TypeShape::Option(inner) => match tagged_toml_option_payload(value)? {
            None => Ok(RawValue::Option(None)),
            Some(TomlValue::String(payload))
                if matches!(inner.as_ref(), TypeShape::Unit) && payload.is_empty() =>
            {
                Ok(RawValue::Option(Some(Box::new(RawValue::Null))))
            }
            Some(payload) => toml_to_raw_value(payload, ShapeRef::Inline(inner), access)
                .map(Box::new)
                .map(Some)
                .map(RawValue::Option),
        },
        TypeShape::Seq(inner) => match value {
            TomlValue::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    toml_to_raw_value(value, ShapeRef::Inline(inner), access)
                        .map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("array", toml_type_name(other))),
        },
        TypeShape::Tuple(items) => match value {
            TomlValue::Array(values) if values.len() == items.len() => items
                .iter()
                .zip(values)
                .enumerate()
                .map(|(index, (item_shape, value))| {
                    toml_to_raw_value(value, ShapeRef::Inline(item_shape), access)
                        .map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            TomlValue::Array(values) => Err(DataError::invalid_type(
                format!("tuple with {} items", items.len()),
                format!("tuple with {} items", values.len()),
            )),
            other => Err(DataError::invalid_type(
                "tuple array",
                toml_type_name(other),
            )),
        },
        TypeShape::Map {
            key, value: inner, ..
        } => {
            let key_shape = ShapeRef::Inline(key).resolve(access)?;
            if matches!(key_shape.as_ref(), TypeShape::String) {
                toml_table_entries(value, inner, access).map(RawValue::Map)
            } else {
                toml_pair_entries(value, key, inner, access).map(RawValue::Map)
            }
        }
        TypeShape::Record { fields, .. } => match value {
            TomlValue::Table(entries) => entries
                .iter()
                .map(|(key, value)| {
                    let raw = match fields.iter().find(|field| field.wire_name == *key) {
                        Some(field) => {
                            let shape = field
                                .resolve_value_shape(access)
                                .map_err(|error| error.at_field(key.clone()))?;
                            toml_to_raw_value(value, ShapeRef::Inline(shape.as_ref()), access)
                                .map_err(|error| error.at_field(key.clone()))
                        }
                        None => toml_dynamic_to_raw(value),
                    }?;
                    Ok((RawValue::String(key.clone()), raw))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Map),
            other => Err(DataError::invalid_type("table", toml_type_name(other))),
        },
        TypeShape::Enum {
            variants,
            tag,
            repr,
            ..
        } => toml_enum_to_raw(value, variants, tag, *repr, access),
        TypeShape::F32 | TypeShape::F64 => toml_float_to_raw(value, shape.as_ref()),
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
        | TypeShape::Usize => toml_integer_to_raw(value, shape.as_ref()),
    }
}

fn tagged_toml_option_payload(value: &TomlValue) -> Result<Option<&TomlValue>> {
    let TomlValue::Table(fields) = value else {
        return Err(DataError::invalid_type(
            "tagged option table",
            toml_type_name(value),
        ));
    };
    if !matches!(fields.get(OPTION_TAG_KEY), Some(TomlValue::String(tag)) if tag == OPTION_TAG_VALUE)
    {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "option value is missing its Arcweft marker",
        ));
    }
    match fields.get(OPTION_PRESENT_KEY) {
        Some(TomlValue::Boolean(false)) if fields.len() == 2 => Ok(None),
        Some(TomlValue::Boolean(true)) if fields.len() == 3 => {
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

fn toml_table_entries(
    value: &TomlValue,
    shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Vec<(RawValue, RawValue)>> {
    match value {
        TomlValue::Table(entries) => entries
            .iter()
            .map(|(key, value)| {
                toml_to_raw_value(value, ShapeRef::Inline(shape), access)
                    .map(|raw| (RawValue::String(key.clone()), raw))
                    .map_err(|error| error.at_field(key.clone()))
            })
            .collect(),
        other => Err(DataError::invalid_type("table", toml_type_name(other))),
    }
}

fn toml_pair_entries(
    value: &TomlValue,
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Vec<(RawValue, RawValue)>> {
    let TomlValue::Array(entries) = value else {
        return Err(DataError::invalid_type(
            "map pair array",
            toml_type_name(value),
        ));
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let TomlValue::Array(pair) = entry else {
                return Err(
                    DataError::invalid_type("map entry pair", toml_type_name(entry))
                        .at_index(index),
                );
            };
            let [key, value] = pair.as_slice() else {
                return Err(
                    DataError::invalid_type("map entry pair of length 2", "other length")
                        .at_index(index),
                );
            };
            let key = toml_to_raw_value(key, ShapeRef::Inline(key_shape), access)
                .map_err(|error| error.at_index(index))?;
            let value = toml_to_raw_value(value, ShapeRef::Inline(value_shape), access)
                .map_err(|error| error.at_index(index))?;
            Ok((key, value))
        })
        .collect()
}

fn toml_integer_to_raw(value: &TomlValue, _shape: &TypeShape) -> Result<RawValue> {
    let TomlValue::Integer(value) = value else {
        return Err(DataError::invalid_type("integer", toml_type_name(value)));
    };
    Ok(RawValue::Signed(i128::from(*value)))
}

fn toml_float_to_raw(value: &TomlValue, shape: &TypeShape) -> Result<RawValue> {
    match value {
        TomlValue::Float(value) => match shape {
            TypeShape::F32 => {
                value
                    .to_string()
                    .parse::<f32>()
                    .map(RawValue::F32)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())
                    })
            }
            TypeShape::F64 => Ok(RawValue::F64(*value)),
            _ => unreachable!("caller passes float shape"),
        },
        TomlValue::Integer(value) => match shape {
            TypeShape::F32 => {
                value
                    .to_string()
                    .parse::<f32>()
                    .map(RawValue::F32)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())
                    })
            }
            TypeShape::F64 => {
                value
                    .to_string()
                    .parse::<f64>()
                    .map(RawValue::F64)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())
                    })
            }
            _ => unreachable!("caller passes float shape"),
        },
        other => Err(DataError::invalid_type("float", toml_type_name(other))),
    }
}

fn toml_dynamic_to_raw(value: &TomlValue) -> Result<RawValue> {
    match value {
        TomlValue::String(value) => Ok(RawValue::String(value.clone())),
        TomlValue::Integer(value) => Ok(RawValue::Signed(i128::from(*value))),
        TomlValue::Float(value) => Ok(RawValue::F64(*value)),
        TomlValue::Boolean(value) => Ok(RawValue::Bool(*value)),
        TomlValue::Datetime(value) => Ok(RawValue::String(value.to_string())),
        TomlValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| toml_dynamic_to_raw(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Seq),
        TomlValue::Table(values) => values
            .iter()
            .map(|(key, value)| {
                toml_dynamic_to_raw(value).map(|decoded| (RawValue::String(key.clone()), decoded))
            })
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Map),
    }
}

fn raw_dynamic_to_toml(raw: &RawValue) -> Result<TomlValue> {
    match raw {
        RawValue::Null => Err(toml_null_error()),
        RawValue::Bool(value) => Ok(TomlValue::Boolean(*value)),
        RawValue::Signed(value) => signed_to_toml(*value),
        RawValue::Unsigned(value) => unsigned_to_toml(*value),
        RawValue::F32(value) => Ok(TomlValue::Float(f64::from(*value))),
        RawValue::F64(value) => Ok(TomlValue::Float(*value)),
        RawValue::String(value) => Ok(TomlValue::String(value.clone())),
        RawValue::Bytes(value) => bytes_to_toml(value, BytesFormat::Base64),
        RawValue::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| raw_dynamic_to_toml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        RawValue::Map(entries) => raw_map_to_toml(entries),
        RawValue::Option(_) => Err(DataError::unsupported(
            "raw option requires shape-aware TOML encoding",
        )),
    }
}

pub fn to_toml(value: &Value, bytes_format: BytesFormat) -> Result<TomlValue> {
    match value {
        Value::Unit => Ok(TomlValue::String(String::new())),
        Value::Bool(value) => Ok(TomlValue::Boolean(*value)),
        Value::Number(Number::I(value)) => {
            i64::try_from(*value).map(TomlValue::Integer).map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "TOML integer is i64-limited",
                )
            })
        }
        Value::Number(Number::U(value)) => {
            i64::try_from(*value).map(TomlValue::Integer).map_err(|_| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "TOML integer is i64-limited",
                )
            })
        }
        Value::Number(Number::F32(value)) => Ok(TomlValue::Float(f64::from(*value))),
        Value::Number(Number::F64(value)) => Ok(TomlValue::Float(*value)),
        Value::String(value) => Ok(TomlValue::String(value.clone())),
        Value::Char(value) => Ok(TomlValue::String(value.to_string())),
        Value::Bytes(bytes) => bytes_to_toml(bytes.as_slice(), bytes_format),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| to_toml(value, bytes_format).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        Value::Tuple(values) => values
            .iter()
            .map(|value| to_toml(value, bytes_format))
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        Value::Option(value) => {
            let mut table = toml::Table::new();
            table.insert(
                OPTION_TAG_KEY.to_owned(),
                TomlValue::String(OPTION_TAG_VALUE.to_owned()),
            );
            table.insert(
                OPTION_PRESENT_KEY.to_owned(),
                TomlValue::Boolean(value.is_some()),
            );
            if let Some(value) = value {
                table.insert(OPTION_VALUE_KEY.to_owned(), to_toml(value, bytes_format)?);
            }
            Ok(TomlValue::Table(table))
        }
        Value::Map { entries, .. }
            if entries
                .iter()
                .all(|(key, _)| matches!(key, Value::String(_))) =>
        {
            let mut table = toml::Table::new();
            for (key, value) in entries {
                let Value::String(key) = key else {
                    unreachable!()
                };
                if table.contains_key(key) {
                    return Err(DataError::new(
                        DataErrorKind::DuplicateField,
                        format!("duplicate TOML map key `{key}`"),
                    )
                    .at_field(key.clone()));
                }
                table.insert(
                    key.clone(),
                    to_toml(value, bytes_format).map_err(|error| error.at_field(key.clone()))?,
                );
            }
            Ok(TomlValue::Table(table))
        }
        Value::Map { entries, .. } => entries
            .iter()
            .enumerate()
            .map(|(index, (key, value))| {
                let key = to_toml(key, bytes_format).map_err(|error| error.at_index(index))?;
                let value = to_toml(value, bytes_format).map_err(|error| error.at_index(index))?;
                Ok(TomlValue::Array(vec![key, value]))
            })
            .collect::<Result<Vec<_>>>()
            .map(TomlValue::Array),
        Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_toml(value, bytes_format)
                    .map(|toml| (key.clone(), toml))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<toml::Table>>()
            .map(TomlValue::Table),
        Value::Enum { variant, payload } => {
            let mut table = toml::Table::new();
            table.insert("variant".to_owned(), TomlValue::String(variant.clone()));
            if let Some(payload) = payload {
                table.insert("payload".to_owned(), to_toml(payload, bytes_format)?);
            }
            Ok(TomlValue::Table(table))
        }
    }
}

fn bytes_to_toml(bytes: &[u8], bytes_format: BytesFormat) -> Result<TomlValue> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            Ok(TomlValue::String(BASE64_STANDARD.encode(bytes)))
        }
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
            bytes.iter().try_for_each(|byte| {
                write!(&mut encoded, "{byte:02x}").map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })
            })?;
            Ok(TomlValue::String(encoded))
        }
        BytesFormat::Array => Ok(TomlValue::Array(
            bytes
                .iter()
                .map(|byte| TomlValue::Integer(i64::from(*byte)))
                .collect(),
        )),
    }
}

fn toml_to_bytes(value: &TomlValue, bytes_format: BytesFormat) -> Result<Vec<u8>> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let TomlValue::String(value) = value else {
                return Err(DataError::invalid_type(
                    "base64 string",
                    toml_type_name(value),
                ));
            };
            BASE64_STANDARD
                .decode(value.as_bytes())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        BytesFormat::Hex => {
            let TomlValue::String(value) = value else {
                return Err(DataError::invalid_type("hex string", toml_type_name(value)));
            };
            decode_hex(value)
        }
        BytesFormat::Array => {
            let TomlValue::Array(values) = value else {
                return Err(DataError::invalid_type("byte array", toml_type_name(value)));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let TomlValue::Integer(value) = value else {
                        return Err(
                            DataError::invalid_type("byte", toml_type_name(value)).at_index(index)
                        );
                    };
                    u8::try_from(*value)
                        .map_err(|_| {
                            DataError::new(
                                DataErrorKind::NumberOutOfRange,
                                format!("byte value {value} is outside 0..=255"),
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

fn toml_null_error() -> DataError {
    DataError::unsupported(
        "TOML has no null value; Option::None is only supported as record field omission",
    )
}

fn toml_integer_range_error() -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        "TOML integer is i64-limited",
    )
}

const fn toml_type_name(value: &TomlValue) -> &'static str {
    match value {
        TomlValue::String(_) => "string",
        TomlValue::Integer(_) => "integer",
        TomlValue::Float(_) => "float",
        TomlValue::Boolean(_) => "bool",
        TomlValue::Datetime(_) => "datetime",
        TomlValue::Array(_) => "array",
        TomlValue::Table(_) => "table",
    }
}

pub fn from_toml(value: &TomlValue) -> Result<Value> {
    match value {
        TomlValue::String(value) => Ok(Value::String(value.clone())),
        TomlValue::Integer(value) => Ok(Value::Number(Number::I(i128::from(*value)))),
        TomlValue::Float(value) => Ok(Value::Number(Number::F64(*value))),
        TomlValue::Boolean(value) => Ok(Value::Bool(*value)),
        TomlValue::Datetime(value) => Ok(Value::String(value.to_string())),
        TomlValue::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_toml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        TomlValue::Table(values) => values
            .iter()
            .map(|(key, value)| {
                from_toml(value)
                    .map(|decoded| (key.clone(), decoded))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
    }
}
