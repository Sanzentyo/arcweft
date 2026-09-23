//! Source projection shared by Project declarations and joined Rust ADTs.
//!
//! Definitions come from the accepted semantic world. Recursive edges retain
//! their exact instantiated identities; runtime rows are never an input.

use std::collections::BTreeMap;

mod defaults;
pub(super) mod limits;
mod project;
mod variants;
use crate::{env::nominal::AcceptedNominalId, types::TypeInstantiationError};
use arcweft_core::entry::RuntimeNominalSchemaGraphError;
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
        budget: ProjectionBudget,
    ) -> Self {
        Self {
            environment,
            symbols,
            types,
            semantic_shapes,
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
        let root = match self.schema(&ty, 1)? {
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
        ))
    }

    pub(super) fn project_checked(
        mut self,
        checked: &crate::final_analysis::CheckedProjectNominal,
        budget: super::ProjectionBudget,
        control: crate::final_analysis::FinalSemanticAnalysisControl<'a>,
    ) -> Result<RuntimeNominalSchemaGraph, Error> {
        self.project_budget = Some((budget, control));
        self.schema(&checked.ty(), 1)?;
        Ok(RuntimeNominalSchemaGraph::try_new(
            self.definitions,
            RuntimeSchemaLimits::engine_default(),
        )?)
    }

    pub(super) fn project_type(
        mut self,
        ty: &TypeKind,
    ) -> Result<RuntimeNominalSchemaGraph, Error> {
        if !matches!(self.schema(ty, 1)?, Schema::NominalRef(_)) {
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
        ty.semantic_identity_digest_in_scope_with_control(
            &GenericScope::default(),
            &mut self.budget,
        )
        .map_err(|error| match error {
            TypeProjectionError::Instantiation(error) => Error::Instantiation(error),
            TypeProjectionError::Control(error) => error,
        })
    }

    fn nominal(
        &mut self,
        ty: &TypeKind,
        nominal: &AcceptedNominalType,
        depth: u64,
    ) -> Result<Schema, Error> {
        self.budget.edge()?;
        let semantic = self.semantic_identity(ty)?;
        let record = self.record(nominal)?;
        match record.semantics() {
            AcceptedNominalSemantics::Opaque(carrier) => {
                if carrier.value_class() != RuntimeOpaqueValueClass::Plain {
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
                let arguments = self.sequence(nominal.arguments(), depth + 1)?;
                return Ok(Schema::ExactOpaque { owner, arguments });
            }
            AcceptedNominalSemantics::RustAdt => {}
            _ => {
                return Err(Error::NotRustAdt {
                    declaration: Box::new(record.id().clone()),
                });
            }
        }
        if let Some(VisitState::Visiting(identity) | VisitState::Complete(identity)) =
            self.states.get(&semantic)
        {
            return Ok(Schema::NominalRef(identity.clone()));
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
        let identity = RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes()),
            semantic.into(),
        );
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        let arguments = self.sequence(nominal.arguments(), depth + 1)?;
        let body = self.body(instantiated.kind(), depth + 1)?;
        let codec = self.rust_default_codec(&instantiated, &body)?;
        let mut definition = RuntimeNominalSchemaDefinition::new(identity.clone(), arguments, body);
        if let Some(codec) = codec {
            definition = definition.with_data_codec(codec);
        }
        self.definitions.push(definition);
        self.active_depth -= 1;
        if self.active_depth == 0 {
            self.root_rust_metadata = Some(instantiated);
        }
        self.states
            .insert(semantic, VisitState::Complete(identity.clone()));
        Ok(Schema::NominalRef(identity))
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

    fn body(
        &mut self,
        kind: &AcceptedRustTypeMetadataKind,
        depth: u64,
    ) -> Result<RuntimeNominalSchemaBody, Error> {
        use RuntimeNominalRecordShape as Shape;
        Ok(match kind {
            AcceptedRustTypeMetadataKind::Struct { shape } => {
                let (shape, fields) = match shape {
                    AcceptedRustStructShape::Unit => (Shape::Unit, Box::new([]) as Box<[_]>),
                    AcceptedRustStructShape::Tuple(fields) => (
                        Shape::Tuple,
                        self.fields(fields.iter().map(|ty| (None, ty)), depth)?,
                    ),
                    AcceptedRustStructShape::Record(fields) => (
                        Shape::Record,
                        self.fields(
                            fields.iter().map(|field| (Some(field.name()), field.ty())),
                            depth,
                        )?,
                    ),
                };
                RuntimeNominalSchemaBody::Record { shape, fields }
            }
            AcceptedRustTypeMetadataKind::Newtype { inner } => RuntimeNominalSchemaBody::Record {
                shape: Shape::Newtype,
                fields: self.fields(std::iter::once((None, inner)), depth)?,
            },
            AcceptedRustTypeMetadataKind::Enum { variants } => RuntimeNominalSchemaBody::Variant {
                cases: variants
                    .iter()
                    .enumerate()
                    .map(|(ordinal, variant)| {
                        let payload = match variant.payload() {
                            EnumVariantPayload::Unit => None,
                            EnumVariantPayload::Tuple(fields) => {
                                self.budget.type_node(depth)?;
                                Some(Schema::Tuple(self.sequence(fields, depth + 1)?))
                            }
                            EnumVariantPayload::Record(fields) => {
                                self.budget.type_node(depth)?;
                                Some(Schema::RecordValue {
                                    fields: self.record_fields(
                                        fields.iter().map(|field| (field.name(), field.ty())),
                                        depth + 1,
                                    )?,
                                })
                            }
                        };
                        Ok(RuntimeNominalSchemaCase::new(
                            u32::try_from(ordinal).expect("cases were bounded"),
                            variant.name().to_owned(),
                            payload,
                        ))
                    })
                    .collect::<Result<_, Error>>()?,
            },
        })
    }

    fn fields<'t>(
        &mut self,
        fields: impl Iterator<Item = (Option<&'t str>, &'t TypeKind)>,
        depth: u64,
    ) -> Result<Box<[RuntimeNominalSchemaField]>, Error> {
        fields
            .enumerate()
            .map(|(ordinal, (name, ty))| {
                Ok(RuntimeNominalSchemaField::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .expect("fields were bounded"),
                    name.map(str::to_owned),
                    self.schema(ty, depth)?,
                ))
            })
            .collect()
    }

    fn record_fields<'t>(
        &mut self,
        fields: impl Iterator<Item = (&'t str, &'t TypeKind)>,
        depth: u64,
    ) -> Result<Box<[RuntimeSchemaValueField]>, Error> {
        fields
            .enumerate()
            .map(|(ordinal, (name, ty))| {
                self.budget.name(name)?;
                Ok(RuntimeSchemaValueField::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .expect("fields were bounded"),
                    name.to_owned(),
                    self.schema(ty, depth)?,
                ))
            })
            .collect()
    }

    fn sequence(&mut self, types: &[TypeKind], depth: u64) -> Result<Box<[Schema]>, Error> {
        types.iter().map(|ty| self.schema(ty, depth)).collect()
    }

    fn schema(&mut self, ty: &TypeKind, depth: u64) -> Result<Schema, Error> {
        if let Some((budget, control)) = &mut self.project_budget {
            budget.enter_node(*control)?;
        }
        let result = self.schema_inner(ty, depth);
        if let Some((budget, _)) = &mut self.project_budget {
            budget.leave_node();
        }
        result
    }

    fn schema_inner(&mut self, ty: &TypeKind, depth: u64) -> Result<Schema, Error> {
        self.budget.type_node(depth)?;
        if matches!(ty, TypeKind::DataError | TypeKind::DataPath) {
            let semantic = self.semantic_identity(ty)?;
            if let Some(record) = self
                .semantic_shapes
                .and_then(|shapes| shapes.environment_record(semantic))
            {
                return self.accepted_environment_record(ty, record, depth);
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
        ) {
            let semantic = self.semantic_identity(ty)?;
            if let Some(owner) = self
                .semantic_shapes
                .and_then(|shapes| shapes.closed_variant(semantic))
            {
                return self.closed_variant(owner, depth);
            }
        }
        Ok(match ty {
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
            TypeKind::Ref(_) => Schema::EntityReference,
            TypeKind::AgentValue => Schema::AgentValue,
            TypeKind::Vec(item) | TypeKind::Seq(item) | TypeKind::Slice(item) => {
                Schema::Seq(Box::new(self.schema(item, depth + 1)?))
            }
            TypeKind::Map { kind, key, value } => Schema::Map {
                kind: match kind {
                    crate::types::MapKind::Ordered => RuntimeMapKind::Ordered,
                    crate::types::MapKind::Sorted => RuntimeMapKind::Sorted,
                    crate::types::MapKind::BTree => RuntimeMapKind::BTree,
                },
                key: Box::new(self.schema(key, depth + 1)?),
                value: Box::new(self.schema(value, depth + 1)?),
            },
            TypeKind::Array {
                item,
                len: ArrayLength::Const(length),
            } => Schema::Array {
                item: Box::new(self.schema(item, depth + 1)?),
                length: u64::try_from(*length)
                    .map_err(|_| crate::types::TypeInstantiationError::EncodingLengthOverflow)?,
            },
            TypeKind::Option(item) => Schema::option(self.schema(item, depth + 1)?),
            TypeKind::Result { ok, error } => {
                Schema::result(self.schema(ok, depth + 1)?, self.schema(error, depth + 1)?)
            }
            TypeKind::Tuple(items) => Schema::Tuple(self.sequence(items, depth + 1)?),
            TypeKind::Choice(items) => Schema::Choice(self.sequence(items, depth + 1)?),
            TypeKind::AcceptedNominal(nominal) => self.nominal(ty, nominal, depth)?,
            TypeKind::ProjectNominal(nominal) => self.project_nominal(ty, nominal, depth)?,
            TypeKind::GenericParam(parameter) => {
                return Err(crate::types::TypeInstantiationError::UnboundType {
                    parameter: parameter.clone(),
                }
                .into());
            }
            TypeKind::VariantPayload(payload) => match payload.shape() {
                VariantPayloadTypeShape::Tuple(items) => {
                    Schema::Tuple(self.sequence(items, depth + 1)?)
                }
                VariantPayloadTypeShape::Record(fields) => {
                    self.budget.members(fields.len())?;
                    Schema::RecordValue {
                        fields: self.record_fields(
                            fields
                                .iter()
                                .map(|field| (field.diagnostic_name(), field.ty())),
                            depth + 1,
                        )?,
                    }
                }
            },
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
        })
    }

    fn accepted_environment_record(
        &mut self,
        ty: &TypeKind,
        record: &AcceptedEnvironmentRecordSemantics,
        depth: u64,
    ) -> Result<Schema, Error> {
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
            return Ok(Schema::NominalRef(identity.clone()));
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
        let fields = record
            .fields()
            .iter()
            .enumerate()
            .map(|(ordinal, field)| {
                self.budget.name(field.diagnostic_name())?;
                Ok(RuntimeNominalSchemaField::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .expect("environment record fields were bounded"),
                    Some(field.diagnostic_name().to_owned()),
                    self.schema(field.ty(), depth + 1)?,
                ))
            })
            .collect::<Result<Box<[_]>, Error>>()?;
        self.definitions.push(RuntimeNominalSchemaDefinition::new(
            identity.clone(),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields,
            },
        ));
        self.active_depth -= 1;
        self.states
            .insert(semantic, VisitState::Complete(identity.clone()));
        Ok(Schema::NominalRef(identity))
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
    checked: &crate::final_analysis::CheckedProjectNominal,
    budget: super::ProjectionBudget,
    control: crate::final_analysis::FinalSemanticAnalysisControl<'a>,
) -> Result<RuntimeNominalSchemaGraph, super::NominalSchemaProjectionError> {
    let budget_graph =
        limits::ProjectionBudget::new(RuntimeNominalGraphProjectionLimits::PRODUCTION)
            .expect("production projection limits are valid");
    NominalGraphProjection::new(environment, symbols, types, semantic_shapes, budget_graph)
        .project_checked(checked, budget, control)
        .map_err(|error| match error {
            RuntimeNominalGraphProjectionError::Project(error) => error,
            other => super::NominalSchemaProjectionError::SourceGraph(Box::new(other)),
        })
}
