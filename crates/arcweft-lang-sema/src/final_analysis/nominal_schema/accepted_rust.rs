//! Generation-bound projection of the joined Rust ADT catalog.

use super::graph;
use super::graph::limits;

use super::{RuntimeNominalGraphProjectionError, RuntimeNominalGraphProjectionLimits};

use std::sync::Arc;

use arcweft_core::{
    entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaGraph,
        RuntimeNominalTypeId, TypeLayoutHash,
    },
    pattern::RuntimeSemanticTypeId,
};

use crate::{
    env::rust_metadata::InstantiatedRustTypeMetadata,
    final_analysis::FinalSemanticAnalysis,
    registration::{
        AcceptedNominalWorldStamp, AcceptedRustProjectionStamp, RegisteredSemanticWorld,
    },
    types::AcceptedNominalType,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAcceptedRustNominalKind {
    Record(RuntimeNominalRecordShape),
    Variant,
}

/// A complete reachable graph issued by one accepted analysis. The retained
/// owners are leases on existing authorities, never copied catalogs.
#[derive(Clone, Debug)]
pub struct RuntimeAcceptedRustNominalProjection {
    nominal_type: AcceptedNominalType,
    metadata: InstantiatedRustTypeMetadata,
    stamp: AcceptedRustProjectionStamp,
    nominal_world: AcceptedNominalWorldStamp,
    root: RuntimeSemanticTypeId,
    nominal: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    kind: RuntimeAcceptedRustNominalKind,
    graph: Arc<RuntimeNominalSchemaGraph>,
    world: RegisteredSemanticWorld,
    generation: Arc<arcweft_lang_hir::project::AcceptedHirProjectGeneration>,
    default_programs: Box<[crate::callable::CheckedRustFieldDefaultProgram]>,
}

impl PartialEq for RuntimeAcceptedRustNominalProjection {
    fn eq(&self, other: &Self) -> bool {
        self.nominal_type == other.nominal_type
            && self.metadata == other.metadata
            && self.stamp == other.stamp
            && self.nominal_world == other.nominal_world
            && self.root == other.root
            && self.nominal == other.nominal
            && self.layout == other.layout
            && self.kind == other.kind
            && self.graph == other.graph
            && std::ptr::eq(self.world.environment(), other.world.environment())
            && std::ptr::eq(self.world.symbols(), other.world.symbols())
            && Arc::ptr_eq(&self.generation, &other.generation)
            && self.default_programs == other.default_programs
    }
}

impl Eq for RuntimeAcceptedRustNominalProjection {}

impl RuntimeAcceptedRustNominalProjection {
    /// Exact original source request, including ordered generic arguments.
    pub const fn nominal_type(&self) -> &AcceptedNominalType {
        &self.nominal_type
    }

    /// Source field and case types substituted in the same bounded transaction
    /// that issued the graph. Executable projections must consume these types,
    /// rather than reconstruct semantic identities from erased schema rows.
    pub const fn metadata(&self) -> &InstantiatedRustTypeMetadata {
        &self.metadata
    }

    pub fn default_programs(&self) -> &[crate::callable::CheckedRustFieldDefaultProgram] {
        &self.default_programs
    }

    /// Issues an exact anonymous payload from the original accepted Rust case.
    /// Unit cases have no payload; empty tuple/record cases keep their shape.
    pub fn case_payload_type(
        &self,
        ordinal: u32,
    ) -> Result<Option<crate::types::TypeKind>, crate::types::VariantPayloadSealError> {
        use crate::{
            env::{EnumVariantPayload, rust_metadata::AcceptedRustTypeMetadataKind},
            types::{
                AcceptedVariantCaseSemanticId, TypeKind, VariantPayloadOwnerFamily,
                VariantPayloadSealError, VariantPayloadShape, VariantPayloadType,
            },
        };
        let AcceptedRustTypeMetadataKind::Enum { variants } = self.metadata.kind() else {
            return Err(VariantPayloadSealError::MissingCase { ordinal });
        };
        let variant = usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| variants.get(ordinal))
            .ok_or(VariantPayloadSealError::MissingCase { ordinal })?;
        let family = VariantPayloadOwnerFamily::AcceptedRust;
        let owner = TypeKind::AcceptedNominal(self.nominal_type.clone());
        let semantic_type = owner.semantic_identity_digest()?;
        let shape = match variant.payload() {
            EnumVariantPayload::Unit => return Ok(None),
            EnumVariantPayload::Tuple(fields) => VariantPayloadShape::try_tuple(
                family,
                semantic_type,
                ordinal,
                fields.iter().cloned(),
            )?,
            EnumVariantPayload::Record(fields) => VariantPayloadShape::try_record(
                family,
                semantic_type,
                ordinal,
                fields
                    .iter()
                    .map(|field| (field.name().to_owned(), field.ty().clone())),
            )?,
        };
        let case = AcceptedVariantCaseSemanticId::issue(family, semantic_type, ordinal, &shape);
        VariantPayloadType::try_new(family, owner, ordinal, case, shape)
            .map(|payload| Some(TypeKind::VariantPayload(Box::new(payload))))
    }

    pub const fn stamp(&self) -> AcceptedRustProjectionStamp {
        self.stamp
    }
    pub const fn nominal_world(&self) -> &AcceptedNominalWorldStamp {
        &self.nominal_world
    }
    pub const fn root(&self) -> RuntimeSemanticTypeId {
        self.root
    }
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.nominal
    }
    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }
    pub const fn kind(&self) -> RuntimeAcceptedRustNominalKind {
        self.kind
    }
    pub const fn graph(&self) -> &Arc<RuntimeNominalSchemaGraph> {
        &self.graph
    }

    /// Rejects stale or foreign evidence before a consumer stages runtime rows.
    pub fn validate_for(
        &self,
        analysis: &FinalSemanticAnalysis,
        world: &RegisteredSemanticWorld,
    ) -> Result<(), RuntimeNominalGraphProjectionError> {
        if !analysis.matches_registered_world(world)
            || !std::ptr::eq(self.world.environment(), world.environment())
            || !std::ptr::eq(self.world.symbols(), world.symbols())
            || !Arc::ptr_eq(&self.generation, analysis.hir_generation())
            || self.stamp != world.environment().accepted_rust_projection_stamp()
            || self.nominal_world != world.environment().nominal_world().stamp()
        {
            return Err(RuntimeNominalGraphProjectionError::StaleGeneration);
        }
        Ok(())
    }

    /// Checks the HIR portion of this lease when normalized facts are handed
    /// to a plan builder that no longer owns the semantic analysis context.
    pub fn validate_project(
        &self,
        project: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    ) -> Result<(), arcweft_lang_hir::project::AcceptedHirProjectLeaseError> {
        self.generation.validate_analysis_lease(project)
    }
}

impl FinalSemanticAnalysis {
    /// Projects every reachable exact Rust instance as one transactional core
    /// schema graph. Runtime ownership is granted only by the later value gate.
    pub fn project_accepted_rust_nominal(
        &self,
        world: &RegisteredSemanticWorld,
        nominal: &AcceptedNominalType,
        limits: RuntimeNominalGraphProjectionLimits,
    ) -> Result<RuntimeAcceptedRustNominalProjection, RuntimeNominalGraphProjectionError> {
        let mut budget = limits::ProjectionBudget::new(limits)?;
        budget.type_node(1)?;
        if !self.matches_registered_world(world) {
            return Err(RuntimeNominalGraphProjectionError::StaleGeneration);
        }
        let (root, graph, metadata, defaults) = graph::NominalGraphProjection::new(
            Some(world.environment()),
            world.symbols(),
            self.accepted_types(),
            Some(self.semantic_shapes()),
            budget,
        )
        .project(nominal)?;
        let default_programs = defaults
            .into_iter()
            .map(|program| {
                program.bind(self.checked_callables()).map_err(|source| {
                    RuntimeNominalGraphProjectionError::FieldDefault {
                        declaration: Box::new(nominal.declaration().clone()),
                        field: "<checked callable>".to_owned(),
                        source,
                    }
                })
            })
            .collect::<Result<_, _>>()?;
        let definition = graph.definition(root.semantic_identity()).ok_or(
            RuntimeNominalGraphProjectionError::Graph(
                arcweft_core::entry::RuntimeNominalSchemaGraphError::UnknownRoot {
                    root: root.semantic_identity(),
                },
            ),
        )?;
        let kind = match definition.body() {
            RuntimeNominalSchemaBody::Record { shape, .. } => {
                RuntimeAcceptedRustNominalKind::Record(*shape)
            }
            RuntimeNominalSchemaBody::Variant { .. } => RuntimeAcceptedRustNominalKind::Variant,
        };
        let layout = graph.try_layout_hash(root.semantic_identity())?;
        Ok(RuntimeAcceptedRustNominalProjection {
            nominal_type: nominal.clone(),
            metadata,
            stamp: world.environment().accepted_rust_projection_stamp(),
            nominal_world: world.environment().nominal_world().stamp(),
            root: root.semantic_identity(),
            nominal: root.nominal().clone(),
            layout,
            kind,
            graph: Arc::new(graph),
            world: world.clone(),
            generation: Arc::clone(self.hir_generation()),
            default_programs,
        })
    }
}
