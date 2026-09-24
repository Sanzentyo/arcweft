//! Correlation of a source schema proof with the prepared plan authorities.
//!
//! Source semantic IDs come from the existing type/domain seeds. The schema
//! supplies the independent nominal structure and layout proof; it never
//! manufactures replacement IDs for inline schemas or survives plan sealing.

use thiserror::Error;

use crate::entry::{
    RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaGraphError, RuntimeSchemaError, RuntimeTypeSchema as Schema,
    TypeLayoutHash,
    schema::{
        traversal::{SchemaPath, SchemaStep, SchemaVisitor},
        value_budget::ValidationWork,
    },
};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId,
};
use crate::plan::{RuntimePlanTypeDeclaration, RuntimePlanTypeProjection as Type};
use crate::runtime_id::RuntimePlanTypeId;
use crate::value::{RuntimeSignedIntWidth as Signed, RuntimeUnsignedIntWidth as Unsigned};

use super::super::{
    nominal_record_domains::PreparedRuntimeNominalRecordDomainBatch,
    type_table::{PreparedRuntimePlanTypeBatch, RuntimePlanTypeTableBuilder},
    variant_domains::PreparedRuntimeVariantDomainBatch,
};

/// A prepared plan disagrees with the supplied nominal schema proof.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanNominalSchemaError {
    #[error("source codec-use extraction failed: {source}")]
    CodecUse {
        source: Box<crate::entry::schema::RuntimeCodecUseError>,
    },
    #[error("nominal type {identity:?} has no schema proof in this batch")]
    MissingProof { identity: RuntimeSemanticTypeId },
    #[error("schema definition {identity:?} has no prepared plan type")]
    MissingType { identity: RuntimeSemanticTypeId },
    #[error("plan type {ty} disagrees with schema {identity:?} at {path}: {component:?}")]
    Mismatch {
        identity: RuntimeSemanticTypeId,
        ty: RuntimePlanTypeId,
        path: String,
        component: RuntimePlanSchemaComponent,
    },
    #[error("nominal type {identity:?} has layout {actual:?}, expected {expected:?}")]
    Layout {
        identity: RuntimeSemanticTypeId,
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
    #[error("nominal schema layout derivation failed: {source}")]
    Graph {
        source: Box<RuntimeNominalSchemaGraphError>,
    },
    #[error("nominal schema correlation exceeds its work allowance: {source}")]
    Limits { source: Box<RuntimeSchemaError> },
}

/// The structural component that could not be correlated with the source proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePlanSchemaComponent {
    Projection,
    NominalIdentity,
    Arguments,
    DomainKind,
    RecordShape,
    RecordFields,
    VariantCases,
}

/// All references borrow the candidate tables before their common commit.
pub(super) struct PreparedNominalSchema<'a> {
    pub(super) existing_types: &'a RuntimePlanTypeTableBuilder,
    pub(super) types: &'a PreparedRuntimePlanTypeBatch,
    pub(super) records: &'a PreparedRuntimeNominalRecordDomainBatch,
    pub(super) variants: &'a PreparedRuntimeVariantDomainBatch,
}

impl PreparedNominalSchema<'_> {
    pub(super) fn validate(
        &self,
        graph: &RuntimeNominalSchemaGraph,
        domain_owners: impl IntoIterator<Item = RuntimePlanTypeId>,
    ) -> Result<(), RuntimePlanNominalSchemaError> {
        let mut work = ValidationWork::new(graph.limits());
        let requested_nominals = self.types.result_ids().iter().copied().filter(|ty| {
            self.existing_types.get(*ty).is_none()
                && matches!(self.declaration(*ty).projection(), Type::Nominal { .. })
        });
        for ty in requested_nominals.chain(domain_owners) {
            work.charge(1)
                .map_err(|source| RuntimePlanNominalSchemaError::Limits {
                    source: Box::new(source),
                })?;
            let identity = self.declaration(ty).semantic_identity();
            if graph.definition(identity).is_none() {
                return Err(RuntimePlanNominalSchemaError::MissingProof { identity });
            }
        }
        let layouts =
            graph
                .try_layouts()
                .map_err(|source| RuntimePlanNominalSchemaError::Graph {
                    source: Box::new(source),
                })?;
        for definition in graph.definitions() {
            work.charge(1)
                .map_err(|source| RuntimePlanNominalSchemaError::Limits {
                    source: Box::new(source),
                })?;
            let identity = definition.identity().semantic_identity();
            let ty = self
                .types
                .id_for_semantic(identity)
                .ok_or(RuntimePlanNominalSchemaError::MissingType { identity })?;
            let mut comparison = DefinitionComparison {
                tables: self,
                layouts: &layouts,
                identity,
                work: &mut work,
            };
            comparison.definition(definition, ty)?;
        }
        Ok(())
    }

    fn declaration(&self, ty: RuntimePlanTypeId) -> &RuntimePlanTypeDeclaration {
        self.types
            .get(ty)
            .expect("prepared types and domains have no dangling plan reference")
    }
}

struct DefinitionComparison<'a, 'tables> {
    tables: &'a PreparedNominalSchema<'tables>,
    layouts: &'a [(RuntimeSemanticTypeId, TypeLayoutHash)],
    identity: RuntimeSemanticTypeId,
    work: &'a mut ValidationWork,
}

impl DefinitionComparison<'_, '_> {
    fn mismatch(
        &self,
        ty: RuntimePlanTypeId,
        path: &SchemaPath<'_>,
        component: RuntimePlanSchemaComponent,
    ) -> RuntimePlanNominalSchemaError {
        RuntimePlanNominalSchemaError::Mismatch {
            identity: self.identity,
            ty,
            path: path.to_string(),
            component,
        }
    }

    fn layout(
        &self,
        identity: RuntimeSemanticTypeId,
        actual: TypeLayoutHash,
    ) -> Result<(), RuntimePlanNominalSchemaError> {
        let index = self
            .layouts
            .binary_search_by_key(&identity, |(identity, _)| *identity)
            .expect("layout operation covers every definition in semantic identity order");
        let expected = self.layouts[index].1;
        if expected != actual {
            return Err(RuntimePlanNominalSchemaError::Layout {
                identity,
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn definition(
        &mut self,
        definition: &RuntimeNominalSchemaDefinition,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanNominalSchemaError> {
        use RuntimePlanSchemaComponent as Component;
        let name = definition.identity().nominal().as_str();
        let path = SchemaPath::root(name);
        let Type::Nominal {
            nominal,
            layout,
            arguments,
        } = self.tables.declaration(ty).projection()
        else {
            return Err(self.mismatch(ty, &path, Component::Projection));
        };
        if nominal != definition.identity().nominal() {
            return Err(self.mismatch(ty, &path, Component::NominalIdentity));
        }
        self.layout(self.identity, *layout)?;
        if arguments.len() != definition.arguments().len() {
            return Err(self.mismatch(ty, &path, Component::Arguments));
        }
        for (ordinal, (schema, ty)) in definition.arguments().iter().zip(arguments).enumerate() {
            schema.walk_with_state(
                &mut SchemaPath::new(name, SchemaStep::Argument(ordinal)),
                *ty,
                self,
            )?;
        }
        match definition.body() {
            RuntimeNominalSchemaBody::Record { shape, fields } => {
                let Some(record) = self.tables.records.get(ty) else {
                    return Err(self.mismatch(ty, &path, Component::DomainKind));
                };
                if self.tables.variants.get(ty).is_some() {
                    return Err(self.mismatch(ty, &path, Component::DomainKind));
                }
                if record.shape() != *shape {
                    return Err(self.mismatch(ty, &path, Component::RecordShape));
                }
                if record.fields().len() != fields.len() {
                    return Err(self.mismatch(ty, &path, Component::RecordFields));
                }
                for (ordinal, (schema, actual)) in fields.iter().zip(record.fields()).enumerate() {
                    let mut path = SchemaPath::new(name, SchemaStep::Field(ordinal, schema.name()));
                    if schema.field() != actual.field() || schema.name() != actual.name() {
                        return Err(self.mismatch(ty, &path, Component::RecordFields));
                    }
                    schema
                        .schema()
                        .walk_with_state(&mut path, actual.ty(), self)?;
                }
            }
            RuntimeNominalSchemaBody::Variant { cases } => {
                let Some(variant) = self.tables.variants.get(ty) else {
                    return Err(self.mismatch(ty, &path, Component::DomainKind));
                };
                if self.tables.records.get(ty).is_some() {
                    return Err(self.mismatch(ty, &path, Component::DomainKind));
                }
                if variant.nominal() != nominal {
                    return Err(self.mismatch(ty, &path, Component::NominalIdentity));
                }
                self.layout(self.identity, variant.layout())?;
                if variant.cases().len() != cases.len() {
                    return Err(self.mismatch(ty, &path, Component::VariantCases));
                }
                for (ordinal, (schema, actual)) in cases.iter().zip(variant.cases()).enumerate() {
                    let mut path = SchemaPath::new(name, SchemaStep::Case(ordinal, schema.name()));
                    if usize::try_from(schema.ordinal()) != Ok(ordinal)
                        || schema.name() != actual.name()
                    {
                        return Err(self.mismatch(ty, &path, Component::VariantCases));
                    }
                    match (schema.payload(), actual.payload()) {
                        (None, None) => {}
                        (Some(schema), Some(payload)) => {
                            schema.walk_with_state(&mut path, payload, self)?;
                        }
                        _ => return Err(self.mismatch(ty, &path, Component::VariantCases)),
                    }
                }
            }
        }
        Ok(())
    }
}

impl SchemaVisitor<'_> for DefinitionComparison<'_, '_> {
    type State = RuntimePlanTypeId;
    type Error = RuntimePlanNominalSchemaError;

    fn enter(
        &mut self,
        schema: &Schema,
        ty: RuntimePlanTypeId,
        depth: usize,
        path: &SchemaPath<'_>,
    ) -> Result<(), Self::Error> {
        use RuntimePlanSchemaComponent as Component;
        self.work
            .charge(depth)
            .map_err(|source| RuntimePlanNominalSchemaError::Limits {
                source: Box::new(source),
            })?;
        let declaration = self.tables.declaration(ty);
        let matches = match (schema, declaration.projection()) {
            (Schema::Never, Type::Never)
            | (Schema::Unit, Type::Unit)
            | (Schema::Bool, Type::Bool)
            | (Schema::I8, Type::Signed(Signed::I8))
            | (Schema::I16, Type::Signed(Signed::I16))
            | (Schema::I32, Type::Signed(Signed::I32))
            | (Schema::I64, Type::Signed(Signed::I64))
            | (Schema::I128, Type::Signed(Signed::I128))
            | (Schema::ISize, Type::Signed(Signed::ISize))
            | (Schema::U8, Type::Unsigned(Unsigned::U8))
            | (Schema::U16, Type::Unsigned(Unsigned::U16))
            | (Schema::U32, Type::Unsigned(Unsigned::U32))
            | (Schema::U64, Type::Unsigned(Unsigned::U64))
            | (Schema::U128, Type::Unsigned(Unsigned::U128))
            | (Schema::USize, Type::Unsigned(Unsigned::USize))
            | (Schema::F32, Type::F32)
            | (Schema::F64, Type::F64)
            | (Schema::String, Type::String)
            | (Schema::Char, Type::Char)
            | (Schema::Bytes { .. }, Type::Bytes)
            | (Schema::Duration, Type::Duration)
            | (Schema::Progress, Type::Progress)
            | (Schema::EntityReference, Type::EntityReference)
            | (Schema::AgentValue, Type::AgentValue)
            | (Schema::Seq(_), Type::Sequence { .. }) => true,
            (Schema::Map { kind: expected, .. }, Type::Map { kind: actual, .. }) => {
                expected == actual
            }
            (
                Schema::Array {
                    length: expected, ..
                },
                Type::Array { length: actual, .. },
            ) => Some(*expected) == actual.constant(),
            (Schema::Tuple(expected), Type::Tuple(actual))
            | (Schema::Choice(expected), Type::Choice(actual)) => expected.len() == actual.len(),
            (Schema::RecordValue { fields }, Type::Record(actual)) => {
                fields.len() == actual.len()
                    && fields
                        .iter()
                        .zip(actual)
                        .enumerate()
                        .all(|(ordinal, (field, actual))| {
                            usize::try_from(field.field().zero_based()) == Ok(ordinal)
                                && field.name() == actual.diagnostic_name()
                        })
            }
            (Schema::Builtin(expected), actual) => match actual {
                Type::Option { .. } => expected.owner() == RuntimeBuiltinVariantIdentity::Option,
                Type::Result { .. } => expected.owner() == RuntimeBuiltinVariantIdentity::Result,
                Type::BuiltinVariant { owner, .. } => expected.owner() == *owner,
                _ => false,
            },
            (
                Schema::ExactOpaque { owner, arguments },
                Type::Opaque {
                    producer,
                    admission,
                    value_class,
                    persistence,
                    arguments: actual,
                },
            ) => {
                owner
                    == &RuntimeOpaqueTypeOwner::with_admission(
                        producer.clone(),
                        declaration.semantic_identity(),
                        *admission,
                        *value_class,
                        *persistence,
                    )
                    && arguments.len() == actual.len()
            }
            (
                Schema::NominalRef(identity),
                Type::Nominal {
                    nominal, layout, ..
                },
            ) => {
                if identity.nominal() != nominal
                    || identity.semantic_identity() != declaration.semantic_identity()
                {
                    return Err(self.mismatch(ty, path, Component::NominalIdentity));
                }
                self.layout(identity.semantic_identity(), *layout)?;
                true
            }
            _ => false,
        };
        if !matches {
            return Err(self.mismatch(ty, path, Component::Projection));
        }
        Ok(())
    }

    fn child(
        &mut self,
        ty: RuntimePlanTypeId,
        step: SchemaStep<'_>,
        path: &SchemaPath<'_>,
    ) -> Result<RuntimePlanTypeId, Self::Error> {
        let declaration = self.tables.declaration(ty);
        let child = match (step, declaration.projection()) {
            (SchemaStep::Argument(index), Type::Opaque { arguments, .. }) => {
                arguments.get(index).copied()
            }
            (SchemaStep::Tuple(index), Type::Tuple(items))
            | (SchemaStep::Choice(index), Type::Choice(items)) => items.get(index).copied(),
            (SchemaStep::Field(index, _), Type::Record(fields)) => {
                fields.get(index).map(|field| *field.ty())
            }
            (SchemaStep::Sequence, Type::Sequence { item, .. })
            | (SchemaStep::Array, Type::Array { item, .. }) => Some(*item),
            (SchemaStep::MapKey, Type::Map { key, .. }) => Some(*key),
            (SchemaStep::MapValue, Type::Map { value, .. }) => Some(*value),
            (
                SchemaStep::Case(ordinal, _),
                Type::Option { .. } | Type::Result { .. } | Type::BuiltinVariant { .. },
            ) => {
                let payload = u32::try_from(ordinal)
                    .ok()
                    .and_then(|ordinal| declaration.select_variant_case(ty, None, ordinal).ok())
                    .and_then(|case| case.payload());
                payload.and_then(
                    |payload| match self.tables.declaration(payload).projection() {
                        Type::Tuple(items) => match items.as_ref() {
                            [item] => Some(*item),
                            _ => None,
                        },
                        _ => None,
                    },
                )
            }
            _ => None,
        };
        child.ok_or_else(|| self.mismatch(ty, path, RuntimePlanSchemaComponent::Projection))
    }
}
