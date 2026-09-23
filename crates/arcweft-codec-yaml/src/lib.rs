#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_data::{
    BytesFormat, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions,
    EnumRepr, EnumTagStyle, FieldShape, FormatId, Number, RawValue, Result, ShapeAccess, ShapeRef,
    TypeShape, Value, VariantShape, decode_with_shape_ref, encode_with_shape_ref,
};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use source_preflight::preflight_yaml_source_scalars;
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::Marker;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

mod source_preflight;

#[derive(Clone, Copy, Debug, Default)]
pub struct YamlCodec;

impl Codec for YamlCodec {
    fn id(&self) -> FormatId {
        FormatId::new("yaml")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/yaml", "application/x-yaml", "text/yaml"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: ShapeRef<'_>,
        access: &dyn ShapeAccess,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape_ref(value, shape, access)?.into_tagged_options();
        let yaml = raw_to_yaml(&raw, shape, access)?;
        let mut out = String::new();
        YamlEmitter::new(&mut out)
            .dump(&yaml)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        Ok(out.into_bytes())
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
        preflight_yaml_source_scalars(source, &options.limits)?;
        validate_yaml_budget(source, &mut budget)?;
        let documents = YamlLoader::load_from_str(source)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        let [document] = documents.as_slice() else {
            return Err(match documents.len() {
                0 => DataError::new(DataErrorKind::MissingField, "YAML document is empty"),
                _ => DataError::new(
                    DataErrorKind::TrailingData,
                    "YAML codec accepts exactly one document",
                ),
            });
        };
        let raw = yaml_to_raw(document, shape, access)?;
        let value = decode_with_shape_ref(&raw, shape, access)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

#[derive(Clone, Copy)]
enum YamlBudgetFrame {
    Sequence { len: usize },
    Mapping { len: usize, expecting_key: bool },
}

struct YamlBudgetReceiver<'budget, 'limits> {
    budget: &'budget mut DecodeBudget<'limits>,
    stack: Vec<YamlBudgetFrame>,
    error: Option<DataError>,
}

impl MarkedEventReceiver for YamlBudgetReceiver<'_, '_> {
    fn on_event(&mut self, event: Event, _mark: Marker) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = self.consume_event(event) {
            self.error = Some(error);
        }
    }
}

impl YamlBudgetReceiver<'_, '_> {
    fn consume_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Scalar(value, ..) => {
                self.note_parent_child()?;
                self.budget.enter_node()?;
                let result = self.budget.string_len(value.len());
                self.budget.exit_node();
                result
            }
            Event::SequenceStart(..) => {
                self.note_parent_child()?;
                self.budget.enter_node()?;
                self.stack.push(YamlBudgetFrame::Sequence { len: 0 });
                Ok(())
            }
            Event::MappingStart(..) => {
                self.note_parent_child()?;
                self.budget.enter_node()?;
                self.stack.push(YamlBudgetFrame::Mapping {
                    len: 0,
                    expecting_key: true,
                });
                Ok(())
            }
            Event::SequenceEnd | Event::MappingEnd => {
                self.stack.pop();
                self.budget.exit_node();
                self.finish_parent_value();
                Ok(())
            }
            Event::Alias(_) => Err(DataError::unsupported(
                "YAML aliases are not supported by Arcweft data",
            )),
            Event::Nothing
            | Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart
            | Event::DocumentEnd => Ok(()),
        }
    }

    fn note_parent_child(&mut self) -> Result<()> {
        match self.stack.last_mut() {
            Some(YamlBudgetFrame::Sequence { len }) => {
                *len = len.saturating_add(1);
                self.budget.sequence_item(*len)
            }
            Some(YamlBudgetFrame::Mapping { len, expecting_key }) if *expecting_key => {
                *len = len.saturating_add(1);
                *expecting_key = false;
                self.budget.map_item(*len)
            }
            Some(YamlBudgetFrame::Mapping { expecting_key, .. }) => {
                *expecting_key = true;
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn finish_parent_value(&mut self) {
        if let Some(YamlBudgetFrame::Mapping { expecting_key, .. }) = self.stack.last_mut()
            && !*expecting_key
        {
            *expecting_key = true;
        }
    }
}

fn validate_yaml_budget(source: &str, budget: &mut DecodeBudget<'_>) -> Result<()> {
    let mut receiver = YamlBudgetReceiver {
        budget,
        stack: Vec::new(),
        error: None,
    };
    let mut parser = Parser::new_from_str(source);
    parser
        .load(&mut receiver, true)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    if let Some(error) = receiver.error {
        return Err(error);
    }
    Ok(())
}

fn raw_to_yaml(raw: &RawValue, shape_ref: ShapeRef<'_>, access: &dyn ShapeAccess) -> Result<Yaml> {
    let shape = shape_ref.resolve(access)?;
    match (shape.as_ref(), raw) {
        (TypeShape::Ref(id), raw) => raw_to_yaml(raw, ShapeRef::Id(*id), access),
        (TypeShape::Bool, RawValue::Bool(value)) => Ok(Yaml::Boolean(*value)),
        (
            TypeShape::I8
            | TypeShape::I16
            | TypeShape::I32
            | TypeShape::I64
            | TypeShape::I128
            | TypeShape::Isize,
            RawValue::Signed(value),
        ) => signed_to_yaml(*value),
        (
            TypeShape::U8
            | TypeShape::U16
            | TypeShape::U32
            | TypeShape::U64
            | TypeShape::U128
            | TypeShape::Usize,
            RawValue::Unsigned(value),
        ) => unsigned_to_yaml(*value),
        (TypeShape::F32, RawValue::F32(value)) => Ok(Yaml::Real(value.to_string())),
        (TypeShape::F64, RawValue::F64(value)) => Ok(Yaml::Real(value.to_string())),
        (TypeShape::String | TypeShape::Char, RawValue::String(value)) => {
            Ok(Yaml::String(value.clone()))
        }
        (TypeShape::Bytes { format }, RawValue::Bytes(bytes)) => bytes_to_yaml(bytes, *format),
        (TypeShape::Unit | TypeShape::Option(_), RawValue::Null) => Ok(Yaml::Null),
        (TypeShape::Option(inner), raw) => raw_tagged_option_to_yaml(raw, inner, access),
        (TypeShape::Seq(inner), RawValue::Seq(values)) => {
            raw_seq_to_yaml(values, ShapeRef::Inline(inner), access)
        }
        (TypeShape::Tuple(items), RawValue::Seq(values)) if items.len() == values.len() => items
            .iter()
            .zip(values)
            .enumerate()
            .map(|(index, (item_shape, value))| {
                raw_to_yaml(value, ShapeRef::Inline(item_shape), access)
                    .map_err(|error| error.at_index(index))
            })
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        (TypeShape::Tuple(items), RawValue::Seq(values)) => Err(DataError::invalid_type(
            format!("tuple with {} items", items.len()),
            format!("tuple with {} items", values.len()),
        )),
        (TypeShape::Map { key, value, .. }, RawValue::Map(entries)) => {
            let key_shape = ShapeRef::Inline(key).resolve(access)?;
            if matches!(key_shape.as_ref(), TypeShape::String) {
                raw_string_map_to_yaml(entries, value, access)
            } else {
                raw_map_pairs_to_yaml(entries, key, value, access)
            }
        }
        (TypeShape::Record { fields, .. }, RawValue::Map(entries)) => {
            raw_record_to_yaml(entries, fields, access)
        }
        (
            TypeShape::Enum {
                variants,
                tag,
                repr,
                ..
            },
            raw,
        ) => raw_enum_to_yaml(raw, variants, tag, *repr, access),
        (shape, raw) => Err(DataError::invalid_type(shape.type_name(), raw.type_name())),
    }
}

fn raw_enum_to_yaml(
    raw: &RawValue,
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<EnumRepr>,
    access: &dyn ShapeAccess,
) -> Result<Yaml> {
    if repr.is_some() {
        return raw_dynamic_to_yaml(raw);
    }
    let RawValue::Map(entries) = raw else {
        return Err(DataError::invalid_type("enum map", raw.type_name()));
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
    let case = yaml_enum_case(variants, variant)?;
    let mut object = Hash::new();
    object.insert(
        Yaml::String(tag_key.to_owned()),
        Yaml::String(variant.to_owned()),
    );
    if let Some(content_key) = content_key {
        match (&case.payload, fields.get(content_key).copied()) {
            (Some(shape), Some(raw)) => {
                object.insert(
                    Yaml::String(content_key.to_owned()),
                    raw_to_yaml(raw, ShapeRef::Inline(shape), access)
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
                object.insert(Yaml::String(key.to_owned()), raw_dynamic_to_yaml(value)?);
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
                let Yaml::Hash(payload) = raw_to_yaml(&payload, ShapeRef::Inline(shape), access)
                    .map_err(|error| error.at_variant(variant))?
                else {
                    return Err(DataError::unsupported(
                        "internally tagged enum payload must encode as a record",
                    )
                    .at_variant(variant));
                };
                for (key, value) in payload {
                    if object.insert(key.clone(), value).is_some() {
                        let key_name = key
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("{key:?}"));
                        return Err(DataError::new(
                            DataErrorKind::DuplicateField,
                            format!("internal enum payload duplicates tag field `{key_name}`"),
                        )
                        .at_variant(variant)
                        .at_field(key_name));
                    }
                }
            }
            None if fields.len() == 1 => {}
            None => {
                for (key, value) in fields {
                    if key != tag_key {
                        object.insert(Yaml::String(key.to_owned()), raw_dynamic_to_yaml(value)?);
                    }
                }
            }
        }
    }
    Ok(Yaml::Hash(object))
}

fn yaml_enum_to_raw(
    value: &Yaml,
    variants: &[VariantShape],
    tag: &EnumTagStyle,
    repr: Option<EnumRepr>,
    access: &dyn ShapeAccess,
) -> Result<RawValue> {
    if let Some(repr) = repr {
        return yaml_integer_to_raw(value, &repr.type_shape());
    }
    let Yaml::Hash(fields) = value else {
        return Err(DataError::invalid_type("enum map", yaml_type_name(value)));
    };
    let (tag_key, content_key) = match tag {
        EnumTagStyle::External => ("variant", Some("payload")),
        EnumTagStyle::Internal { tag } => (tag.as_str(), None),
        EnumTagStyle::Adjacent { tag, content } => (tag.as_str(), Some(content.as_str())),
    };
    let tag_yaml = fields.get(&Yaml::String(tag_key.to_owned()));
    let variant = match tag_yaml {
        Some(Yaml::String(variant)) => variant.as_str(),
        Some(other) => {
            return Err(DataError::invalid_type(
                "enum tag string",
                yaml_type_name(other),
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
    let case = yaml_enum_case(variants, variant)?;
    let mut entries = vec![(
        RawValue::String(tag_key.to_owned()),
        RawValue::String(variant.to_owned()),
    )];
    if let Some(content_key) = content_key {
        for (key, value) in fields {
            let Some(key) = key.as_str() else {
                return Err(DataError::invalid_type(
                    "enum object key",
                    yaml_type_name(key),
                ));
            };
            if key == tag_key {
                continue;
            }
            let raw = if key == content_key {
                match (&case.payload, value) {
                    (Some(shape), value) => yaml_to_raw(value, ShapeRef::Inline(shape), access)
                        .map_err(|error| error.at_variant(variant))?,
                    (None, value) => yaml_dynamic_to_raw(value)?,
                }
            } else {
                yaml_dynamic_to_raw(value)?
            };
            entries.push((RawValue::String(key.to_owned()), raw));
        }
    } else {
        let mut payload = fields.clone();
        payload.remove(&Yaml::String(tag_key.to_owned()));
        if let Some(shape) = &case.payload {
            let raw = yaml_to_raw(&Yaml::Hash(payload), ShapeRef::Inline(shape), access)
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
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type(
                        "enum object key",
                        yaml_type_name(&key),
                    ));
                };
                entries.push((
                    RawValue::String(key.to_owned()),
                    yaml_dynamic_to_raw(&value)?,
                ));
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

fn yaml_enum_case<'a>(variants: &'a [VariantShape], name: &str) -> Result<&'a VariantShape> {
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

fn raw_tagged_option_to_yaml(
    raw: &RawValue,
    inner: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Yaml> {
    let payload = tagged_raw_option_payload(raw)?;
    let mut hash = Hash::new();
    hash.insert(
        Yaml::String(OPTION_TAG_KEY.to_owned()),
        Yaml::String(OPTION_TAG_VALUE.to_owned()),
    );
    hash.insert(
        Yaml::String(OPTION_PRESENT_KEY.to_owned()),
        Yaml::Boolean(payload.is_some()),
    );
    if let Some(payload) = payload {
        hash.insert(
            Yaml::String(OPTION_VALUE_KEY.to_owned()),
            raw_to_yaml(payload, ShapeRef::Inline(inner), access)?,
        );
    }
    Ok(Yaml::Hash(hash))
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

fn signed_to_yaml(value: i128) -> Result<Yaml> {
    i64::try_from(value)
        .map(Yaml::Integer)
        .map_err(|_| yaml_integer_range_error())
}

fn unsigned_to_yaml(value: u128) -> Result<Yaml> {
    i64::try_from(value)
        .map(Yaml::Integer)
        .map_err(|_| yaml_integer_range_error())
}

fn raw_seq_to_yaml(
    values: &[RawValue],
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<Yaml> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            raw_to_yaml(value, shape, access).map_err(|error| error.at_index(index))
        })
        .collect::<Result<Vec<_>>>()
        .map(Yaml::Array)
}

fn raw_string_map_to_yaml(
    entries: &[(RawValue, RawValue)],
    shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Yaml> {
    entries
        .iter()
        .map(|(key, raw_value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("string map key", key.type_name()));
            };
            raw_to_yaml(raw_value, ShapeRef::Inline(shape), access)
                .map(|yaml| (Yaml::String(key.clone()), yaml))
                .map_err(|error| error.at_field(key.clone()))
        })
        .collect::<Result<Hash>>()
        .map(Yaml::Hash)
}

fn raw_map_pairs_to_yaml(
    entries: &[(RawValue, RawValue)],
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Yaml> {
    entries
        .iter()
        .enumerate()
        .map(|(index, (key, value))| {
            let key = raw_to_yaml(key, ShapeRef::Inline(key_shape), access)
                .map_err(|error| error.at_index(index))?;
            let value = raw_to_yaml(value, ShapeRef::Inline(value_shape), access)
                .map_err(|error| error.at_index(index))?;
            Ok(Yaml::Array(vec![key, value]))
        })
        .collect::<Result<Vec<_>>>()
        .map(Yaml::Array)
}

fn raw_record_to_yaml(
    entries: &[(RawValue, RawValue)],
    fields: &[FieldShape],
    access: &dyn ShapeAccess,
) -> Result<Yaml> {
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
                    raw_to_yaml(raw_value, ShapeRef::Inline(shape.as_ref()), access)
                }
                None => raw_to_yaml(raw_value, ShapeRef::Inline(&TypeShape::Unit), access),
            }
            .map_err(|error| error.at_field(key.clone()))?;
            Ok((Yaml::String(key.clone()), value))
        })
        .collect::<Result<Hash>>()
        .map(Yaml::Hash)
}

fn raw_map_to_yaml(entries: &[(RawValue, RawValue)]) -> Result<Yaml> {
    entries
        .iter()
        .map(|(key, value)| {
            let RawValue::String(key) = key else {
                return Err(DataError::invalid_type("hash key", key.type_name()));
            };
            raw_dynamic_to_yaml(value).map(|yaml| (Yaml::String(key.clone()), yaml))
        })
        .collect::<Result<Hash>>()
        .map(Yaml::Hash)
}

fn yaml_to_raw(
    value: &Yaml,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> Result<RawValue> {
    let shape = shape_ref.resolve(access)?;
    match shape.as_ref() {
        TypeShape::Ref(id) => yaml_to_raw(value, ShapeRef::Id(*id), access),
        TypeShape::Unit => match value {
            Yaml::Null => Ok(RawValue::Null),
            other => Err(DataError::invalid_type("null", yaml_type_name(other))),
        },
        TypeShape::Bool => match value {
            Yaml::Boolean(value) => Ok(RawValue::Bool(*value)),
            other => Err(DataError::invalid_type("bool", yaml_type_name(other))),
        },
        TypeShape::String | TypeShape::Char => match value {
            Yaml::String(value) => Ok(RawValue::String(value.clone())),
            other => Err(DataError::invalid_type("string", yaml_type_name(other))),
        },
        TypeShape::Bytes { format } => yaml_to_bytes(value, *format).map(RawValue::Bytes),
        TypeShape::Option(inner) => match tagged_yaml_option_payload(value)? {
            None => Ok(RawValue::Option(None)),
            Some(payload) => yaml_to_raw(payload, ShapeRef::Inline(inner), access)
                .map(Box::new)
                .map(Some)
                .map(RawValue::Option),
        },
        TypeShape::Seq(inner) => match value {
            Yaml::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    yaml_to_raw(value, ShapeRef::Inline(inner), access)
                        .map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            other => Err(DataError::invalid_type("array", yaml_type_name(other))),
        },
        TypeShape::Tuple(items) => match value {
            Yaml::Array(values) if values.len() == items.len() => items
                .iter()
                .zip(values)
                .enumerate()
                .map(|(index, (item_shape, value))| {
                    yaml_to_raw(value, ShapeRef::Inline(item_shape), access)
                        .map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Seq),
            Yaml::Array(values) => Err(DataError::invalid_type(
                format!("tuple with {} items", items.len()),
                format!("tuple with {} items", values.len()),
            )),
            other => Err(DataError::invalid_type(
                "tuple array",
                yaml_type_name(other),
            )),
        },
        TypeShape::Map {
            key, value: inner, ..
        } => {
            let key_shape = ShapeRef::Inline(key).resolve(access)?;
            if matches!(key_shape.as_ref(), TypeShape::String) {
                yaml_hash_entries(value, inner, access).map(RawValue::Map)
            } else {
                yaml_pair_entries(value, key, inner, access).map(RawValue::Map)
            }
        }
        TypeShape::Record { fields, .. } => match value {
            Yaml::Hash(entries) => entries
                .iter()
                .map(|(key, value)| {
                    let Some(key) = key.as_str() else {
                        return Err(DataError::invalid_type(
                            "record field key",
                            yaml_type_name(key),
                        ));
                    };
                    let raw = match fields.iter().find(|field| field.wire_name == key) {
                        Some(field) => {
                            let shape = field
                                .resolve_value_shape(access)
                                .map_err(|error| error.at_field(key.to_owned()))?;
                            yaml_to_raw(value, ShapeRef::Inline(shape.as_ref()), access)
                                .map_err(|error| error.at_field(key.to_owned()))
                        }
                        None => yaml_dynamic_to_raw(value),
                    }?;
                    Ok((RawValue::String(key.to_owned()), raw))
                })
                .collect::<Result<Vec<_>>>()
                .map(RawValue::Map),
            other => Err(DataError::invalid_type("hash", yaml_type_name(other))),
        },
        TypeShape::Enum {
            variants,
            tag,
            repr,
            ..
        } => yaml_enum_to_raw(value, variants, tag, *repr, access),
        TypeShape::F32 | TypeShape::F64 => yaml_float_to_raw(value, shape.as_ref()),
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
        | TypeShape::Usize => yaml_integer_to_raw(value, shape.as_ref()),
    }
}

fn tagged_yaml_option_payload(value: &Yaml) -> Result<Option<&Yaml>> {
    let Yaml::Hash(fields) = value else {
        return Err(DataError::invalid_type(
            "tagged option map",
            yaml_type_name(value),
        ));
    };
    let marker = fields.get(&Yaml::String(OPTION_TAG_KEY.to_owned()));
    if !matches!(marker, Some(Yaml::String(tag)) if tag == OPTION_TAG_VALUE) {
        return Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "option value is missing its Arcweft marker",
        ));
    }
    let present = fields.get(&Yaml::String(OPTION_PRESENT_KEY.to_owned()));
    match present {
        Some(Yaml::Boolean(false)) if fields.len() == 2 => Ok(None),
        Some(Yaml::Boolean(true)) if fields.len() == 3 => fields
            .get(&Yaml::String(OPTION_VALUE_KEY.to_owned()))
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

fn yaml_hash_entries(
    value: &Yaml,
    shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Vec<(RawValue, RawValue)>> {
    match value {
        Yaml::Hash(entries) => entries
            .iter()
            .map(|(key, value)| {
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type(
                        "string map key",
                        yaml_type_name(key),
                    ));
                };
                yaml_to_raw(value, ShapeRef::Inline(shape), access)
                    .map(|raw| (RawValue::String(key.to_owned()), raw))
                    .map_err(|error| error.at_field(key))
            })
            .collect(),
        other => Err(DataError::invalid_type("hash", yaml_type_name(other))),
    }
}

fn yaml_pair_entries(
    value: &Yaml,
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    access: &dyn ShapeAccess,
) -> Result<Vec<(RawValue, RawValue)>> {
    let Yaml::Array(entries) = value else {
        return Err(DataError::invalid_type(
            "map pair array",
            yaml_type_name(value),
        ));
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let Yaml::Array(pair) = entry else {
                return Err(
                    DataError::invalid_type("map entry pair", yaml_type_name(entry))
                        .at_index(index),
                );
            };
            let [key, value] = pair.as_slice() else {
                return Err(
                    DataError::invalid_type("map entry pair of length 2", "other length")
                        .at_index(index),
                );
            };
            let key = yaml_to_raw(key, ShapeRef::Inline(key_shape), access)
                .map_err(|error| error.at_index(index))?;
            let value = yaml_to_raw(value, ShapeRef::Inline(value_shape), access)
                .map_err(|error| error.at_index(index))?;
            Ok((key, value))
        })
        .collect()
}

fn yaml_integer_to_raw(value: &Yaml, _shape: &TypeShape) -> Result<RawValue> {
    let Yaml::Integer(value) = value else {
        return Err(DataError::invalid_type("integer", yaml_type_name(value)));
    };
    Ok(RawValue::Signed(i128::from(*value)))
}

fn yaml_float_to_raw(value: &Yaml, shape: &TypeShape) -> Result<RawValue> {
    let text = match value {
        Yaml::Real(value) => value.as_str(),
        Yaml::Integer(value) => return integer_to_float_raw(*value, shape),
        other => return Err(DataError::invalid_type("float", yaml_type_name(other))),
    };
    match shape {
        TypeShape::F32 => text
            .parse::<f32>()
            .map(RawValue::F32)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        TypeShape::F64 => text
            .parse::<f64>()
            .map(RawValue::F64)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        _ => unreachable!("caller passes float shape"),
    }
}

fn integer_to_float_raw(value: i64, shape: &TypeShape) -> Result<RawValue> {
    let text = value.to_string();
    match shape {
        TypeShape::F32 => text
            .parse::<f32>()
            .map(RawValue::F32)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        TypeShape::F64 => text
            .parse::<f64>()
            .map(RawValue::F64)
            .map_err(|error| DataError::new(DataErrorKind::NumberOutOfRange, error.to_string())),
        _ => unreachable!("caller passes float shape"),
    }
}

fn yaml_dynamic_to_raw(value: &Yaml) -> Result<RawValue> {
    match value {
        Yaml::Real(value) => value
            .parse::<f64>()
            .map(RawValue::F64)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string())),
        Yaml::Integer(value) => Ok(RawValue::Signed(i128::from(*value))),
        Yaml::String(value) => Ok(RawValue::String(value.clone())),
        Yaml::Boolean(value) => Ok(RawValue::Bool(*value)),
        Yaml::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| yaml_dynamic_to_raw(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Seq),
        Yaml::Hash(values) => values
            .iter()
            .map(|(key, value)| {
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type("hash key", yaml_type_name(key)));
                };
                yaml_dynamic_to_raw(value)
                    .map(|decoded| (RawValue::String(key.to_owned()), decoded))
            })
            .collect::<Result<Vec<_>>>()
            .map(RawValue::Map),
        Yaml::Null => Ok(RawValue::Null),
        Yaml::BadValue => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "invalid YAML value",
        )),
        Yaml::Alias(_) => Err(DataError::unsupported(
            "YAML aliases are not supported by Arcweft data",
        )),
    }
}

fn raw_dynamic_to_yaml(raw: &RawValue) -> Result<Yaml> {
    match raw {
        RawValue::Null => Ok(Yaml::Null),
        RawValue::Bool(value) => Ok(Yaml::Boolean(*value)),
        RawValue::Signed(value) => signed_to_yaml(*value),
        RawValue::Unsigned(value) => unsigned_to_yaml(*value),
        RawValue::F32(value) => Ok(Yaml::Real(value.to_string())),
        RawValue::F64(value) => Ok(Yaml::Real(value.to_string())),
        RawValue::String(value) => Ok(Yaml::String(value.clone())),
        RawValue::Bytes(value) => bytes_to_yaml(value, BytesFormat::Base64),
        RawValue::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| raw_dynamic_to_yaml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        RawValue::Map(entries) => raw_map_to_yaml(entries),
        RawValue::Option(_) => Err(DataError::unsupported(
            "raw option requires shape-aware YAML encoding",
        )),
    }
}

pub fn to_yaml(value: &Value, bytes_format: BytesFormat) -> Result<Yaml> {
    match value {
        Value::Unit => Ok(Yaml::Null),
        Value::Bool(value) => Ok(Yaml::Boolean(*value)),
        Value::Number(Number::I(value)) => i64::try_from(*value).map(Yaml::Integer).map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "YAML integer is i64-limited",
            )
        }),
        Value::Number(Number::U(value)) => i64::try_from(*value).map(Yaml::Integer).map_err(|_| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "YAML integer is i64-limited",
            )
        }),
        Value::Number(Number::F32(value)) => Ok(Yaml::Real(value.to_string())),
        Value::Number(Number::F64(value)) => Ok(Yaml::Real(value.to_string())),
        Value::String(value) => Ok(Yaml::String(value.clone())),
        Value::Char(value) => Ok(Yaml::String(value.to_string())),
        Value::Bytes(bytes) => bytes_to_yaml(bytes.as_slice(), bytes_format),
        Value::Seq(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| to_yaml(value, bytes_format).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        Value::Tuple(values) => values
            .iter()
            .map(|value| to_yaml(value, bytes_format))
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        Value::Option(value) => {
            let mut hash = Hash::new();
            hash.insert(
                Yaml::String(OPTION_TAG_KEY.to_owned()),
                Yaml::String(OPTION_TAG_VALUE.to_owned()),
            );
            hash.insert(
                Yaml::String(OPTION_PRESENT_KEY.to_owned()),
                Yaml::Boolean(value.is_some()),
            );
            if let Some(value) = value {
                hash.insert(
                    Yaml::String(OPTION_VALUE_KEY.to_owned()),
                    to_yaml(value, bytes_format)?,
                );
            }
            Ok(Yaml::Hash(hash))
        }
        Value::Map { entries, .. }
            if entries
                .iter()
                .all(|(key, _)| matches!(key, Value::String(_))) =>
        {
            entries
                .iter()
                .map(|(key, value)| {
                    let Value::String(key) = key else {
                        unreachable!()
                    };
                    to_yaml(value, bytes_format)
                        .map(|yaml| (Yaml::String(key.clone()), yaml))
                        .map_err(|error| error.at_field(key.clone()))
                })
                .collect::<Result<Hash>>()
                .map(Yaml::Hash)
        }
        Value::Map { entries, .. } => entries
            .iter()
            .enumerate()
            .map(|(index, (key, value))| {
                let key = to_yaml(key, bytes_format).map_err(|error| error.at_index(index))?;
                let value = to_yaml(value, bytes_format).map_err(|error| error.at_index(index))?;
                Ok(Yaml::Array(vec![key, value]))
            })
            .collect::<Result<Vec<_>>>()
            .map(Yaml::Array),
        Value::Record(values) => values
            .iter()
            .map(|(key, value)| {
                to_yaml(value, bytes_format)
                    .map(|yaml| (Yaml::String(key.clone()), yaml))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<Hash>>()
            .map(Yaml::Hash),
        Value::Enum { variant, payload } => {
            let mut hash = Hash::new();
            hash.insert(
                Yaml::String("variant".to_owned()),
                Yaml::String(variant.clone()),
            );
            if let Some(payload) = payload {
                hash.insert(
                    Yaml::String("payload".to_owned()),
                    to_yaml(payload, bytes_format)?,
                );
            }
            Ok(Yaml::Hash(hash))
        }
    }
}

fn bytes_to_yaml(bytes: &[u8], bytes_format: BytesFormat) -> Result<Yaml> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            Ok(Yaml::String(BASE64_STANDARD.encode(bytes)))
        }
        BytesFormat::Hex => {
            let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
            bytes.iter().try_for_each(|byte| {
                write!(&mut encoded, "{byte:02x}").map_err(|error| {
                    DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                })
            })?;
            Ok(Yaml::String(encoded))
        }
        BytesFormat::Array => Ok(Yaml::Array(
            bytes
                .iter()
                .map(|byte| Yaml::Integer(i64::from(*byte)))
                .collect(),
        )),
    }
}

fn yaml_to_bytes(value: &Yaml, bytes_format: BytesFormat) -> Result<Vec<u8>> {
    match bytes_format {
        BytesFormat::Binary | BytesFormat::Base64 => {
            let Yaml::String(value) = value else {
                return Err(DataError::invalid_type(
                    "base64 string",
                    yaml_type_name(value),
                ));
            };
            BASE64_STANDARD
                .decode(value.as_bytes())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        BytesFormat::Hex => {
            let Yaml::String(value) = value else {
                return Err(DataError::invalid_type("hex string", yaml_type_name(value)));
            };
            decode_hex(value)
        }
        BytesFormat::Array => {
            let Yaml::Array(values) = value else {
                return Err(DataError::invalid_type("byte array", yaml_type_name(value)));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let Yaml::Integer(value) = value else {
                        return Err(
                            DataError::invalid_type("byte", yaml_type_name(value)).at_index(index)
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

fn yaml_integer_range_error() -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        "YAML integer is i64-limited",
    )
}

const fn yaml_type_name(value: &Yaml) -> &'static str {
    match value {
        Yaml::Real(_) => "float",
        Yaml::Integer(_) => "integer",
        Yaml::String(_) => "string",
        Yaml::Boolean(_) => "bool",
        Yaml::Array(_) => "array",
        Yaml::Hash(_) => "hash",
        Yaml::Alias(_) => "alias",
        Yaml::Null => "null",
        Yaml::BadValue => "bad value",
    }
}

pub fn from_yaml(value: &Yaml) -> Result<Value> {
    match value {
        Yaml::Real(value) => value
            .parse::<f64>()
            .map(|value| Value::Number(Number::F64(value)))
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string())),
        Yaml::Integer(value) => Ok(Value::Number(Number::I(i128::from(*value)))),
        Yaml::String(value) => Ok(Value::String(value.clone())),
        Yaml::Boolean(value) => Ok(Value::Bool(*value)),
        Yaml::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| from_yaml(value).map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq),
        Yaml::Hash(values) => values
            .iter()
            .map(|(key, value)| {
                let Some(key) = key.as_str() else {
                    return Err(DataError::invalid_type(
                        "string map key",
                        "non-string YAML key",
                    ));
                };
                from_yaml(value)
                    .map(|decoded| (key.to_owned(), decoded))
                    .map_err(|err| err.at_field(key))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
        Yaml::Null | Yaml::BadValue => Ok(Value::Unit),
        Yaml::Alias(_) => Err(DataError::unsupported(
            "YAML aliases are not supported by Arcweft data",
        )),
    }
}
