//! Structured nominal diagnostics and poison provenance.

use core::cmp::Ordering;

use arcweft_lang_hir::{
    leaf::{HirPath, HirPathRoot, HirPathSegment},
    symbol::{ProjectTypeCandidate, nominal::ProjectNominalDeclarationId},
};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity};

use crate::types::TypePoisonId;

use super::{
    NominalResolutionLimitKind, TypeArgumentExpectation, TypeArgumentKind, TypeArityExpectation,
    TypeArityTarget, TypeSourceEvidence,
};

/// Stable code for one semantic nominal diagnostic family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalTypeDiagnosticCode {
    UnknownType,
    AmbiguousType,
    InaccessibleType,
    WrongKind,
    WrongArity,
    CyclicAlias,
    SelfUnavailable,
    Limit,
    WorkOverflow,
}

/// Typed payload of one semantic nominal diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalTypeDiagnosticKind {
    Unknown {
        path: HirPath,
    },
    Ambiguous {
        path: HirPath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        path: HirPath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        path: HirPath,
        actual: ProjectTypeCandidate,
    },
    WrongArgumentKind {
        target: TypeArityTarget,
        argument: u16,
        expected: TypeArgumentExpectation,
        actual: TypeArgumentKind,
    },
    WrongArity {
        target: TypeArityTarget,
        expected: TypeArityExpectation,
        actual: u16,
    },
    CyclicAlias {
        cycle: Box<[ProjectNominalDeclarationId]>,
    },
    SelfUnavailable,
    Limit {
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    WorkOverflow {
        attempted: u64,
        maximum: u64,
    },
}

/// Meaning of a deterministic secondary label.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalRelatedMessage {
    CandidateDeclaration,
    CandidateBinding,
    ActualDeclaration,
    ExpectedArityDeclaration,
    AliasDeclaration,
    AliasTarget,
    CycleDeclaration,
    CycleTarget,
}

/// One exact secondary source attached to a nominal diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalDiagnosticRelated {
    source: TypeSourceEvidence,
    message: NominalRelatedMessage,
}

/// One structured semantic nominal diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalTypeDiagnostic {
    poison: TypePoisonId,
    kind: NominalTypeDiagnosticKind,
    primary: TypeSourceEvidence,
    secondary: Box<[NominalDiagnosticRelated]>,
    omitted_secondary: u64,
}

/// Authority that originally allocated a semantic type poison.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypePoisonOrigin {
    SyntaxTypeDiagnostic,
    NominalTypeDiagnostic,
    UpstreamTypeDiagnostic,
    DetachedUnavailable,
}

/// Immutable provenance for one resolver-local poison identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypePoisonRecord {
    id: TypePoisonId,
    origin: TypePoisonOrigin,
    primary: TypeSourceEvidence,
    authoritative_for_annotation: bool,
}

impl NominalTypeDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownType => "sema.nominal.unknown_type",
            Self::AmbiguousType => "sema.nominal.ambiguous_type",
            Self::InaccessibleType => "sema.nominal.inaccessible_type",
            Self::WrongKind => "sema.nominal.wrong_kind",
            Self::WrongArity => "sema.nominal.wrong_arity",
            Self::CyclicAlias => "sema.nominal.cyclic_alias",
            Self::SelfUnavailable => "sema.nominal.self_unavailable",
            Self::Limit => "sema.nominal.limit",
            Self::WorkOverflow => "sema.nominal.work_overflow",
        }
    }
}

impl NominalTypeDiagnosticKind {
    pub const fn code(&self) -> NominalTypeDiagnosticCode {
        match self {
            Self::Unknown { .. } => NominalTypeDiagnosticCode::UnknownType,
            Self::Ambiguous { .. } => NominalTypeDiagnosticCode::AmbiguousType,
            Self::Inaccessible { .. } => NominalTypeDiagnosticCode::InaccessibleType,
            Self::WrongKind { .. } | Self::WrongArgumentKind { .. } => {
                NominalTypeDiagnosticCode::WrongKind
            }
            Self::WrongArity { .. } => NominalTypeDiagnosticCode::WrongArity,
            Self::CyclicAlias { .. } => NominalTypeDiagnosticCode::CyclicAlias,
            Self::SelfUnavailable => NominalTypeDiagnosticCode::SelfUnavailable,
            Self::Limit { .. } => NominalTypeDiagnosticCode::Limit,
            Self::WorkOverflow { .. } => NominalTypeDiagnosticCode::WorkOverflow,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Unknown { path } => format!("unknown type `{}`", format_hir_path(path)),
            Self::Ambiguous { path, .. } => {
                format!("type `{}` is ambiguous", format_hir_path(path))
            }
            Self::Inaccessible { path, .. } => {
                format!("type `{}` is inaccessible here", format_hir_path(path))
            }
            Self::WrongKind { path, .. } => {
                format!(
                    "`{}` does not name a type declaration",
                    format_hir_path(path)
                )
            }
            Self::WrongArgumentKind {
                argument, expected, ..
            } => format!(
                "type constructor argument {argument} has the wrong kind; expected {expected:?}"
            ),
            Self::WrongArity {
                expected, actual, ..
            } => match expected {
                TypeArityExpectation::Exact(expected) => format!(
                    "type constructor expects {expected} argument(s), but {actual} were supplied"
                ),
                TypeArityExpectation::Inclusive { minimum, maximum } => format!(
                    "type constructor expects {minimum}..={maximum} arguments, but {actual} were supplied"
                ),
            },
            Self::CyclicAlias { .. } => "cyclic project type alias".to_owned(),
            Self::SelfUnavailable => "`Self` is unavailable in this type scope".to_owned(),
            Self::Limit {
                kind,
                observed,
                maximum,
            } => format!(
                "nominal resolution limit {kind:?} exceeded: observed {observed}, maximum {maximum}"
            ),
            Self::WorkOverflow { attempted, maximum } => format!(
                "nominal resolution work budget exceeded: attempted {attempted}, maximum {maximum}"
            ),
        }
    }
}

fn format_hir_path(path: &HirPath) -> String {
    let mut rendered = match path.root() {
        HirPathRoot::ImplicitCrate => String::new(),
        HirPathRoot::Crate => "crate::".to_owned(),
        HirPathRoot::SelfModule => "self::".to_owned(),
        HirPathRoot::Super { depth } => "super::".repeat(depth),
    };
    rendered.push_str(
        &path
            .segments()
            .iter()
            .map(|segment| match segment {
                HirPathSegment::Identifier(name) => name.as_str(),
                HirPathSegment::ProjectSymbol(name) => name.as_str(),
            })
            .collect::<Vec<_>>()
            .join("::"),
    );
    rendered
}

impl Ord for NominalTypeDiagnosticKind {
    fn cmp(&self, other: &Self) -> Ordering {
        diagnostic_rank(self)
            .cmp(&diagnostic_rank(other))
            .then_with(|| {
                if let Some(ordering) = candidate_diagnostic_cmp(self, other) {
                    return ordering;
                }
                match (self, other) {
                    (
                        Self::WrongArgumentKind {
                            target: left_target,
                            argument: left_argument,
                            expected: left_expected,
                            actual: left_actual,
                        },
                        Self::WrongArgumentKind {
                            target: right_target,
                            argument: right_argument,
                            expected: right_expected,
                            actual: right_actual,
                        },
                    ) => (left_target, left_argument, left_expected)
                        .cmp(&(right_target, right_argument, right_expected))
                        .then_with(|| left_actual.stable_ordering(right_actual)),
                    (
                        Self::WrongArity {
                            target: left_target,
                            expected: left_expected,
                            actual: left_actual,
                        },
                        Self::WrongArity {
                            target: right_target,
                            expected: right_expected,
                            actual: right_actual,
                        },
                    ) => (left_target, left_expected, left_actual).cmp(&(
                        right_target,
                        right_expected,
                        right_actual,
                    )),
                    (Self::CyclicAlias { cycle: left }, Self::CyclicAlias { cycle: right }) => {
                        left.cmp(right)
                    }
                    (
                        Self::Limit {
                            kind: left_kind,
                            observed: left_observed,
                            maximum: left_maximum,
                        },
                        Self::Limit {
                            kind: right_kind,
                            observed: right_observed,
                            maximum: right_maximum,
                        },
                    ) => (left_kind, left_observed, left_maximum).cmp(&(
                        right_kind,
                        right_observed,
                        right_maximum,
                    )),
                    (
                        Self::WorkOverflow {
                            attempted: left_attempted,
                            maximum: left_maximum,
                        },
                        Self::WorkOverflow {
                            attempted: right_attempted,
                            maximum: right_maximum,
                        },
                    ) => (left_attempted, left_maximum).cmp(&(right_attempted, right_maximum)),
                    _ => Ordering::Equal,
                }
            })
    }
}

fn candidate_diagnostic_cmp(
    left: &NominalTypeDiagnosticKind,
    right: &NominalTypeDiagnosticKind,
) -> Option<Ordering> {
    Some(match (left, right) {
        (
            NominalTypeDiagnosticKind::Unknown { path: left },
            NominalTypeDiagnosticKind::Unknown { path: right },
        ) => left.cmp(right),
        (
            NominalTypeDiagnosticKind::Ambiguous {
                path: left_path,
                candidates: left_candidates,
            },
            NominalTypeDiagnosticKind::Ambiguous {
                path: right_path,
                candidates: right_candidates,
            },
        )
        | (
            NominalTypeDiagnosticKind::Inaccessible {
                path: left_path,
                candidates: left_candidates,
            },
            NominalTypeDiagnosticKind::Inaccessible {
                path: right_path,
                candidates: right_candidates,
            },
        ) => left_path
            .cmp(right_path)
            .then_with(|| candidate_slices_cmp(left_candidates, right_candidates)),
        (
            NominalTypeDiagnosticKind::WrongKind {
                path: left_path,
                actual: left_actual,
            },
            NominalTypeDiagnosticKind::WrongKind {
                path: right_path,
                actual: right_actual,
            },
        ) => left_path
            .cmp(right_path)
            .then_with(|| candidate_cmp(left_actual, right_actual)),
        _ => return None,
    })
}

impl PartialOrd for NominalTypeDiagnosticKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NominalRelatedMessage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CandidateDeclaration => "candidate declaration",
            Self::CandidateBinding => "candidate introduced here",
            Self::ActualDeclaration => "this declaration has a non-type kind",
            Self::ExpectedArityDeclaration => "type parameters declared here",
            Self::AliasDeclaration => "alias declared here",
            Self::AliasTarget => "alias target referenced here",
            Self::CycleDeclaration => "alias in this cycle",
            Self::CycleTarget => "cycle continues through this target",
        }
    }
}

impl NominalDiagnosticRelated {
    pub(crate) const fn new(source: TypeSourceEvidence, message: NominalRelatedMessage) -> Self {
        Self { source, message }
    }

    pub const fn source(&self) -> &TypeSourceEvidence {
        &self.source
    }

    pub const fn message(&self) -> NominalRelatedMessage {
        self.message
    }
}

impl NominalTypeDiagnostic {
    pub(crate) fn new(
        poison: TypePoisonId,
        kind: NominalTypeDiagnosticKind,
        primary: TypeSourceEvidence,
        secondary: impl Into<Box<[NominalDiagnosticRelated]>>,
        omitted_secondary: u64,
    ) -> Self {
        Self {
            poison,
            kind,
            primary,
            secondary: secondary.into(),
            omitted_secondary,
        }
    }

    pub const fn poison(&self) -> TypePoisonId {
        self.poison
    }

    pub const fn kind(&self) -> &NominalTypeDiagnosticKind {
        &self.kind
    }

    pub const fn primary(&self) -> &TypeSourceEvidence {
        &self.primary
    }

    pub fn secondary(&self) -> &[NominalDiagnosticRelated] {
        &self.secondary
    }

    /// Related labels omitted after deterministic sort and deduplication.
    pub const fn omitted_secondary(&self) -> u64 {
        self.omitted_secondary
    }

    /// Projects exact accepted source evidence into the shared source diagnostic model.
    /// Detached diagnostics intentionally return `None`.
    pub fn to_source_diagnostic(&self) -> Option<Diagnostic> {
        let primary = self.primary.project()?.clone();
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Error, self.kind.message())
            .with_code(self.kind.code().as_str())
            .with_label(DiagnosticLabel::primary(primary, None));
        Some(
            self.secondary
                .iter()
                .filter_map(|related| {
                    related.source.project().cloned().map(|span| {
                        DiagnosticLabel::secondary(span, Some(related.message.as_str().to_owned()))
                    })
                })
                .fold(diagnostic, Diagnostic::with_label),
        )
    }
}

impl TypePoisonRecord {
    pub(crate) const fn new(
        id: TypePoisonId,
        origin: TypePoisonOrigin,
        primary: TypeSourceEvidence,
        authoritative_for_annotation: bool,
    ) -> Self {
        Self {
            id,
            origin,
            primary,
            authoritative_for_annotation,
        }
    }

    pub const fn id(&self) -> TypePoisonId {
        self.id
    }

    pub const fn origin(&self) -> TypePoisonOrigin {
        self.origin
    }

    pub const fn primary(&self) -> &TypeSourceEvidence {
        &self.primary
    }

    pub const fn authoritative_for_annotation(&self) -> bool {
        self.authoritative_for_annotation
    }
}

const fn diagnostic_rank(kind: &NominalTypeDiagnosticKind) -> u8 {
    match kind {
        NominalTypeDiagnosticKind::Unknown { .. } => 0,
        NominalTypeDiagnosticKind::Ambiguous { .. } => 1,
        NominalTypeDiagnosticKind::Inaccessible { .. } => 2,
        NominalTypeDiagnosticKind::WrongKind { .. } => 3,
        NominalTypeDiagnosticKind::WrongArgumentKind { .. } => 4,
        NominalTypeDiagnosticKind::WrongArity { .. } => 5,
        NominalTypeDiagnosticKind::CyclicAlias { .. } => 6,
        NominalTypeDiagnosticKind::SelfUnavailable => 7,
        NominalTypeDiagnosticKind::Limit { .. } => 8,
        NominalTypeDiagnosticKind::WorkOverflow { .. } => 9,
    }
}

fn candidate_slices_cmp(left: &[ProjectTypeCandidate], right: &[ProjectTypeCandidate]) -> Ordering {
    left.iter()
        .zip(right)
        .find_map(|(left, right)| {
            let ordering = candidate_cmp(left, right);
            (ordering != Ordering::Equal).then_some(ordering)
        })
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn candidate_cmp(left: &ProjectTypeCandidate, right: &ProjectTypeCandidate) -> Ordering {
    left.target()
        .cmp(right.target())
        .then_with(|| left.declaration().cmp(&right.declaration()))
        .then_with(|| left.binding_sites().cmp(right.binding_sites()))
}

#[cfg(test)]
mod tests {
    use arcweft_source::{
        DiagnosticLabelStyle, SourceDocument, SourceDocumentId, SourceName, SourceRange,
    };

    use super::*;

    #[test]
    fn diagnostic_codes_are_stable() {
        assert_eq!(
            NominalTypeDiagnosticCode::UnknownType.as_str(),
            "sema.nominal.unknown_type"
        );
        assert_eq!(
            NominalTypeDiagnosticCode::WorkOverflow.as_str(),
            "sema.nominal.work_overflow"
        );
    }

    #[test]
    fn detached_source_does_not_fabricate_a_project_diagnostic() {
        let diagnostic = NominalTypeDiagnostic::new(
            TypePoisonId::from_index(7),
            NominalTypeDiagnosticKind::SelfUnavailable,
            TypeSourceEvidence::detached(SourceRange::new(2, 6)),
            [],
            0,
        );
        assert!(diagnostic.to_source_diagnostic().is_none());
        assert_eq!(diagnostic.primary().local(), SourceRange::new(2, 6));
    }

    #[test]
    fn accepted_projection_preserves_primary_and_secondary_spans() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://nominal/diagnostic.arcw")
                .expect("source ID"),
            SourceName::Memory,
            "Missing Candidate",
        )
        .expect("source document");
        let primary = document.span(SourceRange::new(0, 7)).expect("primary span");
        let secondary = document
            .span(SourceRange::new(8, 17))
            .expect("secondary span");
        let diagnostic = NominalTypeDiagnostic::new(
            TypePoisonId::from_index(3),
            NominalTypeDiagnosticKind::SelfUnavailable,
            TypeSourceEvidence::accepted(SourceRange::new(0, 7), primary.clone()),
            [NominalDiagnosticRelated::new(
                TypeSourceEvidence::accepted(SourceRange::new(8, 17), secondary.clone()),
                NominalRelatedMessage::CandidateDeclaration,
            )],
            0,
        )
        .to_source_diagnostic()
        .expect("accepted evidence projects");

        assert_eq!(
            diagnostic.code().expect("stable code").as_str(),
            "sema.nominal.self_unavailable"
        );
        assert_eq!(diagnostic.labels().len(), 2);
        assert_eq!(
            diagnostic.labels()[0].style(),
            DiagnosticLabelStyle::Primary
        );
        assert_eq!(diagnostic.labels()[0].span(), &primary);
        assert_eq!(
            diagnostic.labels()[1].style(),
            DiagnosticLabelStyle::Secondary
        );
        assert_eq!(diagnostic.labels()[1].span(), &secondary);
    }
}
