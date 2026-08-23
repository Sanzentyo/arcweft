//! Final semantic pattern records owned by the qualified HIR arena.
//!
//! Pattern payloads retain typed semantic structure and qualified child IDs.
//! The lowering transaction supplies the resolver used here so construction
//! proves liveness and lexical visibility without reopening source text.

mod child_edges;

pub use child_edges::{HirPatternChild, HirPatternChildEdge, HirPatternChildEdgeError};

use thiserror::Error;

use crate::expr::{HirPoisonState, HirRecoveryIssue, literal_recovery_issue};
use crate::identity::{HirModuleId, LocalId, PatternId, ScopeId, TypeId};
use crate::leaf::{
    HirIdRefIssue, HirIdRefValue, HirLiteral, HirName, HirNameInvariantError, HirPath, HirPathIssue,
};

/// Transaction-owned typed lookup required to construct one pattern record.
///
/// Implementations include both previously committed IDs and IDs reserved by
/// the current all-or-nothing lowering transaction. A successful child lookup
/// proves that the child is live and visible from `scope`.
pub(crate) trait HirPatternResolver {
    fn scope_is_live(&self, scope: ScopeId) -> bool;

    fn local_is_visible(&self, scope: ScopeId, local: LocalId) -> bool;

    fn resolve_type_state(&self, scope: ScopeId, ty: TypeId) -> Option<&HirPoisonState>;

    fn resolve_pattern(&self, scope: ScopeId, pattern: PatternId) -> Option<&HirPattern>;
}

/// One immutable pattern-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPattern {
    kind: HirPatternKind,
    scope: ScopeId,
    state: HirPoisonState,
}

impl HirPattern {
    pub(crate) fn try_new<R: HirPatternResolver + ?Sized>(
        kind: HirPatternKind,
        scope: ScopeId,
        state: HirPoisonState,
        resolver: &R,
    ) -> Result<Self, HirPatternInvariantError> {
        if !resolver.scope_is_live(scope) {
            return Err(HirPatternInvariantError::ScopeNotLive { scope });
        }
        kind.validate(scope, resolver)?;
        let primary_recovery = kind.primary_recovery_issue(scope, resolver);
        if matches!(state, HirPoisonState::Clean) && kind.contains_recovery_payload(scope, resolver)
        {
            return Err(HirPatternInvariantError::CleanRecoveryPayload);
        }
        match (primary_recovery, &state) {
            (Some(expected), HirPoisonState::Poisoned(actual)) if actual == &expected => {}
            (
                None
                | Some(HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::RecoveredChild {
                    ..
                })),
                HirPoisonState::Poisoned(actual),
            ) if kind.accepts_state_recovery(actual) => {}
            (Some(expected), _) => {
                return Err(HirPatternInvariantError::PatternRecoveryIssueMismatch { expected });
            }
            (None, HirPoisonState::Poisoned(actual)) => {
                return Err(HirPatternInvariantError::UnexpectedPatternPoison {
                    actual: actual.clone(),
                });
            }
            (None, HirPoisonState::Clean) => {}
        }
        Ok(Self { kind, scope, state })
    }

    /// Returns the exact final semantic pattern payload.
    pub const fn kind(&self) -> &HirPatternKind {
        &self.kind
    }

    /// Returns the lexical scope inherited by this pattern.
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Returns the semantic recovery state retained with this pattern.
    pub const fn state(&self) -> &HirPoisonState {
        &self.state
    }

    pub const fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }
}

impl crate::arena::HirArenaPayload for HirPattern {
    fn is_poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// The exact final semantic pattern inventory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternKind {
    Binding(HirPatternBinding),
    MutableBinding(HirPatternBinding),
    Literal(HirLiteral),
    EntityReference(HirIdRefValue),
    Variant(HirVariantPattern),
    Discard,
    Tuple {
        elements: Box<[PatternId]>,
    },
    Record {
        path: HirPatternRecordPath,
        fields: Box<[HirPatternField]>,
    },
    BracketSequence {
        elements: Box<[PatternId]>,
        rest: HirPatternSequenceRest,
    },
    WholeBinding {
        binding: HirPatternBinding,
        pattern: PatternId,
    },
    Or {
        alternatives: Box<[PatternId]>,
    },
    TypedBinding {
        binding: HirPatternBinding,
        ty: TypeId,
    },
    Error(HirPatternError),
}

impl HirPatternKind {
    #[allow(
        dead_code,
        reason = "retained only as the differential projection of typed pattern child edges"
    )]
    pub(crate) fn direct_pattern_children(&self) -> Vec<PatternId> {
        self.child_edges()
            .into_iter()
            .filter_map(|edge| match edge.child() {
                HirPatternChild::Pattern(pattern) => Some(pattern),
                HirPatternChild::Type(_) | HirPatternChild::Local(_) => None,
            })
            .collect()
    }

    /// Returns the exact type node authored by this pattern, when the pattern
    /// itself owns the annotation.
    ///
    /// Let lowering retains `name: Type` as a typed-binding pattern rather
    /// than duplicating the same source type on the enclosing statement.
    pub const fn authored_type(&self) -> Option<TypeId> {
        match self {
            Self::TypedBinding { ty, .. } => Some(*ty),
            _ => None,
        }
    }

    /// Derives the deterministic poison state for a projected pattern payload.
    ///
    /// Attached lowering uses this after all children are staged so recovery
    /// cannot drift from the same validation order enforced by `try_new`.
    pub(crate) fn inferred_state<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> HirPoisonState {
        self.primary_recovery_issue(scope, resolver)
            .map_or(HirPoisonState::Clean, HirPoisonState::Poisoned)
    }

    fn validate<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirPatternInvariantError> {
        match self {
            Self::Binding(binding) | Self::MutableBinding(binding) => {
                binding.validate(scope, resolver)
            }
            Self::Literal(_) | Self::EntityReference(_) | Self::Discard | Self::Error(_) => Ok(()),
            Self::Variant(pattern) => pattern
                .validate(scope, resolver)
                .map_err(HirPatternInvariantError::InvalidVariant),
            Self::Tuple { elements } => validate_patterns(scope, elements, resolver),
            Self::Record { fields, .. } => validate_fields(scope, fields, resolver),
            Self::BracketSequence { elements, rest } => {
                validate_patterns(scope, elements, resolver)?;
                rest.validate(scope, resolver)
            }
            Self::WholeBinding { binding, pattern } => {
                binding.validate(scope, resolver)?;
                validate_pattern(scope, *pattern, resolver).map(|_| ())
            }
            Self::Or { alternatives } => {
                if alternatives.len() < 2 {
                    return Err(HirPatternInvariantError::OrPatternAlternativeCount {
                        observed: alternatives.len(),
                    });
                }
                validate_patterns(scope, alternatives, resolver)
            }
            Self::TypedBinding { binding, ty } => {
                binding.validate(scope, resolver)?;
                validate_type(scope, *ty, resolver)
            }
        }
    }

    fn contains_recovery_payload<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> bool {
        match self {
            Self::Binding(binding) | Self::MutableBinding(binding) => binding.is_recovered(),
            Self::Literal(literal) => literal_recovery_issue(literal).is_some(),
            Self::EntityReference(reference) => reference.is_recovered(),
            Self::Variant(pattern) => pattern.contains_recovery_payload(scope, resolver),
            Self::Tuple { elements }
            | Self::Or {
                alternatives: elements,
            } => patterns_contain_recovery(scope, elements, resolver),
            Self::BracketSequence { elements, rest } => {
                rest.is_recovered() || patterns_contain_recovery(scope, elements, resolver)
            }
            Self::Record { path, fields } => {
                path.is_recovered() || fields_contain_recovery(scope, fields, resolver)
            }
            Self::WholeBinding { binding, pattern } => {
                binding.is_recovered() || pattern_is_recovered(scope, *pattern, resolver)
            }
            Self::TypedBinding { binding, ty } => {
                binding.is_recovered() || type_is_recovered(scope, *ty, resolver)
            }
            Self::Error(_) => true,
            Self::Discard => false,
        }
    }

    fn primary_recovery_issue<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> Option<HirRecoveryIssue> {
        match self {
            Self::Binding(binding) | Self::MutableBinding(binding) => binding
                .recovery_issue()
                .map(HirPatternRecoveryIssue::Binding)
                .map(HirRecoveryIssue::InvalidPattern),
            Self::Literal(literal) => {
                literal_recovery_issue(literal).map(HirRecoveryIssue::MalformedLiteral)
            }
            Self::EntityReference(reference) => reference
                .recovery_issue()
                .map(HirPatternRecoveryIssue::EntityReference)
                .map(HirRecoveryIssue::InvalidPattern),
            Self::Variant(pattern) => pattern.primary_recovery_issue(scope, resolver),
            Self::Tuple { elements }
            | Self::Or {
                alternatives: elements,
            } => first_recovered_child(scope, elements, resolver, |ordinal| {
                HirPatternChildRole::Element { ordinal }
            })
            .map(HirRecoveryIssue::InvalidPattern),
            Self::BracketSequence { elements, rest } => rest
                .recovery_issue()
                .map(HirPatternRecoveryIssue::SequenceRest)
                .or_else(|| {
                    first_recovered_child(scope, elements, resolver, |ordinal| {
                        HirPatternChildRole::Element { ordinal }
                    })
                })
                .map(HirRecoveryIssue::InvalidPattern),
            Self::Record { path, fields } => path
                .recovery_issue()
                .cloned()
                .map(HirPatternRecoveryIssue::RecordPath)
                .or_else(|| {
                    fields
                        .iter()
                        .enumerate()
                        .find_map(|(field, payload)| match payload {
                            HirPatternField::Explicit { pattern, .. }
                                if pattern_is_recovered(scope, *pattern, resolver) =>
                            {
                                Some(HirPatternRecoveryIssue::RecoveredChild {
                                    role: HirPatternChildRole::RecordField {
                                        field: pattern_ordinal(field),
                                    },
                                })
                            }
                            HirPatternField::Invalid { issue } => {
                                Some(HirPatternRecoveryIssue::InvalidField {
                                    field: pattern_ordinal(field),
                                    issue: *issue,
                                })
                            }
                            HirPatternField::Explicit { .. }
                            | HirPatternField::Shorthand { .. }
                            | HirPatternField::Rest { .. } => None,
                        })
                })
                .map(HirRecoveryIssue::InvalidPattern),
            Self::WholeBinding { binding, pattern } => binding
                .recovery_issue()
                .map(HirPatternRecoveryIssue::Binding)
                .or_else(|| {
                    pattern_is_recovered(scope, *pattern, resolver).then_some(
                        HirPatternRecoveryIssue::RecoveredChild {
                            role: HirPatternChildRole::NestedPattern,
                        },
                    )
                })
                .map(HirRecoveryIssue::InvalidPattern),
            Self::TypedBinding { binding, ty } => binding
                .recovery_issue()
                .map(HirPatternRecoveryIssue::Binding)
                .or_else(|| {
                    type_is_recovered(scope, *ty, resolver).then_some(
                        HirPatternRecoveryIssue::RecoveredChild {
                            role: HirPatternChildRole::TypedBindingType,
                        },
                    )
                })
                .map(HirRecoveryIssue::InvalidPattern),
            Self::Discard => None,
            Self::Error(error) => Some(HirRecoveryIssue::InvalidPattern(match error.issue() {
                HirGenericPatternIssue::UnclassifiedSyntax => {
                    HirPatternRecoveryIssue::UnclassifiedSyntax
                }
                HirGenericPatternIssue::TransactionalChildFailure => {
                    HirPatternRecoveryIssue::TransactionalChildFailure
                }
            })),
        }
    }

    fn accepts_state_recovery(&self, issue: &HirRecoveryIssue) -> bool {
        let HirRecoveryIssue::InvalidPattern(issue) = issue else {
            return false;
        };
        matches!(
            (self, issue),
            (
                Self::Tuple { .. } | Self::Record { .. } | Self::BracketSequence { .. },
                HirPatternRecoveryIssue::MissingCloseDelimiter,
            ) | (
                Self::Or { .. },
                HirPatternRecoveryIssue::MissingOrAlternative { .. },
            ) | (
                Self::BracketSequence { .. },
                HirPatternRecoveryIssue::SequenceRest(
                    HirPatternSequenceRestIssue::MultipleRest { .. }
                ),
            )
        )
    }
}

/// One binding site that either owns a real local or records why no local was
/// admitted. Recovery never manufactures an empty name or synthetic local.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternBinding {
    Bound { name: HirName, local: LocalId },
    Recovered { issue: HirPatternBindingIssue },
}

impl HirPatternBinding {
    fn validate<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirPatternInvariantError> {
        match self {
            Self::Bound { local, .. } => validate_local(scope, *local, resolver),
            Self::Recovered { .. } => Ok(()),
        }
    }

    const fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    const fn recovery_issue(&self) -> Option<HirPatternBindingIssue> {
        match self {
            Self::Bound { .. } => None,
            Self::Recovered { issue } => Some(*issue),
        }
    }
}

/// Typed reason that a known binding family could not admit a local.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternBindingIssue {
    #[error("pattern binding is missing its name")]
    MissingName,
    #[error("pattern binding has an invalid name")]
    InvalidName(HirNameInvariantError),
    #[error("pattern binding has {token_count} unexpected trailing tokens")]
    UnexpectedTrailingInput { token_count: u32 },
}

/// Explicit semantic state of the optional bracket-sequence rest.
///
/// An authored unbound rest is not absence, and failed binding recovery never
/// allocates a placeholder local. A multiple-rest issue is retained by the
/// containing Pattern poison while this value keeps the first admitted rest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternSequenceRest {
    Absent,
    Unbound,
    Bound(LocalId),
    Recovered(HirPatternSequenceRestIssue),
}

impl HirPatternSequenceRest {
    fn validate<R: HirPatternResolver + ?Sized>(
        self,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirPatternInvariantError> {
        match self {
            Self::Absent
            | Self::Unbound
            | Self::Recovered(HirPatternSequenceRestIssue::InvalidBinding(_)) => Ok(()),
            Self::Bound(local) => validate_local(scope, local, resolver),
            Self::Recovered(HirPatternSequenceRestIssue::MultipleRest { .. }) => {
                Err(HirPatternInvariantError::MultipleRestCannotReplaceFirstRest)
            }
        }
    }

    const fn is_recovered(self) -> bool {
        matches!(self, Self::Recovered(_))
    }

    const fn recovery_issue(self) -> Option<HirPatternSequenceRestIssue> {
        match self {
            Self::Recovered(issue) => Some(issue),
            Self::Absent | Self::Unbound | Self::Bound(_) => None,
        }
    }

    pub(crate) const fn has_authored_rest(self) -> bool {
        !matches!(self, Self::Absent)
    }

    pub(crate) const fn has_authored_binding(self) -> bool {
        matches!(self, Self::Bound(_) | Self::Recovered(_))
    }
}

/// Typed recovery for one bracket-sequence rest transaction.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternSequenceRestIssue {
    #[error("sequence-rest binding is invalid")]
    InvalidBinding(HirPatternBindingIssue),
    #[error("sequence pattern contains an additional rest at ordinal {ordinal}")]
    MultipleRest { ordinal: u32 },
}

/// Optional record-pattern path with explicit recovery distinct from authored
/// path absence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternRecordPath {
    Absent,
    Resolved(HirPath),
    Recovered(HirPatternRecordPathIssue),
}

impl HirPatternRecordPath {
    const fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }

    const fn recovery_issue(&self) -> Option<&HirPatternRecordPathIssue> {
        match self {
            Self::Absent | Self::Resolved(_) => None,
            Self::Recovered(issue) => Some(issue),
        }
    }

    /// Returns the validated semantic record-path segment count.
    ///
    /// # Panics
    ///
    /// Panics only on a target where `usize` cannot represent a `u32` count.
    pub fn segment_count(&self) -> usize {
        match self {
            Self::Absent => 0,
            Self::Resolved(path) => path.segments().len(),
            Self::Recovered(issue) => usize::try_from(issue.segment_count)
                .expect("u32 Pattern path segment count fits usize"),
        }
    }
}

/// Typed record-path recovery plus the exact segment-role cardinality.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPatternRecordPathIssue {
    issue: HirPathIssue,
    segment_count: u32,
}

impl HirPatternRecordPathIssue {
    pub(crate) const fn new(issue: HirPathIssue, segment_count: u32) -> Self {
        Self {
            issue,
            segment_count,
        }
    }

    pub const fn issue(&self) -> &HirPathIssue {
        &self.issue
    }

    pub const fn segment_count(&self) -> u32 {
        self.segment_count
    }
}

/// One field owned directly by a record-pattern payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternField {
    Explicit { name: HirName, pattern: PatternId },
    Shorthand { name: HirName, local: LocalId },
    Rest { binding: Option<LocalId> },
    Invalid { issue: HirPatternFieldIssue },
}

/// Typed record-pattern field recovery below hard lowering limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternFieldIssue {
    #[error("record-pattern field is missing its name")]
    MissingName,
    #[error("record-pattern field has an invalid name")]
    InvalidName(HirNameInvariantError),
    #[error("record-pattern shorthand binding is invalid")]
    InvalidBinding(HirPatternBindingIssue),
    #[error("record-pattern field is missing its nested pattern")]
    MissingPattern,
    #[error("record-pattern rest binding is invalid")]
    InvalidRestBinding(HirPatternBindingIssue),
    #[error("record-pattern field repeats a name")]
    DuplicateName,
    #[error("record pattern contains more than one rest field")]
    MultipleRest,
}

/// Deterministic relation of a recovered Pattern child to its parent family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternChildRole {
    BindingLocal,
    MutableBindingLocal,
    VariantPayload,
    Element { ordinal: u32 },
    RecordField { field: u32 },
    RecordShorthandLocal { field: u32 },
    RecordRestLocal { field: u32 },
    SequenceRestLocal,
    WholeBindingLocal,
    NestedPattern,
    OrAlternative { ordinal: u32 },
    TypedBindingLocal,
    TypedBindingType,
}

/// Primary typed recovery retained by a known Pattern family.
///
/// When multiple components recover, the family-owned lowering order chooses
/// the first issue. Child records retain their own more specific poison state.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternRecoveryIssue {
    Binding(HirPatternBindingIssue),
    EntityReference(HirIdRefIssue),
    RecordPath(HirPatternRecordPathIssue),
    VariantHead(HirVariantPatternHeadIssue),
    VariantName(HirVariantPatternNameIssue),
    VariantPayload(HirVariantPatternPayloadIssue),
    InvalidField {
        field: u32,
        issue: HirPatternFieldIssue,
    },
    RecoveredChild {
        role: HirPatternChildRole,
    },
    MissingCloseDelimiter,
    MissingOrAlternative {
        ordinal: u32,
    },
    SequenceRest(HirPatternSequenceRestIssue),
    UnclassifiedSyntax,
    TransactionalChildFailure,
}

/// A variant pattern whose qualified and expected-type heads are disjoint.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirVariantPattern {
    head: HirVariantPatternHeadValue,
    name: HirVariantPatternName,
    payload: HirVariantPatternPayload,
}

impl HirVariantPattern {
    pub(crate) fn try_new<R: HirPatternResolver + ?Sized>(
        head: HirVariantPatternHeadValue,
        name: HirVariantPatternName,
        payload: HirVariantPatternPayload,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<Self, HirVariantPatternInvariantError> {
        let pattern = Self {
            head,
            name,
            payload,
        };
        pattern.validate(scope, resolver)?;
        Ok(pattern)
    }

    /// Returns the resolved or recovered head without inventing a path.
    pub const fn head(&self) -> &HirVariantPatternHeadValue {
        &self.head
    }

    /// Returns the resolved or recovered required variant name.
    pub const fn name(&self) -> &HirVariantPatternName {
        &self.name
    }

    /// Distinguishes authored absence, a typed child, and recovered absence.
    pub const fn payload(&self) -> &HirVariantPatternPayload {
        &self.payload
    }

    fn validate<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirVariantPatternInvariantError> {
        let payload = match self.payload {
            HirVariantPatternPayload::Pattern(payload)
            | HirVariantPatternPayload::Recovered {
                pattern: Some(payload),
                ..
            } => payload,
            HirVariantPatternPayload::Absent
            | HirVariantPatternPayload::Recovered { pattern: None, .. } => return Ok(()),
        };
        if payload.module() != scope.module() {
            return Err(HirVariantPatternInvariantError::ForeignPayload);
        }
        let Some(payload_record) = resolver.resolve_pattern(scope, payload) else {
            return Err(HirVariantPatternInvariantError::ForeignPayload);
        };
        if payload_record.scope() != scope {
            return Err(HirVariantPatternInvariantError::ForeignPayload);
        }
        if !matches!(
            payload_record.kind(),
            HirPatternKind::Tuple { .. } | HirPatternKind::Record { .. }
        ) {
            return Err(HirVariantPatternInvariantError::InvalidPayloadKind);
        }
        Ok(())
    }

    fn contains_recovery_payload<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> bool {
        matches!(&self.head, HirVariantPatternHeadValue::Recovered(_))
            || matches!(&self.name, HirVariantPatternName::Recovered(_))
            || match self.payload {
                HirVariantPatternPayload::Absent => false,
                HirVariantPatternPayload::Pattern(pattern) => {
                    pattern_is_recovered(scope, pattern, resolver)
                }
                HirVariantPatternPayload::Recovered { .. } => true,
            }
    }

    fn primary_recovery_issue<R: HirPatternResolver + ?Sized>(
        &self,
        scope: ScopeId,
        resolver: &R,
    ) -> Option<HirRecoveryIssue> {
        let head_issue = match &self.head {
            HirVariantPatternHeadValue::Resolved(_) => None,
            HirVariantPatternHeadValue::Recovered(issue) => {
                Some(HirPatternRecoveryIssue::VariantHead(*issue))
            }
        };
        let name_issue = match &self.name {
            HirVariantPatternName::Resolved(_) => None,
            HirVariantPatternName::Recovered(issue) => {
                Some(HirPatternRecoveryIssue::VariantName(*issue))
            }
        };
        head_issue
            .or(name_issue)
            .or_else(|| match self.payload {
                HirVariantPatternPayload::Absent => None,
                HirVariantPatternPayload::Pattern(pattern) => pattern_is_recovered(
                    scope, pattern, resolver,
                )
                .then_some(HirPatternRecoveryIssue::RecoveredChild {
                    role: HirPatternChildRole::VariantPayload,
                }),
                HirVariantPatternPayload::Recovered { issue, .. } => {
                    Some(HirPatternRecoveryIssue::VariantPayload(issue))
                }
            })
            .map(HirRecoveryIssue::InvalidPattern)
    }
}

/// A variant head that is either semantically resolved or retains exact typed
/// recovery shape without a placeholder path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternHeadValue {
    Resolved(HirVariantPatternHead),
    Recovered(HirVariantPatternHeadIssue),
}

/// Typed recovery for a required variant head.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternHeadIssue {
    #[error("variant pattern is missing its head")]
    Missing,
    #[error("variant pattern has an invalid qualified path with {segment_count} segments")]
    InvalidQualifiedPath { segment_count: u32 },
}

/// A required variant name that cannot collapse missing input into an empty
/// [`HirName`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternName {
    Resolved(HirName),
    Recovered(HirVariantPatternNameIssue),
}

/// Typed recovery for a required variant name.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternNameIssue {
    #[error("variant pattern is missing its name")]
    Missing,
    #[error("variant pattern has an invalid name")]
    Invalid(HirNameInvariantError),
}

/// An optional variant payload whose legitimate absence remains distinct from
/// syntax that started a payload but could not produce its Pattern child.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternPayload {
    Absent,
    Pattern(PatternId),
    Recovered {
        pattern: Option<PatternId>,
        issue: HirVariantPatternPayloadIssue,
    },
}

/// Typed source recovery for an authored variant payload.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternPayloadIssue {
    #[error("variant pattern is missing its authored payload")]
    MissingPattern,
    #[error("variant pattern payload is missing its close delimiter")]
    MissingCloseDelimiter,
    #[error("variant pattern has an invalid nested pattern")]
    InvalidPattern,
}

/// Qualified or expected-type-relative variant head.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternHead {
    Qualified(HirPath),
    Unqualified(HirUnqualifiedVariantForm),
}

/// Pathless form retained until shared expected-type variant resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnqualifiedVariantForm {
    DotShorthand,
    BareExpectedType,
}

/// Transaction invariant failure while admitting a variant-pattern child.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVariantPatternInvariantError {
    #[error("variant pattern payload is not a live same-scope pattern")]
    ForeignPayload,
    #[error("variant pattern payload is not a tuple or record pattern")]
    InvalidPayloadKind,
}

/// Generic pattern-family recovery retained only for unclassifiable syntax.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPatternError {
    issue: HirGenericPatternIssue,
}

impl HirPatternError {
    pub(crate) const fn new(issue: HirGenericPatternIssue) -> Self {
        Self { issue }
    }

    /// Returns the generic recovery cause.
    pub const fn issue(&self) -> HirGenericPatternIssue {
        self.issue
    }
}

/// Recovery causes reserved for syntax outside every known pattern family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirGenericPatternIssue {
    UnclassifiedSyntax,
    TransactionalChildFailure,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirPatternInvariantError {
    #[error("pattern scope {scope:?} is not live in the lowering transaction")]
    ScopeNotLive { scope: ScopeId },
    #[error("pattern local belongs to module {actual:?}, expected {expected:?}")]
    ForeignLocal {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("pattern local {local:?} is not live and visible from scope {scope:?}")]
    LocalNotVisible { scope: ScopeId, local: LocalId },
    #[error("nested pattern belongs to module {actual:?}, expected {expected:?}")]
    ForeignPattern {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("nested pattern {pattern:?} is not live and visible from scope {scope:?}")]
    PatternNotVisible { scope: ScopeId, pattern: PatternId },
    #[error("or-pattern requires at least two alternatives, observed {observed}")]
    OrPatternAlternativeCount { observed: usize },
    #[error("pattern type belongs to module {actual:?}, expected {expected:?}")]
    ForeignType {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("pattern type {ty:?} is not live and visible from scope {scope:?}")]
    TypeNotVisible { scope: ScopeId, ty: TypeId },
    #[error(transparent)]
    InvalidVariant(HirVariantPatternInvariantError),
    #[error("a clean pattern cannot contain a recovery payload")]
    CleanRecoveryPayload,
    #[error("pattern poison state does not retain the deterministic primary issue {expected:?}")]
    PatternRecoveryIssueMismatch { expected: HirRecoveryIssue },
    #[error("a pattern without recovery payload cannot retain poison state {actual:?}")]
    UnexpectedPatternPoison { actual: HirRecoveryIssue },
    #[error(
        "a multiple-rest issue must poison the containing sequence without replacing its first rest"
    )]
    MultipleRestCannotReplaceFirstRest,
}

fn fields_contain_recovery<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    fields: &[HirPatternField],
    resolver: &R,
) -> bool {
    fields.iter().any(|field| match field {
        HirPatternField::Explicit { pattern, .. } => {
            pattern_is_recovered(scope, *pattern, resolver)
        }
        HirPatternField::Invalid { .. } => true,
        HirPatternField::Shorthand { .. } | HirPatternField::Rest { .. } => false,
    })
}

fn patterns_contain_recovery<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    patterns: &[PatternId],
    resolver: &R,
) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern_is_recovered(scope, *pattern, resolver))
}

fn first_recovered_child<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    patterns: &[PatternId],
    resolver: &R,
    role: impl Fn(u32) -> HirPatternChildRole,
) -> Option<HirPatternRecoveryIssue> {
    patterns.iter().enumerate().find_map(|(index, pattern)| {
        pattern_is_recovered(scope, *pattern, resolver).then(|| {
            HirPatternRecoveryIssue::RecoveredChild {
                role: role(pattern_ordinal(index)),
            }
        })
    })
}

fn pattern_is_recovered<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    pattern: PatternId,
    resolver: &R,
) -> bool {
    resolver
        .resolve_pattern(scope, pattern)
        .is_some_and(HirPattern::is_poisoned)
}

fn pattern_ordinal(index: usize) -> u32 {
    u32::try_from(index).expect("validated Pattern limits fit HIR ordinals")
}

fn validate_fields<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    fields: &[HirPatternField],
    resolver: &R,
) -> Result<(), HirPatternInvariantError> {
    for field in fields {
        match field {
            HirPatternField::Explicit { pattern, .. } => {
                validate_pattern(scope, *pattern, resolver)?;
            }
            HirPatternField::Shorthand { local, .. } => {
                validate_local(scope, *local, resolver)?;
            }
            HirPatternField::Rest { binding } => {
                if let Some(binding) = binding {
                    validate_local(scope, *binding, resolver)?;
                }
            }
            HirPatternField::Invalid { .. } => {}
        }
    }
    Ok(())
}

fn validate_patterns<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    patterns: &[PatternId],
    resolver: &R,
) -> Result<(), HirPatternInvariantError> {
    patterns
        .iter()
        .try_for_each(|pattern| validate_pattern(scope, *pattern, resolver).map(|_| ()))
}

fn validate_pattern<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    pattern: PatternId,
    resolver: &R,
) -> Result<&HirPattern, HirPatternInvariantError> {
    let expected = scope.module();
    let actual = pattern.module();
    if actual != expected {
        return Err(HirPatternInvariantError::ForeignPattern { expected, actual });
    }
    resolver
        .resolve_pattern(scope, pattern)
        .ok_or(HirPatternInvariantError::PatternNotVisible { scope, pattern })
}

fn validate_local<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    local: LocalId,
    resolver: &R,
) -> Result<(), HirPatternInvariantError> {
    let expected = scope.module();
    let actual = local.module();
    if actual != expected {
        return Err(HirPatternInvariantError::ForeignLocal { expected, actual });
    }
    if !resolver.local_is_visible(scope, local) {
        return Err(HirPatternInvariantError::LocalNotVisible { scope, local });
    }
    Ok(())
}

fn validate_type<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    ty: TypeId,
    resolver: &R,
) -> Result<(), HirPatternInvariantError> {
    let expected = scope.module();
    let actual = ty.module();
    if actual != expected {
        return Err(HirPatternInvariantError::ForeignType { expected, actual });
    }
    if resolver.resolve_type_state(scope, ty).is_none() {
        return Err(HirPatternInvariantError::TypeNotVisible { scope, ty });
    }
    Ok(())
}

fn type_is_recovered<R: HirPatternResolver + ?Sized>(
    scope: ScopeId,
    ty: TypeId,
    resolver: &R,
) -> bool {
    resolver
        .resolve_type_state(scope, ty)
        .is_some_and(HirPoisonState::is_poisoned)
}

#[cfg(test)]
#[path = "pattern/tests.rs"]
mod tests;
