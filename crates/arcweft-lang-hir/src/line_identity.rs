//! Typed source and durable-identity facts for dialogue lines.

use std::sync::Arc;

use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use crate::identity::{ExprId, IdResolveError, ScopeId};
use crate::lowering::HirModuleKey;
use crate::symbol::CallableDeclarationId;

mod builder;
mod diagnostic;
pub(crate) mod module_candidates;

pub use self::diagnostic::{
    DialogueIdentityCoordinateKind, DialogueIdentityErrorKind, DialogueLineCollisionSite,
    DialogueLineDiagnostic, DialogueLineDiagnosticCode, DialogueLineLimitKind,
    InvalidCoordinateReason, OwnerlessLineRequestKind,
};

/// Checked complete Flow owner used to derive dialogue line prefixes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueFlowOwner {
    id: PublicId,
}

impl HirDialogueFlowOwner {
    /// Accepts exactly one complete `flow.*` identity.
    pub fn try_new(id: PublicId) -> Result<Self, HirDialogueFlowOwnerError> {
        DeclarationIdentityFamily::Flow
            .validate_public_id(&id)
            .map_err(|_| HirDialogueFlowOwnerError::InvalidFlowIdentity { id: id.clone() })?;
        Ok(Self { id })
    }

    /// Returns the complete accepted Flow identity.
    pub const fn id(&self) -> &PublicId {
        &self.id
    }
}

/// Invalid Flow owner supplied to dialogue line identity construction.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirDialogueFlowOwnerError {
    #[error("dialogue Flow owner `{id}` is not a complete `flow.*` identity")]
    InvalidFlowIdentity { id: PublicId },
}

/// Closed semantic source owner of one dialogue application.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueLineSourceOwner {
    Flow(HirDialogueFlowOwner),
    Callable(CallableDeclarationId),
    Ownerless,
}

/// One authored named lexical scope contributing to a line prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueNamedScope {
    scope: ScopeId,
    segment: ModuleSegment,
    declaration: SourceSpan,
}

impl HirDialogueNamedScope {
    pub(crate) fn new(scope: ScopeId, segment: ModuleSegment, declaration: SourceSpan) -> Self {
        Self {
            scope,
            segment,
            declaration,
        }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn segment(&self) -> &ModuleSegment {
        &self.segment
    }

    pub const fn declaration(&self) -> &SourceSpan {
        &self.declaration
    }
}

/// Deterministic module traversal coordinate for one source dialogue site.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineSourceOrder(u32);

impl DialogueLineSourceOrder {
    pub(crate) fn try_new(value: u32) -> Result<Self, DialogueLineBuildFatal> {
        if value == 0 {
            return Err(DialogueLineBuildFatal::ArithmeticOverflow {
                operation: DialogueLineBuildOperation::SourceOrder,
            });
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// How the final line ID was selected from the authored application.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineIdOrigin {
    ExplicitAbsolute,
    ExplicitRelative,
    ExplicitFamilyRelative,
    Generated,
}

impl DialogueLineIdOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitAbsolute => "explicit_absolute",
            Self::ExplicitRelative => "explicit_relative",
            Self::ExplicitFamilyRelative => "explicit_family_relative",
            Self::Generated => "generated",
        }
    }
}

/// How the final localization text key was selected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueTextKeyOrigin {
    Explicit,
    Derived,
}

impl DialogueTextKeyOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Derived => "derived",
        }
    }
}

/// Complete revision-bound source site retained by a module candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirDialogueLineSourceSite {
    application: ExprId,
    owner: HirDialogueLineSourceOwner,
    named_scopes: Arc<[HirDialogueNamedScope]>,
    source_order: DialogueLineSourceOrder,
    application_span: SourceSpan,
    id_coordinate_span: Option<SourceSpan>,
    text_key_coordinate_span: Option<SourceSpan>,
}

impl HirDialogueLineSourceSite {
    #[allow(
        clippy::too_many_arguments,
        reason = "the source site atomically retains the complete selected line-identity evidence"
    )]
    pub(crate) fn try_new(
        source: &SourceDocumentIdentity,
        application: ExprId,
        owner: HirDialogueLineSourceOwner,
        named_scopes: Arc<[HirDialogueNamedScope]>,
        source_order: DialogueLineSourceOrder,
        application_span: SourceSpan,
        id_coordinate_span: Option<SourceSpan>,
        text_key_coordinate_span: Option<SourceSpan>,
    ) -> Result<Self, DialogueLineBuildFatal> {
        if named_scopes
            .iter()
            .any(|scope| scope.scope().module() != application.module())
        {
            return Err(DialogueLineBuildFatal::InvalidSourceComponent);
        }
        let mismatched_source = std::iter::once(&application_span)
            .chain(id_coordinate_span.iter())
            .chain(text_key_coordinate_span.iter())
            .chain(named_scopes.iter().map(HirDialogueNamedScope::declaration))
            .find(|span| span.source() != source);
        if let Some(span) = mismatched_source {
            return Err(DialogueLineBuildFatal::SourceIdentityMismatch {
                expected: source.clone(),
                actual: span.source().clone(),
            });
        }
        Ok(Self {
            application,
            owner,
            named_scopes,
            source_order,
            application_span,
            id_coordinate_span,
            text_key_coordinate_span,
        })
    }

    pub(crate) const fn application(&self) -> ExprId {
        self.application
    }

    pub(crate) const fn owner(&self) -> &HirDialogueLineSourceOwner {
        &self.owner
    }

    pub(crate) const fn named_scopes(&self) -> &Arc<[HirDialogueNamedScope]> {
        &self.named_scopes
    }

    pub(crate) const fn source_order(&self) -> DialogueLineSourceOrder {
        self.source_order
    }

    pub(crate) const fn application_span(&self) -> &SourceSpan {
        &self.application_span
    }

    pub(crate) const fn id_coordinate_span(&self) -> Option<&SourceSpan> {
        self.id_coordinate_span.as_ref()
    }

    pub(crate) const fn text_key_coordinate_span(&self) -> Option<&SourceSpan> {
        self.text_key_coordinate_span.as_ref()
    }
}

/// One bounded, unaccepted module-local dialogue line candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirDialogueLineCandidate {
    id: DialogueLineId,
    id_origin: DialogueLineIdOrigin,
    text_key: DialogueTextKey,
    text_key_origin: DialogueTextKeyOrigin,
    site: HirDialogueLineSourceSite,
}

impl HirDialogueLineCandidate {
    pub(crate) const fn id(&self) -> &DialogueLineId {
        &self.id
    }

    pub(crate) const fn id_origin(&self) -> DialogueLineIdOrigin {
        self.id_origin
    }

    pub(crate) const fn text_key(&self) -> &DialogueTextKey {
        &self.text_key
    }

    pub(crate) const fn text_key_origin(&self) -> DialogueTextKeyOrigin {
        self.text_key_origin
    }

    pub(crate) const fn site(&self) -> &HirDialogueLineSourceSite {
        &self.site
    }
}

/// Immutable candidate inventory owned by exactly one HIR module revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirDialogueLineCandidates {
    module: HirModuleKey,
    records: Arc<[HirDialogueLineCandidate]>,
}

impl HirDialogueLineCandidates {
    pub(crate) fn empty(module: HirModuleKey) -> Self {
        Self {
            module,
            records: Arc::from([]),
        }
    }

    pub(crate) const fn module(&self) -> &HirModuleKey {
        &self.module
    }

    pub(crate) fn records(&self) -> &[HirDialogueLineCandidate] {
        &self.records
    }
}

/// Checked operation whose arithmetic failed during candidate construction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineBuildOperation {
    SourceOrder,
    Work,
    PrefixBytes,
    GeneratedOrdinal,
}

/// Fatal module candidate failure which publishes no HIR snapshot.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueLineBuildFatal {
    #[error("dialogue line source identity {actual:?} does not match module source {expected:?}")]
    SourceIdentityMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("dialogue line candidate references a stale HIR identity: {error}")]
    StaleHirId { error: IdResolveError },
    #[error("dialogue line candidate arithmetic overflow during {operation:?}")]
    ArithmeticOverflow {
        operation: DialogueLineBuildOperation,
    },
    #[error("dialogue line candidate count {observed} exceeds maximum {maximum}")]
    CandidateLimit { observed: usize, maximum: usize },
    #[error("dialogue line diagnostic count {observed} exceeds maximum {maximum}")]
    DiagnosticLimit { observed: usize, maximum: usize },
    #[error("dialogue line candidate work {observed} exceeds maximum {maximum}")]
    WorkLimit { observed: u32, maximum: u32 },
    #[error("dialogue line candidate contains an invalid internal prefix")]
    InvalidInternalPrefix,
    #[error("dialogue line candidate contains an invalid source component")]
    InvalidSourceComponent,
}
