use std::option::IntoIter;

use arcweft_id::dialogue::DialogueLineId;
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocumentIdentity, SourceSpan,
};

use crate::identity::ExprId;
use crate::lowering::HirModuleKey;

use super::DialogueLineSourceOrder;

/// Stable diagnostic identities for dialogue line construction and acceptance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineDiagnosticCode {
    InvalidLineIdFamily,
    LineIdCollision,
    MissingLineSourceOwner,
    RelativeLineIdEscapesOwner,
    InvalidLineIdentityCoordinate,
    InvalidTextKeyFamily,
    DialogueLineIdentityLimit,
    DialogueLineSourceMismatch,
    DuplicateLineIdentityCoordinate,
    InvalidDialogueLineIdentity,
}

impl DialogueLineDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidLineIdFamily => "AW-CD-013",
            Self::LineIdCollision => "AW-CD-020",
            Self::MissingLineSourceOwner => "AW-CD-021",
            Self::RelativeLineIdEscapesOwner => "AW-CD-022",
            Self::InvalidLineIdentityCoordinate => "AW-CD-023",
            Self::InvalidTextKeyFamily => "AW-CD-024",
            Self::DialogueLineIdentityLimit => "AW-CD-025",
            Self::DialogueLineSourceMismatch => "AW-CD-026",
            Self::DuplicateLineIdentityCoordinate => "AW-CD-027",
            Self::InvalidDialogueLineIdentity => "AW-CD-028",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueIdentityCoordinateKind {
    LineId,
    TextKey,
}

impl DialogueIdentityCoordinateKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LineId => "id",
            Self::TextKey => "text_key",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InvalidCoordinateReason {
    RuntimeExpression,
    RecoveredValue,
}

impl InvalidCoordinateReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeExpression => "runtime_expression",
            Self::RecoveredValue => "recovered_value",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OwnerlessLineRequestKind {
    Generated,
    Relative,
    FamilyRelative,
}

impl OwnerlessLineRequestKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Relative => "relative",
            Self::FamilyRelative => "family_relative",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineLimitKind {
    IdentityBytes,
    CandidateCount,
    DiagnosticCount,
    Work,
    GeneratedOrdinal,
}

impl DialogueLineLimitKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdentityBytes => "identity_bytes",
            Self::CandidateCount => "candidate_count",
            Self::DiagnosticCount => "diagnostic_count",
            Self::Work => "work",
            Self::GeneratedOrdinal => "generated_ordinal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueIdentityErrorKind {
    InvalidBase,
    WrongFamily,
    EmptyTail,
    TooManyBytes,
}

impl DialogueIdentityErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidBase => "invalid_base",
            Self::WrongFamily => "wrong_family",
            Self::EmptyTail => "empty_tail",
            Self::TooManyBytes => "too_many_bytes",
        }
    }
}

/// Exact first or conflicting source site retained by a collision diagnostic.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineCollisionSite {
    module: HirModuleKey,
    application: ExprId,
    source_order: DialogueLineSourceOrder,
    span: SourceSpan,
}

impl DialogueLineCollisionSite {
    pub(crate) fn new(
        module: HirModuleKey,
        application: ExprId,
        source_order: DialogueLineSourceOrder,
        span: SourceSpan,
    ) -> Self {
        Self {
            module,
            application,
            source_order,
            span,
        }
    }

    pub const fn module(&self) -> &HirModuleKey {
        &self.module
    }

    pub const fn application(&self) -> ExprId {
        self.application
    }

    pub const fn source_order(&self) -> DialogueLineSourceOrder {
        self.source_order
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

/// Typed dialogue line diagnostic projected through the shared source transport.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineDiagnostic {
    InvalidLineIdFamily {
        found: String,
        span: SourceSpan,
    },
    LineIdCollision {
        id: DialogueLineId,
        first: Box<DialogueLineCollisionSite>,
        conflicting: Box<DialogueLineCollisionSite>,
    },
    MissingLineSourceOwner {
        application: SourceSpan,
        coordinate: Option<SourceSpan>,
        request: OwnerlessLineRequestKind,
    },
    RelativeLineIdEscapesOwner {
        requested: u16,
        available: u16,
        span: SourceSpan,
    },
    InvalidLineIdentityCoordinate {
        coordinate: DialogueIdentityCoordinateKind,
        reason: InvalidCoordinateReason,
        span: SourceSpan,
    },
    InvalidTextKeyFamily {
        found: Option<String>,
        span: SourceSpan,
    },
    DialogueLineIdentityLimit {
        kind: DialogueLineLimitKind,
        observed: u64,
        maximum: u64,
        span: Option<SourceSpan>,
    },
    DialogueLineSourceMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
        span: Option<SourceSpan>,
    },
    DuplicateLineIdentityCoordinate {
        coordinate: DialogueIdentityCoordinateKind,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    InvalidDialogueLineIdentity {
        coordinate: DialogueIdentityCoordinateKind,
        reason: DialogueIdentityErrorKind,
        span: SourceSpan,
    },
}

impl DialogueLineDiagnostic {
    pub(crate) fn compare_for_publication(&self, other: &Self) -> core::cmp::Ordering {
        self.primary_span()
            .cmp(&other.primary_span())
            .then_with(|| self.code().cmp(&other.code()))
            .then_with(|| self.cmp(other))
    }

    pub const fn code(&self) -> DialogueLineDiagnosticCode {
        match self {
            Self::InvalidLineIdFamily { .. } => DialogueLineDiagnosticCode::InvalidLineIdFamily,
            Self::LineIdCollision { .. } => DialogueLineDiagnosticCode::LineIdCollision,
            Self::MissingLineSourceOwner { .. } => {
                DialogueLineDiagnosticCode::MissingLineSourceOwner
            }
            Self::RelativeLineIdEscapesOwner { .. } => {
                DialogueLineDiagnosticCode::RelativeLineIdEscapesOwner
            }
            Self::InvalidLineIdentityCoordinate { .. } => {
                DialogueLineDiagnosticCode::InvalidLineIdentityCoordinate
            }
            Self::InvalidTextKeyFamily { .. } => DialogueLineDiagnosticCode::InvalidTextKeyFamily,
            Self::DialogueLineIdentityLimit { .. } => {
                DialogueLineDiagnosticCode::DialogueLineIdentityLimit
            }
            Self::DialogueLineSourceMismatch { .. } => {
                DialogueLineDiagnosticCode::DialogueLineSourceMismatch
            }
            Self::DuplicateLineIdentityCoordinate { .. } => {
                DialogueLineDiagnosticCode::DuplicateLineIdentityCoordinate
            }
            Self::InvalidDialogueLineIdentity { .. } => {
                DialogueLineDiagnosticCode::InvalidDialogueLineIdentity
            }
        }
    }

    pub fn primary_span(&self) -> Option<&SourceSpan> {
        match self {
            Self::InvalidLineIdFamily { span, .. }
            | Self::RelativeLineIdEscapesOwner { span, .. }
            | Self::InvalidLineIdentityCoordinate { span, .. }
            | Self::InvalidTextKeyFamily { span, .. }
            | Self::InvalidDialogueLineIdentity { span, .. } => Some(span),
            Self::LineIdCollision { conflicting, .. } => Some(conflicting.span()),
            Self::MissingLineSourceOwner {
                application,
                coordinate,
                ..
            } => coordinate.as_ref().or(Some(application)),
            Self::DialogueLineIdentityLimit { span, .. }
            | Self::DialogueLineSourceMismatch { span, .. } => span.as_ref(),
            Self::DuplicateLineIdentityCoordinate { duplicate, .. } => Some(duplicate),
        }
    }

    pub fn related_spans(&self) -> IntoIter<&SourceSpan> {
        match self {
            Self::LineIdCollision { first, .. } => Some(first.span()),
            Self::MissingLineSourceOwner {
                application,
                coordinate: Some(_),
                ..
            } => Some(application),
            Self::DuplicateLineIdentityCoordinate { first, .. } => Some(first),
            _ => None,
        }
        .into_iter()
    }

    pub const fn line_id(&self) -> Option<&DialogueLineId> {
        match self {
            Self::LineIdCollision { id, .. } => Some(id),
            _ => None,
        }
    }

    pub fn to_source_diagnostic(&self) -> Diagnostic {
        let message = self.message();
        let mut diagnostic =
            Diagnostic::new(DiagnosticSeverity::Error, message).with_code(self.code().as_str());
        if let Some(primary) = self.primary_span() {
            diagnostic = diagnostic.with_label(DiagnosticLabel::primary(
                primary.clone(),
                Some(self.primary_label()),
            ));
        }
        if let Some(related) = self.related_spans().next() {
            diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(
                related.clone(),
                Some(self.secondary_label()),
            ));
        }
        diagnostic
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidLineIdFamily { found, .. } => {
                format!("dialogue line ID family `{found}` must be `say`")
            }
            Self::LineIdCollision { id, .. } => format!(
                "dialogue line ID `@{}` is produced by more than one source site",
                id.as_str()
            ),
            Self::MissingLineSourceOwner { request, .. } => format!(
                "{} dialogue line ID requires a typed Flow or callable owner",
                request.as_str()
            ),
            Self::RelativeLineIdEscapesOwner {
                requested,
                available,
                ..
            } => format!(
                "relative dialogue line ID requests {requested} parent scopes, but only {available} are available"
            ),
            Self::InvalidLineIdentityCoordinate {
                coordinate, reason, ..
            } => format!(
                "dialogue `{}` coordinate is not a durable identity: {}",
                coordinate.as_str(),
                reason.as_str()
            ),
            Self::InvalidTextKeyFamily { found, .. } => format!(
                "dialogue text key must be an absolute `text.*` identity, found {}",
                found.as_deref().unwrap_or("a relative identity")
            ),
            Self::DialogueLineIdentityLimit {
                kind,
                observed,
                maximum,
                ..
            } => format!(
                "dialogue line {} limit exceeded: observed {observed}, maximum {maximum}",
                kind.as_str()
            ),
            Self::DialogueLineSourceMismatch {
                expected, actual, ..
            } => format!(
                "dialogue line source {actual:?} does not match accepted source {expected:?}"
            ),
            Self::DuplicateLineIdentityCoordinate { coordinate, .. } => format!(
                "dialogue `{}` coordinate is supplied more than once",
                coordinate.as_str()
            ),
            Self::InvalidDialogueLineIdentity {
                coordinate, reason, ..
            } => format!(
                "dialogue `{}` identity is invalid: {}",
                coordinate.as_str(),
                reason.as_str()
            ),
        }
    }

    fn primary_label(&self) -> String {
        match self {
            Self::LineIdCollision { id, .. } => {
                format!("this site also produces `@{}`", id.as_str())
            }
            _ => self.message(),
        }
    }

    fn secondary_label(&self) -> String {
        match self {
            Self::LineIdCollision { id, .. } => {
                format!("first site producing `@{}`", id.as_str())
            }
            Self::DuplicateLineIdentityCoordinate { coordinate, .. } => {
                format!("first `{}` coordinate", coordinate.as_str())
            }
            Self::MissingLineSourceOwner { .. } => "ownerless dialogue application".to_owned(),
            _ => "related dialogue line source".to_owned(),
        }
    }
}
