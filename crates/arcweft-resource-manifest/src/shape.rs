use crate::{
    JsonPath, ResourceManifestDiagnosticCode,
    budget::{BudgetError, DecodeBudget},
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

static ABSENT_CONTENT: Value = Value::Null;

#[derive(Clone, Copy)]
enum TagContent {
    Required,
    Optional,
    Forbidden,
}

const SCALAR_TYPE_TOKENS: &[&str] = &[
    "unit",
    "bool",
    "signed_integer",
    "unsigned_integer",
    "float",
    "string",
    "char",
    "duration",
    "ratio",
    "length",
    "gain",
    "pan",
    "locale",
    "public_id",
];
const RETAINED_KIND_TOKENS: &[&str] = &[
    "character",
    "view",
    "action",
    "layer",
    "signal",
    "presentation_target",
    "scroll_region",
];
const LAYOUT_UNIT_TOKENS: &[&str] = &[
    "px",
    "sp",
    "percent",
    "vw",
    "vh",
    "cw",
    "ch",
    "em",
    "glyph_ch",
    "safe_area_top",
    "safe_area_right",
    "safe_area_bottom",
    "safe_area_left",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShapeError {
    pub(crate) code: ResourceManifestDiagnosticCode,
    pub(crate) message: String,
    pub(crate) path: JsonPath,
    pub(crate) related: Option<JsonPath>,
}

pub(crate) fn validate_manifest_shape(
    value: &Value,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    let path = JsonPath::default();
    budget
        .charge_lexical_revisit(value, &path)
        .map_err(ShapeError::from)?;
    let root = closed_object(
        value,
        &path,
        &[
            "format",
            "schema",
            "package",
            "schemas",
            "resource_types",
            "codecs",
        ],
        &[],
    )?;
    package(
        required(root, "package", &path)?,
        &path.field("package"),
        budget,
    )?;
    array(
        required(root, "schemas", &path)?,
        &path.field("schemas"),
        budget,
        true,
        schema,
    )?;
    array(
        required(root, "resource_types", &path)?,
        &path.field("resource_types"),
        budget,
        true,
        descriptor,
    )?;
    array(
        required(root, "codecs", &path)?,
        &path.field("codecs"),
        budget,
        true,
        codec,
    )?;
    validate_uniqueness(root)?;
    Ok(())
}

fn validate_uniqueness(root: &Map<String, Value>) -> Result<(), ShapeError> {
    let root_path = JsonPath::default();
    let schemas_path = root_path.field("schemas");
    let schemas = root["schemas"]
        .as_array()
        .expect("shape validation established schemas array");
    unique_records(schemas, &schemas_path, |value, path| {
        let identity = path.field("value").field("schema_id");
        Some((value["value"]["schema_id"].as_str()?.to_owned(), identity))
    })?;
    for (index, schema) in schemas.iter().enumerate() {
        validate_schema_uniqueness(schema, &schemas_path.index(index))?;
    }

    let types_path = root_path.field("resource_types");
    let resource_types = root["resource_types"]
        .as_array()
        .expect("shape validation established resource-types array");
    unique_records(resource_types, &types_path, |value, path| {
        nominal_key(&value["type_id"]).map(|key| (key, path.field("type_id")))
    })?;

    let codecs_path = root_path.field("codecs");
    let codecs = root["codecs"]
        .as_array()
        .expect("shape validation established codecs array");
    unique_records(codecs, &codecs_path, |value, path| {
        Some((
            value["codec_id"].as_str()?.to_owned(),
            path.field("codec_id"),
        ))
    })?;
    for (index, codec) in codecs.iter().enumerate() {
        let versions_path = codecs_path.index(index).field("versions");
        let versions = codec["versions"]
            .as_array()
            .expect("shape validation established codec-version array");
        unique_records(versions, &versions_path, |value, path| {
            Some((value.as_u64()?.to_string(), path.clone()))
        })?;
    }
    Ok(())
}

fn validate_schema_uniqueness(value: &Value, path: &JsonPath) -> Result<(), ShapeError> {
    let content_path = path.field("value");
    match value["kind"].as_str() {
        Some("record") => {
            let fields_path = content_path.field("fields");
            let fields = value["value"]["fields"]
                .as_array()
                .expect("shape validation established fields array");
            unique_records(fields, &fields_path, |value, path| {
                Some((
                    value["field_id"].as_u64()?.to_string(),
                    path.field("field_id"),
                ))
            })?;
            unique_records(fields, &fields_path, |value, path| {
                Some((value["name"].as_str()?.to_owned(), path.field("name")))
            })?;
            for (index, field) in fields.iter().enumerate() {
                if let Some(default) = field.get("default") {
                    validate_const_uniqueness(default, &fields_path.index(index).field("default"))?;
                }
            }
            Ok(())
        }
        Some("enum") => {
            let variants_path = content_path.field("variants");
            let variants = value["value"]["variants"]
                .as_array()
                .expect("shape validation established variants array");
            unique_records(variants, &variants_path, |value, path| {
                Some((
                    value["variant_id"].as_u64()?.to_string(),
                    path.field("variant_id"),
                ))
            })?;
            unique_records(variants, &variants_path, |value, path| {
                Some((value["name"].as_str()?.to_owned(), path.field("name")))
            })
        }
        Some(_) | None => Ok(()),
    }
}

fn validate_const_uniqueness(value: &Value, path: &JsonPath) -> Result<(), ShapeError> {
    match value["kind"].as_str() {
        Some("option") => value.get("value").map_or(Ok(()), |value| {
            validate_const_uniqueness(value, &path.field("value"))
        }),
        Some("list") => validate_const_array(&value["value"], &path.field("value")),
        Some("ordered_map") => {
            let entries = value["value"]
                .as_array()
                .expect("shape validation established map entries");
            unique_records(entries, &path.field("value"), |entry, entry_path| {
                let key = arcweft_manifest_model::canonical_json_bytes(&entry["key"]).ok()?;
                Some((String::from_utf8(key).ok()?, entry_path.field("key")))
            })?;
            for (index, entry) in entries.iter().enumerate() {
                let entry_path = path.field("value").index(index);
                validate_const_uniqueness(&entry["key"], &entry_path.field("key"))?;
                validate_const_uniqueness(&entry["value"], &entry_path.field("value"))?;
            }
            Ok(())
        }
        Some("record") => {
            let fields_path = path.field("value").field("fields");
            let fields = value["value"]["fields"]
                .as_array()
                .expect("shape validation established record fields");
            unique_records(fields, &fields_path, |field, field_path| {
                Some((
                    field["field_id"].as_u64()?.to_string(),
                    field_path.field("field_id"),
                ))
            })?;
            for (index, field) in fields.iter().enumerate() {
                validate_const_uniqueness(
                    &field["value"],
                    &fields_path.index(index).field("value"),
                )?;
            }
            Ok(())
        }
        Some("enum") => value["value"].get("payload").map_or(Ok(()), |payload| {
            validate_const_uniqueness(payload, &path.field("value").field("payload"))
        }),
        _ => Ok(()),
    }
}

fn validate_const_array(value: &Value, path: &JsonPath) -> Result<(), ShapeError> {
    value
        .as_array()
        .expect("shape validation established constant array")
        .iter()
        .enumerate()
        .try_for_each(|(index, value)| validate_const_uniqueness(value, &path.index(index)))
}

fn unique_records(
    values: &[Value],
    path: &JsonPath,
    identity: impl Fn(&Value, &JsonPath) -> Option<(String, JsonPath)>,
) -> Result<(), ShapeError> {
    let mut first = BTreeMap::<String, JsonPath>::new();
    for (index, value) in values.iter().enumerate() {
        let Some((identity, identity_path)) = identity(value, &path.index(index)) else {
            continue;
        };
        if let Some(first_path) = first.insert(identity.clone(), identity_path.clone()) {
            return Err(ShapeError {
                code: ResourceManifestDiagnosticCode::DuplicateRecord,
                message: format!("duplicate resource manifest identity `{identity}`"),
                path: identity_path,
                related: Some(first_path),
            });
        }
    }
    Ok(())
}

fn nominal_key(value: &Value) -> Option<String> {
    Some(format!(
        "{}::{}::{}",
        value["package"].as_str()?,
        value["module"].as_str()?,
        value["name"].as_str()?
    ))
}

fn package(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    budget.charge_typed(2, path).map_err(ShapeError::from)?;
    closed_object(value, path, &["id", "version"], &[]).map(|_| ())
}
fn nominal(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_typed(3, path).map_err(ShapeError::from)?;
    closed_object(value, path, &["package", "module", "name"], &[]).map(|_| ())
}
fn schema(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    let (kind, content) = tag(value, path, TagContent::Required, budget)?;
    match kind {
        "record" => record_schema(content, path, budget),
        "enum" => enum_schema(content, path, budget),
        other => Err(unknown_tag(other, path)),
    }
}
fn record_schema(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    let p = path.field("value");
    let o = closed_object(
        value,
        &p,
        &["schema_id", "nominal_type", "version", "fields"],
        &[],
    )?;
    budget.charge_typed(2, &p).map_err(ShapeError::from)?;
    nominal(
        required(o, "nominal_type", &p)?,
        &p.field("nominal_type"),
        budget,
    )?;
    array(
        required(o, "fields", &p)?,
        &p.field("fields"),
        budget,
        true,
        field,
    )
}
fn enum_schema(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    let p = path.field("value");
    let o = closed_object(
        value,
        &p,
        &["schema_id", "nominal_type", "version", "variants"],
        &[],
    )?;
    budget.charge_typed(2, &p).map_err(ShapeError::from)?;
    nominal(
        required(o, "nominal_type", &p)?,
        &p.field("nominal_type"),
        budget,
    )?;
    array(
        required(o, "variants", &p)?,
        &p.field("variants"),
        budget,
        true,
        variant,
    )
}
fn field(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    budget.charge_typed(3, path).map_err(ShapeError::from)?;
    let o = closed_object(
        value,
        path,
        &["field_id", "name", "value_type", "presence"],
        &["default", "docs"],
    )?;
    enum_token(
        required(o, "presence", path)?,
        &path.field("presence"),
        &["required", "optional"],
    )?;
    budget
        .charge_edge(&path.field("value_type"))
        .map_err(ShapeError::from)?;
    value_type(
        required(o, "value_type", path)?,
        &path.field("value_type"),
        budget,
    )?;
    if let Some(v) = o.get("default") {
        budget
            .charge_edge(&path.field("default"))
            .map_err(ShapeError::from)?;
        const_value(v, &path.field("default"), budget)?;
    }
    Ok(())
}
fn variant(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    budget.charge_typed(2, path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["variant_id", "name"], &["payload", "docs"])?;
    if let Some(v) = o.get("payload") {
        budget
            .charge_edge(&path.field("payload"))
            .map_err(ShapeError::from)?;
        value_type(v, &path.field("payload"), budget)?;
    }
    Ok(())
}
fn value_type(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_edge(path).map_err(ShapeError::from)?;
    let (kind, content) = tag(value, path, TagContent::Required, budget)?;
    match kind {
        "scalar" => {
            budget.charge_typed(1, path).map_err(ShapeError::from)?;
            enum_token(content, &path.field("value"), SCALAR_TYPE_TOKENS).map(|_| ())
        }
        "record" | "enum" if content.is_string() => {
            budget.charge_typed(1, path).map_err(ShapeError::from)
        }
        "retained_identity_ref" => {
            budget.charge_typed(1, path).map_err(ShapeError::from)?;
            enum_token(content, &path.field("value"), RETAINED_KIND_TOKENS).map(|_| ())
        }
        "record" | "enum" => Err(wrong_tag_content("a typed schema ID string", path)),
        "option" | "list" | "non_empty_list" => value_type(content, &path.field("value"), budget),
        "ordered_map" => {
            let p = path.field("value");
            let o = closed_object(content, &p, &["key", "value"], &[])?;
            value_type(required(o, "key", &p)?, &p.field("key"), budget)?;
            value_type(required(o, "value", &p)?, &p.field("value"), budget)
        }
        "asset_ref" => {
            budget.charge_typed(1, path).map_err(ShapeError::from)?;
            closed_object(content, &path.field("value"), &["payload_kind"], &[]).map(|_| ())
        }
        "resource_ref" => {
            let p = path.field("value");
            let o = closed_object(content, &p, &["type_id"], &[])?;
            nominal(required(o, "type_id", &p)?, &p.field("type_id"), budget)
        }
        "constrained_scalar" => constraint(content, &path.field("value"), budget),
        other => Err(unknown_tag(other, path)),
    }
}
fn constraint(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["scalar"], &["lower", "upper"])?;
    enum_token(
        required(o, "scalar", path)?,
        &path.field("scalar"),
        SCALAR_TYPE_TOKENS,
    )?;
    for name in ["lower", "upper"] {
        if let Some(v) = o.get(name) {
            bound(v, &path.field(name), budget)?;
        }
    }
    Ok(())
}
fn bound(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["kind", "value"], &[])?;
    enum_token(
        required(o, "kind", path)?,
        &path.field("kind"),
        &["inclusive", "exclusive"],
    )?;
    scalar_value(required(o, "value", path)?, &path.field("value"), budget)
}
fn scalar_value(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    let (kind, content) = tag(
        value,
        path,
        if matches!(kind_text(value), Some("unit")) {
            TagContent::Forbidden
        } else {
            TagContent::Required
        },
        budget,
    )?;
    match kind {
        "unit" => Ok(()),
        "bool" if content.is_boolean() => Ok(()),
        "signed_integer" if content.as_i64().is_some() => Ok(()),
        "signed_integer" if content.is_number() => Err(ShapeError {
            code: ResourceManifestDiagnosticCode::IntegerOverflow,
            message: "signed resource integer exceeds i64".into(),
            path: path.field("value"),
            related: None,
        }),
        "unsigned_integer" | "duration" if content.as_u64().is_some() => Ok(()),
        "unsigned_integer" if content.is_number() => Err(ShapeError {
            code: ResourceManifestDiagnosticCode::IntegerOverflow,
            message: "unsigned resource integer exceeds u64".into(),
            path: path.field("value"),
            related: None,
        }),
        "float" => validate_float(content, &path.field("value")),
        "string" | "locale" | "public_id" if content.is_string() => Ok(()),
        "char" => validate_char(content, &path.field("value")),
        "ratio"
            if content
                .as_u64()
                .is_some_and(|value| u32::try_from(value).is_ok()) =>
        {
            Ok(())
        }
        "gain"
            if content
                .as_i64()
                .is_some_and(|value| i32::try_from(value).is_ok()) =>
        {
            Ok(())
        }
        "pan"
            if content
                .as_i64()
                .is_some_and(|value| i16::try_from(value).is_ok()) =>
        {
            Ok(())
        }
        "ratio" | "gain" | "pan" if content.is_number() => Err(ShapeError {
            code: ResourceManifestDiagnosticCode::IntegerOverflow,
            message: "resource scalar integer exceeds its typed range".into(),
            path: path.field("value"),
            related: None,
        }),
        "length" => {
            let content_path = path.field("value");
            let object = closed_object(content, &content_path, &["milli_units", "unit"], &[])?;
            if required(object, "milli_units", &content_path)?
                .as_i64()
                .is_none()
            {
                return Err(wrong_tag_content("i64 milli-unit value", &content_path));
            }
            enum_token(
                required(object, "unit", &content_path)?,
                &content_path.field("unit"),
                LAYOUT_UNIT_TOKENS,
            )?;
            Ok(())
        }
        "bool" | "signed_integer" | "unsigned_integer" | "string" | "duration" | "ratio"
        | "gain" | "pan" | "locale" | "public_id" => {
            Err(wrong_tag_content("the scalar's canonical value", path))
        }
        other => Err(unknown_tag(other, path)),
    }
}

fn validate_float(value: &Value, path: &JsonPath) -> Result<(), ShapeError> {
    let Some(text) = value.as_str() else {
        return Err(wrong_tag_content("a canonical float-bit string", path));
    };
    let Some(hex) = text.strip_prefix("0x") else {
        return Err(noncanonical_float(path));
    };
    if hex.len() != 16
        || hex
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(noncanonical_float(path));
    }
    let bits = u64::from_str_radix(hex, 16).map_err(|_| noncanonical_float(path))?;
    if bits == 0x8000_0000_0000_0000 {
        return Err(noncanonical_float(path));
    }
    if !f64::from_bits(bits).is_finite() {
        return Err(ShapeError {
            code: ResourceManifestDiagnosticCode::NonFiniteFloat,
            message: "resource float bits encode NaN or infinity".into(),
            path: path.clone(),
            related: None,
        });
    }
    Ok(())
}

fn noncanonical_float(path: &JsonPath) -> ShapeError {
    ShapeError {
        code: ResourceManifestDiagnosticCode::NonCanonicalFloat,
        message: "resource float bits are not canonical lowercase 0x plus 16 hex digits".into(),
        path: path.clone(),
        related: None,
    }
}

fn validate_char(value: &Value, path: &JsonPath) -> Result<(), ShapeError> {
    let Some(value) = value.as_str() else {
        return Err(wrong_tag_content("one Unicode scalar string", path));
    };
    let mut chars = value.chars();
    if chars.next().is_none() || chars.next().is_some() {
        return Err(ShapeError {
            code: ResourceManifestDiagnosticCode::InvalidString,
            message: "resource char must contain exactly one Unicode scalar".into(),
            path: path.clone(),
            related: None,
        });
    }
    Ok(())
}
fn const_value(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    budget.charge_edge(path).map_err(ShapeError::from)?;
    let kind = kind_text(value).ok_or_else(|| wrong_shape("tagged constant", path))?;
    let has_content = value
        .as_object()
        .is_some_and(|object| object.contains_key("value"));
    let (kind, content) = tag(
        value,
        path,
        if kind == "option" {
            TagContent::Optional
        } else {
            TagContent::Required
        },
        budget,
    )?;
    match kind {
        "scalar" => scalar_value(content, &path.field("value"), budget),
        "option" => {
            if has_content {
                const_value(content, &path.field("value"), budget)
            } else {
                Ok(())
            }
        }
        "list" => array(content, &path.field("value"), budget, false, const_value),
        "ordered_map" => array(content, &path.field("value"), budget, true, map_entry),
        "record" => record_value(content, &path.field("value"), budget),
        "enum" => enum_value(content, &path.field("value"), budget),
        "asset_ref" => closed_object(
            content,
            &path.field("value"),
            &["public_id", "payload_kind"],
            &[],
        )
        .map(|_| ()),
        "resource_ref" => {
            let p = path.field("value");
            let o = closed_object(content, &p, &["entity_id", "public_id", "type_id"], &[])?;
            budget.charge_typed(2, &p).map_err(ShapeError::from)?;
            nominal(required(o, "type_id", &p)?, &p.field("type_id"), budget)
        }
        "retained_identity_ref" => retained(content, &path.field("value"), budget),
        other => Err(unknown_tag(other, path)),
    }
}
fn map_entry(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["key", "value"], &[])?;
    const_value(required(o, "key", path)?, &path.field("key"), budget)?;
    const_value(required(o, "value", path)?, &path.field("value"), budget)
}
fn record_value(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["schema_id", "fields"], &[])?;
    array(
        required(o, "fields", path)?,
        &path.field("fields"),
        budget,
        true,
        record_field,
    )
}
fn record_field(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["field_id", "value"], &[])?;
    const_value(required(o, "value", path)?, &path.field("value"), budget)
}
fn enum_value(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_typed(2, path).map_err(ShapeError::from)?;
    let o = closed_object(value, path, &["schema_id", "variant_id"], &["payload"])?;
    if let Some(v) = o.get("payload") {
        const_value(v, &path.field("payload"), budget)?;
    }
    Ok(())
}
fn retained(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    let (kind, content) = tag(value, path, TagContent::Required, budget)?;
    let p = path.field("value");
    match kind {
        "character" | "view" | "action" | "layer" | "signal" => {
            budget.charge_typed(1, &p).map_err(ShapeError::from)?;
            closed_object(content, &p, &["entity_id"], &[]).map(|_| ())
        }
        "presentation_target" => {
            let o = closed_object(content, &p, &["scope", "target_id"], &[])?;
            budget.charge_typed(1, &p).map_err(ShapeError::from)?;
            presentation_scope(required(o, "scope", &p)?, &p.field("scope"), budget)
        }
        "scroll_region" => {
            budget.charge_typed(2, &p).map_err(ShapeError::from)?;
            closed_object(content, &p, &["owner_view_entity_id", "region_id"], &[]).map(|_| ())
        }
        other => Err(unknown_tag(other, path)),
    }
}
fn presentation_scope(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    let kind = kind_text(value).ok_or_else(|| wrong_shape("presentation scope", path))?;
    let (kind, content) = tag(
        value,
        path,
        if kind == "view" {
            TagContent::Required
        } else {
            TagContent::Forbidden
        },
        budget,
    )?;
    match kind {
        "global" => Ok(()),
        "view" => {
            budget.charge_typed(1, path).map_err(ShapeError::from)?;
            closed_object(
                content,
                &path.field("value"),
                &["owner_view_entity_id"],
                &[],
            )
            .map(|_| ())
        }
        other => Err(unknown_tag(other, path)),
    }
}
fn descriptor(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    budget.charge_typed(4, path).map_err(ShapeError::from)?;
    let o = closed_object(
        value,
        path,
        &[
            "type_id",
            "public_id_family",
            "family_group",
            "body_schema",
            "capabilities",
            "lowering",
            "descriptor_digest",
        ],
        &["docs"],
    )?;
    nominal(
        required(o, "type_id", path)?,
        &path.field("type_id"),
        budget,
    )?;
    budget
        .charge_record(&path.field("capabilities"))
        .map_err(ShapeError::from)?;
    budget
        .charge_typed(4, &path.field("capabilities"))
        .map_err(ShapeError::from)?;
    budget
        .charge_record(&path.field("lowering"))
        .map_err(ShapeError::from)?;
    budget
        .charge_typed(4, &path.field("lowering"))
        .map_err(ShapeError::from)?;
    let capabilities = closed_object(
        required(o, "capabilities", path)?,
        &path.field("capabilities"),
        &["agent_exposure", "save_definition_reference", "hot_reload"],
        &["runtime_handle_kind"],
    )?;
    let capabilities_path = path.field("capabilities");
    enum_token(
        required(capabilities, "agent_exposure", &capabilities_path)?,
        &capabilities_path.field("agent_exposure"),
        &["hidden", "catalog", "catalog_and_runtime"],
    )?;
    enum_token(
        required(capabilities, "hot_reload", &capabilities_path)?,
        &capabilities_path.field("hot_reload"),
        &[
            "restart_required",
            "replace_definition",
            "update_live_handle",
        ],
    )?;
    closed_object(
        required(o, "lowering", path)?,
        &path.field("lowering"),
        &["codec_id", "codec_version", "section_id", "section_version"],
        &[],
    )?;
    if let Some(v) = o.get("docs") {
        closed_object(v, &path.field("docs"), &["summary"], &[])?;
    }
    Ok(())
}
fn codec(value: &Value, path: &JsonPath, budget: &mut DecodeBudget) -> Result<(), ShapeError> {
    budget.charge_record(path).map_err(ShapeError::from)?;
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    let object = closed_object(value, path, &["codec_id", "versions"], &[])?;
    let versions_path = path.field("versions");
    let versions = required(object, "versions", path)?;
    array(versions, &versions_path, budget, true, codec_version)
}

fn codec_version(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
) -> Result<(), ShapeError> {
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    if value
        .as_u64()
        .is_some_and(|version| u32::try_from(version).is_ok())
    {
        Ok(())
    } else if value.is_number() {
        Err(ShapeError {
            code: ResourceManifestDiagnosticCode::IntegerOverflow,
            message: "resource codec version exceeds u32".into(),
            path: path.clone(),
            related: None,
        })
    } else {
        Err(wrong_shape("an unsigned codec version", path))
    }
}

fn array(
    value: &Value,
    path: &JsonPath,
    budget: &mut DecodeBudget,
    unordered: bool,
    validate: fn(&Value, &JsonPath, &mut DecodeBudget) -> Result<(), ShapeError>,
) -> Result<(), ShapeError> {
    let values = value.as_array().ok_or_else(|| wrong_shape("array", path))?;
    budget
        .charge_collection(values.len(), path)
        .map_err(ShapeError::from)?;
    if unordered {
        budget
            .charge_sort(values.len(), path)
            .map_err(ShapeError::from)?;
    }
    values
        .iter()
        .enumerate()
        .try_for_each(|(i, v)| validate(v, &path.index(i), budget))
}
fn closed_object<'a>(
    value: &'a Value,
    path: &JsonPath,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<&'a Map<String, Value>, ShapeError> {
    let object = value
        .as_object()
        .ok_or_else(|| wrong_shape("object", path))?;
    if let Some(name) = object.keys().find(|name| {
        !required_fields.contains(&name.as_str()) && !optional_fields.contains(&name.as_str())
    }) {
        return Err(ShapeError {
            code: ResourceManifestDiagnosticCode::UnknownField,
            message: format!("unknown resource manifest field `{name}`"),
            path: path.field(name),
            related: None,
        });
    }
    if let Some(name) = required_fields
        .iter()
        .find(|name| !object.contains_key(**name))
    {
        return Err(ShapeError {
            code: ResourceManifestDiagnosticCode::MissingField,
            message: format!("missing required resource manifest field `{name}`"),
            path: path.clone(),
            related: None,
        });
    }
    Ok(object)
}
fn required<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    path: &JsonPath,
) -> Result<&'a Value, ShapeError> {
    object.get(name).ok_or_else(|| ShapeError {
        code: ResourceManifestDiagnosticCode::MissingField,
        message: format!("missing required resource manifest field `{name}`"),
        path: path.clone(),
        related: None,
    })
}
fn tag<'a>(
    value: &'a Value,
    path: &JsonPath,
    content: TagContent,
    budget: &mut DecodeBudget,
) -> Result<(&'a str, &'a Value), ShapeError> {
    let object = closed_object(value, path, &["kind"], &["value"])?;
    match (content, object.contains_key("value")) {
        (TagContent::Required, false) => {
            return Err(ShapeError {
                code: ResourceManifestDiagnosticCode::WrongTagContent,
                message: "resource manifest tag requires a `value` field".into(),
                path: path.clone(),
                related: None,
            });
        }
        (TagContent::Forbidden, true) => {
            return Err(ShapeError {
                code: ResourceManifestDiagnosticCode::WrongTagContent,
                message: "resource manifest tag forbids a `value` field".into(),
                path: path.field("value"),
                related: None,
            });
        }
        _ => {}
    }
    budget.charge_typed(1, path).map_err(ShapeError::from)?;
    if object.contains_key("value") {
        budget.charge_record(path).map_err(ShapeError::from)?;
    }
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| wrong_shape("string tag", &path.field("kind")))?;
    let content = object.get("value").unwrap_or(&ABSENT_CONTENT);
    Ok((kind, content))
}
fn kind_text(value: &Value) -> Option<&str> {
    value.as_object()?.get("kind")?.as_str()
}
fn unknown_tag(tag: &str, path: &JsonPath) -> ShapeError {
    ShapeError {
        code: ResourceManifestDiagnosticCode::UnknownTag,
        message: format!("unknown resource manifest tag `{tag}`"),
        path: path.field("kind"),
        related: None,
    }
}
fn wrong_shape(expected: &str, path: &JsonPath) -> ShapeError {
    ShapeError {
        code: ResourceManifestDiagnosticCode::WrongShape,
        message: format!("resource manifest value must be {expected}"),
        path: path.clone(),
        related: None,
    }
}

fn wrong_tag_content(expected: &str, path: &JsonPath) -> ShapeError {
    ShapeError {
        code: ResourceManifestDiagnosticCode::WrongTagContent,
        message: format!("resource manifest tag content must be {expected}"),
        path: path.clone(),
        related: None,
    }
}

fn enum_token<'a>(
    value: &'a Value,
    path: &JsonPath,
    admitted: &[&str],
) -> Result<&'a str, ShapeError> {
    let Some(token) = value.as_str() else {
        return Err(wrong_tag_content("a string enum token", path));
    };
    if !admitted.contains(&token) {
        return Err(ShapeError {
            code: ResourceManifestDiagnosticCode::UnknownTag,
            message: format!("unknown resource manifest enum token `{token}`"),
            path: path.clone(),
            related: None,
        });
    }
    Ok(token)
}

impl From<BudgetError> for ShapeError {
    fn from(error: BudgetError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            path: error.path,
            related: None,
        }
    }
}
