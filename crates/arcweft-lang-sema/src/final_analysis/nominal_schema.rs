use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use arcweft_core::{
    entry::{
        RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeNominalTypeId,
        RuntimeSchemaField, RuntimeSchemaVariant, RuntimeTypeSchema, TypeLayoutHash,
    },
    pattern::RuntimeSemanticTypeId,
    value::RuntimeRecordFieldId,
};
use arcweft_data::{BytesFormat, EnumRepr, EnumTagStyle, FieldShape, TypeShape, VariantShape};
use arcweft_lang_hir::{
    identity::TypeId,
    symbol::{
        ProjectSymbolTable,
        nominal::{
            ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationId,
            ProjectNominalDeclarationKind,
        },
    },
};
use arcweft_lang_syntax::ast::module_path::ModuleSegment;

use super::report::{FinalSemanticAnalysisDraft, FinalSemanticAnalysisPostEntryDraft};
use crate::{
    env::nominal::AcceptedNominalId,
    final_analysis::{
        CheckedExpression, CheckedExpressionRecordField, CheckedExpressionResolution,
        CheckedFieldSelection, CheckedPattern, CheckedPatternResolution, CheckedProjectNominal,
        CheckedRecordBindingSource, CheckedRecordExpressionSource, CheckedRecordPattern,
        CheckedRecordPatternField, CheckedRecordPatternOwner, CheckedRecordPatternRest,
        CheckedRecordPatternSource, CheckedRecordValueSource, CheckedVariantOwner,
        CheckedVariantResolution, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
        FinalSemanticAnalysisError, FinalSemanticProjectError, PreparedExpressionFact,
        PreparedPatternFact, PreparedRecordPatternFieldIdentity, PreparedRecordPatternOwner,
        PreparedRecordPatternRest, PreparedRecordPatternSource, PreparedRecordValueSource,
    },
    nominal::{
        NominalAggregationLimitKind, NominalAggregationLimits, NominalResolutionLimitKind,
        NominalResolutionLimits,
    },
    record_field::{AcceptedRecordFieldSemanticId, CheckedRecordFieldSemanticId},
    semantic_coordinate::{
        SemanticCoordinateIndex, StablePatternCoordinate, StablePatternCoordinateStep,
    },
    types::{
        GenericParameterOwnerId, GenericTypeParameterId, MapKind, ProjectNominalType,
        SemanticTypeDigest, TypeGenericUseCollector, TypeKind,
    },
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalSchemaPathStep {
    Field {
        ordinal: u32,
        name: ModuleSegment,
    },
    VariantPayload {
        ordinal: u32,
        name: ModuleSegment,
    },
    OptionItem,
    SequenceItem,
    MapKey,
    MapValue,
    ResultOk,
    ResultError,
    TupleItem {
        ordinal: u32,
    },
    GenericArgument {
        ordinal: u32,
    },
    NestedNominal {
        declaration: ProjectNominalDeclarationId,
    },
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalSchemaPath(Box<[NominalSchemaPathStep]>);

impl NominalSchemaPath {
    pub const fn steps(&self) -> &[NominalSchemaPathStep] {
        &self.0
    }

    fn prepended(&self, step: NominalSchemaPathStep) -> Self {
        let mut steps = Vec::with_capacity(self.0.len() + 1);
        steps.push(step);
        steps.extend_from_slice(&self.0);
        Self(steps.into_boxed_slice())
    }
}

/// Generation-bound data-shape projection over already checked nominal types.
///
/// Name selection, imports, aliases, arity, and generic argument validation are
/// owned by the normal semantic nominal resolver. This adapter only projects
/// its accepted `TypeKind` products into the persistence schema required by an
/// entry; it never resolves authored paths itself.
struct NominalSchemaExpander<'a> {
    symbols: &'a ProjectSymbolTable,
    types: &'a BTreeMap<TypeId, TypeKind>,
    control: FinalSemanticAnalysisControl<'a>,
}

/// Mutable construction authority for project-nominal runtime projections.
///
/// This context exists only while one semantic generation is being prepared.
/// C2.4 consumes its accepted cache into the final immutable catalog; it is
/// never retained beside a published [`FinalSemanticAnalysis`].
pub(crate) struct RuntimeNominalProjectionContext<'a> {
    symbols: &'a ProjectSymbolTable,
    types: &'a BTreeMap<TypeId, TypeKind>,
    root_limits: NominalResolutionLimits,
    aggregate_limits: NominalAggregationLimits,
    aggregate_work: u64,
    visiting: BTreeSet<SemanticTypeDigest>,
    accepted: BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
    control: FinalSemanticAnalysisControl<'a>,
}

/// Fresh work budget for one requested nominal projection root.
pub(crate) struct ProjectionBudget {
    limits: NominalResolutionLimits,
    nodes: u64,
    depth: u16,
    work: u64,
}

/// Limit namespace for one projection operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalProjectionLimitKind {
    Root(NominalResolutionLimitKind),
    Project(NominalAggregationLimitKind),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NominalSchemaProjectionError {
    #[error("project-nominal schema projection was cancelled")]
    Cancelled,
    #[error("project-nominal projection exceeded {kind:?}: observed {observed}, maximum {maximum}")]
    LimitExceeded {
        kind: NominalProjectionLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("project-nominal projection accounting overflowed")]
    ArithmeticOverflow,
    #[error("checked nominal semantic identity differs from its canonical type identity")]
    IdentityMismatch {
        requested: SemanticTypeDigest,
        projected: SemanticTypeDigest,
    },
    #[error("checked nominal belongs to a different project-symbol generation")]
    GenerationMismatch,
    #[error("checked project nominal `{nominal}` is absent from its symbol world")]
    MissingDeclaration { nominal: String },
    #[error("checked project nominal `{nominal}` has owner {actual:?}, expected {expected:?}")]
    OwnerMismatch {
        nominal: String,
        expected: arcweft_lang_hir::identity::ItemId,
        actual: arcweft_lang_hir::identity::ItemId,
    },
    #[error("checked project nominal `{nominal}` expected {expected} argument(s), found {actual}")]
    WrongArity {
        nominal: String,
        expected: usize,
        actual: usize,
    },
    #[error("prepared nominal requests disagree for semantic type {semantic_type:?}")]
    ConflictingRequest { semantic_type: SemanticTypeDigest },
    #[error("final semantic inventory dropped nominal request {semantic_type:?}")]
    MissingFinalRequest { semantic_type: SemanticTypeDigest },
    #[error(
        "final semantic inventory introduced nominal request {semantic_type:?} after projection"
    )]
    UnexpectedFinalRequest { semantic_type: SemanticTypeDigest },
    #[error("sealed nominal catalog is missing semantic type {semantic_type:?}")]
    MissingCachedProjection { semantic_type: SemanticTypeDigest },
    #[error("accepted final semantic analysis has no type fact for {ty:?}")]
    MissingTypeFact { ty: TypeId },
    #[error("checked Entry nominal relation is inconsistent for semantic type {semantic_type:?}")]
    InvalidEntryNominalRelation { semantic_type: SemanticTypeDigest },
    #[error(
        "checked project field relation is inconsistent for semantic type {owner:?}, field {ordinal}"
    )]
    InvalidProjectFieldRelation {
        owner: SemanticTypeDigest,
        ordinal: u32,
    },
    #[error("accepted opaque type has no closed project-nominal schema layout")]
    OpaqueLeaf {
        path: NominalSchemaPath,
        nominal: AcceptedNominalId,
        semantic_identity: SemanticTypeDigest,
    },
    #[error("checked type is not a supported closed project-nominal schema leaf")]
    UnsupportedLeaf {
        path: NominalSchemaPath,
        ty: Box<TypeKind>,
    },
    #[error("project-nominal schema contains a cyclic generic substitution")]
    CyclicGenericSubstitution {
        path: NominalSchemaPath,
        parameter: GenericTypeParameterId,
    },
    #[error("project nominal `{nominal}` is not a runtime struct or enum")]
    UnsupportedDeclaration { nominal: String },
    #[error("project nominal `{nominal}` has an invalid runtime identity: {reason}")]
    InvalidRuntimeIdentity { nominal: String, reason: String },
    #[error("project nominal `{nominal}` has an invalid canonical runtime schema: {reason}")]
    InvalidRuntimeSchema { nominal: String, reason: String },
    #[error("{path}: {reason}")]
    InvalidShape { path: String, reason: String },
}

/// Closed declaration family retained with one checked runtime nominal schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProjectNominalKind {
    Record,
    Variant,
}

/// Sole semantic projection of one checked project nominal into its stable
/// runtime identity and canonical layout schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectNominalProjection {
    declaration: ProjectNominalDeclarationId,
    owner: arcweft_lang_hir::identity::ItemId,
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    shape: TypeShape,
    layout: TypeLayoutHash,
    schema: RuntimeTypeSchema,
    kind: RuntimeProjectNominalKind,
    record_fields: Box<[RuntimeProjectRecordFieldProjection]>,
    variant_cases: Box<[RuntimeProjectVariantCaseProjection]>,
}

/// Canonical typed join between one project declaration field and its runtime
/// layout coordinate. This relation is emitted by the sole schema expander;
/// record sealing never rewalks the declaration or reconstructs it by name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectRecordFieldProjection {
    runtime_field: RuntimeRecordFieldId,
    declaration_ordinal: u32,
    ty: TypeKind,
    field_type: SemanticTypeDigest,
}

/// Canonical typed join between one project enum case and its instantiated
/// payload. Runtime consumers never rewalk HIR declarations after C2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectVariantCaseProjection {
    ordinal: u32,
    diagnostic_name: ModuleSegment,
    payload: Option<TypeKind>,
}

impl RuntimeProjectVariantCaseProjection {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn diagnostic_name(&self) -> &ModuleSegment {
        &self.diagnostic_name
    }

    pub const fn payload(&self) -> Option<&TypeKind> {
        self.payload.as_ref()
    }
}

/// Borrowed exact join between one checked project-field selection and the
/// sole sealed runtime nominal catalog.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeProjectFieldProjection<'a> {
    owner: &'a RuntimeProjectNominalProjection,
    field: &'a RuntimeProjectRecordFieldProjection,
}

impl RuntimeProjectFieldProjection<'_> {
    pub const fn owner(&self) -> &RuntimeProjectNominalProjection {
        self.owner
    }

    pub const fn field(&self) -> &RuntimeProjectRecordFieldProjection {
        self.field
    }
}

impl RuntimeProjectRecordFieldProjection {
    pub const fn runtime_field(&self) -> RuntimeRecordFieldId {
        self.runtime_field
    }

    pub const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub const fn field_type(&self) -> SemanticTypeDigest {
        self.field_type
    }
}

/// One generation-bound request collected before deterministic projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNominalProjectionRequest {
    semantic_type: SemanticTypeDigest,
    nominal: CheckedProjectNominal,
}

impl RuntimeNominalProjectionRequest {
    pub(crate) fn new(nominal: CheckedProjectNominal) -> Self {
        Self {
            semantic_type: nominal.identity(),
            nominal,
        }
    }
}

/// Complete projection roots ordered by canonical semantic identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeNominalProjectionRequestInventory {
    by_semantic_type: BTreeMap<SemanticTypeDigest, CheckedProjectNominal>,
}

/// Cache-only C2 state reached by consuming the complete prepared request
/// inventory. This type exposes no projection operation, so seed and Entry
/// sealing cannot discover or project a post-inventory nominal.
pub(crate) struct RuntimeNominalProjectionSeal {
    requested: BTreeMap<SemanticTypeDigest, CheckedProjectNominal>,
    accepted: BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
}

/// Final replay evidence required before the immutable catalog can publish.
pub(crate) struct ValidatedRuntimeNominalProjectionSeal {
    accepted: BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
}

impl RuntimeNominalProjectionRequestInventory {
    pub(super) fn nominals(&self) -> impl ExactSizeIterator<Item = &CheckedProjectNominal> {
        self.by_semantic_type.values()
    }

    pub(crate) fn insert(
        &mut self,
        request: RuntimeNominalProjectionRequest,
    ) -> Result<(), NominalSchemaProjectionError> {
        if request.semantic_type != request.nominal.identity() {
            return Err(NominalSchemaProjectionError::IdentityMismatch {
                requested: request.semantic_type,
                projected: request.nominal.identity(),
            });
        }
        match self.by_semantic_type.entry(request.semantic_type) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(request.nominal);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get() == &request.nominal =>
            {
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                Err(NominalSchemaProjectionError::ConflictingRequest {
                    semantic_type: *entry.key(),
                })
            }
        }
    }

    fn insert_checked(
        &mut self,
        nominal: &CheckedProjectNominal,
    ) -> Result<(), NominalSchemaProjectionError> {
        self.insert(RuntimeNominalProjectionRequest::new(nominal.clone()))
    }

    fn visit_type(
        &mut self,
        symbols: &ProjectSymbolTable,
        ty: &TypeKind,
    ) -> Result<(), NominalSchemaProjectionError> {
        crate::types::visit_project_nominals(ty, &mut |nominal| {
            let nominal_type = ty_for_project_nominal(nominal);
            let generics = TypeGenericUseCollector::collect(&nominal_type)
                .map_err(|error| NominalSchemaProjectionError::new(error.to_string()))?;
            if !generics.types().is_empty() || !generics.consts().is_empty() {
                return Ok(());
            }
            let declaration = symbols.nominal(nominal.declaration()).ok_or_else(|| {
                NominalSchemaProjectionError::MissingDeclaration {
                    nominal: nominal.declaration().qualified_name(),
                }
            })?;
            if declaration.type_parameters().len() != nominal.arguments().len() {
                return Err(NominalSchemaProjectionError::WrongArity {
                    nominal: nominal.declaration().qualified_name(),
                    expected: declaration.type_parameters().len(),
                    actual: nominal.arguments().len(),
                });
            }
            self.insert_checked(&CheckedProjectNominal::new(
                nominal.declaration().clone(),
                declaration.owner(),
                nominal_type.semantic_identity_digest(),
                nominal.arguments().to_vec(),
            ))
        })
    }

    /// Collects every request reachable from the complete unpublished draft.
    /// Each prepared algebra is matched directly; nested types delegate to the
    /// exhaustive `TypeKind` owner visitor.
    pub(crate) fn from_prepared(
        draft: &FinalSemanticAnalysisDraft,
        symbols: &ProjectSymbolTable,
    ) -> Result<Self, NominalSchemaProjectionError> {
        let mut inventory = Self::default();
        for ty in draft.types.values() {
            inventory.visit_type(symbols, ty)?;
        }
        for report in draft.type_resolutions.values() {
            report.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for binding in draft.locals.values().chain(draft.captures.values()) {
            binding.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for fact in draft.expressions.values() {
            fact.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for fact in draft.patterns.values() {
            fact.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for statement in draft.statements.values() {
            statement.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for item in draft.items.values() {
            item.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for call in draft.calls.values() {
            call.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for joined in draft.callable_joins.values() {
            match joined {
                Ok(join) => {
                    join.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
                }
                Err(error) => {
                    error.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
                }
            }
        }
        draft
            .checked_callables
            .visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        Ok(inventory)
    }

    /// Recomputes the exact nominal request inventory after Entry sealing and
    /// before the affine statement/effect seal. The post-Entry draft retains
    /// no executable-ingress fallback authority.
    pub(crate) fn from_post_entry(
        draft: &FinalSemanticAnalysisPostEntryDraft,
        symbols: &ProjectSymbolTable,
    ) -> Result<Self, NominalSchemaProjectionError> {
        let mut inventory = Self::default();
        for ty in draft.types.values() {
            inventory.visit_type(symbols, ty)?;
        }
        for report in draft.type_resolutions.values() {
            report.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for binding in draft.locals.values().chain(draft.captures.values()) {
            binding.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for fact in draft.expressions.values() {
            fact.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for fact in draft.patterns.values() {
            fact.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for statement in draft.statements.values() {
            statement.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for item in draft.items.values() {
            item.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for call in draft.calls.values() {
            call.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        }
        for joined in draft.callable_joins.values() {
            match joined {
                Ok(join) => join.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?,
                Err(error) => error.visit_types(&mut |ty| inventory.visit_type(symbols, ty))?,
            }
        }
        draft
            .checked_callables
            .visit_types(&mut |ty| inventory.visit_type(symbols, ty))?;
        Ok(inventory)
    }
}

fn ty_for_project_nominal(nominal: &ProjectNominalType) -> TypeKind {
    TypeKind::ProjectNominal(ProjectNominalType::new(
        nominal.declaration().clone(),
        nominal.arguments().to_vec(),
    ))
}

/// Immutable complete runtime-nominal catalog published by final analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeNominalProjectionCatalog {
    by_semantic_type: BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
}

impl RuntimeNominalProjectionCatalog {
    fn get_semantic(
        &self,
        semantic_type: SemanticTypeDigest,
    ) -> Option<&RuntimeProjectNominalProjection> {
        self.by_semantic_type
            .get(&semantic_type)
            .filter(|projection| projection_semantic_digest(projection) == semantic_type)
    }
}

fn checked_nominal_semantic_digest(nominal: &CheckedProjectNominal) -> SemanticTypeDigest {
    TypeKind::ProjectNominal(ProjectNominalType::new(
        nominal.declaration().clone(),
        nominal.arguments().to_vec(),
    ))
    .semantic_identity_digest()
}

fn projection_semantic_digest(projection: &RuntimeProjectNominalProjection) -> SemanticTypeDigest {
    SemanticTypeDigest::from_bytes(*projection.semantic_identity().as_bytes())
}

impl RuntimeProjectNominalProjection {
    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn owner(&self) -> arcweft_lang_hir::identity::ItemId {
        self.owner
    }

    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.nominal
    }

    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    /// Canonical checked data shape retained by the sole projection owner.
    pub const fn shape(&self) -> &TypeShape {
        &self.shape
    }

    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    pub const fn schema(&self) -> &RuntimeTypeSchema {
        &self.schema
    }

    pub const fn kind(&self) -> RuntimeProjectNominalKind {
        self.kind
    }

    pub const fn record_fields(&self) -> &[RuntimeProjectRecordFieldProjection] {
        &self.record_fields
    }

    pub const fn variant_cases(&self) -> &[RuntimeProjectVariantCaseProjection] {
        &self.variant_cases
    }

    pub fn record_field(
        &self,
        declaration_ordinal: u32,
    ) -> Option<&RuntimeProjectRecordFieldProjection> {
        self.record_fields
            .get(usize::try_from(declaration_ordinal).ok()?)
            .filter(|field| field.declaration_ordinal == declaration_ordinal)
    }
}

impl NominalSchemaProjectionError {
    fn new(reason: impl Into<String>) -> Self {
        Self::InvalidShape {
            path: "nominal".to_owned(),
            reason: reason.into(),
        }
    }

    fn within_step(self, step: NominalSchemaPathStep) -> Self {
        match self {
            Self::OpaqueLeaf {
                path,
                nominal,
                semantic_identity,
            } => Self::OpaqueLeaf {
                path: path.prepended(step),
                nominal,
                semantic_identity,
            },
            Self::UnsupportedLeaf { path, ty } => Self::UnsupportedLeaf {
                path: path.prepended(step),
                ty,
            },
            Self::CyclicGenericSubstitution { path, parameter } => {
                Self::CyclicGenericSubstitution {
                    path: path.prepended(step),
                    parameter,
                }
            }
            other => other,
        }
    }
}

impl ProjectionBudget {
    const fn new(limits: NominalResolutionLimits) -> Self {
        Self {
            limits,
            nodes: 0,
            depth: 0,
            work: 0,
        }
    }

    fn enter_node(
        &mut self,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), NominalSchemaProjectionError> {
        check_control(control)?;
        let nodes = self
            .nodes
            .checked_add(1)
            .ok_or(NominalSchemaProjectionError::ArithmeticOverflow)?;
        let depth = self
            .depth
            .checked_add(1)
            .ok_or(NominalSchemaProjectionError::ArithmeticOverflow)?;
        let work = self
            .work
            .checked_add(1)
            .ok_or(NominalSchemaProjectionError::ArithmeticOverflow)?;
        check_root_limit(
            NominalResolutionLimitKind::TypeNodesPerReference,
            nodes,
            self.limits.type_nodes_per_reference(),
        )?;
        check_root_limit(
            NominalResolutionLimitKind::RecursiveTypeDepth,
            u64::from(depth),
            u64::from(self.limits.recursive_type_depth()),
        )?;
        check_root_limit(
            NominalResolutionLimitKind::WorkPerReference,
            work,
            self.limits.work_per_reference(),
        )?;
        self.nodes = nodes;
        self.depth = depth;
        self.work = work;
        Ok(())
    }

    fn leave_node(&mut self) {
        debug_assert!(self.depth > 0, "projection depth is balanced");
        self.depth -= 1;
    }

    fn charge_generic_arguments(
        &mut self,
        count: usize,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), NominalSchemaProjectionError> {
        check_control(control)?;
        let count =
            u64::try_from(count).map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
        let work = self
            .work
            .checked_add(count)
            .ok_or(NominalSchemaProjectionError::ArithmeticOverflow)?;
        check_root_limit(
            NominalResolutionLimitKind::GenericArgumentsPerApplication,
            count,
            u64::from(self.limits.generic_arguments_per_application()),
        )?;
        check_root_limit(
            NominalResolutionLimitKind::WorkPerReference,
            work,
            self.limits.work_per_reference(),
        )?;
        self.work = work;
        Ok(())
    }
}

fn check_control(
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<(), NominalSchemaProjectionError> {
    control
        .check()
        .map_err(|_| NominalSchemaProjectionError::Cancelled)
}

fn check_root_limit(
    kind: NominalResolutionLimitKind,
    observed: u64,
    maximum: u64,
) -> Result<(), NominalSchemaProjectionError> {
    if observed > maximum {
        Err(NominalSchemaProjectionError::LimitExceeded {
            kind: NominalProjectionLimitKind::Root(kind),
            observed,
            maximum,
        })
    } else {
        Ok(())
    }
}

impl<'a> RuntimeNominalProjectionContext<'a> {
    pub(crate) const fn new(
        symbols: &'a ProjectSymbolTable,
        types: &'a BTreeMap<TypeId, TypeKind>,
        root_limits: NominalResolutionLimits,
        aggregate_limits: NominalAggregationLimits,
        control: FinalSemanticAnalysisControl<'a>,
    ) -> Self {
        Self {
            symbols,
            types,
            root_limits,
            aggregate_limits,
            aggregate_work: 0,
            visiting: BTreeSet::new(),
            accepted: BTreeMap::new(),
            control,
        }
    }

    pub(crate) fn project_checked(
        &mut self,
        checked: &CheckedProjectNominal,
    ) -> Result<&RuntimeProjectNominalProjection, NominalSchemaProjectionError> {
        self.validate_checked(checked)?;
        self.charge_aggregate_request()?;
        let key = checked.identity();
        if self.accepted.contains_key(&key) {
            return self.accepted.get(&key).ok_or_else(|| {
                NominalSchemaProjectionError::new(
                    "observed projection cache key has no retained projection",
                )
            });
        }
        let mut budget = ProjectionBudget::new(self.root_limits);
        budget.enter_node(self.control)?;
        if !self.visiting.insert(key) {
            budget.leave_node();
            return Err(NominalSchemaProjectionError::new(format!(
                "cyclic projection request for semantic type {key:?}"
            )));
        }

        let Some(declaration) = self.symbols.nominal(checked.declaration()) else {
            self.visiting.remove(&key);
            budget.leave_node();
            return Err(NominalSchemaProjectionError::MissingDeclaration {
                nominal: checked.declaration().qualified_name(),
            });
        };
        let projection = self.expand(declaration, checked, &mut budget);
        self.visiting.remove(&key);
        budget.leave_node();
        let projection = projection?;
        self.accepted.insert(key, projection);
        self.accepted.get(&key).ok_or_else(|| {
            NominalSchemaProjectionError::new(
                "completed projection cache insertion retained no projection",
            )
        })
    }

    pub(crate) fn project_inventory(
        mut self,
        inventory: RuntimeNominalProjectionRequestInventory,
    ) -> Result<RuntimeNominalProjectionSeal, NominalSchemaProjectionError> {
        for (semantic_type, nominal) in &inventory.by_semantic_type {
            if *semantic_type != nominal.identity() {
                return Err(NominalSchemaProjectionError::IdentityMismatch {
                    requested: *semantic_type,
                    projected: nominal.identity(),
                });
            }
            self.project_checked(nominal)?;
        }
        Ok(RuntimeNominalProjectionSeal {
            requested: inventory.by_semantic_type,
            accepted: self.accepted,
        })
    }

    fn validate_checked(
        &self,
        checked: &CheckedProjectNominal,
    ) -> Result<(), NominalSchemaProjectionError> {
        validate_checked_nominal(self.symbols, checked)
    }

    fn charge_aggregate_request(&mut self) -> Result<(), NominalSchemaProjectionError> {
        check_control(self.control)?;
        let observed = self
            .aggregate_work
            .checked_add(1)
            .ok_or(NominalSchemaProjectionError::ArithmeticOverflow)?;
        let maximum = self.aggregate_limits.work_per_project();
        if observed > maximum {
            return Err(NominalSchemaProjectionError::LimitExceeded {
                kind: NominalProjectionLimitKind::Project(
                    NominalAggregationLimitKind::WorkPerProject,
                ),
                observed,
                maximum,
            });
        }
        self.aggregate_work = observed;
        Ok(())
    }

    fn expand(
        &self,
        declaration: &ProjectNominalDeclaration,
        checked: &CheckedProjectNominal,
        budget: &mut ProjectionBudget,
    ) -> Result<RuntimeProjectNominalProjection, NominalSchemaProjectionError> {
        let (kind, record_fields, variant_cases) = match declaration.body() {
            ProjectNominalBody::Struct { fields } => {
                let fields = fields
                    .iter()
                    .enumerate()
                    .map(|(ordinal, field)| {
                        let declaration_ordinal = u32::try_from(ordinal)
                            .map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
                        let runtime_field = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                            .map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
                        let declared = self
                            .types
                            .get(&field.ty())
                            .ok_or(NominalSchemaProjectionError::MissingTypeFact {
                                ty: field.ty(),
                            })?;
                        let ty = checked
                            .instantiate_declaration_type(declaration, declared)
                            .ok_or_else(|| {
                                NominalSchemaProjectionError::new(
                                    "record field cannot be instantiated by its checked nominal owner",
                                )
                            })?;
                        let field_type = ty.semantic_identity_digest();
                        Ok(RuntimeProjectRecordFieldProjection {
                            runtime_field,
                            declaration_ordinal,
                            ty,
                            field_type,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice();
                (
                    RuntimeProjectNominalKind::Record,
                    fields,
                    Vec::new().into_boxed_slice(),
                )
            }
            ProjectNominalBody::Enum { variants } => {
                let cases = variants
                    .iter()
                    .enumerate()
                    .map(|(ordinal, variant)| {
                        let ordinal = u32::try_from(ordinal)
                            .map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
                        let payload = variant
                            .payload()
                            .map(|payload| {
                                let declared = self.types.get(&payload).ok_or(
                                    NominalSchemaProjectionError::MissingTypeFact { ty: payload },
                                )?;
                                checked
                                    .instantiate_declaration_type(declaration, declared)
                                    .ok_or_else(|| {
                                        NominalSchemaProjectionError::new(
                                            "enum payload cannot be instantiated by its checked nominal owner",
                                        )
                                    })
                            })
                            .transpose()?;
                        Ok(RuntimeProjectVariantCaseProjection {
                            ordinal,
                            diagnostic_name: variant.name().clone(),
                            payload,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice();
                (
                    RuntimeProjectNominalKind::Variant,
                    Vec::new().into_boxed_slice(),
                    cases,
                )
            }
            ProjectNominalBody::TypeAlias { .. } => {
                return Err(NominalSchemaProjectionError::UnsupportedDeclaration {
                    nominal: checked.declaration().qualified_name(),
                });
            }
        };
        let semantic_identity = RuntimeSemanticTypeId::from_bytes(*checked.identity().as_bytes());
        let shape = NominalSchemaExpander::new(self.symbols, self.types, self.control)
            .schema_checked_after_root(declaration, checked.arguments(), budget)?;
        let schema = project_runtime_type_schema(&shape);
        let runtime_name = runtime_project_nominal_name(checked.declaration());
        let nominal = RuntimeNominalTypeId::try_new(runtime_name.clone()).map_err(|error| {
            NominalSchemaProjectionError::InvalidRuntimeIdentity {
                nominal: runtime_name.clone(),
                reason: error.to_string(),
            }
        })?;
        let layout = schema.try_layout_hash().map_err(|error| {
            NominalSchemaProjectionError::InvalidRuntimeSchema {
                nominal: runtime_name,
                reason: error.to_string(),
            }
        })?;
        Ok(RuntimeProjectNominalProjection {
            declaration: checked.declaration().clone(),
            owner: checked.owner(),
            nominal,
            semantic_identity,
            shape,
            layout,
            schema,
            kind,
            record_fields,
            variant_cases,
        })
    }

    #[cfg(test)]
    const fn aggregate_work(&self) -> u64 {
        self.aggregate_work
    }

    #[cfg(test)]
    fn accepted_len(&self) -> usize {
        self.accepted.len()
    }
}

impl RuntimeNominalProjectionSeal {
    pub(crate) fn get_cached(
        &self,
        checked: &CheckedProjectNominal,
    ) -> Result<&RuntimeProjectNominalProjection, NominalSchemaProjectionError> {
        let requested = self.requested.get(&checked.identity()).ok_or(
            NominalSchemaProjectionError::UnexpectedFinalRequest {
                semantic_type: checked.identity(),
            },
        )?;
        if requested != checked {
            return Err(NominalSchemaProjectionError::ConflictingRequest {
                semantic_type: checked.identity(),
            });
        }
        let projection = self.accepted.get(&checked.identity()).ok_or(
            NominalSchemaProjectionError::MissingCachedProjection {
                semantic_type: checked.identity(),
            },
        )?;
        let projected = projection_semantic_digest(projection);
        if projected != checked.identity() {
            return Err(NominalSchemaProjectionError::IdentityMismatch {
                requested: checked.identity(),
                projected,
            });
        }
        Ok(projection)
    }

    pub(crate) fn validate_final_inventory(
        self,
        final_inventory: RuntimeNominalProjectionRequestInventory,
    ) -> Result<ValidatedRuntimeNominalProjectionSeal, NominalSchemaProjectionError> {
        for (semantic_type, requested) in &self.requested {
            let Some(final_request) = final_inventory.by_semantic_type.get(semantic_type) else {
                return Err(NominalSchemaProjectionError::MissingFinalRequest {
                    semantic_type: *semantic_type,
                });
            };
            if final_request != requested {
                return Err(NominalSchemaProjectionError::ConflictingRequest {
                    semantic_type: *semantic_type,
                });
            }
            if !self.accepted.contains_key(semantic_type) {
                return Err(NominalSchemaProjectionError::MissingCachedProjection {
                    semantic_type: *semantic_type,
                });
            }
        }
        if let Some(semantic_type) = final_inventory
            .by_semantic_type
            .keys()
            .find(|semantic_type| !self.requested.contains_key(semantic_type))
        {
            return Err(NominalSchemaProjectionError::UnexpectedFinalRequest {
                semantic_type: *semantic_type,
            });
        }
        Ok(ValidatedRuntimeNominalProjectionSeal {
            accepted: self.accepted,
        })
    }
}

impl ValidatedRuntimeNominalProjectionSeal {
    pub(crate) fn finish(self) -> RuntimeNominalProjectionCatalog {
        RuntimeNominalProjectionCatalog {
            by_semantic_type: self.accepted,
        }
    }
}

pub(super) fn validate_checked_nominal(
    symbols: &ProjectSymbolTable,
    checked: &CheckedProjectNominal,
) -> Result<(), NominalSchemaProjectionError> {
    if checked.declaration().world() != symbols.world()
        || checked.declaration().revision() != *symbols.revision()
    {
        return Err(NominalSchemaProjectionError::GenerationMismatch);
    }
    let declaration = symbols.nominal(checked.declaration()).ok_or_else(|| {
        NominalSchemaProjectionError::MissingDeclaration {
            nominal: checked.declaration().qualified_name(),
        }
    })?;
    if declaration.owner() != checked.owner() {
        return Err(NominalSchemaProjectionError::OwnerMismatch {
            nominal: checked.declaration().qualified_name(),
            expected: declaration.owner(),
            actual: checked.owner(),
        });
    }
    if declaration.type_parameters().len() != checked.arguments().len() {
        return Err(NominalSchemaProjectionError::WrongArity {
            nominal: checked.declaration().qualified_name(),
            expected: declaration.type_parameters().len(),
            actual: checked.arguments().len(),
        });
    }
    let projected = checked_nominal_semantic_digest(checked);
    if checked.identity() != projected {
        return Err(NominalSchemaProjectionError::IdentityMismatch {
            requested: checked.identity(),
            projected,
        });
    }
    Ok(())
}

pub(super) fn seal_runtime_nominal_draft(
    draft: FinalSemanticAnalysisDraft,
    project: crate::final_analysis::HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    semantic_shapes: super::AcceptedSemanticShapeCatalog,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<FinalSemanticAnalysis, FinalSemanticProjectError> {
    let inventory = RuntimeNominalProjectionRequestInventory::from_prepared(&draft, symbols)?;
    let project_nominals = super::nominal_semantic::ProjectNominalSemanticCatalog::build(
        inventory.nominals(),
        symbols,
        &draft.types,
        control,
    )?;
    let mut parts = draft.into_parts();
    let context = RuntimeNominalProjectionContext::new(
        symbols,
        &parts.types,
        NominalResolutionLimits::PRODUCTION,
        NominalAggregationLimits::PRODUCTION,
        control,
    );
    let seal = context.project_inventory(inventory)?;
    let prepared_expressions = std::mem::take(&mut parts.expressions);
    let mut sealed_record_expressions = {
        let coordinates =
            SemanticCoordinateIndex::new(parts.accepted_roots.as_ref(), &parts.structural_edges);
        preseal_project_record_expressions(
            &prepared_expressions,
            &parts.locals,
            &seal,
            &coordinates,
        )?
    };
    let mut record_fields = BTreeMap::new();
    parts.expressions = prepared_expressions
        .into_iter()
        .map(|(owner, fact)| {
            let sealed_record = sealed_record_expressions.remove(&owner);
            let (fact, fields) =
                seal_prepared_expression(owner, fact, &seal, &project_nominals, sealed_record)?;
            if let Some(fields) = fields {
                record_fields.insert(owner, fields);
            }
            Ok((owner, fact))
        })
        .collect::<Result<_, FinalSemanticAnalysisError>>()?;
    if !sealed_record_expressions.is_empty() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
    }
    parts
        .structural_edges
        .attach_record_fields(record_fields)
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let prepared_patterns = std::mem::take(&mut parts.patterns);
    let mut sealed_record_patterns = {
        let coordinates =
            SemanticCoordinateIndex::new(parts.accepted_roots.as_ref(), &parts.structural_edges);
        preseal_record_patterns(&prepared_patterns, &parts.locals, &seal, &coordinates)?
    };
    parts.patterns = prepared_patterns
        .into_iter()
        .map(|(owner, fact)| {
            seal_prepared_pattern(
                owner,
                fact,
                &project_nominals,
                sealed_record_patterns.remove(&owner),
            )
            .map(|fact| (owner, fact))
        })
        .collect::<Result<_, _>>()?;
    if !sealed_record_patterns.is_empty() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
    }
    let (entry_ingress, mut draft) = parts.into_post_entry();
    let checked_entries = {
        let authority = crate::entry::PreparedEntrySemanticAuthority::new(
            &draft.types,
            &draft.items,
            &draft.calls,
            draft.checked_callables.as_ref(),
            &seal,
        );
        crate::entry::check_prepared_project_entries(project, symbols, &authority, entry_ingress)
            .map_err(|diagnostics| FinalSemanticProjectError::Entry(diagnostics.into()))?
    };
    draft.expressions = draft
        .expressions
        .into_iter()
        .map(|(owner, fact)| {
            seal_prepared_entry_expression(fact, &checked_entries).map(|fact| (owner, fact))
        })
        .collect::<Result<_, _>>()?;
    let final_inventory =
        RuntimeNominalProjectionRequestInventory::from_post_entry(&draft, symbols)?;
    project_nominals.validate_inventory(final_inventory.nominals())?;
    let runtime_nominals = seal.validate_final_inventory(final_inventory)?.finish();
    Ok(draft.seal(
        project,
        symbols,
        checked_entries,
        project_nominals,
        semantic_shapes,
        runtime_nominals,
        control,
    )?)
}

fn seal_prepared_entry_expression(
    fact: PreparedExpressionFact,
    checked_entries: &crate::entry::CheckedEntryCatalog,
) -> Result<PreparedExpressionFact, FinalSemanticAnalysisError> {
    let PreparedExpressionFact::Entry(prepared) = fact else {
        return Ok(fact);
    };
    let (reference, shell, value_type) = prepared.into_parts();
    let binding = checked_entries
        .get_public(reference.diagnostic_public_id())
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
    let reference =
        crate::final_analysis::CheckedEntryReference::seal(reference, value_type, binding)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let (ty, type_selection, effects) = shell.into_parts();
    Ok(CheckedExpression::new(
        ty,
        type_selection,
        effects,
        CheckedExpressionResolution::Value(crate::final_analysis::CheckedValueResolution::Entry(
            reference,
        )),
    )
    .into())
}

struct SealedProjectRecordExpression {
    nominal: SemanticTypeDigest,
    fields: Box<[CheckedExpressionRecordField]>,
}

fn preseal_project_record_expressions(
    expressions: &BTreeMap<arcweft_lang_hir::identity::ExprId, PreparedExpressionFact>,
    locals: &BTreeMap<arcweft_lang_hir::identity::LocalId, super::CheckedBinding>,
    context: &RuntimeNominalProjectionSeal,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<
    BTreeMap<arcweft_lang_hir::identity::ExprId, SealedProjectRecordExpression>,
    FinalSemanticAnalysisError,
> {
    expressions
        .iter()
        .filter_map(|(owner, fact)| {
            let PreparedExpressionFact::ProjectRecord(prepared) = fact else {
                return None;
            };
            Some((|| {
                let projection = context.get_cached(prepared.nominal())?;
                if projection.kind() != RuntimeProjectNominalKind::Record
                    || projection.record_fields().len() != prepared.fields().len()
                    || prepared.shell().ty().semantic_identity_digest()
                        != prepared.nominal().identity()
                {
                    return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                }
                let mut fields = Vec::with_capacity(prepared.fields().len());
                for (expected_source_ordinal, field) in prepared.fields().iter().enumerate() {
                    let expected_source_ordinal = u32::try_from(expected_source_ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                    if field.source_ordinal() != expected_source_ordinal {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    let field_type_digest = field.field_type().semantic_identity_digest();
                    let projected = projection
                        .record_field(field.declaration_ordinal())
                        .filter(|projected| projected.field_type() == field_type_digest)
                        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                    let source = match field.source() {
                        PreparedRecordValueSource::Expression(source) => {
                            expressions
                                .get(&source)
                                .filter(|checked| {
                                    checked.ty().semantic_identity_digest() == field_type_digest
                                })
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            CheckedRecordValueSource::Expression(
                                CheckedRecordExpressionSource::from_evidence(
                                    coordinates
                                        .expression_evidence(source)
                                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
                                ),
                            )
                        }
                        PreparedRecordValueSource::Local(source) => {
                            locals
                                .get(&source)
                                .filter(|checked| {
                                    checked.ty().semantic_identity_digest() == field_type_digest
                                })
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            CheckedRecordValueSource::Binding(
                                CheckedRecordBindingSource::from_evidence(
                                    coordinates
                                        .binding_evidence(source)
                                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
                                ),
                            )
                        }
                    };
                    let semantic_id = AcceptedRecordFieldSemanticId::issue(
                        prepared.nominal().identity(),
                        field.declaration_ordinal(),
                        field_type_digest,
                    );
                    fields.push(CheckedExpressionRecordField::new(
                        field.source_ordinal(),
                        field.declaration_ordinal(),
                        projected.runtime_field(),
                        CheckedRecordFieldSemanticId::Project(semantic_id),
                        field_type_digest,
                        source,
                    ));
                }
                Ok((
                    *owner,
                    SealedProjectRecordExpression {
                        nominal: prepared.nominal().identity(),
                        fields: fields.into_boxed_slice(),
                    },
                ))
            })())
        })
        .collect()
}

fn seal_prepared_expression(
    _owner: arcweft_lang_hir::identity::ExprId,
    fact: PreparedExpressionFact,
    context: &RuntimeNominalProjectionSeal,
    project_nominals: &super::nominal_semantic::ProjectNominalSemanticCatalog,
    sealed_record: Option<SealedProjectRecordExpression>,
) -> Result<
    (
        PreparedExpressionFact,
        Option<Box<[CheckedExpressionRecordField]>>,
    ),
    FinalSemanticAnalysisError,
> {
    match fact {
        PreparedExpressionFact::ProjectVariant(prepared) => {
            let (shell, owner, selected_ordinal) = prepared.into_parts();
            let owner = seal_project_variant_owner(owner, project_nominals)?;
            let resolution = CheckedVariantResolution::try_new(owner, selected_ordinal)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            let (ty, type_selection, effects) = shell.into_parts();
            Ok((
                CheckedExpression::new(
                    ty,
                    type_selection,
                    effects,
                    CheckedExpressionResolution::Variant(resolution),
                )
                .into(),
                None,
            ))
        }
        PreparedExpressionFact::ProjectField(prepared) => {
            if sealed_record.is_some() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let (shell, nominal, declaration_ordinal, field_type, diagnostic_name) =
                prepared.into_parts();
            let projection = context.get_cached(&nominal)?;
            let field_type_digest = field_type.semantic_identity_digest();
            let projected = projection
                .record_field(declaration_ordinal)
                .filter(|projected| projected.field_type() == field_type_digest)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            let semantic_id =
                CheckedRecordFieldSemanticId::Project(AcceptedRecordFieldSemanticId::issue(
                    nominal.identity(),
                    declaration_ordinal,
                    field_type_digest,
                ));
            let (ty, type_selection, effects) = shell.into_parts();
            if ty != field_type {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let selection = crate::final_analysis::CheckedFieldSelection::try_new(
                nominal.identity(),
                semantic_id,
                declaration_ordinal,
                Some(projected.runtime_field()),
                field_type_digest,
                diagnostic_name,
            )
            .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            Ok((
                CheckedExpression::new(
                    ty,
                    type_selection,
                    effects,
                    CheckedExpressionResolution::Select(
                        crate::final_analysis::CheckedSelectResolution::Field(selection),
                    ),
                )
                .into(),
                None,
            ))
        }
        PreparedExpressionFact::ProjectRecord(prepared) => {
            let (shell, nominal, prepared_fields) = prepared.into_parts();
            let sealed_record = sealed_record
                .filter(|sealed| {
                    sealed.nominal == nominal.identity()
                        && sealed.fields.len() == prepared_fields.len()
                })
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let (ty, type_selection, effects) = shell.into_parts();
            if ty.semantic_identity_digest() != nominal.identity() {
                return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
            }
            Ok((
                CheckedExpression::new(
                    ty,
                    type_selection,
                    effects,
                    CheckedExpressionResolution::Nominal(nominal),
                )
                .into(),
                Some(sealed_record.fields),
            ))
        }
        fact if sealed_record.is_none() => Ok((fact, None)),
        _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
    }
}

struct SealedRecordPatternField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    runtime_field: Option<RuntimeRecordFieldId>,
    semantic_id: CheckedRecordFieldSemanticId,
    field_type: SemanticTypeDigest,
    source: CheckedRecordPatternSource,
}

enum SealedRecordPatternOwner {
    Project {
        nominal: SemanticTypeDigest,
        layout: TypeLayoutHash,
        field_count: u32,
    },
    Environment {
        record: crate::env::nominal::AcceptedEnvironmentRecordIdentity,
    },
    VariantPayload {
        payload: crate::types::VariantPayloadType,
        semantic_type: SemanticTypeDigest,
        field_count: u32,
    },
}

struct SealedRecordPattern {
    owner: SealedRecordPatternOwner,
    fields: Box<[SealedRecordPatternField]>,
    rest: CheckedRecordPatternRest,
}

fn preseal_record_patterns(
    patterns: &BTreeMap<arcweft_lang_hir::identity::PatternId, PreparedPatternFact>,
    locals: &BTreeMap<arcweft_lang_hir::identity::LocalId, super::CheckedBinding>,
    context: &RuntimeNominalProjectionSeal,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<
    BTreeMap<arcweft_lang_hir::identity::PatternId, SealedRecordPattern>,
    FinalSemanticAnalysisError,
> {
    patterns
        .iter()
        .filter_map(|(owner, fact)| {
            let PreparedPatternFact::Record(prepared) = fact else {
                return None;
            };
            Some((|| {
                if prepared.ty().semantic_identity_digest() != prepared.owner().semantic_type() {
                    return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                }
                let (sealed_owner, projection) = match prepared.owner() {
                    PreparedRecordPatternOwner::Project(nominal) => {
                        let projection = context.get_cached(nominal)?;
                        if projection.kind() != RuntimeProjectNominalKind::Record {
                            return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                        }
                        (
                            SealedRecordPatternOwner::Project {
                                nominal: nominal.identity(),
                                layout: projection.layout(),
                                field_count: u32::try_from(projection.record_fields().len())
                                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                            },
                            Some(projection),
                        )
                    }
                    PreparedRecordPatternOwner::Environment { record } => (
                        SealedRecordPatternOwner::Environment {
                            record: record.clone(),
                        },
                        None,
                    ),
                    PreparedRecordPatternOwner::VariantPayload {
                        payload,
                        semantic_type,
                        field_count,
                    } => {
                        let fields = payload
                            .shape()
                            .record_fields()
                            .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                        if u32::try_from(fields.len()).ok() != Some(*field_count)
                            || TypeKind::VariantPayload(Box::new(payload.clone()))
                                .semantic_identity_digest()
                                != *semantic_type
                        {
                            return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                        }
                        (
                            SealedRecordPatternOwner::VariantPayload {
                                payload: payload.clone(),
                                semantic_type: *semantic_type,
                                field_count: *field_count,
                            },
                            None,
                        )
                    }
                };
                let mut fields = Vec::with_capacity(prepared.fields().len());
                for field in prepared.fields() {
                    let field_type = field.field_type().semantic_identity_digest();
                    let (declaration_ordinal, runtime_field, semantic_id) = match (
                        prepared.owner(),
                        field.identity(),
                        projection,
                    ) {
                        (
                            PreparedRecordPatternOwner::Project(nominal),
                            PreparedRecordPatternFieldIdentity::Project {
                                declaration_ordinal,
                            },
                            Some(projection),
                        ) => {
                            let projected = projection
                                .record_field(declaration_ordinal)
                                .filter(|projected| projected.field_type() == field_type)
                                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                            (
                                declaration_ordinal,
                                Some(projected.runtime_field()),
                                CheckedRecordFieldSemanticId::Project(
                                    AcceptedRecordFieldSemanticId::issue(
                                        nominal.identity(),
                                        declaration_ordinal,
                                        field_type,
                                    ),
                                ),
                            )
                        }
                        (
                            PreparedRecordPatternOwner::Environment { record },
                            PreparedRecordPatternFieldIdentity::Environment {
                                declaration_ordinal,
                                semantic_id: CheckedRecordFieldSemanticId::Environment(semantic_id),
                            },
                            None,
                        ) if semantic_id
                            == crate::env::nominal::AcceptedEnvironmentFieldSemanticId::issue(
                                record.semantic_type(),
                                declaration_ordinal,
                                field_type,
                            ) =>
                        {
                            (
                                declaration_ordinal,
                                None,
                                CheckedRecordFieldSemanticId::Environment(semantic_id),
                            )
                        }
                        (
                            PreparedRecordPatternOwner::VariantPayload { payload, .. },
                            PreparedRecordPatternFieldIdentity::VariantPayload {
                                declaration_ordinal,
                                semantic_id:
                                    CheckedRecordFieldSemanticId::VariantPayload(semantic_id),
                            },
                            None,
                        ) => {
                            let index = usize::try_from(declaration_ordinal)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                            let expected = payload
                                .shape()
                                .record_fields()
                                .and_then(|fields| fields.get(index))
                                .filter(|expected| {
                                    expected.ordinal() == declaration_ordinal
                                        && expected.semantic_id() == semantic_id
                                        && expected.ty().semantic_identity_digest() == field_type
                                })
                                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                            (
                                expected.ordinal(),
                                None,
                                CheckedRecordFieldSemanticId::VariantPayload(semantic_id),
                            )
                        }
                        _ => return Err(FinalSemanticAnalysisError::InvalidNominalOwner),
                    };
                    let source = match field.source() {
                        PreparedRecordPatternSource::Pattern(source) => {
                            patterns
                                .get(&source)
                                .filter(|checked| {
                                    checked.ty().semantic_identity_digest() == field_type
                                })
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            CheckedRecordPatternSource::pattern(
                                source,
                                StablePatternCoordinate::new([
                                    StablePatternCoordinateStep::RecordField {
                                        field: semantic_id,
                                        source_ordinal: field.source_ordinal(),
                                    },
                                ]),
                            )
                        }
                        PreparedRecordPatternSource::Binding(source) => {
                            locals
                                .get(&source)
                                .filter(|checked| {
                                    checked.ty().semantic_identity_digest() == field_type
                                })
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                            CheckedRecordPatternSource::from_binding(
                                CheckedRecordBindingSource::from_evidence(
                                    coordinates
                                        .binding_evidence(source)
                                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
                                ),
                            )
                        }
                    };
                    fields.push(SealedRecordPatternField {
                        source_ordinal: field.source_ordinal(),
                        declaration_ordinal,
                        runtime_field,
                        semantic_id,
                        field_type,
                        source,
                    });
                }
                let rest = match prepared.rest() {
                    PreparedRecordPatternRest::Absent => CheckedRecordPatternRest::Absent,
                    PreparedRecordPatternRest::Ignore => CheckedRecordPatternRest::Ignore,
                    PreparedRecordPatternRest::Binding(source) => {
                        locals
                            .get(&source)
                            .filter(|binding| binding.ty() == prepared.ty())
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        CheckedRecordPatternRest::Binding(
                            CheckedRecordBindingSource::from_evidence(
                                coordinates
                                    .binding_evidence(source)
                                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
                            ),
                        )
                    }
                };
                Ok((
                    *owner,
                    SealedRecordPattern {
                        owner: sealed_owner,
                        fields: fields.into_boxed_slice(),
                        rest,
                    },
                ))
            })())
        })
        .collect()
}

fn seal_prepared_pattern(
    _owner: arcweft_lang_hir::identity::PatternId,
    fact: PreparedPatternFact,
    project_nominals: &super::nominal_semantic::ProjectNominalSemanticCatalog,
    sealed_record: Option<SealedRecordPattern>,
) -> Result<PreparedPatternFact, FinalSemanticAnalysisError> {
    match fact {
        PreparedPatternFact::ProjectVariant(prepared) => {
            let (ty, owner, selected_ordinal) = prepared.into_parts();
            let owner = seal_project_variant_owner(owner, project_nominals)?;
            let resolution = CheckedVariantResolution::try_new(owner, selected_ordinal)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            Ok(CheckedPattern::new(ty, CheckedPatternResolution::Variant(resolution)).into())
        }
        PreparedPatternFact::Record(prepared) => {
            let (ty, prepared_owner, prepared_fields, prepared_rest) = prepared.into_parts();
            let sealed_record = sealed_record
                .filter(|sealed| sealed.fields.len() == prepared_fields.len())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let mut fields = Vec::with_capacity(prepared_fields.len());
            for (field, sealed) in prepared_fields
                .into_vec()
                .into_iter()
                .zip(sealed_record.fields.into_vec())
            {
                let (source_ordinal, identity, field_type, source) = field.into_parts();
                let declaration_ordinal = identity.declaration_ordinal();
                let field_type_digest = field_type.semantic_identity_digest();
                let source_matches = match source {
                    PreparedRecordPatternSource::Pattern(source) => {
                        sealed.source.raw_pattern() == Some(source)
                    }
                    PreparedRecordPatternSource::Binding(source) => sealed
                        .source
                        .binding()
                        .is_some_and(|binding| binding.raw() == source),
                };
                if !source_matches
                    || source_ordinal != sealed.source_ordinal
                    || declaration_ordinal != sealed.declaration_ordinal
                    || field_type_digest != sealed.field_type
                    || identity
                        .semantic_id()
                        .is_some_and(|semantic_id| semantic_id != sealed.semantic_id)
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                fields.push(CheckedRecordPatternField::new(
                    source_ordinal,
                    declaration_ordinal,
                    sealed.runtime_field,
                    sealed.semantic_id,
                    field_type,
                    sealed.source,
                ));
            }
            let owner = match (prepared_owner, sealed_record.owner) {
                (
                    PreparedRecordPatternOwner::Project(nominal),
                    SealedRecordPatternOwner::Project {
                        nominal: sealed_nominal,
                        layout,
                        field_count,
                    },
                ) if nominal.identity() == sealed_nominal => {
                    CheckedRecordPatternOwner::project(nominal, layout, field_count)
                }
                (
                    PreparedRecordPatternOwner::Environment { record },
                    SealedRecordPatternOwner::Environment {
                        record: sealed_record,
                    },
                ) if record == sealed_record => CheckedRecordPatternOwner::environment(record),
                (
                    PreparedRecordPatternOwner::VariantPayload {
                        payload,
                        semantic_type,
                        field_count,
                    },
                    SealedRecordPatternOwner::VariantPayload {
                        payload: sealed_payload,
                        semantic_type: sealed_semantic_type,
                        field_count: sealed_field_count,
                    },
                ) if payload == sealed_payload
                    && semantic_type == sealed_semantic_type
                    && field_count == sealed_field_count =>
                {
                    CheckedRecordPatternOwner::variant_payload(payload)
                        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?
                }
                _ => return Err(FinalSemanticAnalysisError::InvalidNominalOwner),
            };
            if ty.semantic_identity_digest() != owner.semantic_type() {
                return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
            }
            let rest_matches = match (&prepared_rest, &sealed_record.rest) {
                (PreparedRecordPatternRest::Absent, CheckedRecordPatternRest::Absent)
                | (PreparedRecordPatternRest::Ignore, CheckedRecordPatternRest::Ignore) => true,
                (
                    PreparedRecordPatternRest::Binding(raw),
                    CheckedRecordPatternRest::Binding(checked),
                ) => *raw == checked.raw(),
                _ => false,
            };
            if !rest_matches {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let record =
                CheckedRecordPattern::try_new(owner, fields.into_boxed_slice(), sealed_record.rest)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            Ok(CheckedPattern::new(ty, CheckedPatternResolution::Record(record)).into())
        }
        fact if sealed_record.is_none() => Ok(fact),
        _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
    }
}

fn seal_project_variant_owner(
    prepared: crate::final_analysis::PreparedProjectVariantOwnerSeed,
    project_nominals: &super::nominal_semantic::ProjectNominalSemanticCatalog,
) -> Result<CheckedVariantOwner, FinalSemanticAnalysisError> {
    let (nominal, prepared_cases) = prepared.into_parts();
    let definition = project_nominals
        .get(nominal.identity())
        .filter(|definition| definition.nominal() == &nominal)
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
    let Some(cases) = definition.cases() else {
        return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
    };
    if cases.len() != prepared_cases.len()
        || cases
            .iter()
            .zip(prepared_cases.iter())
            .any(|(accepted, prepared)| {
                accepted.ordinal() != prepared.ordinal()
                    || accepted.project_payload_field() != prepared.payload()
                    || prepared.diagnostic_name() != Some(accepted.diagnostic_name())
            })
    {
        return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
    }
    let cases = cases.iter().map(|case| {
        (
            case.project_payload_field().cloned(),
            Some(case.diagnostic_name().to_owned()),
        )
    });
    CheckedVariantOwner::try_project(nominal, cases)
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)
}

impl FinalSemanticAnalysis {
    /// Borrows the sole runtime projection sealed for one semantic nominal
    /// identity. Entry/runtime consumers cannot reconstruct or reproject a
    /// schema from a role key after final publication.
    pub fn runtime_nominal_projection(
        &self,
        semantic_type: SemanticTypeDigest,
    ) -> Option<&RuntimeProjectNominalProjection> {
        self.runtime_nominals().get_semantic(semantic_type)
    }

    /// Borrows the exact sealed runtime relation for one checked field.
    /// Accepted environment fields deliberately return `None`: they have no
    /// executable project-layout coordinate and must be rejected by the
    /// compiler before runtime semantic fact publication.
    pub fn project_runtime_field<'a>(
        &'a self,
        selection: &CheckedFieldSelection,
    ) -> Result<Option<RuntimeProjectFieldProjection<'a>>, NominalSchemaProjectionError> {
        let CheckedRecordFieldSemanticId::Project(semantic_id) = selection.field() else {
            let CheckedRecordFieldSemanticId::Environment(semantic_id) = selection.field() else {
                unreachable!("checked record field identity is a closed algebra");
            };
            return if selection.runtime_field().is_none()
                && semantic_id
                    == crate::env::nominal::AcceptedEnvironmentFieldSemanticId::issue(
                        selection.owner_type(),
                        selection.declaration_ordinal(),
                        selection.field_type(),
                    )
            {
                Ok(None)
            } else {
                Err(NominalSchemaProjectionError::InvalidProjectFieldRelation {
                    owner: selection.owner_type(),
                    ordinal: selection.declaration_ordinal(),
                })
            };
        };
        let owner = self
            .runtime_nominal_projection(selection.owner_type())
            .filter(|owner| owner.kind() == RuntimeProjectNominalKind::Record)
            .ok_or(NominalSchemaProjectionError::MissingCachedProjection {
                semantic_type: selection.owner_type(),
            })?;
        let field = owner
            .record_field(selection.declaration_ordinal())
            .filter(|field| {
                selection.runtime_field() == Some(field.runtime_field())
                    && selection.field_type() == field.field_type()
                    && semantic_id
                        == AcceptedRecordFieldSemanticId::issue(
                            selection.owner_type(),
                            field.declaration_ordinal(),
                            field.field_type(),
                        )
            })
            .ok_or(NominalSchemaProjectionError::InvalidProjectFieldRelation {
                owner: selection.owner_type(),
                ordinal: selection.declaration_ordinal(),
            })?;
        Ok(Some(RuntimeProjectFieldProjection { owner, field }))
    }

    /// Borrows one runtime nominal only after validating the complete sealed
    /// Entry-role relation. The Entry digest domain remains Entry-owned; the
    /// compiler neither recomputes it nor compares it with the unrelated
    /// runtime layout-hash domain.
    pub fn checked_entry_runtime_nominal(
        &self,
        role: &crate::entry::CheckedNominalRole,
    ) -> Result<&RuntimeProjectNominalProjection, NominalSchemaProjectionError> {
        let semantic_type = role.semantic_type();
        let projection = self
            .runtime_nominal_projection(semantic_type)
            .ok_or(NominalSchemaProjectionError::MissingCachedProjection { semantic_type })?;
        if projection.nominal() != role.runtime_nominal()
            || projection.layout() != role.layout()
            || &crate::entry::nominal_schema_digest(projection.shape()) != role.schema_digest()
        {
            return Err(NominalSchemaProjectionError::InvalidEntryNominalRelation {
                semantic_type,
            });
        }
        Ok(projection)
    }
}

impl CheckedFieldSelection {
    /// Projects this selection only through the final sealed nominal catalog.
    pub fn project_runtime_field<'a>(
        &self,
        analysis: &'a FinalSemanticAnalysis,
    ) -> Result<Option<RuntimeProjectFieldProjection<'a>>, NominalSchemaProjectionError> {
        analysis.project_runtime_field(self)
    }
}

fn runtime_project_nominal_name(id: &ProjectNominalDeclarationId) -> String {
    let local = id
        .owner_path()
        .iter()
        .map(ModuleSegment::as_str)
        .chain(std::iter::once(id.name().as_str()))
        .collect::<Vec<_>>()
        .join(".");
    format!(
        "{}::{}::{local}",
        id.world().package().as_str(),
        id.module()
    )
}

/// Projects the accepted semantic data-shape algebra into the canonical core
/// runtime schema algebra.
///
/// This is the sole cross-layer projection used by Entry and nominal runtime
/// layout construction. Callers must not retain a second shape-to-schema
/// mapping or derive layout identity from presentation text.
#[must_use]
pub fn project_runtime_type_schema(shape: &TypeShape) -> RuntimeTypeSchema {
    match shape {
        TypeShape::Unit => RuntimeTypeSchema::Unit,
        TypeShape::Bool => RuntimeTypeSchema::Bool,
        TypeShape::I8 => RuntimeTypeSchema::I8,
        TypeShape::I16 => RuntimeTypeSchema::I16,
        TypeShape::I32 => RuntimeTypeSchema::I32,
        TypeShape::I64 => RuntimeTypeSchema::I64,
        TypeShape::I128 => RuntimeTypeSchema::I128,
        TypeShape::Isize => RuntimeTypeSchema::ISize,
        TypeShape::U8 => RuntimeTypeSchema::U8,
        TypeShape::U16 => RuntimeTypeSchema::U16,
        TypeShape::U32 => RuntimeTypeSchema::U32,
        TypeShape::U64 => RuntimeTypeSchema::U64,
        TypeShape::U128 => RuntimeTypeSchema::U128,
        TypeShape::Usize => RuntimeTypeSchema::USize,
        TypeShape::F32 => RuntimeTypeSchema::F32,
        TypeShape::F64 => RuntimeTypeSchema::F64,
        TypeShape::String => RuntimeTypeSchema::String,
        TypeShape::Char => RuntimeTypeSchema::Char,
        TypeShape::Bytes { format } => RuntimeTypeSchema::Bytes {
            format: project_bytes_format(*format),
        },
        TypeShape::Option(inner) => {
            RuntimeTypeSchema::Option(Box::new(project_runtime_type_schema(inner)))
        }
        TypeShape::Seq(inner) => {
            RuntimeTypeSchema::Seq(Box::new(project_runtime_type_schema(inner)))
        }
        TypeShape::Map { key, value } => RuntimeTypeSchema::Map {
            key: Box::new(project_runtime_type_schema(key)),
            value: Box::new(project_runtime_type_schema(value)),
        },
        TypeShape::Record {
            name,
            fields,
            policy,
        } => RuntimeTypeSchema::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| RuntimeSchemaField {
                    rust_name: field.rust_name.clone(),
                    wire_name: field.wire_name.clone(),
                    schema: project_runtime_type_schema(&field.shape),
                    has_default: field.has_default,
                    skip: field.skip,
                    bytes_format: field.bytes_format.map(project_bytes_format),
                })
                .collect(),
            deny_unknown_fields: policy.deny_unknown_fields,
        },
        TypeShape::Enum {
            name,
            variants,
            tag,
            repr,
        } => RuntimeTypeSchema::Enum {
            name: name.clone(),
            variants: variants
                .iter()
                .map(|variant| RuntimeSchemaVariant {
                    rust_name: variant.rust_name.clone(),
                    wire_name: variant.wire_name.clone(),
                    payload: variant.payload.as_ref().map(project_runtime_type_schema),
                    discriminant: variant.discriminant,
                })
                .collect(),
            tag: match tag {
                EnumTagStyle::External => RuntimeEnumTagStyle::External,
                EnumTagStyle::Internal { tag } => {
                    RuntimeEnumTagStyle::Internal { tag: tag.clone() }
                }
                EnumTagStyle::Adjacent { tag, content } => RuntimeEnumTagStyle::Adjacent {
                    tag: tag.clone(),
                    content: content.clone(),
                },
            },
            repr: repr.map(project_enum_repr),
        },
        TypeShape::Named(name) => RuntimeTypeSchema::Named(name.clone()),
    }
}

const fn project_bytes_format(format: BytesFormat) -> RuntimeBytesFormat {
    match format {
        BytesFormat::Binary => RuntimeBytesFormat::Binary,
        BytesFormat::Base64 => RuntimeBytesFormat::Base64,
        BytesFormat::Hex => RuntimeBytesFormat::Hex,
        BytesFormat::Array => RuntimeBytesFormat::Array,
    }
}

const fn project_enum_repr(repr: EnumRepr) -> RuntimeEnumRepr {
    match repr {
        EnumRepr::I8 => RuntimeEnumRepr::I8,
        EnumRepr::I16 => RuntimeEnumRepr::I16,
        EnumRepr::I32 => RuntimeEnumRepr::I32,
        EnumRepr::I64 => RuntimeEnumRepr::I64,
        EnumRepr::I128 => RuntimeEnumRepr::I128,
        EnumRepr::Isize => RuntimeEnumRepr::ISize,
        EnumRepr::U8 => RuntimeEnumRepr::U8,
        EnumRepr::U16 => RuntimeEnumRepr::U16,
        EnumRepr::U32 => RuntimeEnumRepr::U32,
        EnumRepr::U64 => RuntimeEnumRepr::U64,
        EnumRepr::U128 => RuntimeEnumRepr::U128,
        EnumRepr::Usize => RuntimeEnumRepr::USize,
    }
}

impl<'a> NominalSchemaExpander<'a> {
    const fn new(
        symbols: &'a ProjectSymbolTable,
        types: &'a BTreeMap<TypeId, TypeKind>,
        control: FinalSemanticAnalysisControl<'a>,
    ) -> Self {
        Self {
            symbols,
            types,
            control,
        }
    }

    fn schema_checked_after_root(
        &self,
        declaration: &ProjectNominalDeclaration,
        arguments: &[TypeKind],
        budget: &mut ProjectionBudget,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        self.schema_with_stack(
            declaration,
            arguments,
            &BTreeMap::new(),
            &mut BTreeSet::new(),
            budget,
        )
    }

    fn schema_with_stack(
        &self,
        declaration: &ProjectNominalDeclaration,
        arguments: &[TypeKind],
        inherited: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
        budget: &mut ProjectionBudget,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        if declaration.type_parameters().len() != arguments.len() {
            return Err(NominalSchemaProjectionError::WrongArity {
                nominal: declaration.id().qualified_name(),
                expected: declaration.type_parameters().len(),
                actual: arguments.len(),
            });
        }
        budget.charge_generic_arguments(arguments.len(), self.control)?;
        if !stack.insert(declaration.id().clone()) {
            return Ok(TypeShape::Named(canonical_nominal_name(declaration.id())));
        }

        let mut substitutions = inherited.clone();
        for (parameter, argument) in declaration.type_parameters().iter().zip(arguments) {
            substitutions.insert(
                GenericTypeParameterId::new(
                    GenericParameterOwnerId::Nominal(declaration.id().clone()),
                    parameter.ordinal(),
                ),
                argument.clone(),
            );
        }

        let result = match declaration.body() {
            ProjectNominalBody::Struct { fields } => fields
                .iter()
                .enumerate()
                .map(|(ordinal, field)| {
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
                    self.resolved_shape(field.ty(), &substitutions, stack, budget)
                        .map_err(|error| {
                            error.within_step(NominalSchemaPathStep::Field {
                                ordinal,
                                name: field.name().clone(),
                            })
                        })
                        .map(|shape| {
                            FieldShape::new(field.name().as_str(), field.name().as_str(), shape)
                        })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|fields| TypeShape::record(canonical_nominal_name(declaration.id()), fields)),
            ProjectNominalBody::Enum { variants } => variants
                .iter()
                .enumerate()
                .map(|(ordinal, variant)| {
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
                    let unit = VariantShape::unit(variant.name().as_str(), variant.name().as_str());
                    let Some(payload) = variant.payload() else {
                        return Ok(unit);
                    };
                    self.resolved_shape(payload, &substitutions, stack, budget)
                        .map_err(|error| {
                            error.within_step(NominalSchemaPathStep::VariantPayload {
                                ordinal,
                                name: variant.name().clone(),
                            })
                        })
                        .map(|shape| unit.with_payload(shape))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|variants| {
                    TypeShape::enumeration(canonical_nominal_name(declaration.id()), variants)
                }),
            ProjectNominalBody::TypeAlias { .. } => Err(NominalSchemaProjectionError::new(
                "entry data schemas must start from a project struct or enum, not an alias",
            )),
        };

        stack.remove(declaration.id());
        result
    }

    fn resolved_shape(
        &self,
        root: TypeId,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
        budget: &mut ProjectionBudget,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        let ty = self
            .types
            .get(&root)
            .ok_or(NominalSchemaProjectionError::MissingTypeFact { ty: root })?;
        self.type_shape(ty, substitutions, stack, &mut BTreeSet::new(), budget)
    }

    fn type_shape(
        &self,
        ty: &TypeKind,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
        generic_stack: &mut BTreeSet<GenericTypeParameterId>,
        budget: &mut ProjectionBudget,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        budget.enter_node(self.control)?;
        let result = self.type_shape_inner(ty, substitutions, stack, generic_stack, budget);
        budget.leave_node();
        result
    }

    #[expect(
        clippy::too_many_lines,
        reason = "schema projection exhaustively maps the closed checked type vocabulary"
    )]
    fn type_shape_inner(
        &self,
        ty: &TypeKind,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
        stack: &mut BTreeSet<ProjectNominalDeclarationId>,
        generic_stack: &mut BTreeSet<GenericTypeParameterId>,
        budget: &mut ProjectionBudget,
    ) -> Result<TypeShape, NominalSchemaProjectionError> {
        Ok(match ty {
            TypeKind::Unit => TypeShape::Unit,
            TypeKind::Bool => TypeShape::Bool,
            TypeKind::I8 => TypeShape::I8,
            TypeKind::I16 => TypeShape::I16,
            TypeKind::I32 => TypeShape::I32,
            TypeKind::I64 => TypeShape::I64,
            TypeKind::I128 => TypeShape::I128,
            TypeKind::ISize => TypeShape::Isize,
            TypeKind::U8 => TypeShape::U8,
            TypeKind::U16 => TypeShape::U16,
            TypeKind::U32 => TypeShape::U32,
            TypeKind::U64 => TypeShape::U64,
            TypeKind::U128 => TypeShape::U128,
            TypeKind::USize => TypeShape::Usize,
            TypeKind::F32 => TypeShape::F32,
            TypeKind::F64 => TypeShape::F64,
            TypeKind::String => TypeShape::String,
            TypeKind::Char => TypeShape::Char,
            TypeKind::Bytes => TypeShape::Bytes {
                format: BytesFormat::Binary,
            },
            TypeKind::Option(inner) => TypeShape::option(
                self.type_shape(inner, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::OptionItem))?,
            ),
            TypeKind::Vec(inner) | TypeKind::Seq(inner) => TypeShape::seq(
                self.type_shape(inner, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::SequenceItem))?,
            ),
            TypeKind::Map {
                kind: MapKind::Ordered | MapKind::Sorted | MapKind::BTree,
                key,
                value,
            } => TypeShape::map(
                self.type_shape(key, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::MapKey))?,
                self.type_shape(value, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::MapValue))?,
            ),
            TypeKind::ProjectNominal(nominal) => {
                let declaration = self.symbols.nominal(nominal.declaration()).ok_or_else(|| {
                    NominalSchemaProjectionError::MissingDeclaration {
                        nominal: nominal.declaration().qualified_name(),
                    }
                })?;
                self.schema_with_stack(
                    declaration,
                    nominal.arguments(),
                    substitutions,
                    stack,
                    budget,
                )
                .map_err(|error| {
                    error.within_step(NominalSchemaPathStep::NestedNominal {
                        declaration: nominal.declaration().clone(),
                    })
                })?
            }
            TypeKind::AcceptedNominal(nominal) => {
                return Err(NominalSchemaProjectionError::OpaqueLeaf {
                    path: NominalSchemaPath::default(),
                    nominal: nominal.declaration().clone(),
                    semantic_identity: ty.semantic_identity_digest(),
                });
            }
            TypeKind::GenericParam(parameter) => {
                if !generic_stack.insert(parameter.clone()) {
                    return Err(NominalSchemaProjectionError::CyclicGenericSubstitution {
                        path: NominalSchemaPath::default(),
                        parameter: parameter.clone(),
                    });
                }
                let replacement = substitutions.get(parameter).ok_or_else(|| {
                    NominalSchemaProjectionError::new(format!(
                        "unbound generic parameter #{} in checked data schema",
                        parameter.ordinal()
                    ))
                })?;
                let shape =
                    self.type_shape(replacement, substitutions, stack, generic_stack, budget)?;
                generic_stack.remove(parameter);
                shape
            }
            TypeKind::Error(poison) => {
                return Err(NominalSchemaProjectionError::new(format!(
                    "poisoned type {} cannot define a persisted data schema",
                    poison.index()
                )));
            }
            TypeKind::Result { ok, error } => {
                self.type_shape(ok, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::ResultOk))?;
                self.type_shape(error, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::ResultError))?;
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(ty.clone()),
                });
            }
            TypeKind::Tuple(items) => {
                for (ordinal, item) in items.iter().enumerate() {
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| NominalSchemaProjectionError::ArithmeticOverflow)?;
                    self.type_shape(item, substitutions, stack, generic_stack, budget)
                        .map_err(|error| {
                            error.within_step(NominalSchemaPathStep::TupleItem { ordinal })
                        })?;
                }
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(ty.clone()),
                });
            }
            TypeKind::Array { item, .. } | TypeKind::Slice(item) => {
                self.type_shape(item, substitutions, stack, generic_stack, budget)
                    .map_err(|error| error.within_step(NominalSchemaPathStep::SequenceItem))?;
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(ty.clone()),
                });
            }
            unsupported => {
                return Err(NominalSchemaProjectionError::UnsupportedLeaf {
                    path: NominalSchemaPath::default(),
                    ty: Box::new(unsupported.clone()),
                });
            }
        })
    }
}

fn canonical_nominal_name(id: &ProjectNominalDeclarationId) -> String {
    let kind = match id.kind() {
        ProjectNominalDeclarationKind::Struct => "struct",
        ProjectNominalDeclarationKind::Enum => "enum",
        ProjectNominalDeclarationKind::TypeAlias => "type_alias",
    };
    format!(
        "package={};module={};kind={kind};name={}",
        id.world().package(),
        id.module(),
        id.name()
    )
}

#[cfg(test)]
#[path = "nominal_schema/tests.rs"]
mod tests;
