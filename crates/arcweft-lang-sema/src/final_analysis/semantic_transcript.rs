//! Stable generic-Match semantic transcripts.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    CheckedExpressionResolution, CheckedExpressionSemanticDigest, CheckedMatchLimits,
    CheckedMatchRef, CheckedMatchSemanticDigest, CheckedPatternSemanticDigest,
    CheckedSelectResolution, CheckedValueResolution, CheckedVariantOwner, FinalSemanticAnalysis,
    FinalSemanticAnalysisControl,
    match_coverage::{
        CheckedCoverageWitness, CheckedGuardClass, CheckedMatchBudget, CheckedMatchBuildError,
        CheckedMatchCoverage, CheckedMatchLimitKind, CheckedUnreachableReason, CoverageArmInput,
        MatchCoverageAnalyzer, StableMatchArmCoordinate,
    },
    transcript_writer::{TranscriptByteCounter, TranscriptHasher, TranscriptWriteError},
};
use crate::semantic_coordinate::{
    AcceptedSemanticRootCatalogError, CheckedSemanticPath, SemanticCoordinateEncodingError,
    SemanticCoordinateIndex, SemanticCoordinateIndexError, StableCheckedValueCoordinate,
    StablePatternCoordinate, StablePatternCoordinateStep, StableSemanticCoordinate,
};
use crate::types::{SemanticTypeDigest, TypeKind};
use arcweft_lang_hir::{
    expr::{HirExprKind, HirMatchExpr},
    identity::{ExprId, PatternId},
    leaf::HirLiteral,
    module::HirModule,
    pattern::{HirPatternChild, HirPatternChildRole, HirPatternKind},
    project::HirExecutableProjectView,
    symbol::ProjectSymbolTable,
};
use thiserror::Error;

macro_rules! transcript_update {
    ($hasher:expr, $bytes:expr $(,)?) => {
        $hasher.update($bytes)?
    };
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum SemanticTranscriptError {
    #[error(transparent)]
    Generation(Box<super::FinalSemanticAnalysisError>),
    #[error("expression is not a Match")]
    NotMatch,
    #[error("checked Match evidence is missing or stale")]
    MissingMatchFact,
    #[error("checked Match reference does not belong to the exact accepted HIR snapshot")]
    StaleMatchReference,
    #[error("checked expression evidence is missing")]
    MissingExpression,
    #[error("checked pattern evidence is missing")]
    MissingPattern,
    #[error("checked child-edge evidence is missing")]
    MissingChildEdges,
    #[error("selected callable join is missing")]
    MissingCallableJoin,
    #[error("recovered semantic owner cannot be transcribed")]
    RecoveredOwner,
    #[error("semantic transcript work limit exceeded")]
    WorkLimit,
    #[error("semantic transcript byte accounting overflow")]
    TranscriptArithmeticOverflow,
    #[error("semantic transcript byte limit {limit} exceeded by attempt {attempted}")]
    TranscriptLimitExceeded { limit: u64, attempted: u64 },
    #[error(transparent)]
    MatchBuild(#[from] CheckedMatchBuildError),
    #[error("semantic transcript cannot resolve an accepted identity")]
    MissingIdentity,
    #[error(transparent)]
    AcceptedRootCatalog(AcceptedSemanticRootCatalogError),
    #[error("semantic transcript identity family is not supported by this cut")]
    UnsupportedIdentity,
    #[error(transparent)]
    CoordinateEncoding(#[from] SemanticCoordinateEncodingError),
    #[error("Match is not exhaustive; coverage witness is retained in the error")]
    NonExhaustive { witness: CheckedCoverageWitness },
}

impl From<TranscriptWriteError> for SemanticTranscriptError {
    fn from(error: TranscriptWriteError) -> Self {
        match error {
            TranscriptWriteError::ArithmeticOverflow => Self::TranscriptArithmeticOverflow,
            TranscriptWriteError::LimitExceeded { limit, attempted } => {
                Self::TranscriptLimitExceeded { limit, attempted }
            }
        }
    }
}

impl From<super::FinalSemanticAnalysisError> for SemanticTranscriptError {
    fn from(error: super::FinalSemanticAnalysisError) -> Self {
        Self::Generation(Box::new(error))
    }
}

impl From<SemanticCoordinateIndexError> for SemanticTranscriptError {
    fn from(error: SemanticCoordinateIndexError) -> Self {
        match error {
            SemanticCoordinateIndexError::RootCatalog(error) => Self::AcceptedRootCatalog(error),
            SemanticCoordinateIndexError::ControlTransferLookup(_) => Self::MissingIdentity,
            SemanticCoordinateIndexError::MissingChildEdges => Self::MissingChildEdges,
            SemanticCoordinateIndexError::MissingOwner { .. }
            | SemanticCoordinateIndexError::MissingBody { .. }
            | SemanticCoordinateIndexError::InvalidBindingPath { .. }
            | SemanticCoordinateIndexError::ExpressionRoleMismatch
            | SemanticCoordinateIndexError::InvalidRootPath => Self::MissingIdentity,
        }
    }
}

type MatchTranscriptHasher<'a> = TranscriptHasher<'a, CheckedMatchBudget>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedMatchBinding {
    coordinate: StableCheckedValueCoordinate,
    ty: SemanticTypeDigest,
}

#[allow(
    dead_code,
    reason = "checked Match observations are exercised by sema tests, not a runtime consumer"
)]
impl CheckedMatchBinding {
    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub const fn ty(&self) -> SemanticTypeDigest {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedMatchArm {
    ordinal: u32,
    pattern: CheckedPatternSemanticDigest,
    guard: CheckedGuardClass,
    guard_expression: Option<CheckedExpressionSemanticDigest>,
    value: CheckedExpressionSemanticDigest,
    bindings: Box<[CheckedMatchBinding]>,
}

#[allow(
    dead_code,
    reason = "checked Match observations are exercised by sema tests, not a runtime consumer"
)]
impl CheckedMatchArm {
    pub const fn pattern(&self) -> CheckedPatternSemanticDigest {
        self.pattern
    }
    pub fn bindings(&self) -> &[CheckedMatchBinding] {
        &self.bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedMatch {
    semantic_digest: CheckedMatchSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: Box<[CheckedMatchArm]>,
    coverage: CheckedMatchCoverage,
}

#[allow(
    dead_code,
    reason = "checked Match observations are exercised by sema tests, not a runtime consumer"
)]
impl CheckedMatch {
    pub const fn semantic_digest(&self) -> CheckedMatchSemanticDigest {
        self.semantic_digest
    }
    pub fn arms(&self) -> &[CheckedMatchArm] {
        &self.arms
    }
    pub const fn coverage(&self) -> &CheckedMatchCoverage {
        &self.coverage
    }
}

#[allow(
    dead_code,
    reason = "the compiler-local Match query intentionally has no runtime consumer in this cut"
)]
impl FinalSemanticAnalysis {
    /// Binds one checked Match lookup to this report's exact module snapshot.
    pub(crate) fn checked_match_ref(
        &self,
        module: &HirModule,
        symbols: &ProjectSymbolTable,
        expression: ExprId,
    ) -> Result<CheckedMatchRef, SemanticTranscriptError> {
        self.validate_module_generation(module, symbols)?;
        if expression.module() != module.module_id() {
            return Err(SemanticTranscriptError::StaleMatchReference);
        }
        let owner = module
            .resolve_expr(expression)
            .map_err(|_| SemanticTranscriptError::MissingExpression)?;
        if !matches!(owner.kind(), HirExprKind::Match(_)) {
            return Err(SemanticTranscriptError::NotMatch);
        }
        let checked = self
            .expression(expression)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        if checked.match_fact().is_none() {
            return Err(SemanticTranscriptError::MissingMatchFact);
        }
        Ok(CheckedMatchRef::new(module.snapshot_id(), expression))
    }

    /// Revalidates a compiler-local Match reference before constructing its
    /// stable generic semantic product.
    pub(crate) fn build_checked_match_for_ref(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        reference: CheckedMatchRef,
        limits: CheckedMatchLimits,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
        static NOT_CANCELLED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        self.build_checked_match_for_ref_with_control(
            project,
            symbols,
            reference,
            limits,
            FinalSemanticAnalysisControl::new(&NOT_CANCELLED),
        )
    }

    /// Constructs a checked Match while observing caller-owned cancellation.
    pub(crate) fn build_checked_match_for_ref_with_control(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        reference: CheckedMatchRef,
        limits: CheckedMatchLimits,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
        control.check()?;
        self.validate_generation(project, symbols)?;
        let expression = reference.expression();
        let module = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == expression.module()).then_some(module.as_ref())
            })
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        if module.snapshot_id() != reference.snapshot() {
            return Err(SemanticTranscriptError::StaleMatchReference);
        }
        let owner = module
            .resolve_expr(expression)
            .map_err(|_| SemanticTranscriptError::MissingExpression)?;
        let HirExprKind::Match(authored) = owner.kind() else {
            return Err(SemanticTranscriptError::NotMatch);
        };
        let checked = self
            .expression(expression)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let fact = checked
            .match_fact()
            .ok_or(SemanticTranscriptError::MissingMatchFact)?;
        if fact.scrutinee() != authored.scrutinee() || fact.arms().len() != authored.arms().len() {
            return Err(SemanticTranscriptError::MissingMatchFact);
        }
        let coordinates = SemanticCoordinateIndex::new(self.accepted_root_catalog(), self);
        MatchTranscriptBuilder {
            analysis: self,
            module,
            coordinates,
            control,
            budget: CheckedMatchBudget::new(limits),
            expression_digests: BTreeMap::new(),
            expression_paths: BTreeMap::new(),
            expression_visiting: BTreeSet::new(),
            pattern_digests: BTreeMap::new(),
            pattern_coordinates: BTreeMap::new(),
            pattern_paths: BTreeMap::new(),
            pattern_visiting: BTreeSet::new(),
            observed_patterns: Vec::new(),
        }
        .build(expression, authored)
    }
}

struct MatchTranscriptBuilder<'analysis, 'paths, 'edges, 'control> {
    analysis: &'analysis FinalSemanticAnalysis,
    module: &'analysis HirModule,
    coordinates: SemanticCoordinateIndex<'paths, 'edges>,
    control: FinalSemanticAnalysisControl<'control>,
    budget: CheckedMatchBudget,
    expression_digests: BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    expression_paths: BTreeMap<StableSemanticCoordinate, ExprId>,
    expression_visiting: BTreeSet<ExprId>,
    pattern_digests: BTreeMap<PatternId, CheckedPatternSemanticDigest>,
    pattern_coordinates: BTreeMap<PatternId, StableSemanticCoordinate>,
    pattern_paths: BTreeMap<StableSemanticCoordinate, PatternId>,
    pattern_visiting: BTreeSet<PatternId>,
    observed_patterns: Vec<(PatternId, StableSemanticCoordinate)>,
}

impl MatchTranscriptBuilder<'_, '_, '_, '_> {
    fn build(
        &mut self,
        owner: ExprId,
        authored: &HirMatchExpr,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
        let scrutinee = self.expression_digest(authored.scrutinee())?;
        self.control.check()?;
        let scrutinee_ty = self
            .analysis
            .expression(authored.scrutinee())
            .ok_or(SemanticTranscriptError::MissingExpression)?
            .ty()
            .clone();
        let scrutinee_type = scrutinee_ty.semantic_identity_digest();
        let checked_owner = self
            .analysis
            .expression(owner)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let fact = checked_owner
            .match_fact()
            .ok_or(SemanticTranscriptError::MissingMatchFact)?;
        let match_path = self.checked_path(owner)?;
        let arm_count = u64::try_from(authored.arms().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Arms,
            }
        })?;
        self.budget.charge(CheckedMatchLimitKind::Arms, arm_count)?;
        let mut arms = Vec::new();
        arms.try_reserve_exact(authored.arms().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Arms,
            }
        })?;
        let mut coverage_arms = Vec::new();
        coverage_arms
            .try_reserve_exact(authored.arms().len())
            .map_err(|_| CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Arms,
            })?;
        for (ordinal, (arm, checked)) in authored.arms().iter().zip(fact.arms()).enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| SemanticTranscriptError::WorkLimit)?;
            let arm_coordinate = StableMatchArmCoordinate::new(match_path.clone(), ordinal);
            let pattern = self.pattern_digest(
                arm.pattern(),
                &arm_coordinate,
                &StablePatternCoordinate::new([]),
            )?;
            let mut bindings = Vec::new();
            self.collect_pattern_bindings(
                arm.pattern(),
                &StablePatternCoordinate::new([]),
                &mut bindings,
            )?;
            let (guard, guard_expression) = match (arm.guard(), checked.guard()) {
                (None, None) => (CheckedGuardClass::Absent, None),
                (Some(authored), Some(checked)) if authored == checked => (
                    guard_class(self.analysis, authored)?,
                    Some(self.expression_digest(authored)?),
                ),
                _ => return Err(SemanticTranscriptError::MissingMatchFact),
            };
            if arm.value() != checked.value() {
                return Err(SemanticTranscriptError::MissingMatchFact);
            }
            arms.push(CheckedMatchArm {
                ordinal,
                pattern,
                guard,
                guard_expression,
                value: self.expression_digest(arm.value())?,
                bindings: bindings.into_boxed_slice(),
            });
            coverage_arms.push(CoverageArmInput {
                coordinate: arm_coordinate,
                pattern: arm.pattern(),
                guard,
            });
        }
        let mut coverage = MatchCoverageAnalyzer::new(
            self.analysis,
            self.module,
            self.control,
            &mut self.budget,
            StableSemanticCoordinate::new(match_path),
            std::mem::take(&mut self.observed_patterns),
        )
        .analyze(&scrutinee_ty, &coverage_arms)?;
        if let Some(witness) = coverage.witness().cloned() {
            return Err(SemanticTranscriptError::NonExhaustive { witness });
        }
        let semantic_digest = match_digest(
            &mut self.budget,
            scrutinee,
            scrutinee_type,
            &arms,
            &coverage,
        )?;
        coverage.finish_transaction_work(self.budget.work());
        Ok(CheckedMatch {
            semantic_digest,
            scrutinee_type,
            arms: arms.into_boxed_slice(),
            coverage,
        })
    }

    fn expression_digest(
        &mut self,
        owner: ExprId,
    ) -> Result<CheckedExpressionSemanticDigest, SemanticTranscriptError> {
        self.expression_digest_at(owner, 0)
    }

    fn expression_digest_at(
        &mut self,
        owner: ExprId,
        depth: u64,
    ) -> Result<CheckedExpressionSemanticDigest, SemanticTranscriptError> {
        self.budget.observe_depth(depth)?;
        let checked = self
            .analysis
            .expression(owner)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let path = self.checked_path(owner)?;
        let semantic_coordinate = StableSemanticCoordinate::new(path.clone());
        if self
            .expression_paths
            .get(&semantic_coordinate)
            .is_some_and(|existing| *existing != owner)
        {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if self.expression_visiting.contains(&owner) {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if let Some(digest) = self.expression_digests.get(&owner) {
            return Ok(*digest);
        }
        self.budget
            .charge(CheckedMatchLimitKind::ExpressionNodes, 1)?;
        self.expression_paths
            .insert(semantic_coordinate.clone(), owner);
        self.expression_visiting.insert(owner);
        let path_depth = u64::try_from(path.steps().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Depth,
            }
        })?;
        self.budget.observe_depth(path_depth)?;
        let edges = self
            .analysis
            .checked_expression_edge_fact(owner)
            .map_err(|_| SemanticTranscriptError::MissingChildEdges)?;
        let child_depth =
            depth
                .checked_add(1)
                .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::Depth,
                })?;
        let mut child_digests = Vec::new();
        for (child, _) in edges.edges() {
            let digest = self.expression_digest_at(*child, child_depth)?;
            child_digests.try_reserve_exact(1).map_err(|_| {
                CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::ExpressionNodes,
                }
            })?;
            child_digests.push(digest);
        }
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-expression-semantic.v1\0");
        transcript_update!(hasher, &path.canonical_bytes()?);
        transcript_update!(hasher, &checked.resolution().semantic_tag().to_le_bytes());
        transcript_update!(hasher, checked.ty().semantic_identity_digest().as_bytes());
        if let CheckedExpressionResolution::Literal(literal) = checked.resolution() {
            write_literal(&mut hasher, literal, checked.ty())?;
        }
        write_resolution_payload(
            &mut hasher,
            checked.resolution(),
            checked.ty(),
            &self.coordinates,
            self.analysis,
        )?;
        write_record_expression_fields(&mut hasher, edges)?;
        write_effects(&mut hasher, checked.effects())?;
        if matches!(checked.resolution(), CheckedExpressionResolution::Call) {
            let callable = edges
                .callable()
                .ok_or(SemanticTranscriptError::MissingCallableJoin)?;
            transcript_update!(hasher, callable.semantic_digest().as_bytes());
        }
        write_len(&mut hasher, edges.edges().len())?;
        for ((_, role), child_digest) in edges.edges().iter().zip(child_digests) {
            write_bytes(&mut hasher, &role.transcript_bytes()?)?;
            transcript_update!(hasher, child_digest.as_bytes());
        }
        let digest = CheckedExpressionSemanticDigest::from_bytes(hasher.finalize());
        self.expression_visiting.remove(&owner);
        self.expression_digests.insert(owner, digest);
        Ok(digest)
    }

    fn checked_path(&self, owner: ExprId) -> Result<CheckedSemanticPath, SemanticTranscriptError> {
        Ok(self.coordinates.expression(owner)?)
    }

    fn pattern_digest(
        &mut self,
        owner: PatternId,
        arm: &StableMatchArmCoordinate,
        coordinate: &StablePatternCoordinate,
    ) -> Result<CheckedPatternSemanticDigest, SemanticTranscriptError> {
        let depth = u64::try_from(coordinate.steps().len())
            .map_err(|_| SemanticTranscriptError::WorkLimit)?;
        self.budget.observe_depth(depth)?;
        let semantic_coordinate = arm.pattern_coordinate(coordinate.clone());
        if self
            .pattern_coordinates
            .get(&owner)
            .is_some_and(|existing| {
                existing != &semantic_coordinate || self.pattern_visiting.contains(&owner)
            })
        {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if self
            .pattern_paths
            .get(&semantic_coordinate)
            .is_some_and(|existing| *existing != owner)
        {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if let Some(digest) = self.pattern_digests.get(&owner) {
            return Ok(*digest);
        }
        self.pattern_coordinates
            .insert(owner, semantic_coordinate.clone());
        self.pattern_paths
            .insert(semantic_coordinate.clone(), owner);
        if !self.pattern_visiting.insert(owner) {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        self.budget.charge(CheckedMatchLimitKind::PatternNodes, 1)?;
        self.observed_patterns.try_reserve_exact(1).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::PatternNodes,
            }
        })?;
        self.observed_patterns
            .push((owner, semantic_coordinate.clone()));
        let hir = self
            .module
            .resolve_pattern(owner)
            .map_err(|_| SemanticTranscriptError::MissingPattern)?;
        if matches!(hir.kind(), HirPatternKind::Error(_)) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let checked = self
            .analysis
            .pattern(owner)
            .ok_or(SemanticTranscriptError::MissingPattern)?;
        let mut child_digests = Vec::new();
        if let super::CheckedPatternResolution::Record(record) = checked.resolution() {
            for field in record.fields() {
                let Some(child) = field.source().raw_pattern() else {
                    continue;
                };
                let child_coordinate = record_pattern_child_coordinate(coordinate, field)?;
                let digest = self.pattern_digest(child, arm, &child_coordinate)?;
                child_digests.try_reserve_exact(1).map_err(|_| {
                    CheckedMatchBuildError::ArithmeticOverflow {
                        kind: CheckedMatchLimitKind::PatternNodes,
                    }
                })?;
                child_digests.push(digest);
            }
        } else {
            for edge in hir.kind().child_edges() {
                let HirPatternChild::Pattern(child) = edge.child() else {
                    continue;
                };
                let child_coordinate =
                    child_pattern_coordinate(coordinate, hir.kind(), edge.role())?;
                let digest = self.pattern_digest(child, arm, &child_coordinate)?;
                child_digests.try_reserve_exact(1).map_err(|_| {
                    CheckedMatchBuildError::ArithmeticOverflow {
                        kind: CheckedMatchLimitKind::PatternNodes,
                    }
                })?;
                child_digests.push(digest);
            }
        }
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-pattern-semantic.v1\0");
        transcript_update!(hasher, &coordinate.canonical_bytes()?);
        transcript_update!(hasher, &hir.kind().semantic_transcript_tag().to_le_bytes());
        transcript_update!(hasher, &checked.resolution().semantic_tag().to_le_bytes());
        transcript_update!(hasher, checked.ty().semantic_identity_digest().as_bytes());
        write_pattern_resolution(
            &mut hasher,
            checked.resolution(),
            checked.ty(),
            coordinate,
            self.analysis,
        )?;
        for digest in child_digests {
            transcript_update!(hasher, digest.as_bytes());
        }
        let digest = CheckedPatternSemanticDigest::from_bytes(hasher.finalize());
        self.pattern_visiting.remove(&owner);
        self.pattern_digests.insert(owner, digest);
        Ok(digest)
    }

    fn collect_pattern_bindings(
        &self,
        owner: PatternId,
        coordinate: &StablePatternCoordinate,
        bindings: &mut Vec<CheckedMatchBinding>,
    ) -> Result<(), SemanticTranscriptError> {
        let hir = self
            .module
            .resolve_pattern(owner)
            .map_err(|_| SemanticTranscriptError::MissingPattern)?;
        if matches!(hir.kind(), HirPatternKind::Error(_)) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let checked = self
            .analysis
            .pattern(owner)
            .ok_or(SemanticTranscriptError::MissingPattern)?;
        if let super::CheckedPatternResolution::Record(record) = checked.resolution() {
            for field in record.fields() {
                if let Some(binding) = field.source().binding() {
                    bindings.push(CheckedMatchBinding {
                        coordinate: StableCheckedValueCoordinate::Binding(
                            binding.coordinate().clone(),
                        ),
                        ty: field.field_type_digest(),
                    });
                } else if let Some(child) = field.source().raw_pattern() {
                    let child_coordinate = record_pattern_child_coordinate(coordinate, field)?;
                    self.collect_pattern_bindings(child, &child_coordinate, bindings)?;
                } else {
                    return Err(SemanticTranscriptError::UnsupportedIdentity);
                }
            }
            if let super::CheckedRecordPatternRest::Binding(binding) = record.rest() {
                let ty = self
                    .analysis
                    .local(binding.raw())
                    .ok_or(SemanticTranscriptError::MissingIdentity)?
                    .ty()
                    .semantic_identity_digest();
                bindings.push(CheckedMatchBinding {
                    coordinate: StableCheckedValueCoordinate::Binding(binding.coordinate().clone()),
                    ty,
                });
            }
            return Ok(());
        }
        for edge in hir.kind().child_edges() {
            match edge.child() {
                HirPatternChild::Local(local) => {
                    let ty = self
                        .analysis
                        .local(local)
                        .ok_or(SemanticTranscriptError::MissingIdentity)?
                        .ty()
                        .semantic_identity_digest();
                    bindings.push(CheckedMatchBinding {
                        coordinate: StableCheckedValueCoordinate::Binding(
                            self.coordinates.binding(local)?,
                        ),
                        ty,
                    });
                }
                HirPatternChild::Pattern(child) => {
                    let child_coordinate =
                        child_pattern_coordinate(coordinate, hir.kind(), edge.role())?;
                    self.collect_pattern_bindings(child, &child_coordinate, bindings)?;
                }
                HirPatternChild::Type(_) => {}
            }
        }
        Ok(())
    }
}

fn write_record_expression_fields(
    hasher: &mut MatchTranscriptHasher<'_>,
    edges: &super::CheckedExpressionEdgeFact,
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, edges.record_fields().len())?;
    for field in edges.record_fields() {
        transcript_update!(hasher, &field.source_ordinal().to_le_bytes());
        transcript_update!(hasher, &field.declaration_ordinal().to_le_bytes());
        transcript_update!(hasher, field.semantic_id().as_bytes());
        transcript_update!(hasher, field.field_type().as_bytes());
        match field.source() {
            super::CheckedRecordValueSource::Expression(source) => {
                transcript_update!(hasher, &[0]);
                transcript_update!(hasher, &source.coordinate().canonical_bytes()?);
            }
            super::CheckedRecordValueSource::Binding(source) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, &source.coordinate().canonical_bytes()?);
            }
        }
    }
    Ok(())
}

fn guard_class(
    analysis: &FinalSemanticAnalysis,
    owner: ExprId,
) -> Result<CheckedGuardClass, SemanticTranscriptError> {
    let guard = analysis
        .expression(owner)
        .ok_or(SemanticTranscriptError::MissingExpression)?;
    Ok(match guard.resolution() {
        CheckedExpressionResolution::Literal(HirLiteral::Boolean(true)) => {
            CheckedGuardClass::ConstantTrue
        }
        CheckedExpressionResolution::Literal(HirLiteral::Boolean(false)) => {
            CheckedGuardClass::ConstantFalse
        }
        _ => CheckedGuardClass::Dynamic,
    })
}

fn match_digest(
    budget: &mut CheckedMatchBudget,
    scrutinee: CheckedExpressionSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: &[CheckedMatchArm],
    coverage: &CheckedMatchCoverage,
) -> Result<CheckedMatchSemanticDigest, SemanticTranscriptError> {
    let mut hasher = TranscriptHasher::new(budget);
    transcript_update!(hasher, b"arcweft.lang.checked-match-semantic.v1\0");
    transcript_update!(hasher, scrutinee.as_bytes());
    transcript_update!(hasher, scrutinee_type.as_bytes());
    write_len(&mut hasher, arms.len())?;
    for arm in arms {
        transcript_update!(hasher, &arm.ordinal.to_le_bytes());
        transcript_update!(hasher, arm.pattern.as_bytes());
        write_len(&mut hasher, arm.bindings.len())?;
        for binding in &arm.bindings {
            transcript_update!(hasher, &binding.coordinate.canonical_bytes()?);
            transcript_update!(hasher, binding.ty.as_bytes());
        }
        match arm.guard_expression {
            Some(digest) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, digest.as_bytes());
                transcript_update!(hasher, &[guard_tag(arm.guard)]);
            }
            None => {
                transcript_update!(hasher, &[0]);
            }
        }
        transcript_update!(hasher, arm.value.as_bytes());
    }
    transcript_update!(hasher, &[u8::from(coverage.exhaustive())]);
    transcript_update!(hasher, coverage.domain_digest().as_bytes());
    write_len(&mut hasher, coverage.unreachable().len())?;
    for row in coverage.unreachable() {
        transcript_update!(hasher, &row.arm().owner().canonical_bytes()?);
        transcript_update!(hasher, &row.arm().ordinal().to_le_bytes());
        match row.alternative() {
            Some(alternative) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, &alternative.canonical_bytes()?);
            }
            None => transcript_update!(hasher, &[0]),
        }
        transcript_update!(hasher, &[unreachable_tag(row.reason())]);
    }
    Ok(CheckedMatchSemanticDigest::from_bytes(hasher.finalize()))
}

fn child_pattern_coordinate(
    parent: &StablePatternCoordinate,
    owner: &HirPatternKind,
    role: HirPatternChildRole,
) -> Result<StablePatternCoordinate, SemanticTranscriptError> {
    let mut steps = parent.steps().to_vec();
    let next = match role {
        HirPatternChildRole::VariantPayload => StablePatternCoordinateStep::VariantPayload,
        HirPatternChildRole::Element { ordinal } => match owner {
            HirPatternKind::BracketSequence { .. } => {
                StablePatternCoordinateStep::SequenceElement(ordinal)
            }
            _ => StablePatternCoordinateStep::TupleElement(ordinal),
        },
        HirPatternChildRole::RecordField { .. } => {
            // The current checked pattern fact does not retain the accepted
            // record-field identity for this child. Re-resolving it through
            // authored field spelling would make a semantic transcript
            // source-dependent, so this family remains fail-closed until the
            // exact checked field row is added to the pattern authority.
            return Err(SemanticTranscriptError::UnsupportedIdentity);
        }
        HirPatternChildRole::NestedPattern => StablePatternCoordinateStep::WholeBindingInner,
        HirPatternChildRole::OrAlternative { ordinal } => {
            StablePatternCoordinateStep::OrAlternative(ordinal)
        }
        HirPatternChildRole::TypedBindingType
        | HirPatternChildRole::BindingLocal
        | HirPatternChildRole::MutableBindingLocal
        | HirPatternChildRole::RecordShorthandLocal { .. }
        | HirPatternChildRole::RecordRestLocal { .. }
        | HirPatternChildRole::SequenceRestLocal
        | HirPatternChildRole::WholeBindingLocal
        | HirPatternChildRole::TypedBindingLocal => StablePatternCoordinateStep::TypedBindingInner,
    };
    steps.push(next);
    Ok(StablePatternCoordinate::new(steps))
}

fn record_pattern_child_coordinate(
    parent: &StablePatternCoordinate,
    field: &super::CheckedRecordPatternField,
) -> Result<StablePatternCoordinate, SemanticTranscriptError> {
    let relative = field
        .source()
        .pattern_coordinate()
        .ok_or(SemanticTranscriptError::UnsupportedIdentity)?;
    let mut steps = parent.steps().to_vec();
    steps.extend_from_slice(relative.steps());
    Ok(StablePatternCoordinate::new(steps))
}

fn write_literal(
    hasher: &mut MatchTranscriptHasher<'_>,
    literal: &HirLiteral,
    ty: &TypeKind,
) -> Result<(), SemanticTranscriptError> {
    super::match_coverage::encode_canonical_literal(literal, ty, |bytes| hasher.update(bytes))
        .map_err(|error| match error {
            super::match_coverage::CanonicalLiteralEncodingError::Invalid => {
                SemanticTranscriptError::RecoveredOwner
            }
            super::match_coverage::CanonicalLiteralEncodingError::ArithmeticOverflow => {
                CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::TranscriptBytes,
                }
                .into()
            }
            super::match_coverage::CanonicalLiteralEncodingError::Sink(error) => error.into(),
        })
}

fn write_pattern_resolution(
    hasher: &mut MatchTranscriptHasher<'_>,
    resolution: &super::CheckedPatternResolution,
    ty: &TypeKind,
    coordinate: &StablePatternCoordinate,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match resolution {
        super::CheckedPatternResolution::Structural => {}
        super::CheckedPatternResolution::Literal(literal) => write_literal(hasher, literal, ty)?,
        super::CheckedPatternResolution::Record(record) => {
            let nominal = record
                .owner()
                .project_nominal()
                .ok_or(SemanticTranscriptError::UnsupportedIdentity)?;
            write_nominal(hasher, nominal, analysis)?;
            match record.rest() {
                super::CheckedRecordPatternRest::Absent => {
                    transcript_update!(hasher, &[0]);
                }
                super::CheckedRecordPatternRest::Ignore => {
                    transcript_update!(hasher, &[1]);
                }
                super::CheckedRecordPatternRest::Binding(binding) => {
                    transcript_update!(hasher, &[2]);
                    transcript_update!(hasher, &binding.coordinate().canonical_bytes()?);
                }
            }
            write_len(hasher, record.fields().len())?;
            for field in record.fields() {
                transcript_update!(hasher, &field.source_ordinal().to_le_bytes());
                transcript_update!(hasher, &field.declaration_ordinal().to_le_bytes());
                transcript_update!(hasher, field.semantic_id().as_bytes());
                transcript_update!(hasher, field.field_type_digest().as_bytes());
                if field.source().pattern_coordinate().is_some() {
                    transcript_update!(hasher, &[0]);
                    let child = record_pattern_child_coordinate(coordinate, field)?;
                    transcript_update!(hasher, &child.canonical_bytes()?);
                } else if let Some(binding) = field.source().binding() {
                    transcript_update!(hasher, &[1]);
                    transcript_update!(hasher, &binding.coordinate().canonical_bytes()?);
                } else {
                    return Err(SemanticTranscriptError::UnsupportedIdentity);
                }
            }
        }
        super::CheckedPatternResolution::Variant(variant) => {
            write_variant_resolution(hasher, variant)?;
        }
        super::CheckedPatternResolution::TypedBinding(binding) => {
            transcript_update!(hasher, binding.annotation_digest().as_bytes());
        }
        super::CheckedPatternResolution::Entity(_) => {
            return Err(SemanticTranscriptError::UnsupportedIdentity);
        }
    }
    Ok(())
}

fn write_resolution_payload(
    hasher: &mut MatchTranscriptHasher<'_>,
    resolution: &CheckedExpressionResolution,
    ty: &TypeKind,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match resolution {
        CheckedExpressionResolution::Structural
        | CheckedExpressionResolution::Literal(_)
        | CheckedExpressionResolution::Call => {}
        CheckedExpressionResolution::Value(value) => match value {
            CheckedValueResolution::Local(local) => {
                let binding = coordinates.binding(*local)?;
                transcript_update!(hasher, &binding.canonical_bytes()?);
            }
            CheckedValueResolution::LineContext => {}
            CheckedValueResolution::ProjectCallable(callable) => {
                write_project_callable(hasher, analysis, callable)?;
            }
            CheckedValueResolution::Registered(value) => {
                transcript_update!(hasher, value.as_bytes());
            }
            CheckedValueResolution::Constant(literal) => write_literal(hasher, literal, ty)?,
            CheckedValueResolution::CharacterField {
                character, field, ..
            } => {
                write_bytes(hasher, character.as_str().as_bytes())?;
                transcript_update!(
                    hasher,
                    &[match field {
                        crate::types::CharacterField::Stage => 0,
                    }]
                );
            }
            CheckedValueResolution::ProjectItem(_) | CheckedValueResolution::Entry(_) => {
                return Err(SemanticTranscriptError::UnsupportedIdentity);
            }
        },
        CheckedExpressionResolution::Select(select) => match select {
            CheckedSelectResolution::Field(selection) => {
                transcript_update!(hasher, selection.owner_type().as_bytes());
                transcript_update!(hasher, selection.field().as_bytes());
                transcript_update!(hasher, &selection.declaration_ordinal().to_le_bytes());
                transcript_update!(hasher, selection.field_type().as_bytes());
                match selection.runtime_field() {
                    Some(runtime_field) => {
                        transcript_update!(hasher, &[1]);
                        transcript_update!(hasher, &runtime_field.get().get().to_le_bytes());
                    }
                    None => transcript_update!(hasher, &[0]),
                }
            }
            CheckedSelectResolution::ProgressField { field } => {
                transcript_update!(
                    hasher,
                    &[match field {
                        crate::types::ProgressField::Ratio => 0,
                        crate::types::ProgressField::Label => 1,
                    }]
                );
            }
            CheckedSelectResolution::Method(method) => {
                transcript_update!(hasher, method.callable().as_bytes());
                transcript_update!(hasher, method.receiver_type().as_bytes());
                match method.receiver_mode() {
                    crate::callable::CallableReceiverMode::None => {
                        return Err(SemanticTranscriptError::UnsupportedIdentity);
                    }
                    crate::callable::CallableReceiverMode::Value { .. } => {
                        transcript_update!(hasher, &[0]);
                    }
                    crate::callable::CallableReceiverMode::Type { .. } => {
                        transcript_update!(hasher, &[1]);
                    }
                    crate::callable::CallableReceiverMode::Extension {
                        group, parameter, ..
                    } => {
                        transcript_update!(hasher, &[2]);
                        transcript_update!(
                            hasher,
                            &u64::try_from(group.get())
                                .map_err(|_| SemanticTranscriptError::UnsupportedIdentity)?
                                .to_le_bytes(),
                        );
                        transcript_update!(
                            hasher,
                            &u64::try_from(parameter.get())
                                .map_err(|_| SemanticTranscriptError::UnsupportedIdentity)?
                                .to_le_bytes(),
                        );
                    }
                }
            }
            CheckedSelectResolution::DialogueView { .. }
            | CheckedSelectResolution::AgentField { .. } => {
                return Err(SemanticTranscriptError::UnsupportedIdentity);
            }
        },
        CheckedExpressionResolution::Nominal(nominal) => {
            write_nominal(hasher, nominal, analysis)?;
        }
        CheckedExpressionResolution::Variant(variant) => {
            write_variant_resolution(hasher, variant)?;
        }
        CheckedExpressionResolution::Effect(effect) => {
            write_bytes(hasher, effect.as_str().as_bytes())?;
        }
        CheckedExpressionResolution::StageLook(look) => {
            transcript_update!(hasher, look.look().as_bytes());
        }
        CheckedExpressionResolution::Await(_)
        | CheckedExpressionResolution::Choice(_)
        | CheckedExpressionResolution::Try(_)
        | CheckedExpressionResolution::ImplicitCallable(_)
        | CheckedExpressionResolution::Closure(_)
        | CheckedExpressionResolution::ImplicitParameter { .. }
        | CheckedExpressionResolution::Pipe(_)
        | CheckedExpressionResolution::PipeLeft { .. }
        | CheckedExpressionResolution::ViewCall(_)
        | CheckedExpressionResolution::ViewCallee(_)
        | CheckedExpressionResolution::StyleValue(_)
        | CheckedExpressionResolution::StyleCallee(_)
        | CheckedExpressionResolution::DialogueLineReference(_)
        | CheckedExpressionResolution::DialogueLineCoordinate(_)
        | CheckedExpressionResolution::DialogueTextKeyCoordinate(_)
        | CheckedExpressionResolution::CharacterDialogueFactory(_)
        | CheckedExpressionResolution::CharacterDialogueReconfigure(_)
        | CheckedExpressionResolution::DialogueApplication { .. }
        | CheckedExpressionResolution::PostfixBracket(_) => {
            return Err(SemanticTranscriptError::UnsupportedIdentity);
        }
    }
    Ok(())
}

fn write_nominal(
    hasher: &mut MatchTranscriptHasher<'_>,
    nominal: &super::CheckedProjectNominal,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, nominal.identity().as_bytes());
    let definition = analysis
        .project_nominal_semantic(nominal.identity())
        .filter(|definition| definition.nominal() == nominal)
        .ok_or(SemanticTranscriptError::UnsupportedIdentity)?;
    transcript_update!(hasher, definition.digest().as_bytes());
    write_len(hasher, nominal.arguments().len())?;
    for argument in nominal.arguments() {
        transcript_update!(hasher, argument.semantic_identity_digest().as_bytes());
    }
    Ok(())
}

fn write_project_callable(
    hasher: &mut MatchTranscriptHasher<'_>,
    analysis: &FinalSemanticAnalysis,
    callable: &super::CheckedProjectCallable,
) -> Result<(), SemanticTranscriptError> {
    let facts = analysis
        .checked_callables()
        .project_callable(callable.declaration())
        .map_err(|_| SemanticTranscriptError::UnsupportedIdentity)?;
    transcript_update!(hasher, facts.id().semantic_digest().as_bytes());
    transcript_update!(hasher, facts.interface_digest().as_bytes());
    Ok(())
}

fn write_variant_resolution(
    hasher: &mut MatchTranscriptHasher<'_>,
    resolution: &super::CheckedVariantResolution,
) -> Result<(), SemanticTranscriptError> {
    let owner_tag = match resolution.owner() {
        CheckedVariantOwner::Project { .. } => 0,
        CheckedVariantOwner::CharacterNominal { .. } => 1,
        CheckedVariantOwner::BuiltinClosed { .. } => 2,
        CheckedVariantOwner::Option { .. } => 3,
        CheckedVariantOwner::Result { .. } => 4,
        CheckedVariantOwner::RuntimeBuiltin { .. } => 5,
    };
    transcript_update!(hasher, &[owner_tag]);
    transcript_update!(hasher, resolution.owner().semantic_type().as_bytes());
    let selected = resolution.selected();
    transcript_update!(hasher, selected.semantic_id().as_bytes());
    transcript_update!(hasher, &selected.ordinal().to_le_bytes());
    match selected.payload() {
        crate::types::VariantPayloadShape::Unit => {
            transcript_update!(hasher, &[0]);
        }
        crate::types::VariantPayloadShape::Tuple(fields) => {
            transcript_update!(hasher, &[1]);
            write_len(hasher, fields.len())?;
            for field in fields {
                transcript_update!(hasher, &field.ordinal().to_le_bytes());
                transcript_update!(hasher, field.semantic_id().as_bytes());
                transcript_update!(hasher, field.ty().semantic_identity_digest().as_bytes());
            }
        }
        crate::types::VariantPayloadShape::Record(fields) => {
            transcript_update!(hasher, &[2]);
            write_len(hasher, fields.len())?;
            for field in fields {
                transcript_update!(hasher, &field.ordinal().to_le_bytes());
                transcript_update!(hasher, field.semantic_id().as_bytes());
                transcript_update!(hasher, field.ty().semantic_identity_digest().as_bytes());
            }
        }
    }
    Ok(())
}

fn write_effects(
    hasher: &mut MatchTranscriptHasher<'_>,
    effects: &crate::effects::EffectSet,
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, effects.len())?;
    for effect in effects.iter() {
        write_bytes(hasher, effect.as_str().as_bytes())?;
    }
    Ok(())
}

pub(crate) fn write_len<C: TranscriptByteCounter + ?Sized>(
    hasher: &mut TranscriptHasher<'_, C>,
    value: usize,
) -> Result<(), C::Error>
where
    C::Error: From<TranscriptWriteError>,
{
    let value = u64::try_from(value)
        .map_err(|_| C::Error::from(TranscriptWriteError::ArithmeticOverflow))?;
    hasher.update(&value.to_le_bytes())
}

fn write_bytes(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: &[u8],
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, value.len())?;
    transcript_update!(hasher, value);
    Ok(())
}

fn guard_tag(value: CheckedGuardClass) -> u8 {
    match value {
        CheckedGuardClass::Absent => 0,
        CheckedGuardClass::ConstantTrue => 1,
        CheckedGuardClass::ConstantFalse => 2,
        CheckedGuardClass::Dynamic => 3,
    }
}

fn unreachable_tag(value: CheckedUnreachableReason) -> u8 {
    match value {
        CheckedUnreachableReason::CoveredByPriorUsefulArms => 0,
        CheckedUnreachableReason::ConstantFalseGuard => 1,
        CheckedUnreachableReason::CoveredByEarlierOrAlternative => 2,
        CheckedUnreachableReason::UninhabitedDomain => 3,
    }
}
