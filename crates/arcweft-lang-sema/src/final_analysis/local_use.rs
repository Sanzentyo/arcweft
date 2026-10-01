//! Generation-bound ownership of executable local reads.
//!
//! The selected expression graph supplies the only executable child order.
//! This seal adds path-sensitive availability and distinguishes a deep Copy
//! carrier from a value which must transfer its one owner.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_core::value::RuntimeOpaqueValueClass;
use arcweft_dialogue::{
    CharacterDialoguePolicyTypeGraph, CharacterDialoguePolicyTypeSchema,
    CharacterDialoguePolicyVariantOwner,
};
use arcweft_lang_hir::{
    body_edges::{HirBodyChild, HirBodyProjection},
    expr::{HirBinaryOp, HirExprKind, HirExpressionOwnedBodyRole, HirExpressionOwnedChild},
    identity::{ExprId, LocalId, PatternId, StmtId},
    module::HirModule,
    pattern::{
        HirPatternBinding, HirPatternChild, HirPatternField, HirPatternKind,
        HirVariantPatternPayload,
    },
    project::{
        AcceptedHirProjectGeneration, HirAnalysisProjectView, HirBindingSite,
        HirDeclarationAttachedContentRootChild, HirDeclarationBodyTopology,
        HirDeclarationEvaluationPhase, HirDeclarationParameterRootChild, HirExpressionBindingRole,
        HirSemanticPathOwnerId,
    },
    stmt::{
        HirConditionalElseBranch, HirContextualStmtBody, HirStmtEvaluationPlan,
        HirStmtMatchArmBody, HirStmtOrderedPairPlanKind, HirStmtSelectEvaluationPlan,
        HirStmtSelectHeadEvaluation, HirStmtValuePlanKind,
    },
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};
use thiserror::Error;

use crate::callable::{
    CallableCandidateId, CallableInstantiationDigest, CheckedCallArgumentSlotSource,
    CheckedCallOperandDestination, CheckedCallReceiverProjection, CheckedCapacityOperation,
    CheckedProjectFunctionInstanceSolution, LineScheduleCallableId, StageMethodId,
};
use crate::checked_rich_text::{
    CheckedDialogueToken, CheckedDisplayConformance, CheckedDisplayInstantiationError,
    CheckedRichTextReport,
};
use crate::types::{SemanticTypeDigest, TypeInstantiationError, TypeKind, VariantPayloadShape};

use super::{
    CheckedExpressionResolution, CheckedImplicitCallableIdentity, CheckedLocalUseSite,
    CheckedPipeBindingIdentity, CheckedRecordValueSource, CheckedSelectResolution,
    CheckedValueResolution, FinalSemanticAnalysis,
};

mod access;
pub use access::{CheckedLocalAccess, CheckedLocalPlaceAccess, CheckedLocalPlaceMode};

/// A builder-issued runtime local with no HIR LocalId. The checked semantic
/// identity prevents two placeholders with the same spelling from aliasing.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedSyntheticUseOwner {
    Pipe(CheckedPipeBindingIdentity),
    ImplicitParameter(CheckedImplicitCallableIdentity),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedSyntheticUse {
    owner: CheckedSyntheticUseOwner,
    mode: CheckedLocalReadMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedSyntheticCopyRequirement {
    callable: CheckedImplicitCallableIdentity,
    ty: SemanticTypeDigest,
}

impl CheckedSyntheticCopyRequirement {
    pub const fn callable(self) -> CheckedImplicitCallableIdentity {
        self.callable
    }
    pub const fn ty(self) -> SemanticTypeDigest {
        self.ty
    }
}

impl CheckedSyntheticUse {
    pub const fn owner(self) -> CheckedSyntheticUseOwner {
        self.owner
    }
    pub const fn mode(self) -> CheckedLocalReadMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedLocalReadMode {
    Copy,
    Move,
    /// A selected operation observes the live local without producing a value
    /// carrier. This is only issued for a direct StageActor receiver of Look.
    Borrow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedLocalValueTransfer {
    local: LocalId,
    mode: CheckedLocalReadMode,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedIngressParameterCoordinate {
    Parameter { group: u32, parameter: u32 },
    AttachedContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedLocalCopyIngressOwner {
    Declaration {
        declaration: CallableDeclarationKey,
        parameter: CheckedIngressParameterCoordinate,
    },
    Closure {
        closure: ExprId,
        parameter: u32,
    },
}

/// A body may Copy this parameter only after exact call ingress proves its
/// complete supplied runtime carrier unrestricted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocalCopyRequirement {
    owner: CheckedLocalCopyIngressOwner,
    local: LocalId,
    ty: SemanticTypeDigest,
}

impl CheckedLocalCopyRequirement {
    pub const fn owner(&self) -> &CheckedLocalCopyIngressOwner {
        &self.owner
    }
    pub const fn local(&self) -> LocalId {
        self.local
    }
    pub const fn ty(&self) -> SemanticTypeDigest {
        self.ty
    }
}

/// Exact evidence that a local's complete runtime carrier is unrestricted.
/// An initializer proof is issued only for an immutable binding; no caller
/// may infer Copy from a function type without this selected source owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedLocalCopyEvidence {
    StructuralType {
        ty: crate::types::SemanticTypeDigest,
    },
    ImmutableInitializer {
        ty: crate::types::SemanticTypeDigest,
        expression: ExprId,
    },
}

impl CheckedLocalCopyEvidence {
    pub const fn ty(self) -> crate::types::SemanticTypeDigest {
        match self {
            Self::StructuralType { ty } | Self::ImmutableInitializer { ty, .. } => ty,
        }
    }
}

impl CheckedLocalValueTransfer {
    pub const fn local(self) -> LocalId {
        self.local
    }
    pub const fn mode(self) -> CheckedLocalReadMode {
        self.mode
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedLocalUseError {
    #[error("local-use closed instance is not bound to this declaration/generation")]
    ForeignInstance,
    #[error(transparent)]
    TypeInstantiation(#[from] TypeInstantiationError),
    #[error(transparent)]
    DisplayInstantiation(#[from] CheckedDisplayInstantiationError),
    #[error("local {local:?} is unavailable at {site:?}")]
    Unavailable {
        local: LocalId,
        site: CheckedLocalUseSite,
    },
    #[error("local {local:?} cannot be moved across a repeated loop at {site:?}")]
    RepeatedLoopMove {
        local: LocalId,
        site: CheckedLocalUseSite,
    },
    #[error("index expression {expression:?} requires Copy item type {ty:?}")]
    IndexRequiresCopy {
        expression: ExprId,
        ty: SemanticTypeDigest,
    },
    #[error("synthetic value {owner:?} is unavailable at expression {expression:?}")]
    SyntheticUnavailable {
        owner: CheckedSyntheticUseOwner,
        expression: ExprId,
    },
    #[error("local-use source topology is inconsistent with checked facts")]
    InvalidTopology,
    #[error("local-use site {site:?} was projected twice")]
    DuplicateSite { site: CheckedLocalUseSite },
    #[error("pattern {pattern:?} binds one affine value both whole and through a nested field")]
    PatternOverlap { pattern: PatternId },
    #[error("guard {guard:?} cannot read affine pattern binding {local:?} at {site:?}")]
    GuardBoundAffineRead {
        guard: ExprId,
        local: LocalId,
        site: CheckedLocalUseSite,
    },
    #[error("guard {guard:?} cannot mutate pattern binding {local:?} at {site:?}")]
    GuardBoundMutation {
        guard: ExprId,
        local: LocalId,
        site: CheckedLocalUseSite,
    },
    #[error(
        "borrowed receiver {receiver:?} keeps local {local:?} live across operand evaluation at {site:?}"
    )]
    BorrowedReceiverInvalidation {
        receiver: ExprId,
        local: LocalId,
        site: CheckedLocalUseSite,
    },
}

/// Immutable local-access authority for one accepted HIR generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocalUseCatalog {
    generation: Arc<AcceptedHirProjectGeneration>,
    uses: BTreeMap<CheckedLocalUseSite, CheckedLocalAccess>,
    synthetic_uses: BTreeMap<ExprId, CheckedSyntheticUse>,
    copy_bindings: BTreeMap<LocalId, CheckedLocalCopyEvidence>,
    copy_requirements: BTreeMap<LocalId, CheckedLocalCopyRequirement>,
    guard_copy_locals: BTreeMap<ExprId, BTreeSet<LocalId>>,
    synthetic_copy_requirements:
        BTreeMap<CheckedImplicitCallableIdentity, CheckedSyntheticCopyRequirement>,
}

/// The frozen substitution used to close one executable body and its nested
/// closures before publishing local transfer modes.
#[derive(Clone, Copy)]
pub enum CheckedLocalUseInstantiation<'a> {
    ProjectFunction(&'a CheckedProjectFunctionInstanceSolution),
    DisplayText(&'a CheckedDisplayConformance),
}

impl CheckedLocalUseInstantiation<'_> {
    pub(super) fn declaration(self) -> CallableDeclarationKey {
        match self {
            Self::ProjectFunction(instance) => instance.declaration().clone(),
            Self::DisplayText(instance) => {
                CallableDeclarationKey::ImplMethod(instance.method_declaration().clone())
            }
        }
    }

    pub(super) fn instantiate_type(self, ty: &TypeKind) -> Result<TypeKind, CheckedLocalUseError> {
        match self {
            Self::ProjectFunction(instance) => Ok(instance.instantiate_type(ty)?),
            Self::DisplayText(instance) => Ok(instance.instantiate_type(ty)?),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedLocalUseInstanceIdentity {
    ProjectFunction {
        declaration: CallableDeclarationKey,
        instantiation: CallableInstantiationDigest,
    },
    DisplayText {
        declaration: CallableDeclarationKey,
        self_type: SemanticTypeDigest,
    },
}

/// Complete local-use rows for one closed executable declaration, including
/// every nested closure under its frozen substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocalUseInstanceCatalog {
    identity: CheckedLocalUseInstanceIdentity,
    catalog: CheckedLocalUseCatalog,
}

impl CheckedLocalUseInstanceCatalog {
    pub(super) const fn catalog(&self) -> &CheckedLocalUseCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> &CheckedLocalUseInstanceIdentity {
        &self.identity
    }

    pub const fn generation(&self) -> &Arc<AcceptedHirProjectGeneration> {
        self.catalog.generation()
    }

    pub fn rows(
        &self,
    ) -> impl ExactSizeIterator<Item = (CheckedLocalUseSite, &CheckedLocalAccess)> + '_ {
        self.catalog.rows()
    }

    pub fn value_transfers(
        &self,
    ) -> impl Iterator<Item = (CheckedLocalUseSite, CheckedLocalValueTransfer)> + '_ {
        self.catalog.value_transfers()
    }

    pub fn synthetic_rows(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExprId, CheckedSyntheticUse)> + '_ {
        self.catalog.synthetic_rows()
    }

    pub fn copy_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (LocalId, CheckedLocalCopyEvidence)> + '_ {
        self.catalog.copy_bindings()
    }

    pub fn copy_requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = &CheckedLocalCopyRequirement> + '_ {
        self.catalog.copy_requirements()
    }

    pub fn synthetic_copy_requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = CheckedSyntheticCopyRequirement> + '_ {
        self.catalog.synthetic_copy_requirements()
    }

    pub fn synthetic_copy_requirement(
        &self,
        callable: CheckedImplicitCallableIdentity,
    ) -> Option<CheckedSyntheticCopyRequirement> {
        self.catalog.synthetic_copy_requirement(callable)
    }

    pub fn copy_requirement(&self, local: LocalId) -> Option<&CheckedLocalCopyRequirement> {
        self.catalog.copy_requirement(local)
    }

    pub fn guard_copy_locals(&self, guard: ExprId) -> impl Iterator<Item = LocalId> + '_ {
        self.catalog.guard_copy_locals(guard)
    }

    pub fn access_at(&self, site: CheckedLocalUseSite) -> Option<&CheckedLocalAccess> {
        self.catalog.access_at(site)
    }

    pub fn value_transfer_at(
        &self,
        site: CheckedLocalUseSite,
    ) -> Option<CheckedLocalValueTransfer> {
        self.catalog.value_transfer_at(site)
    }

    pub fn synthetic_at(&self, expression: ExprId) -> Option<CheckedSyntheticUse> {
        self.catalog.synthetic_at(expression)
    }

    pub fn captures_at(
        &self,
        owner: ExprId,
    ) -> impl Iterator<Item = CheckedLocalValueTransfer> + '_ {
        self.catalog.captures_at(owner)
    }
}

impl CheckedLocalUseCatalog {
    pub(crate) fn empty(generation: Arc<AcceptedHirProjectGeneration>) -> Self {
        Self {
            generation,
            uses: BTreeMap::new(),
            synthetic_uses: BTreeMap::new(),
            copy_bindings: BTreeMap::new(),
            copy_requirements: BTreeMap::new(),
            guard_copy_locals: BTreeMap::new(),
            synthetic_copy_requirements: BTreeMap::new(),
        }
    }

    pub const fn generation(&self) -> &Arc<AcceptedHirProjectGeneration> {
        &self.generation
    }

    pub fn access_at(&self, site: CheckedLocalUseSite) -> Option<&CheckedLocalAccess> {
        self.uses.get(&site)
    }

    pub fn value_transfer_at(
        &self,
        site: CheckedLocalUseSite,
    ) -> Option<CheckedLocalValueTransfer> {
        self.access_at(site)?.value_transfer()
    }

    pub fn read_at(&self, expression: ExprId) -> Option<CheckedLocalValueTransfer> {
        self.value_transfer_at(CheckedLocalUseSite::Expression(expression))
    }

    pub fn rows(
        &self,
    ) -> impl ExactSizeIterator<Item = (CheckedLocalUseSite, &CheckedLocalAccess)> + '_ {
        self.uses.iter().map(|(site, row)| (*site, row))
    }

    pub fn value_transfers(
        &self,
    ) -> impl Iterator<Item = (CheckedLocalUseSite, CheckedLocalValueTransfer)> + '_ {
        self.rows()
            .filter_map(|(site, access)| access.value_transfer().map(|transfer| (site, transfer)))
    }

    /// One callback's complete checked capture inventory, in stable LocalId
    /// order. Runtime frame slots are keyed by the local identity, so no
    /// source-order reconstruction is required by scheduled-call lowering.
    pub fn captures_at(
        &self,
        owner: ExprId,
    ) -> impl Iterator<Item = CheckedLocalValueTransfer> + '_ {
        self.uses.iter().filter_map(move |(site, row)| {
            matches!(site, CheckedLocalUseSite::Capture { owner: site_owner, .. } if *site_owner == owner)
                .then(|| row.value_transfer()).flatten()
        })
    }

    pub fn synthetic_at(&self, expression: ExprId) -> Option<CheckedSyntheticUse> {
        self.synthetic_uses.get(&expression).copied()
    }

    pub fn synthetic_rows(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExprId, CheckedSyntheticUse)> + '_ {
        self.synthetic_uses
            .iter()
            .map(|(owner, row)| (*owner, *row))
    }

    pub fn copy_evidence(&self, local: LocalId) -> Option<CheckedLocalCopyEvidence> {
        self.copy_bindings.get(&local).copied()
    }

    pub fn copy_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (LocalId, CheckedLocalCopyEvidence)> + '_ {
        self.copy_bindings
            .iter()
            .map(|(local, evidence)| (*local, *evidence))
    }

    pub fn copy_requirement(&self, local: LocalId) -> Option<&CheckedLocalCopyRequirement> {
        self.copy_requirements.get(&local)
    }

    /// Selected pattern-bound locals read by this guard. The runtime must
    /// prove each exact bound carrier deeply unrestricted before duplication.
    pub fn guard_copy_locals(&self, guard: ExprId) -> impl Iterator<Item = LocalId> + '_ {
        self.guard_copy_locals
            .get(&guard)
            .into_iter()
            .flat_map(|locals| locals.iter().copied())
    }

    pub fn copy_requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = &CheckedLocalCopyRequirement> + '_ {
        self.copy_requirements.values()
    }

    pub fn synthetic_copy_requirement(
        &self,
        callable: CheckedImplicitCallableIdentity,
    ) -> Option<CheckedSyntheticCopyRequirement> {
        self.synthetic_copy_requirements.get(&callable).copied()
    }

    pub fn synthetic_copy_requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = CheckedSyntheticCopyRequirement> + '_ {
        self.synthetic_copy_requirements.values().copied()
    }

    pub(crate) fn seal(
        analysis: &FinalSemanticAnalysis,
        project: HirAnalysisProjectView<'_>,
    ) -> Result<Self, CheckedLocalUseError> {
        let mut result = Self::empty(Arc::clone(analysis.hir_generation()));
        let effect_sites =
            effect_capture_sites(analysis, analysis.expressions().map(|(owner, _)| owner))?;
        let scheduled_sites =
            selected_scheduled_callback_roots(analysis, analysis.calls().map(|(owner, _)| owner))?;
        let in_place_receivers =
            selected_in_place_receivers(analysis, analysis.calls().map(|(owner, _)| owner))?;
        let borrowed_receivers =
            selected_borrowed_receivers(analysis, analysis.calls().map(|(owner, _)| owner))?;
        for topology in analysis.hir_topology().modules() {
            let module = project
                .modules()
                .find_map(|(_, module)| (module.module_id() == topology.module()).then_some(module))
                .ok_or(CheckedLocalUseError::InvalidTopology)?;
            let mut checker = LocalUseChecker::new(
                analysis,
                module.as_ref(),
                &mut result.uses,
                &mut result.synthetic_uses,
                &mut result.copy_requirements,
                &mut result.guard_copy_locals,
                &mut result.synthetic_copy_requirements,
                &effect_sites,
                &scheduled_sites,
                &in_place_receivers,
                &borrowed_receivers,
                None,
            );
            let open_declarations = topology
                .entries()
                .iter()
                .filter_map(|entry| entry.body())
                .filter(|body| declaration_has_open_local_types(analysis, body))
                .collect::<Vec<_>>();
            for (pattern, _) in analysis
                .patterns()
                .filter(|(pattern, _)| pattern.module() == topology.module())
            {
                if open_declarations
                    .iter()
                    .any(|body| body.paths().pattern(pattern).is_some())
                {
                    continue;
                }
                checker.check_pattern_overlap(pattern)?;
            }
            for (local, _) in analysis
                .locals()
                .filter(|(local, _)| local.module() == topology.module())
            {
                if open_declarations
                    .iter()
                    .any(|body| body.paths().local(local).is_some())
                {
                    continue;
                }
                if let Some(evidence) =
                    checker.binding_copy_evidence(local, &mut BTreeSet::new())?
                {
                    result.copy_bindings.insert(local, evidence);
                }
            }
            for entry in topology.entries() {
                let mut item_state = Availability::root();
                for root in entry.roots() {
                    checker.body(root.projection(), &mut item_state)?;
                }
                if let Some(body) = entry.body() {
                    if !open_declarations
                        .iter()
                        .any(|open| std::ptr::eq(*open, body))
                    {
                        checker.declaration(body)?;
                    }
                }
            }
        }
        Ok(result)
    }

    pub(crate) fn seal_closed_instance(
        analysis: &FinalSemanticAnalysis,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        instance: CheckedLocalUseInstantiation<'_>,
    ) -> Result<CheckedLocalUseInstanceCatalog, CheckedLocalUseError> {
        analysis
            .validate_generation(project, symbols)
            .map_err(|_| CheckedLocalUseError::ForeignInstance)?;
        let admitted = match instance {
            CheckedLocalUseInstantiation::ProjectFunction(solution) => solution
                .validate_authority(analysis.checked_callables())
                .is_ok(),
            CheckedLocalUseInstantiation::DisplayText(conformance) => {
                conformance.admits_authority(analysis.checked_callables())
            }
        };
        if !admitted {
            return Err(CheckedLocalUseError::ForeignInstance);
        }
        let declaration = instance.declaration();
        let view = analysis
            .hir_topology()
            .declaration(&declaration)
            .map_err(|_| CheckedLocalUseError::ForeignInstance)?;
        let module = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == view.module().module()).then_some(module)
            })
            .ok_or(CheckedLocalUseError::ForeignInstance)?;
        let identity = match instance {
            CheckedLocalUseInstantiation::ProjectFunction(solution) => {
                CheckedLocalUseInstanceIdentity::ProjectFunction {
                    declaration: declaration.clone(),
                    instantiation: solution.instantiation(),
                }
            }
            CheckedLocalUseInstantiation::DisplayText(conformance) => {
                CheckedLocalUseInstanceIdentity::DisplayText {
                    declaration: declaration.clone(),
                    self_type: conformance
                        .target()
                        .semantic_identity_digest()
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?,
                }
            }
        };
        let mut result = Self::empty(Arc::clone(analysis.hir_generation()));
        let scope = view.body().paths();
        let effect_sites = effect_capture_sites(analysis, scope.expression_owners())?;
        let scheduled_sites =
            selected_scheduled_callback_roots(analysis, scope.expression_owners())?;
        let in_place_receivers = selected_in_place_receivers(analysis, scope.expression_owners())?;
        let borrowed_receivers = selected_borrowed_receivers(analysis, scope.expression_owners())?;
        let mut checker = LocalUseChecker::new(
            analysis,
            module.as_ref(),
            &mut result.uses,
            &mut result.synthetic_uses,
            &mut result.copy_requirements,
            &mut result.guard_copy_locals,
            &mut result.synthetic_copy_requirements,
            &effect_sites,
            &scheduled_sites,
            &in_place_receivers,
            &borrowed_receivers,
            Some(instance),
        );
        for pattern in scope.pattern_owners() {
            if analysis.pattern(pattern).is_some() {
                checker.check_pattern_overlap(pattern)?;
            }
        }
        for (local, _) in scope.locals() {
            if analysis.local(local).is_some()
                && let Some(evidence) =
                    checker.binding_copy_evidence(local, &mut BTreeSet::new())?
            {
                result.copy_bindings.insert(local, evidence);
            }
        }
        checker.declaration(view.body())?;
        Ok(CheckedLocalUseInstanceCatalog {
            identity,
            catalog: result,
        })
    }
}

/// Definite unavailability is the union of moves on all reachable incoming
/// paths. Source bindings are arena-unique; a new local never revives an older
/// binding merely because it has the same written name.
#[derive(Clone, Default)]
struct Availability {
    moved: BTreeMap<LocalId, BTreeSet<CheckedLocalUseSite>>,
    moved_synthetic: BTreeMap<CheckedSyntheticUseOwner, BTreeSet<ExprId>>,
    reachable: bool,
}

impl Availability {
    fn root() -> Self {
        Self {
            reachable: true,
            ..Self::default()
        }
    }

    fn join(mut self, other: Self) -> Self {
        match (self.reachable, other.reachable) {
            (true, true) => {
                for (local, sites) in other.moved {
                    self.moved.entry(local).or_default().extend(sites);
                }
                for (owner, sites) in other.moved_synthetic {
                    self.moved_synthetic.entry(owner).or_default().extend(sites);
                }
                self
            }
            (false, true) => other,
            (true, false) => self,
            (false, false) => self,
        }
    }
}

struct LocalUseChecker<'a> {
    analysis: &'a FinalSemanticAnalysis,
    module: &'a HirModule,
    rows: &'a mut BTreeMap<CheckedLocalUseSite, CheckedLocalAccess>,
    synthetic_rows: &'a mut BTreeMap<ExprId, CheckedSyntheticUse>,
    copy_requirements: &'a mut BTreeMap<LocalId, CheckedLocalCopyRequirement>,
    guard_copy_locals: &'a mut BTreeMap<ExprId, BTreeSet<LocalId>>,
    synthetic_copy_requirements:
        &'a mut BTreeMap<CheckedImplicitCallableIdentity, CheckedSyntheticCopyRequirement>,
    effect_capture_sites: &'a BTreeMap<ExprId, Box<[LocalId]>>,
    scheduled_callback_roots: &'a BTreeSet<ExprId>,
    in_place_receivers: &'a BTreeSet<ExprId>,
    borrowed_receivers: &'a BTreeSet<ExprId>,
    active_receiver_loans: Vec<(LocalId, ExprId)>,
    callback_local_uses: Vec<BTreeSet<LocalId>>,
    instance: Option<CheckedLocalUseInstantiation<'a>>,
    repeated_loop: Vec<BTreeSet<LocalId>>,
    guard_bindings: Vec<(ExprId, BTreeSet<LocalId>)>,
}

impl<'a> LocalUseChecker<'a> {
    fn new(
        analysis: &'a FinalSemanticAnalysis,
        module: &'a HirModule,
        rows: &'a mut BTreeMap<CheckedLocalUseSite, CheckedLocalAccess>,
        synthetic_rows: &'a mut BTreeMap<ExprId, CheckedSyntheticUse>,
        copy_requirements: &'a mut BTreeMap<LocalId, CheckedLocalCopyRequirement>,
        guard_copy_locals: &'a mut BTreeMap<ExprId, BTreeSet<LocalId>>,
        synthetic_copy_requirements: &'a mut BTreeMap<
            CheckedImplicitCallableIdentity,
            CheckedSyntheticCopyRequirement,
        >,
        effect_capture_sites: &'a BTreeMap<ExprId, Box<[LocalId]>>,
        scheduled_callback_roots: &'a BTreeSet<ExprId>,
        in_place_receivers: &'a BTreeSet<ExprId>,
        borrowed_receivers: &'a BTreeSet<ExprId>,
        instance: Option<CheckedLocalUseInstantiation<'a>>,
    ) -> Self {
        Self {
            analysis,
            module,
            rows,
            synthetic_rows,
            copy_requirements,
            guard_copy_locals,
            synthetic_copy_requirements,
            effect_capture_sites,
            scheduled_callback_roots,
            in_place_receivers,
            borrowed_receivers,
            active_receiver_loans: Vec::new(),
            callback_local_uses: Vec::new(),
            instance,
            repeated_loop: Vec::new(),
            guard_bindings: Vec::new(),
        }
    }

    fn closed_type(&self, ty: &TypeKind) -> Result<TypeKind, CheckedLocalUseError> {
        match self.instance {
            Some(instance) => instance.instantiate_type(ty),
            None => Ok(ty.clone()),
        }
    }

    fn type_is_copy(&self, ty: &TypeKind) -> Result<bool, CheckedLocalUseError> {
        definitely_copy(ty, self.analysis, self.instance, &mut BTreeSet::new())
    }

    fn use_synthetic(
        &mut self,
        expression: ExprId,
        owner: CheckedSyntheticUseOwner,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        let checked = self
            .analysis
            .expression(expression)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let ty = match (owner, checked.resolution()) {
            (
                CheckedSyntheticUseOwner::ImplicitParameter(identity),
                CheckedExpressionResolution::ImplicitCallable(callable),
            ) if callable.identity() == identity => callable.parameter(),
            _ => checked
                .value_type()
                .ok_or(CheckedLocalUseError::InvalidTopology)?,
        };
        let selected_copy = match owner {
            CheckedSyntheticUseOwner::Pipe(binding) => {
                let source = self.analysis.expressions().find_map(|(_, checked)| {
                    match checked.resolution() {
                        CheckedExpressionResolution::Pipe(pipe)
                            if pipe.binding_identity() == binding =>
                        {
                            Some(pipe.lookup_left())
                        }
                        _ => None,
                    }
                });
                self.expression_is_copy(
                    source.ok_or(CheckedLocalUseError::InvalidTopology)?,
                    &mut BTreeSet::new(),
                )?
            }
            // The implicit callable parameter is supplied at each later call
            // site, so a source-specific initializer proof does not exist.
            CheckedSyntheticUseOwner::ImplicitParameter(_) => false,
        };
        let mut mode = if self.type_is_copy(ty)?
            || selected_copy
            || matches!(owner, CheckedSyntheticUseOwner::ImplicitParameter(callable) if self.synthetic_copy_requirements.contains_key(&callable))
        {
            CheckedLocalReadMode::Copy
        } else {
            CheckedLocalReadMode::Move
        };
        if !state.reachable {
            if self
                .synthetic_rows
                .insert(expression, CheckedSyntheticUse { owner, mode })
                .is_some()
            {
                return Err(CheckedLocalUseError::InvalidTopology);
            }
            return Ok(());
        }
        if mode == CheckedLocalReadMode::Move {
            let prior = state.moved_synthetic.get(&owner).cloned();
            if let Some(prior) = prior {
                let closed = self.closed_type(ty)?;
                if let CheckedSyntheticUseOwner::ImplicitParameter(callable) = owner
                    && matches!(
                        closed,
                        TypeKind::Function { .. } | TypeKind::CharacterDialogue(_)
                    )
                {
                    let ty = closed
                        .semantic_identity_digest()
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                    self.synthetic_copy_requirements
                        .insert(callable, CheckedSyntheticCopyRequirement { callable, ty });
                    state.moved_synthetic.remove(&owner);
                    for previous in prior {
                        let row = self
                            .synthetic_rows
                            .get_mut(&previous)
                            .ok_or(CheckedLocalUseError::InvalidTopology)?;
                        row.mode = CheckedLocalReadMode::Copy;
                    }
                    mode = CheckedLocalReadMode::Copy;
                } else {
                    return Err(CheckedLocalUseError::SyntheticUnavailable { owner, expression });
                }
            }
            if mode == CheckedLocalReadMode::Move {
                state
                    .moved_synthetic
                    .entry(owner)
                    .or_default()
                    .insert(expression);
            }
        }
        if self
            .synthetic_rows
            .insert(expression, CheckedSyntheticUse { owner, mode })
            .is_some()
        {
            return Err(CheckedLocalUseError::InvalidTopology);
        }
        Ok(())
    }

    fn use_local(
        &mut self,
        site: CheckedLocalUseSite,
        local: LocalId,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        if let Some(callback) = self.callback_local_uses.last_mut() {
            callback.insert(local);
        }
        if !state.reachable {
            // Lowering still visits dead source after a definite exit. Seal a
            // read mode for it without issuing an availability obligation or
            // making its transfers reachable at a later join.
            let mode = if self.copy_requirements.contains_key(&local)
                || self.binding_is_copy(local, &mut BTreeSet::new())?
            {
                CheckedLocalReadMode::Copy
            } else {
                CheckedLocalReadMode::Move
            };
            if self
                .rows
                .insert(site, CheckedLocalValueTransfer { local, mode }.into())
                .is_some()
            {
                return Err(CheckedLocalUseError::DuplicateSite { site });
            }
            return Ok(());
        }
        let guard_copy = self.require_guard_copy(site, local)?;
        let mut mode = if guard_copy
            || self.copy_requirements.contains_key(&local)
            || self.binding_is_copy(local, &mut BTreeSet::new())?
        {
            CheckedLocalReadMode::Copy
        } else {
            CheckedLocalReadMode::Move
        };
        if mode == CheckedLocalReadMode::Move
            && let Some(&(_, receiver)) = self
                .active_receiver_loans
                .iter()
                .rev()
                .find(|(loan, _)| *loan == local)
        {
            return Err(CheckedLocalUseError::BorrowedReceiverInvalidation {
                receiver,
                local,
                site,
            });
        }
        if mode == CheckedLocalReadMode::Copy {
            if let Some(prior) = state.moved.remove(&local) {
                for previous in prior {
                    let CheckedLocalAccess::ValueTransfer(row) = self
                        .rows
                        .get_mut(&previous)
                        .ok_or(CheckedLocalUseError::InvalidTopology)?
                    else {
                        return Err(CheckedLocalUseError::InvalidTopology);
                    };
                    row.mode = CheckedLocalReadMode::Copy;
                }
            }
        }
        if mode == CheckedLocalReadMode::Move {
            let prior = state.moved.get(&local).cloned();
            let repeated = self
                .repeated_loop
                .iter()
                .any(|fresh| !fresh.contains(&local));
            if prior.is_some() || repeated {
                if let Some(requirement) = self.ingress_copy_requirement(local)? {
                    self.copy_requirements
                        .insert(requirement.local(), requirement);
                    if let Some(prior) = state.moved.remove(&local) {
                        for previous in prior {
                            let CheckedLocalAccess::ValueTransfer(row) = self
                                .rows
                                .get_mut(&previous)
                                .ok_or(CheckedLocalUseError::InvalidTopology)?
                            else {
                                return Err(CheckedLocalUseError::InvalidTopology);
                            };
                            row.mode = CheckedLocalReadMode::Copy;
                        }
                    }
                    mode = CheckedLocalReadMode::Copy;
                } else if prior.is_some() {
                    return Err(CheckedLocalUseError::Unavailable { local, site });
                } else {
                    return Err(CheckedLocalUseError::RepeatedLoopMove { local, site });
                }
            }
            if mode == CheckedLocalReadMode::Move {
                state.moved.entry(local).or_default().insert(site);
            }
        }
        if self
            .rows
            .insert(site, CheckedLocalValueTransfer { local, mode }.into())
            .is_some()
        {
            return Err(CheckedLocalUseError::DuplicateSite { site });
        }
        Ok(())
    }

    fn borrow_local(
        &mut self,
        site: CheckedLocalUseSite,
        local: LocalId,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        if let Some(callback) = self.callback_local_uses.last_mut() {
            callback.insert(local);
        }
        if !state.reachable {
            if self
                .rows
                .insert(
                    site,
                    CheckedLocalValueTransfer {
                        local,
                        mode: CheckedLocalReadMode::Borrow,
                    }
                    .into(),
                )
                .is_some()
            {
                return Err(CheckedLocalUseError::DuplicateSite { site });
            }
            return Ok(());
        }
        self.require_guard_copy(site, local)?;
        if state.moved.contains_key(&local) {
            return Err(CheckedLocalUseError::Unavailable { local, site });
        }
        if self
            .rows
            .insert(
                site,
                CheckedLocalValueTransfer {
                    local,
                    mode: CheckedLocalReadMode::Borrow,
                }
                .into(),
            )
            .is_some()
        {
            return Err(CheckedLocalUseError::DuplicateSite { site });
        }
        Ok(())
    }

    fn access_place(
        &mut self,
        expression: ExprId,
        place: super::CheckedMutablePlace,
        mode: CheckedLocalPlaceMode,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        let site = CheckedLocalUseSite::Place(expression);
        let local = place.local_id();
        if state.reachable
            && let Some(&(_, receiver)) = self
                .active_receiver_loans
                .iter()
                .rev()
                .find(|(loan, _)| *loan == local)
        {
            return Err(CheckedLocalUseError::BorrowedReceiverInvalidation {
                receiver,
                local,
                site,
            });
        }
        if let Some(callback) = self.callback_local_uses.last_mut() {
            callback.insert(local);
        }
        if let Some(guard) = self.guard_for_local(local) {
            return Err(CheckedLocalUseError::GuardBoundMutation { guard, local, site });
        }
        if state.reachable && state.moved.contains_key(&local) {
            return Err(CheckedLocalUseError::Unavailable { local, site });
        }
        if self
            .rows
            .insert(
                site,
                CheckedLocalAccess::PlaceAccess(Box::new(CheckedLocalPlaceAccess::new(
                    place, mode,
                ))),
            )
            .is_some()
        {
            return Err(CheckedLocalUseError::DuplicateSite { site });
        }
        Ok(())
    }

    fn binding_is_copy(
        &self,
        local: LocalId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<bool, CheckedLocalUseError> {
        if self.copy_requirements.contains_key(&local) {
            return Ok(true);
        }
        self.binding_copy_evidence(local, visiting)
            .map(|evidence| evidence.is_some())
    }

    fn ingress_copy_requirement(
        &self,
        local: LocalId,
    ) -> Result<Option<CheckedLocalCopyRequirement>, CheckedLocalUseError> {
        self.ingress_copy_requirement_from(local, &mut BTreeSet::new())
    }

    fn ingress_copy_requirement_from(
        &self,
        local: LocalId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<Option<CheckedLocalCopyRequirement>, CheckedLocalUseError> {
        if !visiting.insert(local) {
            return Ok(None);
        }
        let binding = self
            .analysis
            .local(local)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let closed = self.closed_type(binding.ty())?;
        // These carriers can be unrestricted only when the exact supplied
        // runtime value is deeply unrestricted. Plain accepted opaque values
        // need that ingress proof; affine-handle and resource leaves cannot
        // satisfy it.
        let may_copy_at_ingress = match &closed {
            TypeKind::Function { .. } | TypeKind::CharacterDialogue(_) => true,
            TypeKind::AcceptedNominal(nominal) => {
                self.analysis.accepted_plain_opaque_nominal(nominal)
            }
            _ => false,
        };
        if !may_copy_at_ingress {
            return Ok(None);
        }
        if self
            .module
            .resolve_local(local)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            .is_mutable_binding()
        {
            return Ok(None);
        }
        let origin = self
            .analysis
            .hir_topology()
            .module(local.module())
            .and_then(|module| module.local_origins().binding(local))
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let owner = match origin.site() {
            HirBindingSite::DeclarationParameter { .. }
            | HirBindingSite::DeclarationAttachedContent { .. } => {
                let parameter = match origin.site() {
                    HirBindingSite::DeclarationParameter {
                        group, parameter, ..
                    } => CheckedIngressParameterCoordinate::Parameter { group, parameter },
                    HirBindingSite::DeclarationAttachedContent { .. } => {
                        CheckedIngressParameterCoordinate::AttachedContent
                    }
                    _ => unreachable!("matched declaration ingress above"),
                };
                let declaration = self
                    .analysis
                    .hir_topology()
                    .module(local.module())
                    .and_then(|module| {
                        module.entries().iter().find_map(|entry| {
                            entry.body().and_then(|body| {
                                body.paths()
                                    .local(local)
                                    .is_some()
                                    .then(|| body.declaration().clone())
                            })
                        })
                    })
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                CheckedLocalCopyIngressOwner::Declaration {
                    declaration,
                    parameter,
                }
            }
            HirBindingSite::Expression {
                expression: closure,
                role: HirExpressionBindingRole::ClosureParameter { parameter },
            } => CheckedLocalCopyIngressOwner::Closure { closure, parameter },
            HirBindingSite::Statement { .. } => {
                let Some(initializer) = origin.value() else {
                    return Ok(None);
                };
                let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Local(source))) =
                    self.analysis
                        .expression(initializer)
                        .map(super::CheckedExpression::resolution)
                else {
                    return Ok(None);
                };
                let source_ty = self
                    .analysis
                    .local(*source)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?
                    .ty();
                if self.closed_type(source_ty)? != closed {
                    return Ok(None);
                }
                return self.ingress_copy_requirement_from(*source, visiting);
            }
            _ => return Ok(None),
        };
        let ty = closed
            .semantic_identity_digest()
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        Ok(Some(CheckedLocalCopyRequirement { owner, local, ty }))
    }

    fn binding_copy_evidence(
        &self,
        local: LocalId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<Option<CheckedLocalCopyEvidence>, CheckedLocalUseError> {
        let binding = self
            .analysis
            .local(local)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let ty = self
            .closed_type(binding.ty())?
            .semantic_identity_digest()
            .ok();
        if self.type_is_copy(binding.ty())? {
            return Ok(ty.map(|ty| CheckedLocalCopyEvidence::StructuralType { ty }));
        }
        if !matches!(
            self.closed_type(binding.ty())?,
            TypeKind::Function { .. } | TypeKind::CharacterDialogue(_)
        ) || !visiting.insert(local)
        {
            return Ok(None);
        }
        if self
            .module
            .resolve_local(local)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            .is_mutable_binding()
        {
            visiting.remove(&local);
            return Ok(None);
        }
        let origin = self
            .analysis
            .hir_topology()
            .module(local.module())
            .and_then(|module| module.local_origins().binding(local));
        let result = match (ty, origin.and_then(|origin| origin.value())) {
            (Some(ty), Some(expression)) if self.expression_is_copy(expression, visiting)? => {
                Some(CheckedLocalCopyEvidence::ImmutableInitializer { ty, expression })
            }
            _ => None,
        };
        visiting.remove(&local);
        Ok(result)
    }

    fn expression_is_copy(
        &self,
        owner: ExprId,
        visiting: &mut BTreeSet<LocalId>,
    ) -> Result<bool, CheckedLocalUseError> {
        let expression = self
            .analysis
            .expression(owner)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        if let Some(ty) = expression.value_type() {
            if self.type_is_copy(ty)? {
                return Ok(true);
            }
        }
        match expression.resolution() {
            CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)) => {
                return self.binding_is_copy(*local, visiting);
            }
            CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(_)) => {
                return Ok(true);
            }
            CheckedExpressionResolution::Closure(closure) => {
                return closure.captures().iter().try_fold(true, |copy, capture| {
                    self.binding_is_copy(capture.local(), visiting)
                        .map(|next| copy && next)
                });
            }
            CheckedExpressionResolution::CharacterDialogueFactory(_)
            | CheckedExpressionResolution::CharacterDialogueReconfigure(_) => {}
            _ => {
                if !self
                    .analysis
                    .call(owner)
                    .and_then(|facts| facts.selected_application())
                    .is_some_and(|application| {
                        matches!(
                            application.result(),
                            crate::callable::CheckedCallResult::Continuation(_)
                        )
                    })
                {
                    let hir = self
                        .module
                        .resolve_expr(owner)
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                    return match hir.kind() {
                        HirExprKind::If(branch) => Ok(self
                            .expression_is_copy(branch.then_branch(), visiting)?
                            && self.expression_is_copy(branch.else_branch(), visiting)?),
                        HirExprKind::Block(block) => {
                            self.expression_is_copy(block.tail(), visiting)
                        }
                        HirExprKind::NamedBlock(block) => {
                            self.expression_is_copy(block.tail(), visiting)
                        }
                        _ => Ok(false),
                    };
                }
            }
        }
        let children = self
            .analysis
            .checked_expression_edge_fact(owner)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            .child_expressions()
            .collect::<Vec<_>>();
        for child in children {
            if !self.expression_is_copy(child, visiting)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn bind(&mut self, locals: &[LocalId]) {
        if let Some(fresh) = self.repeated_loop.last_mut() {
            fresh.extend(locals.iter().copied());
        }
    }

    fn pattern_bound_locals(
        &self,
        pattern: PatternId,
    ) -> Result<BTreeSet<LocalId>, CheckedLocalUseError> {
        let mut pending = vec![pattern];
        let mut visited = BTreeSet::new();
        let mut locals = BTreeSet::new();
        while let Some(pattern) = pending.pop() {
            if !visited.insert(pattern) {
                continue;
            }
            let hir = self
                .module
                .resolve_pattern(pattern)
                .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
            for edge in hir
                .kind()
                .try_child_edges()
                .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            {
                match edge.child() {
                    HirPatternChild::Pattern(child) => pending.push(child),
                    HirPatternChild::Local(local) => {
                        locals.insert(local);
                    }
                    HirPatternChild::Type(_) => {}
                }
            }
        }
        Ok(locals)
    }

    fn guard_expression(
        &mut self,
        guard: ExprId,
        locals: impl IntoIterator<Item = LocalId>,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        self.guard_bindings
            .push((guard, locals.into_iter().collect()));
        let result = self.expression(guard, state);
        self.guard_bindings.pop();
        result
    }

    fn require_guard_copy(
        &mut self,
        site: CheckedLocalUseSite,
        local: LocalId,
    ) -> Result<bool, CheckedLocalUseError> {
        let Some(guard) = self.guard_for_local(local) else {
            return Ok(false);
        };
        let ty = self
            .analysis
            .local(local)
            .ok_or(CheckedLocalUseError::InvalidTopology)?
            .ty();
        if !self.binding_is_copy(local, &mut BTreeSet::new())?
            && !self.dynamic_guard_copy_candidate(ty, &mut BTreeSet::new())?
        {
            return Err(CheckedLocalUseError::GuardBoundAffineRead { guard, local, site });
        }
        self.guard_copy_locals
            .entry(guard)
            .or_default()
            .insert(local);
        Ok(true)
    }

    fn guard_for_local(&self, local: LocalId) -> Option<ExprId> {
        self.guard_bindings
            .iter()
            .rev()
            .find_map(|(guard, locals)| locals.contains(&local).then_some(*guard))
    }

    fn dynamic_guard_copy_candidate(
        &self,
        ty: &TypeKind,
        visiting: &mut BTreeSet<SemanticTypeDigest>,
    ) -> Result<bool, CheckedLocalUseError> {
        let closed = self.closed_type(ty)?;
        if self.type_is_copy(&closed)? {
            return Ok(true);
        }
        match &closed {
            TypeKind::Function { .. } | TypeKind::CharacterDialogue(_) => Ok(true),
            TypeKind::Tuple(items) | TypeKind::Choice(items) => {
                for item in items {
                    if !self.dynamic_guard_copy_candidate(item, visiting)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            TypeKind::Vec(item)
            | TypeKind::Array { item, .. }
            | TypeKind::Slice(item)
            | TypeKind::Seq(item)
            | TypeKind::Option(item)
            | TypeKind::Range(item) => self.dynamic_guard_copy_candidate(item, visiting),
            TypeKind::Result { ok, error }
            | TypeKind::Map {
                key: ok,
                value: error,
                ..
            } => Ok(self.dynamic_guard_copy_candidate(ok, visiting)?
                && self.dynamic_guard_copy_candidate(error, visiting)?),
            TypeKind::ProjectNominal(_) => {
                let digest = closed
                    .semantic_identity_digest()
                    .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                if !visiting.insert(digest) {
                    return Ok(false);
                }
                let result = (|| {
                    let Some(definition) = self.analysis.project_nominal_semantic(digest) else {
                        return Ok(false);
                    };
                    if let Some(fields) = definition.fields() {
                        for field in fields {
                            if !self.dynamic_guard_copy_candidate(field.ty(), visiting)? {
                                return Ok(false);
                            }
                        }
                        return Ok(true);
                    }
                    let Some(cases) = definition.cases() else {
                        return Ok(false);
                    };
                    for case in cases {
                        match case.payload() {
                            VariantPayloadShape::Unit => {}
                            VariantPayloadShape::Tuple(fields) => {
                                for field in fields {
                                    if !self.dynamic_guard_copy_candidate(field.ty(), visiting)? {
                                        return Ok(false);
                                    }
                                }
                            }
                            VariantPayloadShape::Record(fields) => {
                                for field in fields {
                                    if !self.dynamic_guard_copy_candidate(field.ty(), visiting)? {
                                        return Ok(false);
                                    }
                                }
                            }
                        }
                    }
                    Ok(true)
                })();
                visiting.remove(&digest);
                result
            }
            _ => Ok(false),
        }
    }

    fn declaration(
        &mut self,
        body: &HirDeclarationBodyTopology,
    ) -> Result<(), CheckedLocalUseError> {
        let mut state = Availability::root();
        for phase in body.phases() {
            match phase {
                HirDeclarationEvaluationPhase::Parameter(root) => {
                    if let HirDeclarationParameterRootChild::Expression(expression) = root.child() {
                        self.expression(expression, &mut state)?;
                    }
                }
                HirDeclarationEvaluationPhase::AttachedContent(root) => {
                    if let HirDeclarationAttachedContentRootChild::Default(expression) =
                        root.child()
                    {
                        self.expression(expression, &mut state)?;
                    }
                }
                HirDeclarationEvaluationPhase::Contract(root) => {
                    // Contract evaluation has its own entry path. It does not
                    // transfer a local into the callable body.
                    self.expression(root.child(), &mut Availability::root())?;
                }
                HirDeclarationEvaluationPhase::Body(root) => {
                    self.body(root.projection(), &mut state)?;
                }
            }
        }
        Ok(())
    }

    fn check_pattern_overlap(&self, pattern: PatternId) -> Result<(), CheckedLocalUseError> {
        let hir = self
            .module
            .resolve_pattern(pattern)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        match hir.kind() {
            HirPatternKind::WholeBinding {
                binding: HirPatternBinding::Bound { local, .. },
                pattern: inner,
            } => {
                let whole = self
                    .analysis
                    .local(*local)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                if !self.type_is_copy(whole.ty())? && self.pattern_has_affine_binding(*inner)? {
                    return Err(CheckedLocalUseError::PatternOverlap { pattern });
                }
            }
            HirPatternKind::Record { fields, .. } => {
                let rest = fields.iter().find_map(|field| match field {
                    HirPatternField::Rest { binding } => *binding,
                    _ => None,
                });
                if let Some(rest) = rest {
                    let whole = self
                        .analysis
                        .local(rest)
                        .ok_or(CheckedLocalUseError::InvalidTopology)?;
                    if !self.type_is_copy(whole.ty())? {
                        for field in fields {
                            let overlaps = match field {
                                HirPatternField::Explicit { pattern, .. } => {
                                    self.pattern_has_affine_binding(*pattern)?
                                }
                                HirPatternField::Shorthand { local, .. } => {
                                    match self.analysis.local(*local) {
                                        Some(binding) => !self.type_is_copy(binding.ty())?,
                                        None => true,
                                    }
                                }
                                HirPatternField::Rest { .. } | HirPatternField::Invalid { .. } => {
                                    false
                                }
                            };
                            if overlaps {
                                return Err(CheckedLocalUseError::PatternOverlap { pattern });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn pattern_has_affine_binding(&self, pattern: PatternId) -> Result<bool, CheckedLocalUseError> {
        let hir = self
            .module
            .resolve_pattern(pattern)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        Ok(match hir.kind() {
            HirPatternKind::Binding(HirPatternBinding::Bound { local, .. })
            | HirPatternKind::MutableBinding(HirPatternBinding::Bound { local, .. })
            | HirPatternKind::TypedBinding {
                binding: HirPatternBinding::Bound { local, .. },
                ..
            } => match self.analysis.local(*local) {
                Some(binding) => !self.type_is_copy(binding.ty())?,
                None => true,
            },
            HirPatternKind::WholeBinding {
                binding,
                pattern: inner,
            } => {
                let whole = match binding {
                    HirPatternBinding::Bound { local, .. } => match self.analysis.local(*local) {
                        Some(binding) => !self.type_is_copy(binding.ty())?,
                        None => true,
                    },
                    HirPatternBinding::Recovered { .. } => false,
                };
                whole || self.pattern_has_affine_binding(*inner)?
            }
            HirPatternKind::Tuple { elements } => {
                elements.iter().try_fold(false, |any, child| {
                    self.pattern_has_affine_binding(*child)
                        .map(|next| any || next)
                })?
            }
            HirPatternKind::Record { fields, .. } => {
                let mut any = false;
                for field in fields {
                    any |= match field {
                        HirPatternField::Explicit { pattern, .. } => {
                            self.pattern_has_affine_binding(*pattern)?
                        }
                        HirPatternField::Shorthand { local, .. }
                        | HirPatternField::Rest {
                            binding: Some(local),
                        } => match self.analysis.local(*local) {
                            Some(binding) => !self.type_is_copy(binding.ty())?,
                            None => true,
                        },
                        HirPatternField::Rest { binding: None }
                        | HirPatternField::Invalid { .. } => false,
                    };
                }
                any
            }
            HirPatternKind::BracketSequence { elements, .. } => {
                elements.iter().try_fold(false, |any, child| {
                    self.pattern_has_affine_binding(*child)
                        .map(|next| any || next)
                })?
            }
            HirPatternKind::Or { alternatives } => {
                alternatives.iter().try_fold(false, |any, child| {
                    self.pattern_has_affine_binding(*child)
                        .map(|next| any || next)
                })?
            }
            HirPatternKind::Variant(variant) => match variant.payload() {
                HirVariantPatternPayload::Pattern(inner) => {
                    self.pattern_has_affine_binding(*inner)?
                }
                HirVariantPatternPayload::Absent | HirVariantPatternPayload::Recovered { .. } => {
                    false
                }
            },
            HirPatternKind::Binding(HirPatternBinding::Recovered { .. })
            | HirPatternKind::MutableBinding(HirPatternBinding::Recovered { .. })
            | HirPatternKind::TypedBinding {
                binding: HirPatternBinding::Recovered { .. },
                ..
            }
            | HirPatternKind::Literal(_)
            | HirPatternKind::EntityReference(_)
            | HirPatternKind::Discard
            | HirPatternKind::Error(_) => false,
        })
    }

    fn body(
        &mut self,
        body: &HirBodyProjection,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        for edge in body.children() {
            match edge.child() {
                HirBodyChild::Expression(expression) => self.expression(expression, state)?,
                HirBodyChild::Statement(statement) => self.statement(statement, state)?,
            }
        }
        Ok(())
    }

    fn contextual_body(
        &mut self,
        body: &HirContextualStmtBody,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        match body {
            HirContextualStmtBody::Ordinary { statements, .. } => {
                for statement in statements.iter().copied() {
                    self.statement(statement, state)?;
                }
            }
            HirContextualStmtBody::Thread(body) => {
                for item in body.items() {
                    match item {
                        arcweft_lang_hir::expr::HirThreadFlowItem::DialogueApplication(
                            expression,
                        ) => self.expression(*expression, state)?,
                        arcweft_lang_hir::expr::HirThreadFlowItem::Statement(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::Choice(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::If(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::IfLet(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::Match(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::While(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::WhileLet(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::For(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::Select(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::SourceLocale(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::Scope(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::Include(statement)
                        | arcweft_lang_hir::expr::HirThreadFlowItem::Error(statement) => {
                            self.statement(*statement, state)?
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn branch(
        &mut self,
        state: &mut Availability,
        left: impl FnOnce(&mut Self, &mut Availability) -> Result<(), CheckedLocalUseError>,
        right: impl FnOnce(&mut Self, &mut Availability) -> Result<(), CheckedLocalUseError>,
    ) -> Result<(), CheckedLocalUseError> {
        let mut then_state = state.clone();
        let mut else_state = state.clone();
        left(self, &mut then_state)?;
        right(self, &mut else_state)?;
        *state = then_state.join(else_state);
        Ok(())
    }

    fn repeat(
        &mut self,
        state: &mut Availability,
        body: impl FnOnce(&mut Self, &mut Availability) -> Result<(), CheckedLocalUseError>,
    ) -> Result<(), CheckedLocalUseError> {
        let mut iteration = state.clone();
        self.repeated_loop.push(BTreeSet::new());
        let result = body(self, &mut iteration);
        self.repeated_loop.pop();
        result?;
        for (local, sites) in iteration.moved {
            state.moved.entry(local).or_default().extend(sites);
        }
        Ok(())
    }

    fn expression(
        &mut self,
        owner: ExprId,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        if self.analysis.expression(owner).is_none() {
            return Ok(());
        }
        if self.in_place_receivers.contains(&owner) {
            // The selected call writes through this direct place. Its target
            // is resolved as a place and never read as a value operand.
            let place = self
                .analysis
                .expression(owner)
                .and_then(super::CheckedExpression::mutable_place)
                .ok_or(CheckedLocalUseError::InvalidTopology)?;
            return self.access_place(owner, place, CheckedLocalPlaceMode::Mutate, state);
        }
        if self.borrowed_receivers.contains(&owner) {
            let inputs = self
                .analysis
                .checked_capture_inputs(owner)
                .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
            let mut sources = inputs.sources();
            let input = sources
                .next()
                .ok_or(CheckedLocalUseError::InvalidTopology)?;
            if sources.next().is_some() {
                return Err(CheckedLocalUseError::InvalidTopology);
            }
            return self.borrow_local(input.site(), input.local(), state);
        }
        if let Some(CheckedExpressionResolution::ImplicitCallable(_)) = self
            .analysis
            .expression(owner)
            .map(super::CheckedExpression::resolution)
        {
            let captures = self
                .analysis
                .checked_capture_inputs(owner)
                .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
            for input in captures.sources() {
                self.use_local(input.site(), input.local(), state)?;
            }
            return self.expression_inner(owner, &mut Availability::root());
        }
        if self.scheduled_callback_roots.contains(&owner)
            && !matches!(
                self.module
                    .resolve_expr(owner)
                    .map_err(|_| CheckedLocalUseError::InvalidTopology)?
                    .kind(),
                HirExprKind::Closure(_)
            )
        {
            self.callback_local_uses.push(BTreeSet::new());
            let result = self.expression_inner(owner, &mut Availability::root());
            let locals = self
                .callback_local_uses
                .pop()
                .ok_or(CheckedLocalUseError::InvalidTopology)?;
            result?;
            for local in scheduled_free_locals(self.analysis, owner, locals)? {
                self.use_local(CheckedLocalUseSite::Capture { owner, local }, local, state)?;
            }
            return Ok(());
        }
        if let Some(captures) = self.effect_capture_sites.get(&owner).cloned() {
            for local in captures {
                self.use_local(CheckedLocalUseSite::Capture { owner, local }, local, state)?;
            }
            // The effect body runs later in its own callback frame. Its HIR
            // expression reads still receive modes, but cannot consume the
            // outer scope a second time after the capture packet is moved.
            let mut callback_state = Availability::root();
            return self.expression_inner(owner, &mut callback_state);
        }
        self.expression_inner(owner, state)
    }

    fn expression_inner(
        &mut self,
        owner: ExprId,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        let expression = self
            .module
            .resolve_expr(owner)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        match expression.kind() {
            HirExprKind::Index(_) => {
                let Ok(edges) = self.analysis.checked_expression_edge_fact(owner) else {
                    return Ok(());
                };
                let children = edges.child_expressions().collect::<Vec<_>>();
                for child in children {
                    self.expression(child, state)?;
                }
                let selected_type = self
                    .analysis
                    .expression(owner)
                    .and_then(super::CheckedExpression::value_type)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                let selected_type = self.closed_type(selected_type)?;
                if !self.type_is_copy(&selected_type)? {
                    let ty = selected_type
                        .semantic_identity_digest()
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                    return Err(CheckedLocalUseError::IndexRequiresCopy {
                        expression: owner,
                        ty,
                    });
                }
            }
            HirExprKind::Await(value) => {
                let Some(CheckedExpressionResolution::Await(checked)) = self
                    .analysis
                    .expression(owner)
                    .map(super::CheckedExpression::resolution)
                else {
                    return Err(CheckedLocalUseError::InvalidTopology);
                };
                if checked.operand() != value.operand()
                    || checked.observers().len() != value.branches().len()
                    || !value.branches().iter().zip(checked.observers()).all(
                        |(branch, observer)| {
                            branch.kind() == arcweft_lang_hir::expr::HirAwaitBranchKind::Pending
                                && branch.pattern() == Some(observer.pattern())
                        },
                    )
                {
                    return Err(CheckedLocalUseError::InvalidTopology);
                }
                self.expression(value.operand(), state)?;
                for branch in value.branches() {
                    self.repeat(state, |this, iteration| {
                        this.bind(branch.locals());
                        this.contextual_body(branch.body(), iteration)
                    })?;
                }
            }
            HirExprKind::If(value) => {
                self.expression(value.condition(), state)?;
                self.branch(
                    state,
                    |this, state| this.expression(value.then_branch(), state),
                    |this, state| this.expression(value.else_branch(), state),
                )?;
            }
            HirExprKind::IfLet(value) => {
                self.expression(value.scrutinee(), state)?;
                let incoming = state.clone();
                let mut matched = incoming.clone();
                let mut unmatched = incoming;
                if let Some(guard) = value.guard() {
                    let locals = self.pattern_bound_locals(value.pattern())?;
                    self.guard_expression(guard, locals, &mut matched)?;
                    unmatched = unmatched.join(matched.clone());
                }
                self.expression(value.then_branch(), &mut matched)?;
                self.expression(value.else_branch(), &mut unmatched)?;
                *state = matched.join(unmatched);
            }
            HirExprKind::Match(value) => {
                self.expression(value.scrutinee(), state)?;
                let mut joined = None;
                let mut fallthrough = state.clone();
                for arm in value.arms() {
                    let mut arm_state = fallthrough.clone();
                    if let Some(guard) = arm.guard() {
                        self.guard_expression(guard, arm.locals().iter().copied(), &mut arm_state)?;
                        fallthrough = fallthrough.join(arm_state.clone());
                    }
                    self.expression(arm.value(), &mut arm_state)?;
                    joined = Some(joined.map_or(arm_state.clone(), |previous: Availability| {
                        previous.join(arm_state)
                    }));
                }
                if let Some(joined) = joined {
                    *state = joined;
                }
            }
            HirExprKind::Binary(value)
                if matches!(
                    value.operator(),
                    HirBinaryOp::And | HirBinaryOp::Or | HirBinaryOp::Implies
                ) =>
            {
                self.expression(value.left(), state)?;
                self.branch(
                    state,
                    |this, state| this.expression(value.right(), state),
                    |_, _| Ok(()),
                )?;
            }
            HirExprKind::Block(value) => {
                for statement in value.statements() {
                    self.statement(*statement, state)?;
                }
                self.expression(value.tail(), state)?;
            }
            HirExprKind::ComputationBlock(value) => {
                for statement in value.statements() {
                    self.statement(*statement, state)?;
                }
                self.expression(value.tail(), state)?;
            }
            HirExprKind::NamedBlock(value) => {
                for statement in value.statements() {
                    self.statement(*statement, state)?;
                }
                self.expression(value.tail(), state)?;
            }
            HirExprKind::Loop(value) => {
                self.repeat(state, |this, state| {
                    for statement in value.statements() {
                        this.statement(*statement, state)?;
                    }
                    this.expression(value.tail(), state)
                })?;
            }
            HirExprKind::Closure(value) => {
                let Some(CheckedExpressionResolution::Closure(_)) = self
                    .analysis
                    .expression(owner)
                    .map(super::CheckedExpression::resolution)
                else {
                    return Err(CheckedLocalUseError::InvalidTopology);
                };
                let captures = self
                    .analysis
                    .checked_capture_inputs(owner)
                    .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                let mut body_state = Availability::root();
                for input in captures.sources() {
                    self.use_local(input.site(), input.local(), state)?;
                }
                self.expression(value.body(), &mut body_state)?;
            }
            HirExprKind::Record(_) | HirExprKind::RecordLiteral(_) => {
                let Ok(edges) = self.analysis.checked_expression_edge_fact(owner) else {
                    return Ok(());
                };
                let inputs = self
                    .analysis
                    .checked_capture_inputs(owner)
                    .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                for field in edges.record_fields() {
                    match field.source() {
                        CheckedRecordValueSource::Expression(source) => {
                            self.expression(source.raw(), state)?
                        }
                        CheckedRecordValueSource::Binding(_) => {
                            let input = inputs
                                .record_source_at(field.source_ordinal())
                                .ok_or(CheckedLocalUseError::InvalidTopology)?;
                            self.use_local(input.site(), input.local(), state)?;
                        }
                    }
                }
            }
            HirExprKind::Path(_)
                if matches!(
                    self.analysis
                        .expression(owner)
                        .map(super::CheckedExpression::resolution),
                    Some(CheckedExpressionResolution::Select(
                        CheckedSelectResolution::Field(_)
                    ))
                ) =>
            {
                if self
                    .analysis
                    .expression(owner)
                    .and_then(super::CheckedExpression::mutable_place)
                    .filter(|place| place.nominal_field().is_some())
                    .is_some()
                {
                    let inputs = self
                        .analysis
                        .checked_capture_inputs(owner)
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                    let mut sources = inputs.sources();
                    let input = sources
                        .next()
                        .ok_or(CheckedLocalUseError::InvalidTopology)?;
                    if sources.next().is_some() {
                        return Err(CheckedLocalUseError::InvalidTopology);
                    }
                    self.use_local(input.site(), input.local(), state)?;
                }
            }
            HirExprKind::Placeholder(_) => {
                match self
                    .analysis
                    .expression(owner)
                    .map(super::CheckedExpression::resolution)
                {
                    Some(CheckedExpressionResolution::PipeLeft(pipe)) => self.use_synthetic(
                        owner,
                        CheckedSyntheticUseOwner::Pipe(pipe.binding_identity()),
                        state,
                    )?,
                    Some(CheckedExpressionResolution::ImplicitParameter(parameter)) => self
                        .use_synthetic(
                            owner,
                            CheckedSyntheticUseOwner::ImplicitParameter(parameter.callable()),
                            state,
                        )?,
                    Some(CheckedExpressionResolution::ImplicitCallable(callable))
                        if matches!(
                            callable.body(),
                            super::CheckedImplicitCallableBody::Plain(body)
                                if matches!(body.as_ref(), CheckedExpressionResolution::ImplicitParameter(_))
                        ) =>
                    {
                        self.use_synthetic(
                            owner,
                            CheckedSyntheticUseOwner::ImplicitParameter(callable.identity()),
                            state,
                        )?
                    }
                    _ => {}
                }
            }
            _ => {
                if let Ok(edges) = self.analysis.checked_expression_edge_fact(owner) {
                    let children = edges.child_expressions().collect::<Vec<_>>();
                    let loans_before = self.active_receiver_loans.len();
                    if let Some(receiver) = self.selected_receiver_source(owner)
                        && self.borrowed_receivers.contains(&receiver)
                    {
                        let local = self
                            .analysis
                            .expression(receiver)
                            .and_then(super::CheckedExpression::execution_local_use)
                            .ok_or(CheckedLocalUseError::InvalidTopology)?;
                        self.active_receiver_loans.push((local, receiver));
                    }
                    for child in children {
                        if let Err(error) = self.expression(child, state) {
                            self.active_receiver_loans.truncate(loans_before);
                            return Err(error);
                        }
                    }
                    self.active_receiver_loans.truncate(loans_before);
                    if state.reachable
                        && let Some(receiver) = self.selected_receiver_source(owner)
                        && self.in_place_receivers.contains(&receiver)
                    {
                        let place = self
                            .analysis
                            .expression(receiver)
                            .and_then(super::CheckedExpression::mutable_place)
                            .ok_or(CheckedLocalUseError::InvalidTopology)?;
                        let local = place.local_id();
                        if state.moved.contains_key(&local) {
                            return Err(CheckedLocalUseError::Unavailable {
                                local,
                                site: CheckedLocalUseSite::Place(receiver),
                            });
                        }
                    }
                } else if !matches!(
                    self.analysis
                        .expression(owner)
                        .map(super::CheckedExpression::resolution),
                    Some(CheckedExpressionResolution::DialogueApplication { .. })
                ) {
                    // A rejected call/record may retain checked tooling facts
                    // without an executable child graph.
                    return Ok(());
                }
                if matches!(
                    (
                        expression.kind(),
                        self.analysis
                            .expression(owner)
                            .map(super::CheckedExpression::resolution),
                    ),
                    (
                        HirExprKind::AttachedContentApplication(_) | HirExprKind::PostfixBracket(_),
                        Some(CheckedExpressionResolution::DialogueApplication { .. })
                    )
                ) {
                    for edge in expression
                        .kind()
                        .expression_owned_child_edges()
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?
                    {
                        if !matches!(
                            edge.role(),
                            HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. }
                        ) {
                            continue;
                        }
                        match edge.child() {
                            HirExpressionOwnedChild::Statement(statement) => {
                                self.statement(statement, state)?;
                            }
                            HirExpressionOwnedChild::Body(child) => match child.child() {
                                HirBodyChild::Statement(statement) => {
                                    self.statement(statement, state)?;
                                }
                                HirBodyChild::Expression(expression) => {
                                    self.expression(expression, state)?;
                                }
                            },
                            HirExpressionOwnedChild::Pattern(_) => {}
                        }
                    }
                }
            }
        }
        if self
            .analysis
            .expression(owner)
            .and_then(super::CheckedExpression::execution_local_use)
            .is_some()
        {
            let inputs = self
                .analysis
                .checked_capture_inputs(owner)
                .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
            for input in inputs.sources() {
                self.use_local(input.site(), input.local(), state)?;
            }
        }
        Ok(())
    }

    fn selected_receiver_source(&self, owner: ExprId) -> Option<ExprId> {
        let application = self.analysis.call(owner)?.selected_application()?;
        let CheckedCallReceiverProjection::Operand { source, .. } =
            application.core().execution().receiver()
        else {
            return None;
        };
        match source.raw() {
            CheckedCallArgumentSlotSource::Expression(receiver) => Some(receiver),
            _ => None,
        }
    }

    fn statement(
        &mut self,
        owner: StmtId,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        if self.analysis.statement(owner).is_none() {
            return Ok(());
        }
        let statement = self
            .module
            .resolve_stmt(owner)
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
        match statement.kind().evaluation_plan() {
            HirStmtEvaluationPlan::Binding { input, locals, .. } => {
                self.expression(input, state)?;
                self.bind(locals);
            }
            HirStmtEvaluationPlan::OrderedPair {
                kind: HirStmtOrderedPairPlanKind::Assign,
                first,
                second,
                ..
            } => {
                // The checked assignment target is a direct place. Reading
                // its local would spuriously consume the old value.
                self.expression(second, state)?;
                let place = self
                    .analysis
                    .expression(first)
                    .and_then(super::CheckedExpression::mutable_place)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                let mode = if place.nominal_field().is_some() {
                    CheckedLocalPlaceMode::Mutate
                } else {
                    CheckedLocalPlaceMode::Replace
                };
                self.access_place(first, place, mode, state)?;
            }
            HirStmtEvaluationPlan::Value {
                kind: arcweft_lang_hir::stmt::HirStmtValuePlanKind::Defer,
                expression: Some(body),
                ..
            } => {
                let fact = self
                    .analysis
                    .statement(owner)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                let super::CheckedStatementPayload::Defer(defer) = fact.payload() else {
                    return Err(CheckedLocalUseError::InvalidTopology);
                };
                if defer.body() != body {
                    return Err(CheckedLocalUseError::InvalidTopology);
                }
                let captures =
                    super::free_capture::CheckedCaptureExpression::from_statement(owner, fact)
                        .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                for input in captures.sources() {
                    self.use_local(input.site(), input.local(), state)?;
                }
                self.expression(body, &mut Availability::root())?;
            }
            HirStmtEvaluationPlan::If {
                condition,
                then_body,
                else_branch,
            } => {
                self.expression(condition, state)?;
                self.branch(
                    state,
                    |this, state| this.contextual_body(then_body, state),
                    |this, state| this.else_branch(else_branch, state),
                )?;
            }
            HirStmtEvaluationPlan::IfLet {
                scrutinee,
                guard,
                branch_locals,
                then_body,
                else_branch,
                ..
            } => {
                self.expression(scrutinee, state)?;
                let incoming = state.clone();
                let mut matched = incoming.clone();
                let mut unmatched = incoming;
                self.bind(branch_locals);
                if let Some(guard) = guard {
                    self.guard_expression(guard, branch_locals.iter().copied(), &mut matched)?;
                    unmatched = unmatched.join(matched.clone());
                }
                self.contextual_body(then_body, &mut matched)?;
                self.else_branch(else_branch, &mut unmatched)?;
                *state = matched.join(unmatched);
            }
            HirStmtEvaluationPlan::Match { scrutinee, arms } => {
                self.expression(scrutinee, state)?;
                let mut joined = None;
                let mut fallthrough = state.clone();
                for arm in arms {
                    let mut arm_state = fallthrough.clone();
                    self.bind(arm.locals());
                    if let Some(guard) = arm.guard() {
                        self.guard_expression(guard, arm.locals().iter().copied(), &mut arm_state)?;
                        fallthrough = fallthrough.join(arm_state.clone());
                    }
                    match arm.body() {
                        HirStmtMatchArmBody::Expression(expression) => {
                            self.expression(*expression, &mut arm_state)?
                        }
                        HirStmtMatchArmBody::Body(body) => {
                            self.contextual_body(body, &mut arm_state)?
                        }
                    }
                    joined = Some(joined.map_or(arm_state.clone(), |previous: Availability| {
                        previous.join(arm_state)
                    }));
                }
                if let Some(joined) = joined {
                    *state = joined;
                }
            }
            HirStmtEvaluationPlan::LetElse {
                initializer,
                else_body,
                success_locals,
                ..
            } => {
                self.expression(initializer, state)?;
                let mut failed = state.clone();
                for statement in else_body {
                    self.statement(*statement, &mut failed)?;
                }
                // The failed pattern branch is required to diverge. Its
                // moves cannot poison the successful continuation.
                self.bind(success_locals);
            }
            HirStmtEvaluationPlan::While { condition, body } => {
                self.repeat(state, |this, state| {
                    this.expression(condition, state)?;
                    this.contextual_body(body, state)
                })?;
            }
            HirStmtEvaluationPlan::WhileLet {
                scrutinee,
                guard,
                branch_locals,
                body,
                ..
            } => {
                self.repeat(state, |this, state| {
                    this.expression(scrutinee, state)?;
                    this.bind(branch_locals);
                    if let Some(guard) = guard {
                        this.guard_expression(guard, branch_locals.iter().copied(), state)?;
                    }
                    this.contextual_body(body, state)
                })?;
            }
            HirStmtEvaluationPlan::For {
                source,
                iterator,
                next_value,
                branch_locals,
                body,
                ..
            } => {
                self.expression(source, state)?;
                self.expression(iterator, state)?;
                self.expression(next_value, state)?;
                self.repeat(state, |this, state| {
                    this.bind(branch_locals);
                    this.contextual_body(body, state)
                })?;
            }
            HirStmtEvaluationPlan::Select { plan, .. } => match plan {
                HirStmtSelectEvaluationPlan::Operand { expression } => {
                    self.expression(expression, state)?
                }
                HirStmtSelectEvaluationPlan::Branches { branches } => {
                    let mut joined = None;
                    for branch in branches.entries() {
                        let mut branch_state = state.clone();
                        match branch.head() {
                            HirStmtSelectHeadEvaluation::Bind { source, .. } => {
                                self.expression(source, &mut branch_state)?
                            }
                            HirStmtSelectHeadEvaluation::Frame { locals, .. }
                            | HirStmtSelectHeadEvaluation::Event { locals, .. } => {
                                self.bind(locals)
                            }
                            HirStmtSelectHeadEvaluation::Recovered => {}
                        }
                        self.contextual_body(branch.body(), &mut branch_state)?;
                        joined = Some(
                            joined.map_or(branch_state.clone(), |previous: Availability| {
                                previous.join(branch_state)
                            }),
                        );
                    }
                    if let Some(joined) = joined {
                        *state = joined;
                    }
                }
            },
            HirStmtEvaluationPlan::Value {
                kind, expression, ..
            } if matches!(
                kind,
                HirStmtValuePlanKind::Return
                    | HirStmtValuePlanKind::Goto
                    | HirStmtValuePlanKind::Break
            ) =>
            {
                if let Some(expression) = expression {
                    self.expression(expression, state)?;
                }
                state.reachable = false;
            }
            HirStmtEvaluationPlan::Continue { .. } => {
                state.reachable = false;
            }
            other => {
                let mut steps = Vec::new();
                other
                    .try_visit_evaluation_steps(|step| steps.push(step))
                    .map_err(|_| CheckedLocalUseError::InvalidTopology)?;
                for step in steps {
                    match step {
                        arcweft_lang_hir::stmt::HirStmtEvaluationStep::Expression {
                            expression,
                            ..
                        } => self.expression(expression, state)?,
                        arcweft_lang_hir::stmt::HirStmtEvaluationStep::Statement {
                            statement,
                            ..
                        } => self.statement(statement, state)?,
                        arcweft_lang_hir::stmt::HirStmtEvaluationStep::ThreadBody {
                            edge, ..
                        } => match edge.child() {
                            HirBodyChild::Expression(expression) => {
                                self.expression(expression, state)?
                            }
                            HirBodyChild::Statement(statement) => {
                                self.statement(statement, state)?
                            }
                        },
                        arcweft_lang_hir::stmt::HirStmtEvaluationStep::Publication {
                            locals,
                            ..
                        } => self.bind(locals),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn else_branch(
        &mut self,
        branch: Option<&HirConditionalElseBranch>,
        state: &mut Availability,
    ) -> Result<(), CheckedLocalUseError> {
        match branch {
            Some(HirConditionalElseBranch::Body(body)) => self.contextual_body(body, state),
            Some(HirConditionalElseBranch::ElseIf(statement)) => self.statement(*statement, state),
            None => Ok(()),
        }
    }
}

fn effect_capture_sites(
    analysis: &FinalSemanticAnalysis,
    owners: impl Iterator<Item = ExprId>,
) -> Result<BTreeMap<ExprId, Box<[LocalId]>>, CheckedLocalUseError> {
    fn collect(
        report: &CheckedRichTextReport,
        sites: &mut BTreeMap<ExprId, Box<[LocalId]>>,
    ) -> Result<(), CheckedLocalUseError> {
        for site in report.effect_plan().effect_sites() {
            if sites
                .insert(
                    site.root(),
                    site.captures()
                        .iter()
                        .map(super::CheckedExecutableCapture::local)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )
                .is_some()
            {
                return Err(CheckedLocalUseError::InvalidTopology);
            }
        }
        for token in report.content().tokens() {
            if let CheckedDialogueToken::ContentInsert(insertion) = token
                && let Some(child) = insertion.argument().checked_content()
            {
                collect(child, sites)?;
            }
        }
        Ok(())
    }

    let mut sites = BTreeMap::new();
    for owner in owners {
        let Some(expression) = analysis.expression(owner) else {
            continue;
        };
        if let CheckedExpressionResolution::DialogueApplication { rich_text, .. } =
            expression.resolution()
        {
            collect(rich_text, &mut sites)?;
        }
    }
    Ok(sites)
}

/// A scheduled line callback is constructed when its checked `at` call is
/// evaluated, even when the source callback is a block or line-action call
/// rather than an explicit closure. The selected call slot names the callback;
/// the selected local-use traversal identifies exactly which local uses cross
/// that callback's lexical boundary.
fn selected_scheduled_callback_roots(
    analysis: &FinalSemanticAnalysis,
    owners: impl Iterator<Item = ExprId>,
) -> Result<BTreeSet<ExprId>, CheckedLocalUseError> {
    let mut sites = BTreeSet::new();
    for owner in owners {
        let Some(call) = analysis.call(owner) else {
            continue;
        };
        let Some(application) = call.selected_application() else {
            continue;
        };
        let core = application.core();
        if !matches!(
            core.candidates().selected().id(),
            CallableCandidateId::LineSchedule(LineScheduleCallableId::At)
        ) || core.current_group().get() != 1
        {
            continue;
        }
        let mut callbacks = core
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .filter(|slot| {
                matches!(
                    slot.destination(),
                    CheckedCallOperandDestination::Parameter(coordinate)
                        if coordinate.group().get() == 1 && coordinate.parameter().get() == 0
                )
            });
        let Some(slot) = callbacks.next() else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        if callbacks.next().is_some() {
            return Err(CheckedLocalUseError::InvalidTopology);
        }
        let CheckedCallArgumentSlotSource::Expression(callback) = slot.source().raw() else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        // Explicit and implicit callable captures are already sealed by their
        // own selected fact. Only the accepted non-closure callback forms need
        // this source-region projection.
        if matches!(
            analysis
                .expression(callback)
                .map(super::CheckedExpression::resolution),
            Some(CheckedExpressionResolution::Closure(_))
                | Some(CheckedExpressionResolution::ImplicitCallable(_))
        ) {
            continue;
        }
        if !sites.insert(callback) {
            return Err(CheckedLocalUseError::InvalidTopology);
        }
    }
    Ok(sites)
}

/// Selected capacity operations lower their receiver as a direct mutable
/// place, not as a value operand. The compiler uses the same checked
/// `capacity_operation` and receiver source to form `RuntimeResolvedMutablePlace`.
fn selected_in_place_receivers(
    analysis: &FinalSemanticAnalysis,
    owners: impl Iterator<Item = ExprId>,
) -> Result<BTreeSet<ExprId>, CheckedLocalUseError> {
    let mut receivers = BTreeSet::new();
    for owner in owners {
        let Some(call) = analysis.call(owner) else {
            continue;
        };
        let Some(application) = call.selected_application() else {
            continue;
        };
        if !application
            .core()
            .candidates()
            .selected()
            .capacity_operation()
            .is_some_and(CheckedCapacityOperation::requires_place_receiver)
        {
            continue;
        }
        let CheckedCallReceiverProjection::Operand { source, .. } =
            application.core().execution().receiver()
        else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        let CheckedCallArgumentSlotSource::Expression(receiver) = source.raw() else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        // A selected capacity family does not itself prove that the authored
        // receiver denotes a writable place. Nested/computed receiver paths
        // are diagnosed by the place-lowering owner; they must still undergo
        // ordinary expression traversal here rather than becoming a final
        // analysis topology failure.
        if analysis
            .expression(receiver)
            .and_then(super::CheckedExpression::mutable_place)
            .is_some()
        {
            receivers.insert(receiver);
        }
    }
    Ok(receivers)
}

/// Look borrows its exact StageActor receiver for the duration of the line
/// operation. Only a direct local can be projected as a stable runtime place;
/// other receiver expressions retain ordinary value-transfer semantics.
fn selected_borrowed_receivers(
    analysis: &FinalSemanticAnalysis,
    owners: impl Iterator<Item = ExprId>,
) -> Result<BTreeSet<ExprId>, CheckedLocalUseError> {
    let mut receivers = BTreeSet::new();
    for owner in owners {
        let Some(call) = analysis.call(owner) else {
            continue;
        };
        let Some(application) = call.selected_application() else {
            continue;
        };
        if application.core().candidates().selected().id()
            != &CallableCandidateId::StageMethod(StageMethodId::Look)
        {
            continue;
        }
        let CheckedCallReceiverProjection::Operand { source, .. } =
            application.core().execution().receiver()
        else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        let CheckedCallArgumentSlotSource::Expression(receiver) = source.raw() else {
            return Err(CheckedLocalUseError::InvalidTopology);
        };
        if analysis
            .expression(receiver)
            .and_then(super::CheckedExpression::execution_local_use)
            .is_some()
        {
            receivers.insert(receiver);
        }
    }
    Ok(receivers)
}

fn scheduled_free_locals(
    analysis: &FinalSemanticAnalysis,
    callback: ExprId,
    locals: BTreeSet<LocalId>,
) -> Result<Box<[LocalId]>, CheckedLocalUseError> {
    let topology = analysis.hir_topology();
    let callback_location = topology
        .semantic_path(HirSemanticPathOwnerId::Expression(callback))
        .map_err(|_| CheckedLocalUseError::InvalidTopology)?
        .ok_or(CheckedLocalUseError::InvalidTopology)?;
    let mut captures = Vec::new();
    for local in locals {
        let location = topology
            .semantic_path(HirSemanticPathOwnerId::Local(local))
            .map_err(|_| CheckedLocalUseError::InvalidTopology)?
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        if location.root() != callback_location.root()
            || !location
                .path()
                .steps()
                .starts_with(callback_location.path().steps())
        {
            captures.push(local);
        }
    }
    Ok(captures.into_boxed_slice())
}

fn declaration_has_open_local_types(
    analysis: &FinalSemanticAnalysis,
    body: &HirDeclarationBodyTopology,
) -> bool {
    analysis
        .checked_callables()
        .project_callable(body.declaration())
        .is_ok_and(|checked| {
            !checked.signature().generic_inventory().types().is_empty()
                || !checked.signature().generic_inventory().consts().is_empty()
        })
        || analysis.locals().any(|(local, binding)| {
            body.paths().local(local).is_some()
                && crate::types::contains_generic_parameter(binding.ty())
        })
        || analysis.expressions().any(|(owner, expression)| {
            body.paths().expression(owner).is_some()
                && expression
                    .value_type()
                    .is_some_and(crate::types::contains_generic_parameter)
        })
}

fn definitely_copy(
    ty: &TypeKind,
    analysis: &FinalSemanticAnalysis,
    instance: Option<CheckedLocalUseInstantiation<'_>>,
    visiting: &mut BTreeSet<SemanticTypeDigest>,
) -> Result<bool, CheckedLocalUseError> {
    match ty {
        TypeKind::Bool
        | TypeKind::I8
        | TypeKind::I16
        | TypeKind::I32
        | TypeKind::I64
        | TypeKind::I128
        | TypeKind::ISize
        | TypeKind::U8
        | TypeKind::U16
        | TypeKind::U32
        | TypeKind::U64
        | TypeKind::U128
        | TypeKind::USize
        | TypeKind::F32
        | TypeKind::F64
        | TypeKind::String
        | TypeKind::Char
        | TypeKind::Bytes
        | TypeKind::Duration
        | TypeKind::Progress
        | TypeKind::Unit
        | TypeKind::Never
        | TypeKind::Ref(_) => Ok(true),
        TypeKind::GenericParam(_) => {
            let Some(instance) = instance else {
                return Ok(false);
            };
            let closed = instance.instantiate_type(ty)?;
            if closed == *ty {
                return Err(CheckedLocalUseError::InvalidTopology);
            }
            definitely_copy(&closed, analysis, None, visiting)
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            items.iter().try_fold(true, |all, item| {
                definitely_copy(item, analysis, instance, visiting).map(|next| all && next)
            })
        }
        TypeKind::Vec(item)
        | TypeKind::Array { item, .. }
        | TypeKind::Slice(item)
        | TypeKind::Seq(item)
        | TypeKind::Option(item)
        | TypeKind::Range(item) => definitely_copy(item, analysis, instance, visiting),
        TypeKind::Result { ok, error }
        | TypeKind::Map {
            key: ok,
            value: error,
            ..
        } => Ok(definitely_copy(ok, analysis, instance, visiting)?
            && definitely_copy(error, analysis, instance, visiting)?),
        TypeKind::ProjectNominal(_) => {
            let Ok(digest) = ty.semantic_identity_digest() else {
                return Ok(false);
            };
            if !visiting.insert(digest) {
                return Ok(false);
            }
            let result = (|| {
                let Some(definition) = analysis.project_nominal_semantic(digest) else {
                    return Ok(false);
                };
                if let Some(fields) = definition.fields() {
                    for field in fields {
                        if !definitely_copy(field.ty(), analysis, instance, visiting)? {
                            return Ok(false);
                        }
                    }
                    return Ok(true);
                }
                let Some(cases) = definition.cases() else {
                    return Ok(false);
                };
                for case in cases {
                    match case.payload() {
                        VariantPayloadShape::Unit => {}
                        VariantPayloadShape::Tuple(fields) => {
                            for field in fields {
                                if !definitely_copy(field.ty(), analysis, instance, visiting)? {
                                    return Ok(false);
                                }
                            }
                        }
                        VariantPayloadShape::Record(fields) => {
                            for field in fields {
                                if !definitely_copy(field.ty(), analysis, instance, visiting)? {
                                    return Ok(false);
                                }
                            }
                        }
                    }
                }
                Ok(true)
            })();
            visiting.remove(&digest);
            result
        }
        TypeKind::Named(name) => {
            let Some(graph) = analysis.character_dialogue_policy_types() else {
                return Ok(false);
            };
            let Some(owner) = CharacterDialoguePolicyTypeGraph::owner_for_language_type(name)
            else {
                return Ok(false);
            };
            Ok(policy_owner_is_copy(graph, owner, &mut BTreeSet::new()))
        }
        _ => Ok(false),
    }
}

fn policy_owner_is_copy(
    graph: &CharacterDialoguePolicyTypeGraph,
    owner: CharacterDialoguePolicyVariantOwner,
    visiting: &mut BTreeSet<CharacterDialoguePolicyVariantOwner>,
) -> bool {
    if !visiting.insert(owner) {
        return false;
    }
    let copy = graph.cases(owner).iter().all(|case| {
        case.payload()
            .is_none_or(|payload| policy_payload_is_copy(graph, payload, visiting))
    });
    visiting.remove(&owner);
    copy
}

fn policy_payload_is_copy(
    graph: &CharacterDialoguePolicyTypeGraph,
    payload: CharacterDialoguePolicyTypeSchema,
    visiting: &mut BTreeSet<CharacterDialoguePolicyVariantOwner>,
) -> bool {
    match payload {
        CharacterDialoguePolicyTypeSchema::String
        | CharacterDialoguePolicyTypeSchema::EntityReference => true,
        // The registered RichText role codec admits only plain property data;
        // the graph retains that exact accepted role owner.
        CharacterDialoguePolicyTypeSchema::RichText => {
            graph.rich_text_owner().value_class() == RuntimeOpaqueValueClass::Plain
        }
        CharacterDialoguePolicyTypeSchema::Nominal(owner) => {
            policy_owner_is_copy(graph, owner, visiting)
        }
        CharacterDialoguePolicyTypeSchema::Sequence(item) => {
            policy_payload_is_copy(graph, *item, visiting)
        }
        CharacterDialoguePolicyTypeSchema::Tuple(items)
        | CharacterDialoguePolicyTypeSchema::Choice(items) => items
            .iter()
            .all(|item| policy_payload_is_copy(graph, *item, visiting)),
    }
}
