//! Schema predicates consumed by the canonical value visitor.

use std::collections::{BTreeMap, BTreeSet};

use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeVariantIdentity};
use crate::value::{
    RuntimeInt, RuntimeScalarView as Scalar, RuntimeUInt, RuntimeValueView as View,
};

use super::{
    RuntimeNominalSchemaBody, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
    RuntimeSchemaError as Error, RuntimeSchemaField, RuntimeSchemaVariant,
    RuntimeTypeSchema as Schema,
    nominal::encoding::LayoutOperation,
    schema_definitions,
    value_encoding::{ValueAdmission, ValueValidation},
};

use super::value_budget::ValidationWork;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) enum Expected<'a> {
    Schema(&'a Schema),
    GraphRoot(crate::pattern::RuntimeSemanticTypeId),
    Tuple(&'a [Schema]),
    MapEntry { key: &'a Schema, value: &'a Schema },
    OpaquePayload,
}

pub(super) enum Children<'a> {
    None,
    Any,
    Single(Expected<'a>),
    Repeated(Expected<'a>),
    Tuple(&'a [Schema]),
    Fields(Box<[&'a Schema]>),
    MapEntry { key: &'a Schema, value: &'a Schema },
}

pub(super) struct SchemaValueValidation<'a> {
    authority: SchemaAuthority<'a>,
    work: ValidationWork,
}

pub(super) struct ChoiceAlternatives<'a> {
    schemas: std::slice::Iter<'a, Schema>,
}

impl<'a> Iterator for ChoiceAlternatives<'a> {
    type Item = Expected<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.schemas.next().map(Expected::Schema)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.schemas.size_hint()
    }
}

impl ExactSizeIterator for ChoiceAlternatives<'_> {}

enum SchemaAuthority<'a> {
    Tree(BTreeMap<&'a str, &'a Schema>),
    Graph(LayoutOperation<'a>),
}

impl<'a> SchemaValueValidation<'a> {
    pub(super) fn tree(
        schema: &'a Schema,
        limits: super::RuntimeSchemaLimits,
    ) -> Result<Self, Error> {
        Ok(Self {
            authority: SchemaAuthority::Tree(schema_definitions(schema)?),
            work: ValidationWork::new(limits),
        })
    }

    pub(super) fn graph(
        graph: &'a RuntimeNominalSchemaGraph,
        limits: super::RuntimeSchemaLimits,
    ) -> Self {
        Self {
            authority: SchemaAuthority::Graph(LayoutOperation::new(graph)),
            work: ValidationWork::new(limits),
        }
    }

    pub(super) fn validate(
        mut self,
        value: &crate::value::RuntimeValue,
        expected: Expected<'a>,
    ) -> Result<crate::entry::RuntimeValueDigest, Error> {
        let limits = self.work.limits();
        super::value_encoding::validate_and_hash(value, limits, &mut self, expected)
    }

    fn nominal(
        &mut self,
        identity: &'a RuntimeNominalSchemaIdentity,
        value: View<'_>,
    ) -> Result<Children<'a>, Error> {
        let SchemaAuthority::Graph(operation) = &mut self.authority else {
            return Err(Error::NominalGraphRequired {
                identity: identity.clone(),
            });
        };
        let definition = operation
            .graph()
            .definition(identity.semantic_identity())
            .filter(|definition| definition.identity() == identity)
            .ok_or_else(|| Error::UnresolvedNominal {
                identity: identity.clone(),
            })?;
        let actual_layout = match (definition.body(), value) {
            (RuntimeNominalSchemaBody::Record { .. }, View::NominalRecord(record)) => {
                super::validate_nominal_identity(identity.nominal(), record.type_id(), "$")?;
                if record.semantic_identity() != identity.semantic_identity() {
                    return Err(Error::NominalSemanticIdentity {
                        path: "$".to_owned(),
                        expected: identity.semantic_identity(),
                        actual: record.semantic_identity(),
                    });
                }
                record.layout()
            }
            (
                RuntimeNominalSchemaBody::Variant { .. },
                View::Variant {
                    owner:
                        RuntimeVariantIdentity::Nominal {
                            nominal,
                            semantic_identity,
                            layout,
                        },
                    ..
                },
            ) => {
                super::validate_nominal_identity(identity.nominal(), nominal, "$")?;
                if *semantic_identity != identity.semantic_identity() {
                    return Err(Error::NominalSemanticIdentity {
                        path: "$".to_owned(),
                        expected: identity.semantic_identity(),
                        actual: *semantic_identity,
                    });
                }
                *layout
            }
            (RuntimeNominalSchemaBody::Record { .. }, _) => {
                return Err(Self::mismatch("nominal record", value));
            }
            (RuntimeNominalSchemaBody::Variant { .. }, _) => {
                return Err(Self::mismatch("nominal variant", value));
            }
        };
        let layout = operation
            .hash(identity.semantic_identity())
            .map_err(|source| Error::NominalGraph {
                source: Box::new(source),
            })?;
        if actual_layout != layout {
            return Err(Error::NominalLayout {
                path: "$".to_owned(),
            });
        }
        match (definition.body(), value) {
            (RuntimeNominalSchemaBody::Record { fields, .. }, View::NominalRecord(record)) => {
                Self::arity(fields.len(), record.fields().len())?;
                Ok(Children::Fields(
                    fields.iter().map(|field| field.schema()).collect(),
                ))
            }
            (
                RuntimeNominalSchemaBody::Variant { cases },
                View::Variant {
                    ordinal,
                    name,
                    payload,
                    ..
                },
            ) => {
                let case = usize::try_from(ordinal)
                    .ok()
                    .and_then(|index| cases.get(index))
                    .filter(|case| case.ordinal() == ordinal && case.name() == name)
                    .ok_or_else(|| Error::UnknownVariant {
                        path: "$".to_owned(),
                        variant: name.to_owned(),
                    })?;
                match (case.payload(), payload.is_some()) {
                    (Some(schema), true) => Ok(Children::Single(Expected::Schema(schema))),
                    (None, false) => Ok(Children::None),
                    _ => Err(Error::VariantPayload {
                        path: "$".to_owned(),
                    }),
                }
            }
            _ => unreachable!("nominal header selected this exact value family"),
        }
    }

    fn mismatch(expected: &'static str, actual: View<'_>) -> Error {
        Error::Type {
            path: "$".to_owned(),
            expected,
            actual: actual.type_name(),
        }
    }

    fn arity(expected: usize, actual: usize) -> Result<(), Error> {
        if actual != expected {
            return Err(Error::Arity {
                path: "$".to_owned(),
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn serde_record(
        fields: &'a [RuntimeSchemaField],
        actual: crate::value::RuntimeRecordView<'_>,
    ) -> Result<Children<'a>, Error> {
        let mut expected = BTreeMap::new();
        for field in fields.iter().filter(|field| !field.skip) {
            if expected.insert(field.rust_name.as_str(), field).is_some() {
                return Err(Error::Encoding {
                    message: format!("duplicate schema field `{}`", field.rust_name),
                });
            }
        }
        let mut children = Vec::with_capacity(actual.len());
        let mut present = BTreeSet::new();
        for index in 0..actual.len() {
            let (_, name, _) = actual
                .get(index)
                .expect("record view has every admitted field");
            let field = expected.get(name).ok_or_else(|| Error::UnknownField {
                path: "$".to_owned(),
                field: name.to_owned(),
            })?;
            present.insert(name);
            children.push(&field.schema);
        }
        for field in expected.values() {
            if !field.has_default && !present.contains(field.rust_name.as_str()) {
                return Err(Error::MissingField {
                    path: "$".to_owned(),
                    field: field.rust_name.clone(),
                });
            }
        }
        Ok(Children::Fields(children.into_boxed_slice()))
    }

    fn serde_enum(
        variants: &'a [RuntimeSchemaVariant],
        ordinal: u32,
        name: &str,
        has_payload: bool,
    ) -> Result<Children<'a>, Error> {
        let case = usize::try_from(ordinal)
            .ok()
            .and_then(|index| variants.get(index))
            .filter(|case| case.rust_name == name)
            .ok_or_else(|| Error::UnknownVariant {
                path: "$".to_owned(),
                variant: name.to_owned(),
            })?;
        match (&case.payload, has_payload) {
            (None, false) => Ok(Children::None),
            (Some(schema), true) => Ok(Children::Single(Expected::Schema(schema))),
            _ => Err(Error::VariantPayload {
                path: "$".to_owned(),
            }),
        }
    }

    fn schema(&self, schema: &'a Schema, value: View<'_>) -> Result<Children<'a>, Error> {
        match (schema, value) {
            (Schema::Unit, View::Scalar(Scalar::Unit))
            | (Schema::Bool, View::Scalar(Scalar::Bool(_)))
            | (Schema::I8, View::Scalar(Scalar::Int(RuntimeInt::I8(_))))
            | (Schema::I16, View::Scalar(Scalar::Int(RuntimeInt::I16(_))))
            | (Schema::I32, View::Scalar(Scalar::Int(RuntimeInt::I32(_))))
            | (Schema::I64, View::Scalar(Scalar::Int(RuntimeInt::I64(_))))
            | (Schema::I128, View::Scalar(Scalar::Int(RuntimeInt::I128(_))))
            | (Schema::ISize, View::Scalar(Scalar::Int(RuntimeInt::ISize(_))))
            | (Schema::U8, View::Scalar(Scalar::UInt(RuntimeUInt::U8(_))))
            | (Schema::U16, View::Scalar(Scalar::UInt(RuntimeUInt::U16(_))))
            | (Schema::U32, View::Scalar(Scalar::UInt(RuntimeUInt::U32(_))))
            | (Schema::U64, View::Scalar(Scalar::UInt(RuntimeUInt::U64(_))))
            | (Schema::U128, View::Scalar(Scalar::UInt(RuntimeUInt::U128(_))))
            | (Schema::USize, View::Scalar(Scalar::UInt(RuntimeUInt::USize(_))))
            | (Schema::F32, View::Scalar(Scalar::F32(_)))
            | (Schema::F64, View::Scalar(Scalar::F64(_)))
            | (Schema::String, View::Scalar(Scalar::String(_)))
            | (Schema::Color, View::Scalar(Scalar::Color(_)))
            | (Schema::Char, View::Scalar(Scalar::Char(_)))
            | (Schema::Duration, View::Scalar(Scalar::Duration(_)))
            | (Schema::Progress, View::Scalar(Scalar::Progress(_)))
            | (Schema::EntityReference, View::Scalar(Scalar::EntityRef(_))) => Ok(Children::None),
            (Schema::AgentValue, value) if value.is_agent_value_node() => {
                Ok(Children::Repeated(Expected::Schema(schema)))
            }
            (Schema::Bytes { .. }, View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::Schema(&Schema::U8)))
            }
            (Schema::Seq(inner), View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::Schema(inner)))
            }
            (Schema::Array { item, length }, View::Sequence(actual)) => {
                if u64::try_from(actual.len()) != Ok(*length) {
                    return Err(Error::ArrayLength {
                        path: "$".to_owned(),
                        expected: *length,
                        actual: actual.len(),
                    });
                }
                Ok(Children::Repeated(Expected::Schema(item)))
            }
            (Schema::Map { key, value, .. }, View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::MapEntry { key, value }))
            }
            (Schema::Tuple(items), View::Tuple(actual)) => {
                Self::arity(items.len(), actual.len())?;
                Ok(Children::Tuple(items))
            }
            (Schema::RecordValue { fields }, View::Record(actual)) => {
                Self::arity(fields.len(), actual.len())?;
                for (index, field) in fields.iter().enumerate() {
                    let (id, name, _) = actual.get(index).expect("record arity has been checked");
                    if id != field.field() || name != field.name() {
                        return Err(Error::RecordField {
                            path: "$".to_owned(),
                            ordinal: index,
                        });
                    }
                }
                Ok(Children::Fields(
                    fields.iter().map(|field| field.schema()).collect(),
                ))
            }
            (Schema::ExactOpaque { owner, .. }, View::Opaque(actual)) => {
                if owner.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity
                    || !owner.accepts_opaque_value(actual)
                {
                    return Err(Error::OpaqueOwner {
                        path: "$".to_owned(),
                    });
                }
                Ok(Children::Single(Expected::OpaquePayload))
            }
            (
                Schema::Builtin(builtin),
                View::Variant {
                    owner,
                    ordinal,
                    name,
                    payload,
                },
            ) => {
                if owner != &RuntimeVariantIdentity::Builtin(builtin.owner()) {
                    return Err(Error::BuiltinVariantOwner {
                        path: "$".to_owned(),
                        expected: builtin.owner(),
                        actual: owner.clone(),
                    });
                }
                let (_, expected) = usize::try_from(ordinal)
                    .ok()
                    .and_then(|ordinal| builtin.case(ordinal))
                    .filter(|(case, _)| case.name() == name)
                    .ok_or_else(|| Error::UnknownVariant {
                        path: "$".to_owned(),
                        variant: name.to_owned(),
                    })?;
                match (expected, payload.is_some()) {
                    (None, false) => Ok(Children::None),
                    (Some(expected), true) => Ok(Children::Single(Expected::Tuple(
                        std::slice::from_ref(expected),
                    ))),
                    _ => Err(Error::VariantPayload {
                        path: "$".to_owned(),
                    }),
                }
            }
            (Schema::Record { fields, .. }, View::Record(actual)) => {
                Self::serde_record(fields, actual)
            }
            (Schema::Record { name, fields, .. }, View::NominalRecord(actual))
                if actual.type_id().as_str() == name =>
            {
                Self::arity(fields.len(), actual.fields().len())?;
                Ok(Children::Fields(
                    fields.iter().map(|field| &field.schema).collect(),
                ))
            }
            (
                Schema::Enum {
                    name: expected_owner,
                    variants,
                    ..
                },
                View::Variant {
                    owner: RuntimeVariantIdentity::Nominal { nominal, .. },
                    ordinal,
                    name,
                    payload,
                },
            ) if nominal.as_str() == expected_owner => {
                Self::serde_enum(variants, ordinal, name, payload.is_some())
            }
            _ => Err(Self::mismatch(schema.type_label(), value)),
        }
    }
}

impl<'a> SchemaValueValidation<'a> {
    fn check(&mut self, expected: Expected<'a>, value: View<'_>) -> Result<Children<'a>, Error> {
        match expected {
            Expected::OpaquePayload => Ok(Children::Any),
            Expected::Schema(Schema::NominalRef(identity)) => self.nominal(identity, value),
            Expected::GraphRoot(root) => {
                let SchemaAuthority::Graph(operation) = &self.authority else {
                    unreachable!("graph roots are supplied by the graph API");
                };
                let definition =
                    operation
                        .graph()
                        .definition(root)
                        .ok_or_else(|| Error::NominalGraph {
                            source: Box::new(super::RuntimeNominalSchemaGraphError::UnknownRoot {
                                root,
                            }),
                        })?;
                self.nominal(definition.identity(), value)
            }
            Expected::Tuple(items) => {
                let View::Tuple(tuple) = value else {
                    return Err(Self::mismatch("tuple", value));
                };
                Self::arity(items.len(), tuple.len())?;
                Ok(Children::Tuple(items))
            }
            Expected::Schema(Schema::Named(name)) => {
                let schema = match &self.authority {
                    SchemaAuthority::Tree(definitions) => definitions.get(name.as_str()).copied(),
                    SchemaAuthority::Graph(_) => None,
                }
                .ok_or_else(|| Error::UnresolvedNamed {
                    path: "$".to_owned(),
                    name: name.clone(),
                })?;
                self.schema(schema, value)
            }
            Expected::Schema(schema) => self.schema(schema, value),
            Expected::MapEntry {
                key,
                value: expected_value,
            } => {
                let View::Tuple(tuple) = value else {
                    return Err(Self::mismatch("map entry tuple", value));
                };
                Self::arity(2, tuple.len())?;
                Ok(Children::MapEntry {
                    key,
                    value: expected_value,
                })
            }
        }
    }
}

impl<'a> ValueValidation for SchemaValueValidation<'a> {
    type Expected = Expected<'a>;
    type Children = Children<'a>;
    type Alternatives = ChoiceAlternatives<'a>;

    fn preflight(&mut self, expected: &Expected<'a>, depth: usize) -> Result<(), Error> {
        if !Self::is_admitted(expected) {
            self.work.charge(depth)?;
        }
        Ok(())
    }

    fn alternative(&mut self, depth: usize) -> Result<(), Error> {
        self.work.charge(depth)
    }

    fn is_admitted(expected: &Expected<'a>) -> bool {
        matches!(expected, Expected::OpaquePayload)
    }

    fn admitted_children() -> Children<'a> {
        Children::Any
    }

    fn enter(
        &mut self,
        expected: Expected<'a>,
        value: View<'_>,
    ) -> Result<ValueAdmission<Children<'a>, ChoiceAlternatives<'a>>, Error> {
        if Self::is_admitted(&expected) {
            return Ok(ValueAdmission::Admitted);
        }
        if let Expected::Schema(Schema::Choice(alternatives)) = expected {
            self.work.collection(alternatives.len())?;
            return Ok(ValueAdmission::Choice(ChoiceAlternatives {
                schemas: alternatives.iter(),
            }));
        }
        self.check(expected, value).map(ValueAdmission::Children)
    }

    fn child(&mut self, children: &Children<'a>, index: usize) -> Result<Expected<'a>, Error> {
        match children {
            Children::Any => Ok(Expected::OpaquePayload),
            Children::Single(expected) if index == 0 => Ok(*expected),
            Children::Repeated(expected) => Ok(*expected),
            Children::Tuple(items) => {
                items
                    .get(index)
                    .map(Expected::Schema)
                    .ok_or_else(|| Error::Arity {
                        path: "$".to_owned(),
                        expected: items.len(),
                        actual: index + 1,
                    })
            }
            Children::Fields(fields) => fields
                .get(index)
                .map(|schema| Expected::Schema(schema))
                .ok_or_else(|| Error::Arity {
                    path: "$".to_owned(),
                    expected: fields.len(),
                    actual: index + 1,
                }),
            Children::MapEntry { key, .. } if index == 0 => Ok(Expected::Schema(key)),
            Children::MapEntry { value, .. } if index == 1 => Ok(Expected::Schema(value)),
            Children::None | Children::Single(_) | Children::MapEntry { .. } => {
                Err(Error::Encoding {
                    message: "validated value has an unexpected child".to_owned(),
                })
            }
        }
    }
}
