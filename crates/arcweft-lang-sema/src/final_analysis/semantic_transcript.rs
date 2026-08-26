//! Stable generic-Match semantic transcripts.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    CheckedCoverageDomainDigest, CheckedExpressionResolution, CheckedExpressionSemanticDigest,
    CheckedMatchRef, CheckedMatchSemanticDigest, CheckedPatternSemanticDigest,
    CheckedSelectResolution, CheckedValueResolution, CheckedVariantOwner, FinalSemanticAnalysis,
};
use crate::semantic_coordinate::{
    AcceptedSemanticRootCatalogError, CheckedSemanticPath, SemanticCoordinateIndex,
    SemanticCoordinateIndexError, StableCheckedValueCoordinate, StablePatternCoordinate,
    StablePatternCoordinateStep,
};
use crate::types::{SemanticTypeDigest, TypeKind};
use arcweft_core::entry::TypeLayoutHash;
use arcweft_lang_hir::{
    expr::{HirExprKind, HirMatchExpr},
    identity::{ExprId, PatternId},
    leaf::{
        HirCharacterLiteral, HirDurationLiteral, HirFloatLiteral, HirIntegerLiteral, HirLiteral,
        HirStringLiteral, HirUnitNumberLiteral, HirUnitNumberUnit,
    },
    module::HirModule,
    pattern::{HirPatternChild, HirPatternChildRole, HirPatternKind, HirVariantPatternPayload},
    project::HirExecutableProjectView,
    symbol::{ProjectSymbolTable, nominal::ProjectNominalBody},
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedMatchLimits {
    max_arms: u32,
    max_pattern_nodes: u64,
    max_expression_nodes: u64,
    max_transcript_bytes: u64,
    max_unreachable_rows: u32,
    max_depth: u32,
    max_coverage_states: u32,
}

impl CheckedMatchLimits {
    pub const PRODUCTION: Self = Self {
        max_arms: 4_096,
        max_pattern_nodes: 65_536,
        max_expression_nodes: 65_536,
        max_transcript_bytes: 16 * 1024 * 1024,
        max_unreachable_rows: 4_096,
        max_depth: 256,
        max_coverage_states: 65_536,
    };

    /// Constructs an explicit bounded transcript policy for callers that need
    /// a smaller admission budget (for example, a tooling preview).
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        max_arms: u32,
        max_pattern_nodes: u64,
        max_expression_nodes: u64,
        max_transcript_bytes: u64,
        max_unreachable_rows: u32,
        max_depth: u32,
        max_coverage_states: u32,
    ) -> Self {
        Self {
            max_arms,
            max_pattern_nodes,
            max_expression_nodes,
            max_transcript_bytes,
            max_unreachable_rows,
            max_depth,
            max_coverage_states,
        }
    }

    pub const fn max_transcript_bytes(self) -> u64 {
        self.max_transcript_bytes
    }

    pub const fn max_depth(self) -> u32 {
        self.max_depth
    }
}

impl Default for CheckedMatchLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SemanticTranscriptError {
    #[error(transparent)]
    Generation(#[from] super::FinalSemanticAnalysisError),
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
    #[error("semantic transcript cannot resolve an accepted identity")]
    MissingIdentity,
    #[error(transparent)]
    AcceptedRootCatalog(AcceptedSemanticRootCatalogError),
    #[error("semantic transcript identity family is not supported by this cut")]
    UnsupportedIdentity,
    #[error("Match coverage family is outside the bounded exact transcript space")]
    UnsupportedCoverage,
    #[error("Match is not exhaustive; coverage witness is retained in the error")]
    NonExhaustive { witness: CheckedCoverageWitness },
}

impl From<SemanticCoordinateIndexError> for SemanticTranscriptError {
    fn from(error: SemanticCoordinateIndexError) -> Self {
        match error {
            SemanticCoordinateIndexError::RootCatalog(error) => Self::AcceptedRootCatalog(error),
            SemanticCoordinateIndexError::MissingChildEdges => Self::MissingChildEdges,
            SemanticCoordinateIndexError::MissingExpression
            | SemanticCoordinateIndexError::MissingBinding
            | SemanticCoordinateIndexError::InvalidBindingPath
            | SemanticCoordinateIndexError::ExpressionRoleMismatch
            | SemanticCoordinateIndexError::InvalidRootPath => Self::MissingIdentity,
        }
    }
}

/// Counts the exact bytes fed to one semantic digest hasher.
///
/// The byte ceiling is checked at the digest boundary.  Hashing is kept
/// private to this module so callers receive a typed rejection rather than a
/// partial digest when a transcript exceeds its admission policy.
pub(crate) struct TranscriptHasher<'a> {
    hasher: blake3::Hasher,
    used: &'a mut u64,
    limit: u64,
    exceeded: bool,
}

impl<'a> TranscriptHasher<'a> {
    pub(crate) fn new(used: &'a mut u64, limit: u64) -> Self {
        Self {
            hasher: blake3::Hasher::new(),
            used,
            limit,
            exceeded: false,
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        let next = self.used.saturating_add(bytes.len() as u64);
        if next > self.limit {
            self.exceeded = true;
            return;
        }
        *self.used = next;
        self.hasher.update(bytes);
    }

    pub(crate) fn finalize(self) -> Result<[u8; 32], SemanticTranscriptError> {
        if self.exceeded {
            Err(SemanticTranscriptError::WorkLimit)
        } else {
            Ok(*self.hasher.finalize().as_bytes())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedGuardClass {
    Absent,
    ConstantTrue,
    ConstantFalse,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedUnreachableReason {
    CoveredByPriorRows,
    FalseGuard,
    RedundantOrAlternative,
    UninhabitedDomain,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedUnreachableArm {
    arm: u32,
    reason: CheckedUnreachableReason,
}

impl CheckedUnreachableArm {
    pub const fn arm(self) -> u32 {
        self.arm
    }

    pub const fn reason(self) -> CheckedUnreachableReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCoverageWitness {
    OpenDomain,
    BooleanFalse,
    BooleanTrue,
    Variant { ordinal: u32 },
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchCoverage {
    exhaustive: bool,
    unreachable: Box<[CheckedUnreachableArm]>,
    witness: Option<CheckedCoverageWitness>,
    domain_digest: CheckedCoverageDomainDigest,
}

impl CheckedMatchCoverage {
    pub const fn exhaustive(&self) -> bool {
        self.exhaustive
    }

    pub fn unreachable(&self) -> &[CheckedUnreachableArm] {
        &self.unreachable
    }

    pub const fn witness(&self) -> Option<&CheckedCoverageWitness> {
        self.witness.as_ref()
    }

    pub const fn domain_digest(&self) -> CheckedCoverageDomainDigest {
        self.domain_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchBinding {
    coordinate: StableCheckedValueCoordinate,
    ty: SemanticTypeDigest,
}

impl CheckedMatchBinding {
    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub const fn ty(&self) -> SemanticTypeDigest {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchArm {
    ordinal: u32,
    pattern: CheckedPatternSemanticDigest,
    guard: CheckedGuardClass,
    guard_expression: Option<CheckedExpressionSemanticDigest>,
    value: CheckedExpressionSemanticDigest,
    bindings: Box<[CheckedMatchBinding]>,
}

impl CheckedMatchArm {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub const fn pattern(&self) -> CheckedPatternSemanticDigest {
        self.pattern
    }
    pub const fn guard(&self) -> CheckedGuardClass {
        self.guard
    }
    pub const fn guard_expression(&self) -> Option<CheckedExpressionSemanticDigest> {
        self.guard_expression
    }
    pub const fn value(&self) -> CheckedExpressionSemanticDigest {
        self.value
    }
    pub fn bindings(&self) -> &[CheckedMatchBinding] {
        &self.bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatch {
    semantic_digest: CheckedMatchSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: Box<[CheckedMatchArm]>,
    coverage: CheckedMatchCoverage,
}

impl CheckedMatch {
    pub const fn semantic_digest(&self) -> CheckedMatchSemanticDigest {
        self.semantic_digest
    }
    pub const fn scrutinee_type(&self) -> SemanticTypeDigest {
        self.scrutinee_type
    }
    pub fn arms(&self) -> &[CheckedMatchArm] {
        &self.arms
    }
    pub const fn coverage(&self) -> &CheckedMatchCoverage {
        &self.coverage
    }
}

impl FinalSemanticAnalysis {
    /// Binds one checked Match lookup to this report's exact module snapshot.
    pub fn checked_match_ref(
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
    pub fn build_checked_match_for_ref(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        reference: CheckedMatchRef,
        limits: CheckedMatchLimits,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
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
        if authored.arms().len() > usize::try_from(limits.max_arms).unwrap_or(usize::MAX) {
            return Err(SemanticTranscriptError::WorkLimit);
        }
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
            symbols,
            coordinates,
            limits,
            expression_nodes: 0,
            pattern_nodes: 0,
            transcript_bytes: 0,
            expression_digests: BTreeMap::new(),
            pattern_digests: BTreeMap::new(),
        }
        .build(expression, authored)
    }
}

struct MatchTranscriptBuilder<'analysis, 'paths, 'edges> {
    analysis: &'analysis FinalSemanticAnalysis,
    module: &'analysis HirModule,
    symbols: &'analysis ProjectSymbolTable,
    coordinates: SemanticCoordinateIndex<'paths, 'edges>,
    limits: CheckedMatchLimits,
    expression_nodes: u64,
    pattern_nodes: u64,
    transcript_bytes: u64,
    expression_digests: BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    pattern_digests: BTreeMap<(PatternId, StablePatternCoordinate), CheckedPatternSemanticDigest>,
}

impl<'analysis, 'paths, 'edges> MatchTranscriptBuilder<'analysis, 'paths, 'edges> {
    fn build(
        &mut self,
        owner: ExprId,
        authored: &HirMatchExpr,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
        let scrutinee = self.expression_digest(authored.scrutinee())?;
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
        let mut arms = Vec::with_capacity(authored.arms().len());
        for (ordinal, (arm, checked)) in authored.arms().iter().zip(fact.arms()).enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| SemanticTranscriptError::WorkLimit)?;
            let pattern = self.pattern_digest(arm.pattern(), &StablePatternCoordinate::new([]))?;
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
        }
        let coverage = basic_coverage(
            self.analysis,
            self.module,
            self.symbols,
            &scrutinee_ty,
            authored,
            &arms,
            self.limits,
        )?;
        if let Some(witness) = coverage.witness().copied() {
            return Err(SemanticTranscriptError::NonExhaustive { witness });
        }
        Ok(CheckedMatch {
            semantic_digest: match_digest(
                &mut self.transcript_bytes,
                self.limits.max_transcript_bytes,
                scrutinee,
                scrutinee_type,
                &arms,
                &coverage,
            )?,
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
        depth: u32,
    ) -> Result<CheckedExpressionSemanticDigest, SemanticTranscriptError> {
        if depth > self.limits.max_depth {
            return Err(SemanticTranscriptError::WorkLimit);
        }
        if let Some(digest) = self.expression_digests.get(&owner) {
            return Ok(*digest);
        }
        self.expression_nodes = self.expression_nodes.saturating_add(1);
        if self.expression_nodes > self.limits.max_expression_nodes {
            return Err(SemanticTranscriptError::WorkLimit);
        }
        let checked = self
            .analysis
            .expression(owner)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let path = self.checked_path(owner)?;
        if path.steps().len() > usize::try_from(self.limits.max_depth).unwrap_or(usize::MAX) {
            return Err(SemanticTranscriptError::WorkLimit);
        }
        let edges = self
            .analysis
            .checked_expression_edge_fact(owner)
            .map_err(|_| SemanticTranscriptError::MissingChildEdges)?;
        let child_digests = edges
            .edges()
            .iter()
            .map(|(child, _)| self.expression_digest_at(*child, depth.saturating_add(1)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut hasher =
            TranscriptHasher::new(&mut self.transcript_bytes, self.limits.max_transcript_bytes);
        hasher.update(b"arcweft.lang.checked-expression-semantic.v1\0");
        hasher.update(&path.canonical_bytes());
        hasher.update(&checked.resolution().semantic_tag().to_le_bytes());
        hasher.update(checked.ty().semantic_identity_digest().as_bytes());
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
        write_record_expression_fields(&mut hasher, edges);
        write_effects(&mut hasher, checked.effects());
        if matches!(checked.resolution(), CheckedExpressionResolution::Call) {
            let callable = edges
                .callable()
                .ok_or(SemanticTranscriptError::MissingCallableJoin)?;
            hasher.update(callable.semantic_digest().as_bytes());
        }
        write_len(&mut hasher, edges.edges().len());
        for ((_, role), child_digest) in edges.edges().iter().zip(child_digests) {
            write_bytes(&mut hasher, &role.transcript_bytes());
            hasher.update(child_digest.as_bytes());
        }
        let digest = CheckedExpressionSemanticDigest::from_bytes(hasher.finalize()?);
        self.expression_digests.insert(owner, digest);
        Ok(digest)
    }

    fn checked_path(&self, owner: ExprId) -> Result<CheckedSemanticPath, SemanticTranscriptError> {
        Ok(self.coordinates.expression(owner)?)
    }

    fn pattern_digest(
        &mut self,
        owner: PatternId,
        coordinate: &StablePatternCoordinate,
    ) -> Result<CheckedPatternSemanticDigest, SemanticTranscriptError> {
        let depth = u32::try_from(coordinate.steps().len())
            .map_err(|_| SemanticTranscriptError::WorkLimit)?;
        if depth > self.limits.max_depth {
            return Err(SemanticTranscriptError::WorkLimit);
        }
        if let Some(digest) = self.pattern_digests.get(&(owner, coordinate.clone())) {
            return Ok(*digest);
        }
        self.pattern_nodes = self.pattern_nodes.saturating_add(1);
        if self.pattern_nodes > self.limits.max_pattern_nodes {
            return Err(SemanticTranscriptError::WorkLimit);
        }
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
        let child_digests = if let super::CheckedPatternResolution::Record(record) =
            checked.resolution()
        {
            record
                .fields()
                .iter()
                .filter_map(|field| field.source().raw_pattern().map(|child| (child, field)))
                .map(|(child, field)| {
                    let child_coordinate = record_pattern_child_coordinate(coordinate, field)?;
                    self.pattern_digest(child, &child_coordinate)
                })
                .collect::<Result<Vec<_>, SemanticTranscriptError>>()?
        } else {
            hir.kind()
                .child_edges()
                .into_iter()
                .filter_map(|edge| match edge.child() {
                    HirPatternChild::Pattern(child) => Some((child, edge.role())),
                    HirPatternChild::Type(_) | HirPatternChild::Local(_) => None,
                })
                .map(|(child, role)| {
                    let child_coordinate = child_pattern_coordinate(coordinate, hir.kind(), role)?;
                    self.pattern_digest(child, &child_coordinate)
                })
                .collect::<Result<Vec<_>, SemanticTranscriptError>>()?
        };
        let mut hasher =
            TranscriptHasher::new(&mut self.transcript_bytes, self.limits.max_transcript_bytes);
        hasher.update(b"arcweft.lang.checked-pattern-semantic.v1\0");
        hasher.update(&coordinate.canonical_bytes());
        hasher.update(&[pattern_kind_tag(hir.kind())]);
        hasher.update(&checked.resolution().semantic_tag().to_le_bytes());
        hasher.update(checked.ty().semantic_identity_digest().as_bytes());
        write_pattern_resolution(
            &mut hasher,
            checked.resolution(),
            checked.ty(),
            coordinate,
            self.analysis,
        )?;
        for digest in child_digests {
            hasher.update(digest.as_bytes());
        }
        let digest = CheckedPatternSemanticDigest::from_bytes(hasher.finalize()?);
        self.pattern_digests
            .insert((owner, coordinate.clone()), digest);
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
    hasher: &mut TranscriptHasher<'_>,
    edges: &super::CheckedExpressionEdgeFact,
) {
    write_len(hasher, edges.record_fields().len());
    for field in edges.record_fields() {
        hasher.update(&field.source_ordinal().to_le_bytes());
        hasher.update(&field.declaration_ordinal().to_le_bytes());
        hasher.update(field.semantic_id().as_bytes());
        hasher.update(field.field_type().as_bytes());
        match field.source() {
            super::CheckedRecordValueSource::Expression(source) => {
                hasher.update(&[0]);
                hasher.update(&source.coordinate().canonical_bytes());
            }
            super::CheckedRecordValueSource::Binding(source) => {
                hasher.update(&[1]);
                hasher.update(&source.coordinate().canonical_bytes());
            }
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum CoverageAtom {
    BooleanFalse,
    BooleanTrue,
    Variant(u32),
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CoverageShape {
    All,
    Atoms(BTreeSet<CoverageAtom>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CoverageDomain {
    Open,
    Finite(Box<[CoverageAtom]>),
}

fn basic_coverage(
    analysis: &FinalSemanticAnalysis,
    module: &HirModule,
    symbols: &ProjectSymbolTable,
    scrutinee_type: &TypeKind,
    authored: &HirMatchExpr,
    arms: &[CheckedMatchArm],
    limits: CheckedMatchLimits,
) -> Result<CheckedMatchCoverage, SemanticTranscriptError> {
    let domain = coverage_domain(scrutinee_type, symbols)?;
    let domain_digest = coverage_domain_digest(analysis, symbols, scrutinee_type)?;
    if let CoverageDomain::Finite(values) = &domain {
        if values.len() > usize::try_from(limits.max_coverage_states).unwrap_or(usize::MAX) {
            return Err(SemanticTranscriptError::WorkLimit);
        }
    }
    let mut unreachable = Vec::new();
    let mut covered = BTreeSet::new();
    let mut covered_all = false;
    for (arm, checked) in authored.arms().iter().zip(arms) {
        if checked.guard == CheckedGuardClass::ConstantFalse {
            unreachable.push(CheckedUnreachableArm {
                arm: checked.ordinal,
                reason: CheckedUnreachableReason::FalseGuard,
            });
            continue;
        }
        let shape = pattern_coverage(analysis, module, arm.pattern(), &domain)?;
        let guarded = matches!(
            checked.guard,
            CheckedGuardClass::Absent | CheckedGuardClass::ConstantTrue
        );
        let possible = match (&domain, &shape) {
            (CoverageDomain::Open, CoverageShape::All) => None,
            (CoverageDomain::Open, CoverageShape::Atoms(_)) => {
                return Err(SemanticTranscriptError::UnsupportedCoverage);
            }
            (CoverageDomain::Finite(values), CoverageShape::All) => {
                Some(values.iter().copied().collect::<BTreeSet<_>>())
            }
            (CoverageDomain::Finite(values), CoverageShape::Atoms(atoms)) => {
                let values = values.iter().copied().collect::<BTreeSet<_>>();
                let intersection = atoms
                    .intersection(&values)
                    .copied()
                    .collect::<BTreeSet<_>>();
                if intersection.is_empty() {
                    unreachable.push(CheckedUnreachableArm {
                        arm: checked.ordinal,
                        reason: CheckedUnreachableReason::UninhabitedDomain,
                    });
                    continue;
                }
                Some(intersection)
            }
        };
        match possible {
            None => {
                if covered_all {
                    unreachable.push(CheckedUnreachableArm {
                        arm: checked.ordinal,
                        reason: CheckedUnreachableReason::CoveredByPriorRows,
                    });
                } else if guarded {
                    covered_all = true;
                }
            }
            Some(possible) => {
                let uncovered = possible
                    .difference(&covered)
                    .copied()
                    .collect::<BTreeSet<_>>();
                if uncovered.is_empty() {
                    unreachable.push(CheckedUnreachableArm {
                        arm: checked.ordinal,
                        reason: CheckedUnreachableReason::CoveredByPriorRows,
                    });
                } else if guarded {
                    covered.extend(uncovered);
                }
            }
        }
    }
    if unreachable.len() > usize::try_from(limits.max_unreachable_rows).unwrap_or(usize::MAX) {
        return Err(SemanticTranscriptError::WorkLimit);
    }
    let exhaustive = match &domain {
        CoverageDomain::Open => covered_all,
        CoverageDomain::Finite(values) => values.iter().all(|value| covered.contains(value)),
    };
    let witness = (!exhaustive).then(|| match &domain {
        CoverageDomain::Open => CheckedCoverageWitness::OpenDomain,
        CoverageDomain::Finite(values) => values
            .iter()
            .find(|value| !covered.contains(value))
            .map_or(CheckedCoverageWitness::OpenDomain, |value| match value {
                CoverageAtom::BooleanFalse => CheckedCoverageWitness::BooleanFalse,
                CoverageAtom::BooleanTrue => CheckedCoverageWitness::BooleanTrue,
                CoverageAtom::Variant(ordinal) => {
                    CheckedCoverageWitness::Variant { ordinal: *ordinal }
                }
                CoverageAtom::Unit => CheckedCoverageWitness::Unit,
            }),
    });
    Ok(CheckedMatchCoverage {
        exhaustive,
        unreachable: unreachable.into_boxed_slice(),
        witness,
        domain_digest,
    })
}

fn coverage_domain(
    ty: &TypeKind,
    symbols: &ProjectSymbolTable,
) -> Result<CoverageDomain, SemanticTranscriptError> {
    let values = match ty {
        TypeKind::Bool => vec![CoverageAtom::BooleanFalse, CoverageAtom::BooleanTrue],
        TypeKind::Unit => vec![CoverageAtom::Unit],
        TypeKind::Option(_) | TypeKind::Result { .. } => {
            vec![CoverageAtom::Variant(0), CoverageAtom::Variant(1)]
        }
        TypeKind::ProjectNominal(nominal) => {
            let declaration = symbols
                .nominal(nominal.declaration())
                .ok_or(SemanticTranscriptError::MissingIdentity)?;
            let ProjectNominalBody::Enum { variants } = declaration.body() else {
                return Ok(CoverageDomain::Open);
            };
            variants
                .iter()
                .enumerate()
                .map(|(ordinal, _)| {
                    u32::try_from(ordinal)
                        .map(CoverageAtom::Variant)
                        .map_err(|_| SemanticTranscriptError::WorkLimit)
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Ok(CoverageDomain::Open),
    };
    Ok(CoverageDomain::Finite(values.into_boxed_slice()))
}

fn coverage_domain_digest(
    analysis: &FinalSemanticAnalysis,
    symbols: &ProjectSymbolTable,
    ty: &TypeKind,
) -> Result<CheckedCoverageDomainDigest, SemanticTranscriptError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.lang.checked-coverage-domain.v1\0");
    match ty {
        TypeKind::Bool => {
            hasher.update(&[0]);
        }
        TypeKind::Unit => {
            hasher.update(&[1]);
        }
        TypeKind::Option(item) => {
            hasher.update(&[2]);
            hasher.update(item.semantic_identity_digest().as_bytes());
        }
        TypeKind::Result { ok, error } => {
            hasher.update(&[3]);
            hasher.update(ok.semantic_identity_digest().as_bytes());
            hasher.update(error.semantic_identity_digest().as_bytes());
        }
        TypeKind::ProjectNominal(nominal) => {
            hasher.update(&[4]);
            let declaration = symbols
                .nominal(nominal.declaration())
                .ok_or(SemanticTranscriptError::MissingIdentity)?;
            let nominal_identity =
                TypeKind::ProjectNominal(nominal.clone()).semantic_identity_digest();
            hasher.update(nominal_identity.as_bytes());
            let checked_nominal = super::CheckedProjectNominal::new(
                declaration.id().clone(),
                declaration.owner(),
                nominal_identity,
                nominal.arguments().to_vec(),
            );
            hasher.update(nominal_layout_hash(analysis, &checked_nominal)?.as_bytes());
            let ProjectNominalBody::Enum { variants } = declaration.body() else {
                hasher.update(&[255]);
                return Ok(CheckedCoverageDomainDigest::from_bytes(
                    *hasher.finalize().as_bytes(),
                ));
            };
            hasher.update(
                &u32::try_from(variants.len())
                    .map_err(|_| SemanticTranscriptError::WorkLimit)?
                    .to_le_bytes(),
            );
            for (ordinal, variant) in variants.iter().enumerate() {
                hasher.update(
                    &u32::try_from(ordinal)
                        .map_err(|_| SemanticTranscriptError::WorkLimit)?
                        .to_le_bytes(),
                );
                match variant.payload() {
                    Some(payload) => {
                        hasher.update(&[1]);
                        let payload = analysis
                            .ty(payload)
                            .ok_or(SemanticTranscriptError::MissingIdentity)?;
                        hasher.update(payload.semantic_identity_digest().as_bytes());
                    }
                    None => {
                        hasher.update(&[0]);
                    }
                }
            }
        }
        _ => {
            hasher.update(&[255]);
            hasher.update(ty.semantic_identity_digest().as_bytes());
        }
    }
    Ok(CheckedCoverageDomainDigest::from_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

fn pattern_coverage(
    analysis: &FinalSemanticAnalysis,
    module: &HirModule,
    owner: arcweft_lang_hir::identity::PatternId,
    domain: &CoverageDomain,
) -> Result<CoverageShape, SemanticTranscriptError> {
    let pattern = module
        .resolve_pattern(owner)
        .map_err(|_| SemanticTranscriptError::MissingPattern)?;
    if matches!(pattern.kind(), HirPatternKind::Error(_)) {
        return Err(SemanticTranscriptError::RecoveredOwner);
    }
    match pattern.kind() {
        HirPatternKind::Discard
        | HirPatternKind::Binding(_)
        | HirPatternKind::MutableBinding(_) => Ok(CoverageShape::All),
        HirPatternKind::WholeBinding { pattern, .. } => {
            pattern_coverage(analysis, module, *pattern, domain)
        }
        HirPatternKind::Literal(HirLiteral::Boolean(value)) => {
            if !matches!(domain, CoverageDomain::Finite(_)) {
                return Err(SemanticTranscriptError::UnsupportedCoverage);
            }
            let atom = if *value {
                CoverageAtom::BooleanTrue
            } else {
                CoverageAtom::BooleanFalse
            };
            Ok(CoverageShape::Atoms(BTreeSet::from([atom])))
        }
        HirPatternKind::Tuple { elements } if elements.is_empty() => {
            if matches!(domain, CoverageDomain::Finite(values) if values.contains(&CoverageAtom::Unit))
            {
                Ok(CoverageShape::Atoms(BTreeSet::from([CoverageAtom::Unit])))
            } else {
                Err(SemanticTranscriptError::UnsupportedCoverage)
            }
        }
        HirPatternKind::Variant(variant) => {
            let checked = analysis
                .pattern(owner)
                .ok_or(SemanticTranscriptError::MissingPattern)?;
            let super::CheckedPatternResolution::Variant(resolution) = checked.resolution() else {
                return Err(SemanticTranscriptError::MissingIdentity);
            };
            match variant.payload() {
                HirVariantPatternPayload::Pattern(child) => {
                    if !is_total_pattern(module, *child)? {
                        return Err(SemanticTranscriptError::UnsupportedCoverage);
                    }
                }
                HirVariantPatternPayload::Absent => {}
                HirVariantPatternPayload::Recovered { .. } => {
                    return Err(SemanticTranscriptError::RecoveredOwner);
                }
            }
            let atom = CoverageAtom::Variant(resolution.ordinal());
            let CoverageDomain::Finite(values) = domain else {
                return Err(SemanticTranscriptError::UnsupportedCoverage);
            };
            if !values.contains(&atom) {
                return Err(SemanticTranscriptError::MissingIdentity);
            }
            Ok(CoverageShape::Atoms(BTreeSet::from([atom])))
        }
        HirPatternKind::Or { alternatives } => {
            let mut atoms = BTreeSet::new();
            for alternative in alternatives {
                match pattern_coverage(analysis, module, *alternative, domain)? {
                    CoverageShape::All => return Ok(CoverageShape::All),
                    CoverageShape::Atoms(values) => atoms.extend(values),
                }
            }
            Ok(CoverageShape::Atoms(atoms))
        }
        HirPatternKind::Literal(_)
        | HirPatternKind::EntityReference(_)
        | HirPatternKind::Tuple { .. }
        | HirPatternKind::Record { .. }
        | HirPatternKind::BracketSequence { .. }
        | HirPatternKind::TypedBinding { .. } => Err(SemanticTranscriptError::UnsupportedCoverage),
        HirPatternKind::Error(_) => Err(SemanticTranscriptError::RecoveredOwner),
    }
}

fn is_total_pattern(
    module: &HirModule,
    owner: arcweft_lang_hir::identity::PatternId,
) -> Result<bool, SemanticTranscriptError> {
    let pattern = module
        .resolve_pattern(owner)
        .map_err(|_| SemanticTranscriptError::MissingPattern)?;
    Ok(match pattern.kind() {
        HirPatternKind::Discard
        | HirPatternKind::Binding(_)
        | HirPatternKind::MutableBinding(_) => true,
        HirPatternKind::WholeBinding { pattern, .. } => is_total_pattern(module, *pattern)?,
        HirPatternKind::Tuple { elements } => {
            let mut total = true;
            for element in elements {
                total &= is_total_pattern(module, *element)?;
            }
            total
        }
        HirPatternKind::Or { alternatives } => {
            let mut total = false;
            for alternative in alternatives {
                total |= is_total_pattern(module, *alternative)?;
            }
            total
        }
        HirPatternKind::Error(_) => return Err(SemanticTranscriptError::RecoveredOwner),
        _ => false,
    })
}

fn match_digest(
    used: &mut u64,
    limit: u64,
    scrutinee: CheckedExpressionSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: &[CheckedMatchArm],
    coverage: &CheckedMatchCoverage,
) -> Result<CheckedMatchSemanticDigest, SemanticTranscriptError> {
    let mut hasher = TranscriptHasher::new(used, limit);
    hasher.update(b"arcweft.lang.checked-match-semantic.v1\0");
    hasher.update(scrutinee.as_bytes());
    hasher.update(scrutinee_type.as_bytes());
    write_len(&mut hasher, arms.len());
    for arm in arms {
        hasher.update(&arm.ordinal.to_le_bytes());
        hasher.update(arm.pattern.as_bytes());
        write_len(&mut hasher, arm.bindings.len());
        for binding in &arm.bindings {
            hasher.update(&binding.coordinate.canonical_bytes());
            hasher.update(binding.ty.as_bytes());
        }
        match arm.guard_expression {
            Some(digest) => {
                hasher.update(&[1]);
                hasher.update(digest.as_bytes());
                hasher.update(&[guard_tag(arm.guard)]);
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hasher.update(arm.value.as_bytes());
    }
    hasher.update(&[u8::from(coverage.exhaustive)]);
    hasher.update(coverage.domain_digest.as_bytes());
    write_len(&mut hasher, coverage.unreachable.len());
    for row in &coverage.unreachable {
        hasher.update(&row.arm.to_le_bytes());
        hasher.update(&[unreachable_tag(row.reason)]);
    }
    Ok(CheckedMatchSemanticDigest::from_bytes(hasher.finalize()?))
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
        HirPatternChildRole::TypedBindingType => StablePatternCoordinateStep::TypedBindingInner,
        HirPatternChildRole::BindingLocal
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

fn pattern_kind_tag(kind: &HirPatternKind) -> u8 {
    match kind {
        HirPatternKind::Binding(_) => 0,
        HirPatternKind::MutableBinding(_) => 1,
        HirPatternKind::Literal(_) => 2,
        HirPatternKind::EntityReference(_) => 3,
        HirPatternKind::Variant(_) => 4,
        HirPatternKind::Discard => 5,
        HirPatternKind::Tuple { .. } => 6,
        HirPatternKind::Record { .. } => 7,
        HirPatternKind::BracketSequence { .. } => 8,
        HirPatternKind::WholeBinding { .. } => 9,
        HirPatternKind::Or { .. } => 10,
        HirPatternKind::TypedBinding { .. } => 11,
        HirPatternKind::Error(_) => 12,
    }
}

fn write_literal(
    hasher: &mut TranscriptHasher<'_>,
    literal: &HirLiteral,
    ty: &TypeKind,
) -> Result<(), SemanticTranscriptError> {
    match literal {
        HirLiteral::String(HirStringLiteral::Value(value)) => {
            hasher.update(&[0]);
            write_bytes(hasher, value.as_bytes());
        }
        HirLiteral::Character(HirCharacterLiteral::Value(value)) => {
            hasher.update(&[1]);
            hasher.update(&u32::from(*value).to_le_bytes());
        }
        HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }) => {
            hasher.update(&[2]);
            write_len(hasher, magnitude.limbs_le().len());
            for limb in magnitude.limbs_le() {
                hasher.update(&limb.to_le_bytes());
            }
        }
        HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }) => {
            hasher.update(&[3]);
            let canonical = decimal.to_decimal_string();
            match ty {
                TypeKind::F32 => {
                    hasher.update(&[0]);
                    let value = canonical
                        .parse::<f32>()
                        .map_err(|_| SemanticTranscriptError::RecoveredOwner)?;
                    hasher.update(&value.to_bits().to_le_bytes());
                }
                TypeKind::F64 => {
                    hasher.update(&[1]);
                    let value = canonical
                        .parse::<f64>()
                        .map_err(|_| SemanticTranscriptError::RecoveredOwner)?;
                    hasher.update(&value.to_bits().to_le_bytes());
                }
                _ => return Err(SemanticTranscriptError::RecoveredOwner),
            }
        }
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { decimal, unit }) => {
            hasher.update(&[4, unit_number_tag(*unit)]);
            write_bytes(hasher, decimal.coefficient().digits());
            hasher.update(&decimal.scale().to_le_bytes());
            hasher.update(&decimal.exponent10().to_le_bytes());
        }
        HirLiteral::Boolean(value) => {
            hasher.update(&[5, u8::from(*value)]);
        }
        HirLiteral::Duration(HirDurationLiteral::Value(value)) => {
            hasher.update(&[6]);
            let limbs = value.semantic_value().nanoseconds().limbs_le();
            write_len(hasher, limbs.len());
            for limb in limbs {
                hasher.update(&limb.to_le_bytes());
            }
        }
        HirLiteral::String(HirStringLiteral::Invalid(_))
        | HirLiteral::Character(HirCharacterLiteral::Invalid(_))
        | HirLiteral::Integer(HirIntegerLiteral::Invalid(_))
        | HirLiteral::Float(HirFloatLiteral::Invalid(_))
        | HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_))
        | HirLiteral::Duration(HirDurationLiteral::Invalid(_)) => {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
    }
    Ok(())
}

fn write_pattern_resolution(
    hasher: &mut TranscriptHasher<'_>,
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
                    hasher.update(&[0]);
                }
                super::CheckedRecordPatternRest::Ignore => {
                    hasher.update(&[1]);
                }
                super::CheckedRecordPatternRest::Binding(binding) => {
                    hasher.update(&[2]);
                    hasher.update(&binding.coordinate().canonical_bytes());
                }
            }
            write_len(hasher, record.fields().len());
            for field in record.fields() {
                hasher.update(&field.source_ordinal().to_le_bytes());
                hasher.update(&field.declaration_ordinal().to_le_bytes());
                hasher.update(field.semantic_id().as_bytes());
                hasher.update(field.field_type_digest().as_bytes());
                if field.source().pattern_coordinate().is_some() {
                    hasher.update(&[0]);
                    let child = record_pattern_child_coordinate(coordinate, field)?;
                    hasher.update(&child.canonical_bytes());
                } else if let Some(binding) = field.source().binding() {
                    hasher.update(&[1]);
                    hasher.update(&binding.coordinate().canonical_bytes());
                } else {
                    return Err(SemanticTranscriptError::UnsupportedIdentity);
                }
            }
        }
        super::CheckedPatternResolution::Variant(variant) => {
            write_variant_resolution(hasher, variant);
        }
        super::CheckedPatternResolution::TypedBinding(binding) => {
            hasher.update(binding.annotation_digest().as_bytes());
        }
        super::CheckedPatternResolution::Entity(_) => {
            return Err(SemanticTranscriptError::UnsupportedIdentity);
        }
    }
    Ok(())
}

fn write_resolution_payload<'paths, 'edges>(
    hasher: &mut TranscriptHasher<'_>,
    resolution: &CheckedExpressionResolution,
    ty: &TypeKind,
    coordinates: &SemanticCoordinateIndex<'paths, 'edges>,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match resolution {
        CheckedExpressionResolution::Structural
        | CheckedExpressionResolution::Literal(_)
        | CheckedExpressionResolution::Call => {}
        CheckedExpressionResolution::Value(value) => match value {
            CheckedValueResolution::Local(local) => {
                let binding = coordinates.binding(*local)?;
                hasher.update(&binding.canonical_bytes());
            }
            CheckedValueResolution::LineContext => {}
            CheckedValueResolution::ProjectCallable(callable) => {
                write_project_callable(hasher, analysis, callable)?;
            }
            CheckedValueResolution::Registered(value) => {
                hasher.update(value.as_bytes());
            }
            CheckedValueResolution::Constant(literal) => write_literal(hasher, literal, ty)?,
            CheckedValueResolution::CharacterField {
                character, field, ..
            } => {
                write_bytes(hasher, character.as_str().as_bytes());
                hasher.update(&[match field {
                    crate::types::CharacterField::Stage => 0,
                }]);
            }
            CheckedValueResolution::ProjectItem(_) | CheckedValueResolution::Entry(_) => {
                return Err(SemanticTranscriptError::UnsupportedIdentity);
            }
        },
        CheckedExpressionResolution::Select(select) => match select {
            CheckedSelectResolution::Field(selection) => {
                hasher.update(selection.owner_type().as_bytes());
                hasher.update(selection.field().as_bytes());
                hasher.update(&selection.declaration_ordinal().to_le_bytes());
                hasher.update(selection.field_type().as_bytes());
                match selection.runtime_field() {
                    Some(runtime_field) => {
                        hasher.update(&[1]);
                        hasher.update(&runtime_field.get().get().to_le_bytes());
                    }
                    None => hasher.update(&[0]),
                }
            }
            CheckedSelectResolution::ProgressField { field } => {
                hasher.update(&[match field {
                    crate::types::ProgressField::Ratio => 0,
                    crate::types::ProgressField::Label => 1,
                }]);
            }
            CheckedSelectResolution::Method(method) => {
                hasher.update(method.callable().as_bytes());
                hasher.update(method.receiver_type().as_bytes());
                match method.receiver_mode() {
                    crate::callable::CallableReceiverMode::None => {
                        return Err(SemanticTranscriptError::UnsupportedIdentity);
                    }
                    crate::callable::CallableReceiverMode::Value { .. } => {
                        hasher.update(&[0]);
                    }
                    crate::callable::CallableReceiverMode::Type { .. } => {
                        hasher.update(&[1]);
                    }
                    crate::callable::CallableReceiverMode::Extension {
                        group, parameter, ..
                    } => {
                        hasher.update(&[2]);
                        hasher.update(
                            &u64::try_from(group.get())
                                .map_err(|_| SemanticTranscriptError::UnsupportedIdentity)?
                                .to_le_bytes(),
                        );
                        hasher.update(
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
            write_variant_resolution(hasher, variant);
        }
        CheckedExpressionResolution::Effect(effect) => {
            write_bytes(hasher, effect.as_str().as_bytes());
        }
        CheckedExpressionResolution::StageLook(look) => {
            hasher.update(look.look().as_bytes());
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
    hasher: &mut TranscriptHasher<'_>,
    nominal: &super::CheckedProjectNominal,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    hasher.update(nominal.identity().as_bytes());
    let layout = nominal_layout_hash(analysis, nominal)?;
    hasher.update(layout.as_bytes());
    write_len(hasher, nominal.arguments().len());
    for argument in nominal.arguments() {
        hasher.update(argument.semantic_identity_digest().as_bytes());
    }
    Ok(())
}

fn write_project_callable(
    hasher: &mut TranscriptHasher<'_>,
    analysis: &FinalSemanticAnalysis,
    callable: &super::CheckedProjectCallable,
) -> Result<(), SemanticTranscriptError> {
    let facts = analysis
        .checked_callables()
        .project_callable(callable.declaration())
        .map_err(|_| SemanticTranscriptError::UnsupportedIdentity)?;
    hasher.update(facts.id().semantic_digest().as_bytes());
    hasher.update(facts.interface_digest().as_bytes());
    Ok(())
}

fn write_variant_resolution(
    hasher: &mut TranscriptHasher<'_>,
    resolution: &super::CheckedVariantResolution,
) {
    let owner_tag = match resolution.owner() {
        CheckedVariantOwner::Project { .. } => 0,
        CheckedVariantOwner::CharacterNominal { .. } => 1,
        CheckedVariantOwner::BuiltinClosed { .. } => 2,
        CheckedVariantOwner::Option { .. } => 3,
        CheckedVariantOwner::Result { .. } => 4,
    };
    hasher.update(&[owner_tag]);
    hasher.update(resolution.owner().semantic_type().as_bytes());
    match resolution.owner().layout() {
        Some(layout) => {
            hasher.update(&[1]);
            hasher.update(layout.as_bytes());
        }
        None => hasher.update(&[0]),
    }
    let selected = resolution.selected();
    hasher.update(selected.semantic_id().as_bytes());
    hasher.update(&selected.ordinal().to_le_bytes());
    match selected.payload() {
        Some(payload) => {
            hasher.update(&[1]);
            hasher.update(payload.semantic_identity_digest().as_bytes());
        }
        None => hasher.update(&[0]),
    }
}

fn nominal_layout_hash(
    analysis: &FinalSemanticAnalysis,
    nominal: &super::CheckedProjectNominal,
) -> Result<TypeLayoutHash, SemanticTranscriptError> {
    analysis
        .checked_runtime_nominal_projection(nominal)
        .map_err(|_| SemanticTranscriptError::UnsupportedIdentity)
        .map(|projection| projection.layout())
}

fn unit_number_tag(unit: HirUnitNumberUnit) -> u8 {
    match unit {
        HirUnitNumberUnit::Percent => 0,
        HirUnitNumberUnit::Px => 1,
        HirUnitNumberUnit::Pt => 2,
        HirUnitNumberUnit::Em => 3,
        HirUnitNumberUnit::Rem => 4,
        HirUnitNumberUnit::Vw => 5,
        HirUnitNumberUnit::Vh => 6,
        HirUnitNumberUnit::Deg => 7,
        HirUnitNumberUnit::Rad => 8,
        HirUnitNumberUnit::Turn => 9,
        HirUnitNumberUnit::Db => 10,
        HirUnitNumberUnit::Lufs => 11,
        HirUnitNumberUnit::Bpm => 12,
        HirUnitNumberUnit::Bars => 13,
    }
}

fn write_effects(hasher: &mut TranscriptHasher<'_>, effects: &crate::effects::EffectSet) {
    write_len(hasher, effects.len());
    for effect in effects.iter() {
        write_bytes(hasher, effect.as_str().as_bytes());
    }
}

pub(crate) fn write_len(hasher: &mut TranscriptHasher<'_>, value: usize) {
    hasher.update(
        &u32::try_from(value)
            .expect("accepted transcript sequences fit checked u32 limits")
            .to_le_bytes(),
    );
}

fn write_bytes(hasher: &mut TranscriptHasher<'_>, value: &[u8]) {
    write_len(hasher, value.len());
    hasher.update(value);
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
        CheckedUnreachableReason::CoveredByPriorRows => 0,
        CheckedUnreachableReason::FalseGuard => 1,
        CheckedUnreachableReason::RedundantOrAlternative => 2,
        CheckedUnreachableReason::UninhabitedDomain => 3,
    }
}
