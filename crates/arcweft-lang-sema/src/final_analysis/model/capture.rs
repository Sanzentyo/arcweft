//! Generation-bound terminal capture facts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use arcweft_lang_hir::identity::SyntheticOwner;
use arcweft_lang_hir::project::HirSelectedCapture;
use arcweft_lang_hir::{project::HirProjectEvaluationTopology, scope::CaptureAccess};
use thiserror::Error;

use super::super::{
    CheckedExpression, CheckedExpressionResolution, CheckedPipe, CheckedTry, ExprId, LocalId,
    SemanticTypeDigest, TypeKind,
};
use crate::semantic_coordinate::{CheckedSemanticPath, StableCheckedBindingCoordinate};

const CHECKED_IMPLICIT_CALLABLE_IDENTITY_DOMAIN: &[u8] =
    b"arcweft.lang.checked-implicit-callable-identity.v1\0";

/// Failure to seal or consume terminal capture evidence against its exact HIR
/// topology authority.
///
/// These are compiler invariants, not ordinary overload-candidate rejection.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedCaptureAuthorityViolation {
    #[error(transparent)]
    CandidateSelection(#[from] arcweft_lang_hir::project::HirCaptureSelectionError),
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error("checked capture fact belongs to another HIR topology allocation")]
    TopologyMismatch,
    #[error("checked capture fact producer differs: expected {expected:?}, found {actual:?}")]
    ProducerMismatch { expected: ExprId, actual: ExprId },
    #[error("checked capture producer is absent from the HIR topology: {owner:?}")]
    MissingProducer { owner: ExprId },
    #[error("checked capture use is absent from the HIR topology: {expression:?}")]
    MissingExpressionUse { expression: ExprId },
    #[error("checked capture local has no topology binding origin: {local:?}")]
    MissingLocalBinding { local: LocalId },
    #[error("checked implicit capture use resolves to a region-internal binding: {local:?}")]
    InternalLocalBinding { local: LocalId },
    #[error("checked implicit capture contains duplicate use evidence: {expression:?}")]
    DuplicateUse { expression: ExprId },
    #[error("checked implicit callable placeholder evidence differs from HIR topology")]
    PlaceholderEvidenceMismatch,
    #[error("checked terminal capture evidence differs from HIR topology")]
    CaptureEvidenceMismatch,
    #[error("checked implicit callable identity coordinate cannot be canonically encoded")]
    IdentityCoordinateEncoding,
}

/// Opaque semantic identity of one accepted implicit callable.
///
/// The tuple field is private and this type intentionally has no Serde
/// implementation.  Only the owner-bound seal may issue an identity; the
/// bytes are exposed for transcript/runtime consumers after that seal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedImplicitCallableIdentity([u8; 32]);

impl CheckedImplicitCallableIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn issue(
        owner: &CheckedSemanticPath,
        function_type: SemanticTypeDigest,
        parameter: SemanticTypeDigest,
        result: SemanticTypeDigest,
        parameter_occurrences: &[CheckedImplicitParameterOccurrence],
        capture_occurrences: &[CheckedImplicitCaptureOccurrence],
        captures: &[CheckedImplicitCapture],
    ) -> Result<Self, CheckedCaptureAuthorityViolation> {
        let mut bytes = Vec::new();
        append_path(&mut bytes, owner)?;
        append_digest(&mut bytes, function_type);
        append_digest(&mut bytes, parameter);
        append_digest(&mut bytes, result);
        append_len(&mut bytes, parameter_occurrences.len())?;
        for occurrence in parameter_occurrences {
            bytes.extend_from_slice(&occurrence.ordinal.to_le_bytes());
            append_path(&mut bytes, &occurrence.coordinate)?;
        }
        append_len(&mut bytes, capture_occurrences.len())?;
        for occurrence in capture_occurrences {
            bytes.extend_from_slice(&occurrence.ordinal.to_le_bytes());
            append_path(&mut bytes, &occurrence.coordinate)?;
            append_binding(&mut bytes, &occurrence.origin)?;
            append_digest(&mut bytes, occurrence.value_type);
            bytes.push(capture_access_tag(occurrence.access));
        }
        append_len(&mut bytes, captures.len())?;
        for capture in captures {
            append_binding(&mut bytes, &capture.origin)?;
            append_digest(&mut bytes, capture.value_type);
            bytes.push(capture_access_tag(capture.mode));
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHECKED_IMPLICIT_CALLABLE_IDENTITY_DOMAIN);
        hasher.update(&bytes);
        Ok(Self(*hasher.finalize().as_bytes()))
    }
}

/// Body-independent evidence issued by the owner-bound phase before any
/// callable body is closed.  This is the transaction's phase-one result: it
/// carries the complete identity inputs, but no provisional expression
/// resolution and no final callable publication.
#[derive(Clone)]
pub(crate) struct CheckedImplicitCallableIdentityEvidence {
    topology: Arc<HirProjectEvaluationTopology>,
    lookup_owner: ExprId,
    coordinate: CheckedSemanticPath,
    identity: CheckedImplicitCallableIdentity,
    function_type: SemanticTypeDigest,
    parameter: TypeKind,
    result: TypeKind,
    parameter_occurrences: Box<[CheckedImplicitParameterOccurrence]>,
    capture_occurrences: Box<[CheckedImplicitCaptureOccurrence]>,
    captures: Box<[CheckedImplicitCapture]>,
}

impl CheckedImplicitCallableIdentityEvidence {
    pub(crate) fn issue(
        topology: Arc<HirProjectEvaluationTopology>,
        lookup_owner: ExprId,
        coordinate: CheckedSemanticPath,
        function_type: SemanticTypeDigest,
        parameter: TypeKind,
        result: TypeKind,
        parameter_occurrences: impl Into<Box<[CheckedImplicitParameterOccurrence]>>,
        capture_occurrences: impl Into<Box<[CheckedImplicitCaptureOccurrence]>>,
        captures: impl Into<Box<[CheckedImplicitCapture]>>,
    ) -> Result<Self, CheckedCaptureAuthorityViolation> {
        let parameter_occurrences = parameter_occurrences.into();
        let capture_occurrences = capture_occurrences.into();
        let captures = captures.into();
        let identity = CheckedImplicitCallableIdentity::issue(
            &coordinate,
            function_type,
            parameter.semantic_identity_digest()?,
            result.semantic_identity_digest()?,
            &parameter_occurrences,
            &capture_occurrences,
            &captures,
        )?;
        validate_implicit_callable_evidence(
            &topology,
            lookup_owner,
            &parameter_occurrences,
            &capture_occurrences,
            &captures,
        )?;
        Ok(Self {
            topology,
            lookup_owner,
            coordinate,
            identity,
            function_type,
            parameter,
            result,
            parameter_occurrences,
            capture_occurrences,
            captures,
        })
    }

    pub(crate) const fn identity(&self) -> CheckedImplicitCallableIdentity {
        self.identity
    }

    pub(crate) fn finalize(
        self,
        body: CheckedImplicitCallableBody,
    ) -> Result<CheckedImplicitCallable, CheckedCaptureAuthorityViolation> {
        let checked = CheckedImplicitCallable {
            topology: self.topology,
            lookup_owner: self.lookup_owner,
            coordinate: self.coordinate,
            identity: self.identity,
            function_type: self.function_type,
            parameter: self.parameter,
            result: self.result,
            parameter_occurrences: self.parameter_occurrences,
            capture_occurrences: self.capture_occurrences,
            captures: self.captures,
            body,
        };
        checked.validate_evidence()?;
        Ok(checked)
    }
}

/// Closed checked body authority for one implicit callable.
///
/// `Try` and `Pipe` retain their complete typed owner-bound payloads so runtime
/// consumers can project nested facts without reopening the raw HIR. Every
/// other body resolution is a plain checked resolution and is intentionally
/// kept in the third, exhaustive branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedImplicitCallableBody {
    Plain(Box<CheckedExpressionResolution>),
    Try(CheckedTry),
    Pipe(CheckedPipe),
}

/// One source-order occurrence of a partial-application placeholder.
///
/// `lookup_expression` is retained only so the generation-bound topology can
/// revalidate this row.  Stable identity uses `coordinate` and `ordinal`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedImplicitParameterOccurrence {
    lookup_expression: ExprId,
    coordinate: CheckedSemanticPath,
    ordinal: u32,
}

impl CheckedImplicitParameterOccurrence {
    pub(crate) fn new(
        lookup_expression: ExprId,
        coordinate: CheckedSemanticPath,
        ordinal: u32,
    ) -> Self {
        Self {
            lookup_expression,
            coordinate: coordinate.clone(),
            ordinal,
        }
    }

    pub(in crate::final_analysis) const fn lookup_expression(&self) -> ExprId {
        self.lookup_expression
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// One source-order occurrence of a captured local in an implicit callable.
///
/// The expression and local are lookup-only validation evidence.  Transcript
/// and identity code consume the accepted coordinate, binding origin, type
/// digest, access mode, and callable-local ordinal instead.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedImplicitCaptureOccurrence {
    lookup_expression: ExprId,
    lookup_local: LocalId,
    coordinate: CheckedSemanticPath,
    origin: StableCheckedBindingCoordinate,
    value_type: SemanticTypeDigest,
    access: CaptureAccess,
    ordinal: u32,
}

impl CheckedImplicitCaptureOccurrence {
    pub(crate) fn new(
        lookup_expression: ExprId,
        lookup_local: LocalId,
        coordinate: CheckedSemanticPath,
        origin: StableCheckedBindingCoordinate,
        value_type: SemanticTypeDigest,
        access: CaptureAccess,
        ordinal: u32,
    ) -> Self {
        Self {
            lookup_expression,
            lookup_local,
            coordinate: coordinate.clone(),
            origin,
            value_type,
            access,
            ordinal,
        }
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub const fn origin(&self) -> &StableCheckedBindingCoordinate {
        &self.origin
    }

    pub const fn value_type(&self) -> SemanticTypeDigest {
        self.value_type
    }

    pub const fn access(&self) -> CaptureAccess {
        self.access
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// Aggregate capture row for one distinct local of an implicit callable.
///
/// The local is retained only for topology validation.  `origin`, `value_type`,
/// and `mode` are the accepted semantic payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedImplicitCapture {
    lookup_local: LocalId,
    origin: StableCheckedBindingCoordinate,
    value_type: SemanticTypeDigest,
    mode: CaptureAccess,
}

impl CheckedImplicitCapture {
    pub(crate) fn new(
        lookup_local: LocalId,
        origin: StableCheckedBindingCoordinate,
        value_type: SemanticTypeDigest,
        mode: CaptureAccess,
    ) -> Self {
        Self {
            lookup_local,
            origin,
            value_type,
            mode,
        }
    }

    pub(crate) const fn lookup_local(&self) -> LocalId {
        self.lookup_local
    }

    pub const fn origin(&self) -> &StableCheckedBindingCoordinate {
        &self.origin
    }

    pub const fn value_type(&self) -> SemanticTypeDigest {
        self.value_type
    }

    pub const fn mode(&self) -> CaptureAccess {
        self.mode
    }
}

/// Final payload of one partial-application placeholder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedImplicitParameter {
    callable: CheckedImplicitCallableIdentity,
    occurrence_ordinal: u32,
    parameter_type: SemanticTypeDigest,
}

impl CheckedImplicitParameter {
    pub(crate) const fn new(
        callable: CheckedImplicitCallableIdentity,
        occurrence_ordinal: u32,
        parameter_type: SemanticTypeDigest,
    ) -> Self {
        Self {
            callable,
            occurrence_ordinal,
            parameter_type,
        }
    }

    pub const fn callable(&self) -> CheckedImplicitCallableIdentity {
        self.callable
    }

    pub const fn occurrence_ordinal(&self) -> u32 {
        self.occurrence_ordinal
    }

    pub const fn parameter_type(&self) -> SemanticTypeDigest {
        self.parameter_type
    }
}

fn append_len(bytes: &mut Vec<u8>, length: usize) -> Result<(), CheckedCaptureAuthorityViolation> {
    let length = u64::try_from(length)
        .map_err(|_| CheckedCaptureAuthorityViolation::IdentityCoordinateEncoding)?;
    bytes.extend_from_slice(&length.to_le_bytes());
    Ok(())
}

fn append_path(
    bytes: &mut Vec<u8>,
    path: &CheckedSemanticPath,
) -> Result<(), CheckedCaptureAuthorityViolation> {
    let path = path
        .canonical_bytes()
        .map_err(|_| CheckedCaptureAuthorityViolation::IdentityCoordinateEncoding)?;
    append_len(bytes, path.len())?;
    bytes.extend_from_slice(&path);
    Ok(())
}

fn append_binding(
    bytes: &mut Vec<u8>,
    binding: &StableCheckedBindingCoordinate,
) -> Result<(), CheckedCaptureAuthorityViolation> {
    append_path(bytes, binding.path())
}

fn append_digest(bytes: &mut Vec<u8>, digest: SemanticTypeDigest) {
    bytes.extend_from_slice(digest.as_bytes());
}

const fn capture_access_tag(access: CaptureAccess) -> u8 {
    match access {
        CaptureAccess::Read => 0,
        CaptureAccess::Reassign => 1,
    }
}

/// Exact capture authority for one explicit closure producer.
#[derive(Clone)]
pub struct CheckedClosure {
    topology: Arc<HirProjectEvaluationTopology>,
    owner: ExprId,
    choices: BTreeMap<ExprId, ExprId>,
    captures: Box<[HirSelectedCapture]>,
}

impl CheckedClosure {
    pub(crate) fn seal(
        topology: Arc<HirProjectEvaluationTopology>,
        owner: ExprId,
        mut selected: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<Self, CheckedCaptureAuthorityViolation> {
        let mut choices = BTreeMap::new();
        let captures = project_closure_captures(&topology, owner, |selector| {
            let candidate = selected(selector)?;
            choices.insert(selector, candidate);
            Some(candidate)
        })?;
        let checked = Self {
            topology,
            owner,
            choices,
            captures,
        };
        checked.validate_evidence()?;
        Ok(checked)
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }

    #[cfg(test)]
    pub(crate) const fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub const fn captures(&self) -> &[HirSelectedCapture] {
        &self.captures
    }

    pub(crate) fn validate_authority(
        &self,
        expected: &Arc<HirProjectEvaluationTopology>,
        producer: ExprId,
    ) -> Result<&[HirSelectedCapture], CheckedCaptureAuthorityViolation> {
        if !Arc::ptr_eq(&self.topology, expected) {
            return Err(CheckedCaptureAuthorityViolation::TopologyMismatch);
        }
        if self.owner != producer {
            return Err(CheckedCaptureAuthorityViolation::ProducerMismatch {
                expected: producer,
                actual: self.owner,
            });
        }
        self.validate_evidence()?;
        Ok(&self.captures)
    }

    fn validate_evidence(&self) -> Result<(), CheckedCaptureAuthorityViolation> {
        let mut used = BTreeSet::new();
        let expected = project_closure_captures(&self.topology, self.owner, |selector| {
            used.insert(selector);
            self.choices.get(&selector).copied()
        })?;
        (expected == self.captures && used.len() == self.choices.len())
            .then_some(())
            .ok_or(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
    }

    /// Joins this probe-local seal to the final program before any executable
    /// fact escapes. A selected closure must agree on every consumed choice.
    pub(in crate::final_analysis) fn validate_selection(
        &self,
        selected: &crate::final_analysis::match_edges::CheckedSelectedExpressionGraph,
    ) -> Result<(), CheckedCaptureAuthorityViolation> {
        self.validate_authority(selected.topology(), self.owner)?;
        if !selected.contains_owner(SyntheticOwner::Expr(self.owner))
            || self.choices.iter().any(|(selector, candidate)| {
                !selected
                    .expression_edges(*selector)
                    .iter()
                    .any(|edge| edge.child() == *candidate)
            })
        {
            return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
        }
        Ok(())
    }
}

fn project_closure_captures(
    topology: &HirProjectEvaluationTopology,
    owner: ExprId,
    selected: impl FnMut(ExprId) -> Option<ExprId>,
) -> Result<Box<[HirSelectedCapture]>, CheckedCaptureAuthorityViolation> {
    let module = topology
        .module(owner.module())
        .ok_or(CheckedCaptureAuthorityViolation::MissingProducer { owner })?;
    module
        .select_closure_captures(owner, selected)
        .map_err(Into::into)
}

impl fmt::Debug for CheckedClosure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedClosure")
            .field("topology", &"generation-bound")
            .field("owner", &self.owner)
            .field("choices", &self.choices)
            .field("captures", &self.captures)
            .finish()
    }
}

impl PartialEq for CheckedClosure {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.topology, &other.topology)
            && self.owner == other.owner
            && self.choices == other.choices
            && self.captures == other.captures
    }
}

impl Eq for CheckedClosure {}

/// Checked implicit callable introduced by one or more `_` placeholders.
///
/// The topology and `lookup_owner` are generation-local validation evidence.
/// All semantic identity is carried by the accepted coordinate, opaque
/// callable identity, typed digest fields, and stable occurrence rows.
#[derive(Clone)]
pub struct CheckedImplicitCallable {
    topology: Arc<HirProjectEvaluationTopology>,
    lookup_owner: ExprId,
    coordinate: CheckedSemanticPath,
    identity: CheckedImplicitCallableIdentity,
    function_type: SemanticTypeDigest,
    parameter: TypeKind,
    result: TypeKind,
    parameter_occurrences: Box<[CheckedImplicitParameterOccurrence]>,
    capture_occurrences: Box<[CheckedImplicitCaptureOccurrence]>,
    captures: Box<[CheckedImplicitCapture]>,
    body: CheckedImplicitCallableBody,
}

impl CheckedImplicitCallable {
    #[cfg(test)]
    pub(crate) const fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub const fn identity(&self) -> CheckedImplicitCallableIdentity {
        self.identity
    }

    pub const fn function_type(&self) -> SemanticTypeDigest {
        self.function_type
    }

    pub const fn parameter(&self) -> &TypeKind {
        &self.parameter
    }

    pub const fn result(&self) -> &TypeKind {
        &self.result
    }

    pub(in crate::final_analysis) const fn parameter_occurrences(
        &self,
    ) -> &[CheckedImplicitParameterOccurrence] {
        &self.parameter_occurrences
    }

    pub(in crate::final_analysis) const fn capture_occurrences(
        &self,
    ) -> &[CheckedImplicitCaptureOccurrence] {
        &self.capture_occurrences
    }

    pub(in crate::final_analysis) const fn captures(&self) -> &[CheckedImplicitCapture] {
        &self.captures
    }

    pub const fn body(&self) -> &CheckedImplicitCallableBody {
        &self.body
    }

    pub(crate) fn validate_authority(
        &self,
        expected: &Arc<HirProjectEvaluationTopology>,
        producer: ExprId,
    ) -> Result<&[CheckedImplicitCapture], CheckedCaptureAuthorityViolation> {
        if !Arc::ptr_eq(&self.topology, expected) {
            return Err(CheckedCaptureAuthorityViolation::TopologyMismatch);
        }
        if self.lookup_owner != producer {
            return Err(CheckedCaptureAuthorityViolation::ProducerMismatch {
                expected: producer,
                actual: self.lookup_owner,
            });
        }
        self.validate_evidence()?;
        Ok(&self.captures)
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.parameter())?;
        visitor(self.result())?;
        match self.body() {
            CheckedImplicitCallableBody::Plain(resolution) => resolution.visit_types(visitor),
            CheckedImplicitCallableBody::Try(tried) => tried.visit_types(visitor),
            CheckedImplicitCallableBody::Pipe(_) => Ok(()),
        }
    }

    fn validate_evidence(&self) -> Result<(), CheckedCaptureAuthorityViolation> {
        validate_implicit_callable_evidence(
            &self.topology,
            self.lookup_owner,
            &self.parameter_occurrences,
            &self.capture_occurrences,
            &self.captures,
        )
    }

    /// Validates the capture occurrences against the final published
    /// expression map. The map is the typed execution authority; HIR is used
    /// only for region membership and source order, never to rediscover local
    /// uses for runtime lowering.
    pub(crate) fn validate_execution_uses(
        &self,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
    ) -> Result<(), CheckedCaptureAuthorityViolation> {
        let module = self.topology.module(self.lookup_owner.module()).ok_or(
            CheckedCaptureAuthorityViolation::MissingProducer {
                owner: self.lookup_owner,
            },
        )?;
        let region = module
            .expression_uses()
            .implicit_callable_region(self.lookup_owner)
            .map_err(|_| CheckedCaptureAuthorityViolation::MissingProducer {
                owner: self.lookup_owner,
            })?;
        let expected = module
            .expression_uses()
            .rows()
            .iter()
            .filter(|row| region.contains_expression(row.expression()))
            .map(|row| {
                let expression = row.expression();
                let fact = expressions
                    .get(&expression)
                    .ok_or(CheckedCaptureAuthorityViolation::MissingExpressionUse { expression })?;
                let Some(local) = fact.execution_local_use() else {
                    // Non-local execution facts, including compile-time
                    // scalars, intentionally contribute no capture row.
                    return Ok(None);
                };
                let binding = module
                    .local_origins()
                    .binding(local)
                    .ok_or(CheckedCaptureAuthorityViolation::MissingLocalBinding { local })?;
                Ok((!region.contains_binding(binding)).then_some((expression, local)))
            })
            .collect::<Result<Vec<_>, CheckedCaptureAuthorityViolation>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let observed = self
            .capture_occurrences
            .iter()
            .map(|occurrence| (occurrence.lookup_expression, occurrence.lookup_local))
            .collect::<Vec<_>>();
        if expected != observed {
            return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
        }
        validate_aggregate_captures(&self.capture_occurrences, &self.captures)
    }
}

impl fmt::Debug for CheckedImplicitCallable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedImplicitCallable")
            .field("topology", &"generation-bound")
            .field("lookup_owner", &self.lookup_owner)
            .field("coordinate", &self.coordinate)
            .field("identity", &self.identity)
            .field("function_type", &self.function_type)
            .field("parameter", &self.parameter)
            .field("result", &self.result)
            .field("parameter_occurrences", &self.parameter_occurrences)
            .field("capture_occurrences", &self.capture_occurrences)
            .field("captures", &self.captures)
            .field("body", &self.body)
            .finish()
    }
}

impl PartialEq for CheckedImplicitCallable {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.topology, &other.topology)
            && self.lookup_owner == other.lookup_owner
            && self.coordinate == other.coordinate
            && self.identity == other.identity
            && self.function_type == other.function_type
            && self.parameter == other.parameter
            && self.result == other.result
            && self.parameter_occurrences == other.parameter_occurrences
            && self.capture_occurrences == other.capture_occurrences
            && self.captures == other.captures
            && self.body == other.body
    }
}

impl Eq for CheckedImplicitCallable {}

fn validate_implicit_callable_evidence(
    topology: &Arc<HirProjectEvaluationTopology>,
    lookup_owner: ExprId,
    parameter_occurrences: &[CheckedImplicitParameterOccurrence],
    capture_occurrences: &[CheckedImplicitCaptureOccurrence],
    captures: &[CheckedImplicitCapture],
) -> Result<(), CheckedCaptureAuthorityViolation> {
    let module = topology.module(lookup_owner.module()).ok_or(
        CheckedCaptureAuthorityViolation::MissingProducer {
            owner: lookup_owner,
        },
    )?;
    let region = module
        .expression_uses()
        .implicit_callable_region(lookup_owner)
        .map_err(|_| CheckedCaptureAuthorityViolation::MissingProducer {
            owner: lookup_owner,
        })?;
    let placeholders = region.placeholders().collect::<Vec<_>>();
    if placeholders.len() != parameter_occurrences.len()
        || !placeholders
            .iter()
            .zip(parameter_occurrences.iter())
            .enumerate()
            .all(|(ordinal, (placeholder, occurrence))| {
                u32::try_from(ordinal).ok() == Some(occurrence.ordinal)
                    && *placeholder == occurrence.lookup_expression
            })
    {
        return Err(CheckedCaptureAuthorityViolation::PlaceholderEvidenceMismatch);
    }
    if parameter_occurrences.is_empty() {
        return Err(CheckedCaptureAuthorityViolation::PlaceholderEvidenceMismatch);
    }
    let mut previous_source_ordinal = None;
    for occurrence in capture_occurrences {
        if previous_source_ordinal.is_some_and(|previous| {
            module
                .expression_uses()
                .row(occurrence.lookup_expression)
                .is_none_or(|row| previous >= row.source_ordinal())
        }) {
            return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
        }
        let row = module
            .expression_uses()
            .row(occurrence.lookup_expression)
            .filter(|_| region.contains_expression(occurrence.lookup_expression))
            .ok_or(CheckedCaptureAuthorityViolation::MissingExpressionUse {
                expression: occurrence.lookup_expression,
            })?;
        let binding = module
            .local_origins()
            .binding(occurrence.lookup_local)
            .ok_or(CheckedCaptureAuthorityViolation::MissingLocalBinding {
                local: occurrence.lookup_local,
            })?;
        if region.contains_binding(binding) || row.capture_access() != occurrence.access {
            return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
        }
        previous_source_ordinal = Some(row.source_ordinal());
    }
    if !capture_occurrences
        .iter()
        .enumerate()
        .all(|(ordinal, row)| u32::try_from(ordinal).ok() == Some(row.ordinal))
    {
        return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
    }
    validate_aggregate_captures(capture_occurrences, captures)
}

fn validate_aggregate_captures(
    occurrences: &[CheckedImplicitCaptureOccurrence],
    captures: &[CheckedImplicitCapture],
) -> Result<(), CheckedCaptureAuthorityViolation> {
    let mut by_local = BTreeMap::<LocalId, usize>::new();
    for occurrence in occurrences {
        if let Some(index) = by_local.get(&occurrence.lookup_local).copied() {
            let capture = captures
                .get(index)
                .ok_or(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)?;
            if capture.origin != occurrence.origin.clone()
                || capture.value_type != occurrence.value_type
                || (matches!(occurrence.access, CaptureAccess::Reassign)
                    && capture.mode != CaptureAccess::Reassign)
            {
                return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
            }
        } else {
            let index = by_local.len();
            let capture = captures
                .get(index)
                .ok_or(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)?;
            if capture.lookup_local != occurrence.lookup_local
                || capture.origin != occurrence.origin.clone()
                || capture.value_type != occurrence.value_type
                || capture.mode != occurrence.access
            {
                return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
            }
            by_local.insert(occurrence.lookup_local, index);
        }
    }
    (by_local.len() == captures.len())
        .then_some(())
        .ok_or(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
}

#[cfg(test)]
mod tests {
    use arcweft_lang_hir::expr::HirExprKind;
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

    use super::*;

    #[test]
    fn checked_closure_requires_exact_candidate_choice_receipts() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() -> Unit { let offset = 0i64; let values = [42i64]; let read = || -> i64 { values[offset] }; () }\n",
            None,
        );
        let report =
            crate::final_analysis::tests::analyze(&fixture).expect("selected Index closure");
        let checked = report
            .expressions()
            .find_map(|(_, expression)| match expression.resolution() {
                CheckedExpressionResolution::Closure(closure) => Some(closure),
                _ => None,
            })
            .expect("checked closure");
        let (&selector, &candidate) = checked
            .choices
            .first_key_value()
            .expect("capture choice receipt");
        assert_eq!(checked.choices.len(), 1);
        assert_eq!(checked.captures.len(), 2);
        checked.validate_evidence().unwrap();

        let mut missing = checked.clone();
        missing.choices.clear();
        assert!(matches!(
            missing.validate_evidence(),
            Err(CheckedCaptureAuthorityViolation::CandidateSelection(_))
        ));
        let mut extra = checked.clone();
        extra.choices.insert(checked.owner, candidate);
        assert_eq!(
            extra.validate_evidence(),
            Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
        );
        let mut invalid = checked.clone();
        invalid.choices.insert(selector, checked.owner);
        assert!(matches!(
            invalid.validate_evidence(),
            Err(CheckedCaptureAuthorityViolation::CandidateSelection(_))
        ));

        let module = fixture
            .project
            .analysis_view()
            .unwrap()
            .module(&CanonicalModulePath::crate_root())
            .unwrap();
        let HirExprKind::PostfixBracket(postfix) = module.resolve_expr(selector).unwrap().kind()
        else {
            panic!("selector")
        };
        let arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
            dialogue,
            ..
        } = postfix.candidates()
        else {
            panic!("two retained candidates")
        };
        let mut changed = checked.clone();
        changed.choices.insert(selector, *dialogue);
        assert_eq!(
            changed.validate_evidence(),
            Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
        );
        checked
            .validate_evidence()
            .expect("failed receipts leave the accepted proof unchanged");
    }

    #[test]
    fn checked_closure_equality_and_validation_require_exact_topology_and_evidence() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() { let first = 1i64; let second = 2i64; let value = || -> i64 { second + first }; value(); }\n",
            None,
        );
        let executable = fixture.project.analysis_view().expect("executable HIR");
        let module = executable
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Closure(_)).then_some(owner)
            })
            .expect("closure owner");
        let topology = executable
            .accept_symbol_generation(&fixture.symbols)
            .expect("accepted generation")
            .into_evaluation_topology()
            .expect("topology");
        let foreign = executable
            .accept_symbol_generation(&fixture.symbols)
            .expect("second accepted generation")
            .into_evaluation_topology()
            .expect("foreign topology allocation");
        let checked =
            CheckedClosure::seal(Arc::clone(&topology), owner, |_| None).expect("sealed closure");
        let foreign_checked = CheckedClosure::seal(Arc::clone(&foreign), owner, |_| None)
            .expect("foreign sealed closure");

        assert_eq!(checked, checked.clone());
        assert_ne!(checked, foreign_checked);
        assert_eq!(
            checked.validate_authority(&foreign, owner),
            Err(CheckedCaptureAuthorityViolation::TopologyMismatch),
        );

        let mut tampered = checked.clone();
        assert!(tampered.captures.len() >= 2);
        tampered.captures.swap(0, 1);
        assert_eq!(
            tampered.validate_authority(&topology, owner),
            Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch),
        );
    }

    #[test]
    fn implicit_callable_seal_retains_owner_identity_and_occurrence_rows() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() { let matcher = _ > 80i64; }\n",
            None,
        );
        let report =
            crate::final_analysis::tests::analyze(&fixture).expect("sealed implicit callable");
        let (owner, callable) = report
            .expressions()
            .find_map(|(owner, expression)| {
                if let CheckedExpressionResolution::ImplicitCallable(callable) =
                    expression.resolution()
                {
                    Some((owner, callable.as_ref()))
                } else {
                    None
                }
            })
            .expect("implicit callable root");

        assert_ne!(callable.identity().as_bytes(), &[0; 32]);
        assert_eq!(callable.parameter_occurrences().len(), 1);
        assert_eq!(callable.parameter_occurrences()[0].ordinal(), 0);
        assert!(callable.capture_occurrences().is_empty());
        assert!(callable.captures().is_empty());
        callable
            .validate_authority(callable.topology(), owner)
            .expect("topology-authenticated owner-bound seal");
    }

    #[test]
    fn execution_use_validation_rejects_a_missing_final_expression_fact() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() { let threshold = 80i64; let matcher = _ > threshold; }\n",
            None,
        );
        let report = crate::final_analysis::tests::analyze(&fixture)
            .expect("sealed implicit callable with an external local");
        let (owner, callable) = report
            .expressions()
            .find_map(|(owner, expression)| {
                if let CheckedExpressionResolution::ImplicitCallable(callable) =
                    expression.resolution()
                {
                    Some((owner, callable.as_ref()))
                } else {
                    None
                }
            })
            .expect("implicit callable root");
        let mut expressions = report
            .expressions()
            .map(|(owner, expression)| (owner, expression.clone()))
            .collect::<BTreeMap<_, _>>();
        let module = callable
            .topology()
            .module(owner.module())
            .expect("callable HIR module");
        let region = module
            .expression_uses()
            .implicit_callable_region(owner)
            .expect("callable HIR region");
        let missing = module
            .expression_uses()
            .rows()
            .iter()
            .filter(|row| region.contains_expression(row.expression()))
            .find_map(|row| {
                expressions
                    .get(&row.expression())
                    .and_then(CheckedExpression::execution_local_use)
                    .map(|_| row.expression())
            })
            .expect("external local execution row");
        expressions.remove(&missing);

        assert_eq!(
            callable.validate_execution_uses(&expressions),
            Err(CheckedCaptureAuthorityViolation::MissingExpressionUse {
                expression: missing,
            }),
        );
    }

    #[test]
    fn execution_use_validation_rejects_a_missing_local_binding() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() { let threshold = 80i64; let matcher = _ > threshold; }\n",
            Some("fn child(value: i64) { value; }\n"),
        );
        let report = crate::final_analysis::tests::analyze(&fixture)
            .expect("sealed implicit callable with an external local");
        let (owner, callable) = report
            .expressions()
            .find_map(|(owner, expression)| {
                if let CheckedExpressionResolution::ImplicitCallable(callable) =
                    expression.resolution()
                {
                    Some((owner, callable.as_ref()))
                } else {
                    None
                }
            })
            .expect("implicit callable root");
        let mut expressions = report
            .expressions()
            .map(|(owner, expression)| (owner, expression.clone()))
            .collect::<BTreeMap<_, _>>();
        let module = callable
            .topology()
            .module(owner.module())
            .expect("callable HIR module");
        let region = module
            .expression_uses()
            .implicit_callable_region(owner)
            .expect("callable HIR region");
        let target = module
            .expression_uses()
            .rows()
            .iter()
            .filter(|row| region.contains_expression(row.expression()))
            .find_map(|row| {
                expressions
                    .get(&row.expression())
                    .and_then(CheckedExpression::execution_local_use)
                    .map(|_| row.expression())
            })
            .expect("external local execution row");
        let foreign_local = report
            .locals()
            .map(|(local, _)| local)
            .find(|local| local.module() != owner.module())
            .expect("child-module local");
        let original = expressions
            .get(&target)
            .cloned()
            .expect("target expression fact");
        expressions.insert(
            target,
            original.with_resolution(CheckedExpressionResolution::Value(
                crate::final_analysis::CheckedValueResolution::Local(foreign_local),
            )),
        );

        assert_eq!(
            callable.validate_execution_uses(&expressions),
            Err(CheckedCaptureAuthorityViolation::MissingLocalBinding {
                local: foreign_local,
            }),
        );
    }

    #[test]
    fn execution_use_validation_rejects_capture_occurrence_reordering() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() { let first = 80i64; let second = 1i64; let matcher = _ > first + second; }\n",
            None,
        );
        let report = crate::final_analysis::tests::analyze(&fixture)
            .expect("sealed implicit callable with ordered external locals");
        let callable = report
            .expressions()
            .find_map(|(_, expression)| match expression.resolution() {
                CheckedExpressionResolution::ImplicitCallable(callable)
                    if callable.capture_occurrences().len() >= 2 =>
                {
                    Some(callable.as_ref())
                }
                _ => None,
            })
            .expect("implicit callable with at least two captures");
        let mut tampered = callable.clone();
        tampered.capture_occurrences.swap(0, 1);
        let expressions = report
            .expressions()
            .map(|(owner, expression)| (owner, expression.clone()))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(
            tampered.validate_execution_uses(&expressions),
            Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch),
        );
    }
}
