//! Source projection shared by Project declarations and joined Rust ADTs.
//!
//! Definitions come from the accepted semantic world. Recursive edges retain
//! their exact instantiated identities; runtime rows are never an input.

use std::collections::BTreeMap;

mod defaults;
pub(super) mod limits;
mod project;
mod storage;
mod variants;
mod walk;
use crate::{env::nominal::AcceptedNominalId, types::TypeInstantiationError};
use arcweft_core::entry::RuntimeNominalSchemaGraphError;
pub(crate) use limits::RuntimeNominalGraphProjectionWork;
pub use limits::{RuntimeNominalGraphProjectionLimitKind, RuntimeNominalGraphProjectionLimits};
use thiserror::Error;

use arcweft_core::{
    entry::{
        RuntimeBytesFormat, RuntimeMapKind, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
        RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
        RuntimeSchemaLimits, RuntimeSchemaValueField, RuntimeTypeSchema as Schema,
    },
    pattern::RuntimeOpaqueTypeOwner,
    value::{RuntimeOpaqueValueClass, RuntimeRecordFieldId},
};
use arcweft_lang_hir::{identity::TypeId, symbol::ProjectSymbolTable};

use self::RuntimeNominalGraphProjectionError as Error;
use crate::{
    env::{
        EnumVariantPayload,
        nominal::{
            AcceptedEnvironmentRecordSemantics, AcceptedNominalRecord, AcceptedNominalSemantics,
        },
        rust_metadata::{
            AcceptedRustStructShape, AcceptedRustTypeMetadataKind, InstantiatedRustTypeMetadata,
            RustMetadataInstantiationError,
        },
    },
    registration::RegisteredTypeCheckEnv,
    types::{
        AcceptedNominalType, ArrayLength, GenericScope, SemanticTypeDigest, TypeKind,
        TypeProjectionError, VariantPayloadTypeShape,
    },
};
use limits::ProjectionBudget;
use storage::ProjectionStorage;
use walk::{Assembly, Body, Definition, Task, Walk};

/// Absence is Unseen. Back-edges to Visiting retain only the exact identity;
/// a definition is appended once, after all its children have completed.
enum VisitState {
    Visiting(RuntimeNominalSchemaIdentity),
    Complete(RuntimeNominalSchemaIdentity),
}

pub(super) struct NominalGraphProjection<'a> {
    environment: Option<&'a RegisteredTypeCheckEnv>,
    symbols: &'a ProjectSymbolTable,
    types: &'a BTreeMap<TypeId, TypeKind>,
    semantic_shapes: Option<&'a super::super::AcceptedSemanticShapeCatalog>,
    project_nominals: &'a super::super::nominal_semantic::ProjectNominalSemanticCatalog,
    scope: GenericScope,
    budget: ProjectionBudget,
    states: BTreeMap<SemanticTypeDigest, VisitState>,
    definitions: Vec<RuntimeNominalSchemaDefinition>,
    active_depth: u64,
    root_rust_metadata: Option<InstantiatedRustTypeMetadata>,
    default_programs: BTreeMap<
        arcweft_id::runtime_program::RuntimePureProgramId,
        crate::callable::RegisteredRustFieldDefaultProgram,
    >,
    project_budget: Option<(
        super::ProjectionBudget,
        crate::final_analysis::FinalSemanticAnalysisControl<'a>,
    )>,
}

impl<'a> NominalGraphProjection<'a> {
    pub(super) fn new(
        environment: Option<&'a RegisteredTypeCheckEnv>,
        symbols: &'a ProjectSymbolTable,
        types: &'a BTreeMap<TypeId, TypeKind>,
        semantic_shapes: Option<&'a super::super::AcceptedSemanticShapeCatalog>,
        project_nominals: &'a super::super::nominal_semantic::ProjectNominalSemanticCatalog,
        budget: ProjectionBudget,
    ) -> Self {
        Self {
            environment,
            symbols,
            types,
            semantic_shapes,
            project_nominals,
            scope: GenericScope::default(),
            budget,
            states: BTreeMap::new(),
            definitions: Vec::new(),
            active_depth: 0,
            root_rust_metadata: None,
            default_programs: BTreeMap::new(),
            project_budget: None,
        }
    }

    pub(super) fn project(
        mut self,
        nominal: &AcceptedNominalType,
    ) -> Result<
        (
            RuntimeNominalSchemaIdentity,
            RuntimeNominalSchemaGraph,
            InstantiatedRustTypeMetadata,
            Vec<crate::callable::RegisteredRustFieldDefaultProgram>,
            RuntimeNominalGraphProjectionWork,
        ),
        Error,
    > {
        self.environment
            .ok_or(Error::StaleGeneration)?
            .nominal_world()
            .accepted_record(nominal.declaration())?;
        let record = self.record(nominal)?;
        if !matches!(record.semantics(), AcceptedNominalSemantics::RustAdt) {
            return Err(Error::NotRustAdt {
                declaration: Box::new(nominal.declaration().clone()),
            });
        }
        // Bound the borrowed argument trees before constructing the root term.
        for argument in nominal.arguments() {
            self.semantic_identity(argument)?;
        }
        let ty = TypeKind::AcceptedNominal(nominal.clone());
        let root = match self.schema(&ty)? {
            Schema::NominalRef(root) => root,
            _ => {
                return Err(Error::UnsupportedType {
                    semantic_type: self.semantic_identity(&ty)?,
                });
            }
        };
        let graph = RuntimeNominalSchemaGraph::try_new(
            self.definitions,
            RuntimeSchemaLimits::engine_default(),
        )?;
        let metadata = self
            .root_rust_metadata
            .ok_or_else(|| Error::MetadataMismatch {
                declaration: Box::new(nominal.declaration().clone()),
            })?;
        Ok((
            root,
            graph,
            metadata,
            self.default_programs.into_values().collect(),
            self.budget.work(),
        ))
    }

    pub(super) fn project_checked(
        mut self,
        checked: &crate::final_analysis::CheckedProjectNominal,
        budget: super::ProjectionBudget,
        control: crate::final_analysis::FinalSemanticAnalysisControl<'a>,
    ) -> Result<RuntimeNominalSchemaGraph, Error> {
        self.project_budget = Some((budget, control));
        self.scope = checked.scope().clone();
        self.schema(&checked.ty())?;
        Ok(RuntimeNominalSchemaGraph::try_new(
            self.definitions,
            RuntimeSchemaLimits::engine_default(),
        )?)
    }

    pub(super) fn project_type(
        mut self,
        ty: &TypeKind,
    ) -> Result<RuntimeNominalSchemaGraph, Error> {
        if !matches!(self.schema(ty)?, Schema::NominalRef(_)) {
            return Err(Error::UnsupportedType {
                semantic_type: self.semantic_identity(ty)?,
            });
        }
        Ok(RuntimeNominalSchemaGraph::try_new(
            self.definitions,
            RuntimeSchemaLimits::engine_default(),
        )?)
    }

    fn record(&self, nominal: &AcceptedNominalType) -> Result<&'a AcceptedNominalRecord, Error> {
        let record = self
            .environment
            .ok_or(Error::StaleGeneration)?
            .nominal_catalog()
            .exact(nominal.declaration().canonical_path())
            .filter(|record| record.id() == nominal.declaration())
            .ok_or_else(|| Error::MissingNominal {
                declaration: Box::new(nominal.declaration().clone()),
            })?;
        if usize::from(record.arity()) != nominal.arguments().len() {
            return Err(Error::Arity {
                declaration: Box::new(nominal.declaration().clone()),
                expected: usize::from(record.arity()),
                actual: nominal.arguments().len(),
            });
        }
        Ok(record)
    }

    fn semantic_identity(&mut self, ty: &TypeKind) -> Result<SemanticTypeDigest, Error> {
        ty.semantic_identity_digest_in_scope_with_control(&self.scope, &mut self.budget)
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => Error::Instantiation(error),
                TypeProjectionError::Control(error) => error,
            })
    }

    fn nominal<'walk>(
        &mut self,
        walk: &mut Walk<'walk>,
        ty: &TypeKind,
        nominal: &'walk AcceptedNominalType,
        depth: u64,
    ) -> Result<(), Error> {
        self.budget.edge()?;
        let semantic = self.semantic_identity(ty)?;
        let record = self.record(nominal)?;
        if let Some(carrier) = record.runtime_carrier() {
            if carrier.value_class() != RuntimeOpaqueValueClass::Plain {
                if self.project_budget.is_some() {
                    walk.emit(Schema::ExecutableRef(semantic.into()));
                    return Ok(());
                }
                return Err(Error::NonRetainableOpaque {
                    declaration: Box::new(record.id().clone()),
                });
            }
            let owner = RuntimeOpaqueTypeOwner::exact_with(
                carrier.producer().clone(),
                semantic.into(),
                carrier.value_class(),
                carrier.persistence(),
            );
            walk.push(Task::BuildSchema(Assembly::ExactOpaque {
                owner,
                arguments: nominal.arguments().len(),
            }));
            walk.push(Task::Types(nominal.arguments(), depth + 1));
            return Ok(());
        }
        if !matches!(record.semantics(), AcceptedNominalSemantics::RustAdt) {
            return Err(Error::NotRustAdt {
                declaration: Box::new(record.id().clone()),
            });
        }
        if let Some(VisitState::Visiting(identity) | VisitState::Complete(identity)) =
            self.states.get(&semantic)
        {
            walk.emit(Schema::NominalRef(identity.clone()));
            return Ok(());
        }
        self.budget.definition(self.active_depth + 1)?;
        let environment = self.environment.ok_or(Error::StaleGeneration)?;
        let metadata = environment
            .rust_metadata()
            .get(record.id())
            .ok_or_else(|| Error::MetadataMismatch {
                declaration: Box::new(record.id().clone()),
            })?;
        let visibility = environment.nominal_world().visibility();
        let publication = match (
            visibility.visible(record.id()),
            visibility.inaccessible(record.id()),
        ) {
            (Some(publication), None) | (None, Some(publication)) => publication,
            _ => {
                return Err(Error::MetadataMismatch {
                    declaration: Box::new(record.id().clone()),
                });
            }
        };
        if metadata.item() != publication.item()
            || Some(metadata.source()) != record.source()
            || metadata.source() != publication.declaration()
        {
            return Err(Error::MetadataMismatch {
                declaration: Box::new(record.id().clone()),
            });
        }
        self.preflight_shape(metadata.kind())?;
        let instantiated = environment
            .rust_metadata()
            .instantiate_with_control(nominal, &mut self.budget)
            .map_err(|error| match error {
                RustMetadataInstantiationError::Control(error) => error,
                RustMetadataInstantiationError::Type(error) => Error::Instantiation(error),
                RustMetadataInstantiationError::UnknownNominal { id }
                | RustMetadataInstantiationError::WrongArity { id, .. } => {
                    Error::MetadataMismatch {
                        declaration: Box::new(id),
                    }
                }
            })?;
        let (slot, instantiated) = walk.storage.rust_metadata.insert(instantiated);
        let identity = RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes()),
            semantic.into(),
        );
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        let definition = Definition {
            semantic,
            declaration: arcweft_core::entry::RuntimeNominalDeclarationId::from_bytes(
                *nominal.declaration().semantic_digest().as_bytes(),
            ),
            identity,
            arguments: nominal.arguments().len(),
            rust: Some((slot, instantiated)),
        };
        walk.push(Task::RustBody(definition, depth + 1));
        walk.push(Task::Types(nominal.arguments(), depth + 1));
        Ok(())
    }

    fn preflight_shape(&mut self, kind: &AcceptedRustTypeMetadataKind) -> Result<(), Error> {
        match kind {
            AcceptedRustTypeMetadataKind::Struct { shape } => match shape {
                AcceptedRustStructShape::Unit => Ok(()),
                AcceptedRustStructShape::Tuple(fields) => self.budget.members(fields.len()),
                AcceptedRustStructShape::Record(fields) => {
                    self.budget.members(fields.len())?;
                    fields
                        .iter()
                        .try_for_each(|field| self.budget.name(field.name()))
                }
            },
            AcceptedRustTypeMetadataKind::Newtype { .. } => self.budget.members(1),
            AcceptedRustTypeMetadataKind::Enum { variants } => {
                self.budget.members(variants.len())?;
                variants.iter().try_for_each(|variant| {
                    self.budget.name(variant.name())?;
                    match variant.payload() {
                        EnumVariantPayload::Unit => Ok(()),
                        EnumVariantPayload::Tuple(fields) => self.budget.members(fields.len()),
                        EnumVariantPayload::Record(fields) => {
                            self.budget.members(fields.len())?;
                            fields
                                .iter()
                                .try_for_each(|field| self.budget.name(field.name()))
                        }
                    }
                })
            }
        }
    }

    fn schedule_rust_body<'walk>(
        walk: &mut Walk<'walk>,
        definition: Definition<'walk>,
        depth: u64,
    ) {
        use RuntimeNominalRecordShape as Shape;
        let (_, metadata) = definition
            .rust
            .expect("Rust body has instantiated metadata");
        match metadata.kind() {
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Unit,
            } => {
                walk.push(Task::FinishDefinition(
                    definition,
                    Body::Record(Shape::Unit, 0),
                ));
            }
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Tuple(fields),
            } => {
                walk.push(Task::FinishDefinition(
                    definition,
                    Body::Record(Shape::Tuple, fields.len()),
                ));
                for (ordinal, field) in fields.iter().enumerate().rev() {
                    walk.push_field(ordinal, None, field, depth);
                }
            }
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Record(fields),
            } => {
                walk.push(Task::FinishDefinition(
                    definition,
                    Body::Record(Shape::Record, fields.len()),
                ));
                for (ordinal, field) in fields.iter().enumerate().rev() {
                    walk.push_field(ordinal, Some(field.name()), field.ty(), depth);
                }
            }
            AcceptedRustTypeMetadataKind::Newtype { inner } => {
                walk.push(Task::FinishDefinition(
                    definition,
                    Body::Record(Shape::Newtype, 1),
                ));
                walk.push_field(0, None, inner, depth);
            }
            AcceptedRustTypeMetadataKind::Enum { variants } => {
                walk.push(Task::FinishDefinition(
                    definition,
                    Body::Variant(variants.len()),
                ));
                for (ordinal, variant) in variants.iter().enumerate().rev() {
                    walk.push(Task::Case {
                        ordinal: u32::try_from(ordinal).expect("cases were bounded"),
                        name: variant.name(),
                        has_payload: !matches!(variant.payload(), EnumVariantPayload::Unit),
                    });
                    match variant.payload() {
                        EnumVariantPayload::Unit => {}
                        EnumVariantPayload::Tuple(fields) => {
                            walk.push(Task::BuildSchema(Assembly::Tuple(fields.len())));
                            walk.push(Task::Types(fields, depth + 1));
                            walk.push(Task::CountPayload(depth));
                        }
                        EnumVariantPayload::Record(fields) => {
                            walk.push(Task::BuildSchema(Assembly::RecordValue(fields.len())));
                            for (ordinal, field) in fields.iter().enumerate().rev() {
                                walk.push_value_field(ordinal, field.name(), field.ty(), depth + 1);
                            }
                            walk.push(Task::CountPayload(depth));
                        }
                    }
                }
            }
        }
    }

    fn schema(&mut self, ty: &TypeKind) -> Result<Schema, Error> {
        let mut storage = ProjectionStorage::new();
        let (root, root_rust_slot) = Walk::run(&storage, self, ty)?;
        if let Some(slot) = root_rust_slot {
            self.root_rust_metadata = Some(storage.rust_metadata.take(slot));
        }
        Ok(root)
    }

    fn visit_type<'walk>(
        &mut self,
        walk: &mut Walk<'walk>,
        ty: &'walk TypeKind,
        depth: u64,
    ) -> Result<(), Error>
    where
        'a: 'walk,
    {
        if let TypeKind::ProjectNominal(nominal) = ty {
            return self.project_nominal(walk, ty, nominal, depth);
        }
        if matches!(
            ty,
            TypeKind::DataError
                | TypeKind::DataPath
                | TypeKind::DisplayContext
                | TypeKind::DisplayError
        ) {
            let semantic = self.semantic_identity(ty)?;
            if let Some(record) = self
                .semantic_shapes
                .and_then(|shapes| shapes.environment_record(semantic))
            {
                return self.accepted_environment_record(walk, ty, record, depth);
            }
        }
        if matches!(
            ty,
            TypeKind::DataFormat
                | TypeKind::DataValue
                | TypeKind::DataErrorKind
                | TypeKind::DataPathSegment
                | TypeKind::DataMapKind
                | TypeKind::CharacterNominal(_)
                | TypeKind::Named(_)
                | TypeKind::AcceptedNominal(_)
                | TypeKind::CompileTimeScalar(_)
        ) {
            let semantic = self.semantic_identity(ty)?;
            if let Some(owner) = self
                .semantic_shapes
                .and_then(|shapes| shapes.closed_variant(semantic))
            {
                return self.closed_variant(walk, owner, depth);
            }
        }
        let schema = match ty {
            TypeKind::Unit => Schema::Unit,
            TypeKind::Never => Schema::Never,
            TypeKind::Bool => Schema::Bool,
            TypeKind::I8 => Schema::I8,
            TypeKind::I16 => Schema::I16,
            TypeKind::I32 => Schema::I32,
            TypeKind::I64 => Schema::I64,
            TypeKind::I128 => Schema::I128,
            TypeKind::ISize => Schema::ISize,
            TypeKind::U8 => Schema::U8,
            TypeKind::U16 => Schema::U16,
            TypeKind::U32 => Schema::U32,
            TypeKind::U64 => Schema::U64,
            TypeKind::U128 => Schema::U128,
            TypeKind::USize => Schema::USize,
            TypeKind::F32 => Schema::F32,
            TypeKind::F64 => Schema::F64,
            TypeKind::String => Schema::String,
            TypeKind::Char => Schema::Char,
            TypeKind::Bytes => Schema::Bytes {
                format: RuntimeBytesFormat::Binary,
            },
            TypeKind::Duration => Schema::Duration,
            TypeKind::Progress => Schema::Progress,
            TypeKind::DenseNumeric(dense) => Schema::DenseNumeric(*dense),
            TypeKind::Ref(entity) => Schema::EntityReference(
                entity
                    .runtime_reference_type()
                    .ok_or_else(|| {
                        Error::from(super::NominalSchemaProjectionError::UnsupportedLeaf {
                            path: super::NominalSchemaPath::default(),
                            ty: Box::new(ty.clone()),
                        })
                    })?
                    .try_map(|payload| self.semantic_identity(payload).map(Into::into))?,
            ),
            TypeKind::AgentValue => Schema::AgentValue,
            TypeKind::Vec(item) | TypeKind::Seq(item) | TypeKind::Slice(item) => {
                walk.push(Task::BuildSchema(Assembly::Seq));
                walk.push(Task::Type(item, depth + 1));
                return Ok(());
            }
            TypeKind::Map { kind, key, value } => {
                let kind = match kind {
                    crate::types::MapKind::Ordered => RuntimeMapKind::Ordered,
                    crate::types::MapKind::Sorted => RuntimeMapKind::Sorted,
                    crate::types::MapKind::BTree => RuntimeMapKind::BTree,
                };
                walk.push(Task::BuildSchema(Assembly::Map(kind)));
                walk.push(Task::Type(value, depth + 1));
                walk.push(Task::Type(key, depth + 1));
                return Ok(());
            }
            TypeKind::Array {
                item,
                len: ArrayLength::Const(length),
            } => {
                walk.push(Task::BuildSchema(Assembly::Array(*length)));
                walk.push(Task::Type(item, depth + 1));
                return Ok(());
            }
            TypeKind::Option(item) => {
                walk.push(Task::BuildSchema(Assembly::Option));
                walk.push(Task::Type(item, depth + 1));
                return Ok(());
            }
            TypeKind::Result { ok, error } => {
                walk.push(Task::BuildSchema(Assembly::Result));
                walk.push(Task::Type(error, depth + 1));
                walk.push(Task::Type(ok, depth + 1));
                return Ok(());
            }
            TypeKind::Tuple(items) => {
                walk.push(Task::BuildSchema(Assembly::Tuple(items.len())));
                walk.push(Task::Types(items, depth + 1));
                return Ok(());
            }
            TypeKind::Choice(items) => {
                walk.push(Task::BuildSchema(Assembly::Choice(items.len())));
                walk.push(Task::Types(items, depth + 1));
                return Ok(());
            }
            TypeKind::Function { .. } => Schema::ExecutableRef(self.semantic_identity(ty)?.into()),
            TypeKind::Need(_) if self.project_budget.is_some() => {
                Schema::ExecutableRef(self.semantic_identity(ty)?.into())
            }
            TypeKind::Handle { .. } | TypeKind::SlotRef(_) => {
                let semantic = self.semantic_identity(ty)?;
                let owner = ty
                    .presentation_opaque_owner(semantic.into())
                    .ok_or_else(|| {
                        Error::from(super::NominalSchemaProjectionError::UnsupportedLeaf {
                            path: super::NominalSchemaPath::default(),
                            ty: Box::new(ty.clone()),
                        })
                    })?;
                if owner.value_class() == RuntimeOpaqueValueClass::Plain {
                    Schema::ExactOpaque {
                        owner,
                        arguments: Box::new([]),
                    }
                } else if self.project_budget.is_some() {
                    Schema::ExecutableRef(semantic.into())
                } else {
                    return Err(Error::UnsupportedType {
                        semantic_type: semantic,
                    });
                }
            }
            TypeKind::AcceptedNominal(nominal) => return self.nominal(walk, ty, nominal, depth),
            TypeKind::GenericParam(parameter) => {
                return Err(crate::types::TypeInstantiationError::UnboundType {
                    parameter: parameter.clone(),
                }
                .into());
            }
            TypeKind::VariantPayload(payload) => {
                match payload.shape() {
                    VariantPayloadTypeShape::Tuple(items) => {
                        walk.push(Task::BuildSchema(Assembly::Tuple(items.len())));
                        walk.push(Task::Types(items, depth + 1));
                    }
                    VariantPayloadTypeShape::Record(fields) => {
                        self.budget.members(fields.len())?;
                        walk.push(Task::BuildSchema(Assembly::RecordValue(fields.len())));
                        for (ordinal, field) in fields.iter().enumerate().rev() {
                            walk.push_value_field(
                                ordinal,
                                field.diagnostic_name(),
                                field.ty(),
                                depth + 1,
                            );
                        }
                    }
                }
                return Ok(());
            }
            _ => {
                if self.project_budget.is_some() {
                    return Err(super::NominalSchemaProjectionError::UnsupportedLeaf {
                        path: super::NominalSchemaPath::default(),
                        ty: Box::new(ty.clone()),
                    }
                    .into());
                }
                return Err(Error::UnsupportedType {
                    semantic_type: self.semantic_identity(ty)?,
                });
            }
        };
        walk.emit(schema);
        Ok(())
    }

    fn accepted_environment_record<'walk>(
        &mut self,
        walk: &mut Walk<'walk>,
        ty: &TypeKind,
        record: &'walk AcceptedEnvironmentRecordSemantics,
        depth: u64,
    ) -> Result<(), Error> {
        self.budget.edge()?;
        let semantic = self.semantic_identity(ty)?;
        if record.ty() != ty
            || record.semantic_type() != semantic
            || record.runtime_carrier().is_some()
        {
            return Err(Error::InvalidEnvironmentRecord {
                semantic_type: semantic,
            });
        }
        let nominal_record = self
            .environment
            .and_then(|environment| {
                environment
                    .nominal_catalog()
                    .environment_record_for_semantic_type(semantic)
            })
            .ok_or(Error::StaleGeneration)?;
        if let Some(VisitState::Visiting(identity) | VisitState::Complete(identity)) =
            self.states.get(&semantic)
        {
            walk.emit(Schema::NominalRef(identity.clone()));
            return Ok(());
        }
        self.budget.definition(self.active_depth + 1)?;
        self.budget.members(record.fields().len())?;
        let identity = RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::try_new(nominal_record.id().source_label())?,
            semantic.into(),
        );
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        walk.push(Task::FinishDefinition(
            Definition {
                semantic,
                identity,
                declaration: arcweft_core::entry::RuntimeNominalDeclarationId::from_bytes(
                    *semantic.as_bytes(),
                ),
                arguments: 0,
                rust: None,
            },
            Body::Record(RuntimeNominalRecordShape::Record, record.fields().len()),
        ));
        for (ordinal, field) in record.fields().iter().enumerate().rev() {
            let name = field.diagnostic_name();
            walk.push_field(ordinal, Some(name), field.ty(), depth + 1);
            walk.push(Task::Name(name));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RuntimeNominalGraphProjectionError {
    #[error("Rust field default {declaration:?}.{field} is invalid: {source}")]
    FieldDefault {
        declaration: Box<AcceptedNominalId>,
        field: String,
        source: crate::callable::RustFieldDefaultBindingError,
    },
    #[error(transparent)]
    CodecUse(#[from] arcweft_core::entry::RuntimeCodecUseError),
    #[error("closed variant {semantic_type:?} has no valid payload row for case {ordinal}")]
    InvalidVariantPayload {
        semantic_type: SemanticTypeDigest,
        ordinal: u32,
    },
    #[error("accepted environment record {semantic_type:?} has an invalid schema owner")]
    InvalidEnvironmentRecord { semantic_type: SemanticTypeDigest },
    #[error(transparent)]
    NominalIdentity(#[from] arcweft_core::entry::RuntimeIdentityError),
    #[error("nominal graph projection exceeded {kind:?}: observed {observed}, maximum {maximum}")]
    Limit {
        kind: RuntimeNominalGraphProjectionLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("nominal graph projection has no matching semantic registration or generation")]
    StaleGeneration,
    #[error(transparent)]
    Lookup(#[from] crate::registration::AcceptedNominalWorldLookupError),
    #[error("nominal graph projection has no exact accepted nominal {declaration:?}")]
    MissingNominal { declaration: Box<AcceptedNominalId> },
    #[error("accepted nominal {declaration:?} is not a structural Rust ADT")]
    NotRustAdt { declaration: Box<AcceptedNominalId> },
    #[error("accepted nominal {declaration:?} expected {expected} arguments, received {actual}")]
    Arity {
        declaration: Box<AcceptedNominalId>,
        expected: usize,
        actual: usize,
    },
    #[error("Rust metadata does not match the accepted publication {declaration:?}")]
    MetadataMismatch { declaration: Box<AcceptedNominalId> },
    #[error(transparent)]
    Instantiation(#[from] TypeInstantiationError),
    #[error("semantic type {semantic_type:?} has no retained runtime schema")]
    UnsupportedType { semantic_type: SemanticTypeDigest },
    #[error("accepted opaque {declaration:?} is not a retainable value")]
    NonRetainableOpaque { declaration: Box<AcceptedNominalId> },
    #[error(transparent)]
    Graph(#[from] RuntimeNominalSchemaGraphError),
    #[error(transparent)]
    Project(#[from] super::NominalSchemaProjectionError),
}

pub(super) fn project_checked_nominal<'a>(
    environment: Option<&'a crate::registration::RegisteredTypeCheckEnv>,
    symbols: &'a arcweft_lang_hir::symbol::ProjectSymbolTable,
    types: &'a std::collections::BTreeMap<
        arcweft_lang_hir::identity::TypeId,
        crate::types::TypeKind,
    >,
    semantic_shapes: Option<&'a super::super::AcceptedSemanticShapeCatalog>,
    project_nominals: &'a super::super::nominal_semantic::ProjectNominalSemanticCatalog,
    checked: &crate::final_analysis::CheckedProjectNominal,
    budget: super::ProjectionBudget,
    control: crate::final_analysis::FinalSemanticAnalysisControl<'a>,
) -> Result<RuntimeNominalSchemaGraph, super::NominalSchemaProjectionError> {
    let budget_graph =
        limits::ProjectionBudget::new(RuntimeNominalGraphProjectionLimits::PRODUCTION)
            .expect("production projection limits are valid");
    NominalGraphProjection::new(
        environment,
        symbols,
        types,
        semantic_shapes,
        project_nominals,
        budget_graph,
    )
    .project_checked(checked, budget, control)
    .map_err(|error| match error {
        RuntimeNominalGraphProjectionError::Project(error) => error,
        other => super::NominalSchemaProjectionError::SourceGraph(Box::new(other)),
    })
}
