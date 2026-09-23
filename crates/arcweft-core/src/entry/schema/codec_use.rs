//! Per-occurrence codec policy attached to the existing typed graph.
//!
//! These nodes carry no semantic identity, type reference, nominal identity,
//! layout, scalar width, field identity, or case ordinal. Their child slots
//! are zipped with the original logical row during admission. A nominal edge
//! ends at `NominalRef`; its body stays on the existing nominal domain.
//! Default producers refer to the existing pure-program bindings, which retain
//! their own executable body and exact signature.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::value_budget::ValidationWork;
use super::{
    RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeSchemaError,
    RuntimeSchemaLimits, RuntimeTypeSchema,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCodecUse {
    Plain,
    Bytes {
        format: RuntimeBytesFormat,
    },
    Unary {
        item: Box<Self>,
    },
    Newtype {
        inner: Box<Self>,
    },
    Tuple {
        items: Box<[Self]>,
    },
    Map {
        key: Box<Self>,
        value: Box<Self>,
    },
    /// Annotation-free structural record fields. Authored codec record policy
    /// is represented by `Record`, never fabricated from this variant.
    RecordFields {
        fields: Box<[Self]>,
    },
    Record {
        name: String,
        deny_unknown_fields: bool,
        fields: Box<[RuntimeFieldCodecUse]>,
    },
    Enum {
        name: String,
        tag: RuntimeEnumTagStyle,
        repr: Option<RuntimeEnumRepr>,
        cases: Box<[RuntimeVariantCodecUse]>,
    },
    /// Selects the core owner's fixed external-tag case names and one-item
    /// tuple payload ABI; only the payload item policies vary by occurrence.
    Builtin {
        payloads: Box<[Self]>,
    },
    Choice {
        alternatives: Box<[Self]>,
    },
    Opaque {
        arguments: Box<[Self]>,
    },
    NominalRef,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFieldCodecUse {
    pub wire_name: String,
    /// Declaration annotation only. This flag never supplies a default value.
    pub has_default: bool,
    /// Source-admitted nullary pure program producing this field's value.
    /// Constant defaults use the existing pure-program literal body as well.
    pub default_program: Option<arcweft_id::runtime_program::RuntimePureProgramId>,
    pub skip: bool,
    pub bytes_format: Option<RuntimeBytesFormat>,
    pub value: RuntimeCodecUse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeVariantCodecUse {
    pub wire_name: String,
    pub discriminant: Option<i128>,
    pub payload: Option<RuntimeCodecUse>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNominalCodecUses {
    pub body: RuntimeCodecUse,
    pub arguments: Box<[RuntimeCodecUse]>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCodecUseError {
    #[error("codec-use policy does not match its source schema at {path}")]
    Mismatch { path: String },
    #[error("unresolved named schema `{name}` cannot issue codec policy")]
    UnresolvedName { name: String },
    #[error(transparent)]
    Schema(#[from] RuntimeSchemaError),
}

impl RuntimeCodecUse {
    /// Checks the finite policy tree before cloning or projecting it.
    pub fn validate_limits(&self, limits: RuntimeSchemaLimits) -> Result<(), RuntimeCodecUseError> {
        let mut nodes = 0_usize;
        let mut work = ValidationWork::new(limits);
        self.walk(|node, depth| {
            nodes = nodes
                .checked_add(1)
                .ok_or(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })?;
            if !limits.permits_nodes(nodes) {
                return Err(RuntimeSchemaError::BudgetExceeded { budget: "nodes" });
            }
            if !limits.permits_depth(depth) {
                return Err(RuntimeSchemaError::BudgetExceeded { budget: "depth" });
            }
            work.charge(depth)?;
            let check_string = |text: &str| {
                if limits.permits_string_bytes(text.len()) {
                    Ok(())
                } else {
                    Err(RuntimeSchemaError::BudgetExceeded {
                        budget: "string_bytes",
                    })
                }
            };
            let count = match node {
                Self::Tuple { items }
                | Self::RecordFields { fields: items }
                | Self::Builtin { payloads: items }
                | Self::Choice {
                    alternatives: items,
                }
                | Self::Opaque { arguments: items } => items.len(),
                Self::Record { fields, .. } => fields.len(),
                Self::Enum { cases, .. } => cases.len(),
                _ => 0,
            };
            if !limits.permits_sequence_items(count) {
                return Err(RuntimeSchemaError::BudgetExceeded {
                    budget: "sequence_items",
                });
            }
            match node {
                Self::Record { name, fields, .. } => {
                    check_string(name)?;
                    for field in fields {
                        check_string(&field.wire_name)?;
                    }
                }
                Self::Enum {
                    name, tag, cases, ..
                } => {
                    check_string(name)?;
                    for case in cases {
                        check_string(&case.wire_name)?;
                    }
                    match tag {
                        RuntimeEnumTagStyle::External => {}
                        RuntimeEnumTagStyle::Internal { tag } => check_string(tag)?,
                        RuntimeEnumTagStyle::Adjacent { tag, content } => {
                            check_string(tag)?;
                            check_string(content)?;
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
        let mut sink = super::CanonicalBlake3Sink::default();
        let mut writer = super::CanonicalWriter {
            sink: &mut sink,
            max_encoded_bytes: limits.max_encoded_bytes,
            max_string_bytes: Some(limits.max_string_bytes),
        };
        encode(self, &mut writer)?;
        Ok(())
    }
    fn take_children(&mut self, pending: &mut Vec<Self>) {
        match self {
            Self::Unary { item } | Self::Newtype { inner: item } => {
                pending.push(*std::mem::replace(item, Box::new(Self::Plain)))
            }
            Self::Tuple { items }
            | Self::RecordFields { fields: items }
            | Self::Builtin { payloads: items }
            | Self::Choice {
                alternatives: items,
            }
            | Self::Opaque { arguments: items } => {
                pending.extend(std::mem::replace(items, Box::new([])).into_vec())
            }
            Self::Map { key, value } => {
                pending.push(*std::mem::replace(key, Box::new(Self::Plain)));
                pending.push(*std::mem::replace(value, Box::new(Self::Plain)));
            }
            Self::Record { fields, .. } => pending.extend(
                std::mem::replace(fields, Box::new([]))
                    .into_vec()
                    .into_iter()
                    .map(|field| field.value),
            ),
            Self::Enum { cases, .. } => pending.extend(
                std::mem::replace(cases, Box::new([]))
                    .into_vec()
                    .into_iter()
                    .filter_map(|case| case.payload),
            ),
            Self::Plain | Self::Bytes { .. } | Self::NominalRef => {}
        }
    }
    pub(crate) fn children(&self) -> Vec<&Self> {
        match self {
            Self::Unary { item } | Self::Newtype { inner: item } => vec![item],
            Self::Tuple { items }
            | Self::RecordFields { fields: items }
            | Self::Builtin { payloads: items }
            | Self::Choice {
                alternatives: items,
            }
            | Self::Opaque { arguments: items } => items.iter().collect(),
            Self::Map { key, value } => vec![key, value],
            Self::Record { fields, .. } => fields.iter().map(|field| &field.value).collect(),
            Self::Enum { cases, .. } => cases
                .iter()
                .filter_map(|case| case.payload.as_ref())
                .collect(),
            Self::Plain | Self::Bytes { .. } | Self::NominalRef => vec![],
        }
    }

    pub(crate) fn walk<E>(
        &self,
        mut visit: impl FnMut(&Self, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut pending = vec![(self, 0_usize)];
        while let Some((node, depth)) = pending.pop() {
            visit(node, depth)?;
            pending.extend(
                node.children()
                    .into_iter()
                    .rev()
                    .map(|child| (child, depth + 1)),
            );
        }
        Ok(())
    }
    /// Extracts only policy and typed child positions from the original schema.
    pub fn from_schema(
        schema: &RuntimeTypeSchema,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeCodecUseError> {
        let codec = Self::extract(schema, &mut ValidationWork::new(limits), 0)?;
        codec.validate_limits(limits)?;
        Ok(codec)
    }

    fn extract(
        schema: &RuntimeTypeSchema,
        work: &mut ValidationWork,
        depth: usize,
    ) -> Result<Self, RuntimeCodecUseError> {
        use RuntimeTypeSchema as Schema;
        work.charge(depth)?;
        let child_depth = depth + 1;
        let limits = work.limits();
        let clone_string = |text: &str| {
            if limits.permits_string_bytes(text.len()) {
                Ok(text.to_owned())
            } else {
                Err(RuntimeSchemaError::BudgetExceeded {
                    budget: "string_bytes",
                })
            }
        };
        Ok(match schema {
            Schema::Bytes { format } => Self::Bytes { format: *format },
            Schema::Seq(item) | Schema::Array { item, .. } => Self::Unary {
                item: Box::new(Self::extract(item, work, child_depth)?),
            },
            Schema::Tuple(items) => {
                work.collection(items.len())?;
                Self::Tuple {
                    items: items
                        .iter()
                        .map(|item| Self::extract(item, work, child_depth))
                        .collect::<Result<_, _>>()?,
                }
            }
            Schema::Map { key, value, .. } => Self::Map {
                key: Box::new(Self::extract(key, work, child_depth)?),
                value: Box::new(Self::extract(value, work, child_depth)?),
            },
            Schema::RecordValue { fields } => {
                work.collection(fields.len())?;
                Self::RecordFields {
                    fields: fields
                        .iter()
                        .map(|field| Self::extract(field.schema(), work, child_depth))
                        .collect::<Result<_, _>>()?,
                }
            }
            Schema::Record {
                name,
                fields,
                deny_unknown_fields,
            } => {
                work.collection(fields.len())?;
                Self::Record {
                    name: clone_string(name)?,
                    deny_unknown_fields: *deny_unknown_fields,
                    fields: fields
                        .iter()
                        .map(|field| {
                            Ok(RuntimeFieldCodecUse {
                                wire_name: clone_string(&field.wire_name)?,
                                has_default: field.has_default,
                                default_program: None,
                                skip: field.skip,
                                bytes_format: field.bytes_format,
                                value: Self::extract(&field.schema, work, child_depth)?,
                            })
                        })
                        .collect::<Result<_, RuntimeCodecUseError>>()?,
                }
            }
            Schema::Enum {
                name,
                variants,
                tag,
                repr,
            } => {
                work.collection(variants.len())?;
                Self::Enum {
                    name: clone_string(name)?,
                    tag: match tag {
                        RuntimeEnumTagStyle::External => RuntimeEnumTagStyle::External,
                        RuntimeEnumTagStyle::Internal { tag } => RuntimeEnumTagStyle::Internal {
                            tag: clone_string(tag)?,
                        },
                        RuntimeEnumTagStyle::Adjacent { tag, content } => {
                            RuntimeEnumTagStyle::Adjacent {
                                tag: clone_string(tag)?,
                                content: clone_string(content)?,
                            }
                        }
                    },
                    repr: *repr,
                    cases: variants
                        .iter()
                        .map(|case| {
                            Ok(RuntimeVariantCodecUse {
                                wire_name: clone_string(&case.wire_name)?,
                                discriminant: case.discriminant,
                                payload: case
                                    .payload
                                    .as_ref()
                                    .map(|schema| Self::extract(schema, work, child_depth))
                                    .transpose()?,
                            })
                        })
                        .collect::<Result<_, RuntimeCodecUseError>>()?,
                }
            }
            Schema::Builtin(builtin) => {
                work.collection(builtin.payloads().len())?;
                Self::Builtin {
                    payloads: builtin
                        .payloads()
                        .iter()
                        .map(|payload| Self::extract(payload, work, child_depth))
                        .collect::<Result<_, _>>()?,
                }
            }
            Schema::Choice(items) => {
                work.collection(items.len())?;
                Self::Choice {
                    alternatives: items
                        .iter()
                        .map(|item| Self::extract(item, work, child_depth))
                        .collect::<Result<_, _>>()?,
                }
            }
            Schema::ExactOpaque { arguments, .. } => {
                work.collection(arguments.len())?;
                Self::Opaque {
                    arguments: arguments
                        .iter()
                        .map(|argument| Self::extract(argument, work, child_depth))
                        .collect::<Result<_, _>>()?,
                }
            }
            Schema::NominalRef(_) => Self::NominalRef,
            Schema::Named(name) => {
                return Err(RuntimeCodecUseError::UnresolvedName {
                    name: clone_string(name)?,
                });
            }
            Schema::Unit
            | Schema::Never
            | Schema::Bool
            | Schema::I8
            | Schema::I16
            | Schema::I32
            | Schema::I64
            | Schema::I128
            | Schema::ISize
            | Schema::U8
            | Schema::U16
            | Schema::U32
            | Schema::U64
            | Schema::U128
            | Schema::USize
            | Schema::F32
            | Schema::F64
            | Schema::String
            | Schema::Char
            | Schema::Duration
            | Schema::Progress
            | Schema::EntityReference
            | Schema::AgentValue => Self::Plain,
        })
    }

    /// Checks policy-bearing descendants against their exact original schema.
    /// A RecordFields source may receive declaration-owned field policies, but
    /// no nested type or Bytes use can be rewritten by those annotations.
    pub fn validate_schema(
        &self,
        schema: &RuntimeTypeSchema,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeCodecUseError> {
        self.zip_schema(schema, &mut ValidationWork::new(limits), 0, "$")
    }

    fn zip_schema(
        &self,
        schema: &RuntimeTypeSchema,
        work: &mut ValidationWork,
        depth: usize,
        path: &str,
    ) -> Result<(), RuntimeCodecUseError> {
        use RuntimeTypeSchema as Schema;
        work.charge(depth)?;
        let mismatch = || RuntimeCodecUseError::Mismatch {
            path: path.to_owned(),
        };
        let zip = |left: &[Self],
                   right: &[Schema],
                   work: &mut ValidationWork|
         -> Result<(), RuntimeCodecUseError> {
            if left.len() != right.len() {
                return Err(mismatch());
            }
            work.collection(left.len())?;
            left.iter()
                .zip(right)
                .enumerate()
                .try_for_each(|(index, (left, right))| {
                    left.zip_schema(right, work, depth + 1, &format!("{path}[{index}]"))
                })
        };
        match (self, schema) {
            (Self::Bytes { format: actual }, Schema::Bytes { format: expected })
                if actual == expected =>
            {
                Ok(())
            }
            (Self::Unary { item }, Schema::Seq(schema) | Schema::Array { item: schema, .. }) => {
                item.zip_schema(schema, work, depth + 1, &format!("{path}.item"))
            }
            (Self::Tuple { items }, Schema::Tuple(schemas)) => zip(items, schemas, work),
            (Self::Newtype { inner }, Schema::Tuple(schemas)) if schemas.len() == 1 => {
                inner.zip_schema(&schemas[0], work, depth + 1, &format!("{path}.newtype"))
            }
            (
                Self::Map { key, value },
                Schema::Map {
                    key: key_schema,
                    value: value_schema,
                    ..
                },
            ) => {
                key.zip_schema(key_schema, work, depth + 1, &format!("{path}.key"))?;
                value.zip_schema(value_schema, work, depth + 1, &format!("{path}.value"))
            }
            (Self::RecordFields { fields }, Schema::RecordValue { fields: schemas })
                if fields.len() == schemas.len() =>
            {
                fields
                    .iter()
                    .zip(schemas)
                    .enumerate()
                    .try_for_each(|(index, (field, schema))| {
                        field.zip_schema(
                            schema.schema(),
                            work,
                            depth + 1,
                            &format!("{path}.field[{index}]"),
                        )
                    })
            }
            (Self::Record { fields, .. }, Schema::RecordValue { fields: schemas })
                if fields.len() == schemas.len() =>
            {
                fields
                    .iter()
                    .zip(schemas)
                    .enumerate()
                    .try_for_each(|(index, (field, schema))| {
                        field.value.zip_schema(
                            schema.schema(),
                            work,
                            depth + 1,
                            &format!("{path}.field[{index}]"),
                        )
                    })
            }
            (Self::Builtin { payloads }, Schema::Builtin(builtin)) => {
                zip(payloads, builtin.payloads(), work)
            }
            (Self::Choice { alternatives }, Schema::Choice(schemas)) => {
                zip(alternatives, schemas, work)
            }
            (
                Self::Opaque { arguments },
                Schema::ExactOpaque {
                    arguments: schemas, ..
                },
            ) => zip(arguments, schemas, work),
            (Self::NominalRef, Schema::NominalRef(_)) => Ok(()),
            _ => {
                let expected = Self::extract(schema, work, depth)?;
                if self == &expected {
                    Ok(())
                } else {
                    Err(mismatch())
                }
            }
        }
    }
}

impl Drop for RuntimeCodecUse {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.take_children(&mut pending);
        while let Some(mut node) = pending.pop() {
            node.take_children(&mut pending);
        }
    }
}

pub(super) fn encode<S: super::CanonicalSink + ?Sized>(
    root: &RuntimeCodecUse,
    writer: &mut super::CanonicalWriter<'_, S>,
) -> Result<(), RuntimeSchemaError> {
    enum Work<'a> {
        Node(&'a RuntimeCodecUse),
        Nodes(std::slice::Iter<'a, RuntimeCodecUse>),
        Fields(std::slice::Iter<'a, RuntimeFieldCodecUse>),
        Cases(std::slice::Iter<'a, RuntimeVariantCodecUse>),
    }
    let mut work = vec![Work::Node(root)];
    while let Some(next) = work.pop() {
        match next {
            Work::Nodes(mut items) => {
                if let Some(item) = items.next() {
                    work.push(Work::Nodes(items));
                    work.push(Work::Node(item));
                }
            }
            Work::Fields(mut fields) => {
                if let Some(field) = fields.next() {
                    writer.string(&field.wire_name)?;
                    writer.u8(u8::from(field.has_default))?;
                    writer.option(field.default_program.as_ref(), |writer, program| {
                        writer.extend(&program.as_bytes())
                    })?;
                    writer.u8(u8::from(field.skip))?;
                    writer.option(field.bytes_format.as_ref(), |writer, format| {
                        writer.u8(format.tag())
                    })?;
                    work.push(Work::Fields(fields));
                    work.push(Work::Node(&field.value));
                }
            }
            Work::Cases(mut cases) => {
                if let Some(case) = cases.next() {
                    writer.string(&case.wire_name)?;
                    writer.option(case.discriminant.as_ref(), |writer, value| {
                        writer.i128(*value)
                    })?;
                    writer.u8(u8::from(case.payload.is_some()))?;
                    work.push(Work::Cases(cases));
                    if let Some(payload) = &case.payload {
                        work.push(Work::Node(payload));
                    }
                }
            }
            Work::Node(node) => match node {
                RuntimeCodecUse::Plain => writer.u8(0)?,
                RuntimeCodecUse::Bytes { format } => {
                    writer.u8(1)?;
                    writer.u8(format.tag())?;
                }
                RuntimeCodecUse::Unary { item } => {
                    writer.u8(2)?;
                    work.push(Work::Node(item));
                }
                RuntimeCodecUse::Tuple { items } => {
                    writer.u8(3)?;
                    writer.len(items.len())?;
                    work.push(Work::Nodes(items.iter()));
                }
                RuntimeCodecUse::Map { key, value } => {
                    writer.u8(4)?;
                    work.push(Work::Node(value));
                    work.push(Work::Node(key));
                }
                RuntimeCodecUse::RecordFields { fields } => {
                    writer.u8(5)?;
                    writer.len(fields.len())?;
                    work.push(Work::Nodes(fields.iter()));
                }
                RuntimeCodecUse::Record {
                    name,
                    deny_unknown_fields,
                    fields,
                } => {
                    writer.u8(6)?;
                    writer.string(name)?;
                    writer.u8(u8::from(*deny_unknown_fields))?;
                    writer.len(fields.len())?;
                    work.push(Work::Fields(fields.iter()));
                }
                RuntimeCodecUse::Enum {
                    name,
                    tag,
                    repr,
                    cases,
                } => {
                    writer.u8(7)?;
                    writer.string(name)?;
                    match tag {
                        RuntimeEnumTagStyle::External => writer.u8(0)?,
                        RuntimeEnumTagStyle::Internal { tag } => {
                            writer.u8(1)?;
                            writer.string(tag)?;
                        }
                        RuntimeEnumTagStyle::Adjacent { tag, content } => {
                            writer.u8(2)?;
                            writer.string(tag)?;
                            writer.string(content)?;
                        }
                    }
                    writer.option(repr.as_ref(), |writer, repr| writer.u8(repr.tag()))?;
                    writer.len(cases.len())?;
                    work.push(Work::Cases(cases.iter()));
                }
                RuntimeCodecUse::Builtin { payloads } => {
                    writer.u8(8)?;
                    writer.len(payloads.len())?;
                    work.push(Work::Nodes(payloads.iter()));
                }
                RuntimeCodecUse::Choice { alternatives } => {
                    writer.u8(9)?;
                    writer.len(alternatives.len())?;
                    work.push(Work::Nodes(alternatives.iter()));
                }
                RuntimeCodecUse::Opaque { arguments } => {
                    writer.u8(10)?;
                    writer.len(arguments.len())?;
                    work.push(Work::Nodes(arguments.iter()));
                }
                RuntimeCodecUse::NominalRef => writer.u8(11)?,
                RuntimeCodecUse::Newtype { inner } => {
                    writer.u8(12)?;
                    work.push(Work::Node(inner));
                }
            },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
