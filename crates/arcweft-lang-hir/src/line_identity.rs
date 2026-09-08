//! Typed source and durable-identity facts for dialogue lines.

use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use crate::identity::{ExprId, IdResolveError, ScopeId};
use crate::leaf::HirIdRef;
use crate::lowering::HirModuleKey;
use crate::symbol::CallableDeclarationId;

pub(crate) mod builder;
mod diagnostic;
pub(crate) mod sites;

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

/// Exact relation between the authored source application and the semantic
/// expression that can produce a dialogue line.
///
/// An ambiguous postfix retains its authored outer expression as the source
/// application while its synthetic dialogue candidate is the semantic
/// application. The index candidate is topology evidence only and is never a
/// line identity candidate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueLineSiteTopology {
    Direct,
    OuterPostfixBracket { index_candidate: ExprId },
}

impl HirDialogueLineSiteTopology {
    pub const fn index_candidate(self) -> Option<ExprId> {
        match self {
            Self::Direct => None,
            Self::OuterPostfixBracket { index_candidate } => Some(index_candidate),
        }
    }
}

/// Complete revision-bound dialogue source site retained before semantic
/// selection. This is source/topology evidence, not an accepted line ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueLineSite {
    source_application: ExprId,
    semantic_application: ExprId,
    topology: HirDialogueLineSiteTopology,
    owner: HirDialogueLineSourceOwner,
    named_scopes: Arc<[HirDialogueNamedScope]>,
    source_order: DialogueLineSourceOrder,
    application_span: SourceSpan,
    id_coordinate_span: Option<SourceSpan>,
    text_key_coordinate_span: Option<SourceSpan>,
}

impl HirDialogueLineSite {
    #[allow(
        clippy::too_many_arguments,
        reason = "the source site atomically retains source, semantic, topology, and range evidence"
    )]
    pub(crate) fn try_new(
        source: &SourceDocumentIdentity,
        source_application: ExprId,
        semantic_application: ExprId,
        topology: HirDialogueLineSiteTopology,
        owner: HirDialogueLineSourceOwner,
        named_scopes: Arc<[HirDialogueNamedScope]>,
        source_order: DialogueLineSourceOrder,
        application_span: SourceSpan,
        id_coordinate_span: Option<SourceSpan>,
        text_key_coordinate_span: Option<SourceSpan>,
    ) -> Result<Self, DialogueLineBuildFatal> {
        if source_application.module() != semantic_application.module()
            || named_scopes
                .iter()
                .any(|scope| scope.scope().module() != source_application.module())
        {
            return Err(DialogueLineBuildFatal::InvalidSourceComponent);
        }
        if topology
            .index_candidate()
            .is_some_and(|candidate| candidate.module() != source_application.module())
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
            source_application,
            semantic_application,
            topology,
            owner,
            named_scopes,
            source_order,
            application_span,
            id_coordinate_span,
            text_key_coordinate_span,
        })
    }

    pub const fn source_application(&self) -> ExprId {
        self.source_application
    }

    pub const fn semantic_application(&self) -> ExprId {
        self.semantic_application
    }

    pub const fn topology(&self) -> HirDialogueLineSiteTopology {
        self.topology
    }

    pub const fn owner(&self) -> &HirDialogueLineSourceOwner {
        &self.owner
    }

    pub fn named_scopes(&self) -> &[HirDialogueNamedScope] {
        &self.named_scopes
    }

    pub const fn source_order(&self) -> DialogueLineSourceOrder {
        self.source_order
    }

    pub const fn application_span(&self) -> &SourceSpan {
        &self.application_span
    }

    pub const fn id_coordinate_span(&self) -> Option<&SourceSpan> {
        self.id_coordinate_span.as_ref()
    }

    pub const fn text_key_coordinate_span(&self) -> Option<&SourceSpan> {
        self.text_key_coordinate_span.as_ref()
    }

    /// Resolves an authored line-ID coordinate as typed evidence only. This
    /// never allocates a generated ordinal and never performs project
    /// collision acceptance.
    pub fn resolve_explicit_line_id(
        &self,
        module: &HirModuleKey,
        reference: &HirIdRef,
    ) -> Option<(DialogueLineId, DialogueLineIdOrigin)> {
        builder::resolve_explicit_line_id(module, self, reference)
    }

    /// Resolves an authored absolute text-key coordinate as typed evidence.
    pub fn resolve_explicit_text_key(&self, reference: &HirIdRef) -> Option<DialogueTextKey> {
        let HirIdRef::Absolute(reference) = reference else {
            return None;
        };
        (reference.segments().next() == Some(DialogueTextKey::family_prefix()))
            .then(|| DialogueTextKey::try_new(reference.as_str().to_owned()).ok())
            .flatten()
    }
}

/// Immutable module-local source-site inventory for one exact HIR revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueLineSiteInventory {
    module: HirModuleKey,
    records: Arc<[HirDialogueLineSite]>,
    by_source_expr: BTreeMap<ExprId, usize>,
    by_semantic_expr: BTreeMap<ExprId, usize>,
    source_order: Arc<[usize]>,
}

impl HirDialogueLineSiteInventory {
    pub(crate) fn empty(module: HirModuleKey) -> Self {
        Self {
            module,
            records: Arc::from([]),
            by_source_expr: BTreeMap::new(),
            by_semantic_expr: BTreeMap::new(),
            source_order: Arc::from([]),
        }
    }

    pub(crate) fn new(
        module: HirModuleKey,
        mut records: Vec<HirDialogueLineSite>,
    ) -> Result<Self, DialogueLineBuildFatal> {
        records.sort_by(|left, right| {
            left.application_span
                .range()
                .start()
                .cmp(&right.application_span.range().start())
                .then_with(|| {
                    left.application_span
                        .range()
                        .end()
                        .cmp(&right.application_span.range().end())
                })
                .then_with(|| left.source_application.cmp(&right.source_application))
        });
        let mut by_source_expr = BTreeMap::new();
        let mut by_semantic_expr = BTreeMap::new();
        for (offset, site) in records.iter_mut().enumerate() {
            let value = u32::try_from(offset)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                    operation: DialogueLineBuildOperation::SourceOrder,
                })?;
            site.source_order = DialogueLineSourceOrder::try_new(value)?;
            if by_source_expr
                .insert(site.source_application, offset)
                .is_some()
                || by_semantic_expr
                    .insert(site.semantic_application, offset)
                    .is_some()
            {
                return Err(DialogueLineBuildFatal::DuplicateSite);
            }
            if site.application_span.source() != module.source() {
                return Err(DialogueLineBuildFatal::SourceIdentityMismatch {
                    expected: module.source().clone(),
                    actual: site.application_span.source().clone(),
                });
            }
        }
        let source_order = (0..records.len()).collect::<Vec<_>>();
        Ok(Self {
            module,
            records: Arc::from(records),
            by_source_expr,
            by_semantic_expr,
            source_order: Arc::from(source_order),
        })
    }

    pub const fn module(&self) -> &HirModuleKey {
        &self.module
    }

    pub fn records(&self) -> &[HirDialogueLineSite] {
        &self.records
    }

    pub fn for_source_expr(&self, expression: ExprId) -> Option<&HirDialogueLineSite> {
        self.by_source_expr
            .get(&expression)
            .map(|offset| &self.records[*offset])
    }

    pub fn for_semantic_expr(&self, expression: ExprId) -> Option<&HirDialogueLineSite> {
        self.by_semantic_expr
            .get(&expression)
            .map(|offset| &self.records[*offset])
    }

    pub fn source_ordered(&self) -> impl ExactSizeIterator<Item = &HirDialogueLineSite> {
        self.source_order
            .iter()
            .map(|offset| &self.records[*offset])
    }
}

/// Internal materialization row used only after the selected-expression seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirDialogueLineCandidate {
    id: DialogueLineId,
    id_origin: DialogueLineIdOrigin,
    text_key: DialogueTextKey,
    text_key_origin: DialogueTextKeyOrigin,
    site: HirDialogueLineSite,
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

    pub(crate) const fn site(&self) -> &HirDialogueLineSite {
        &self.site
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
    #[error("dialogue line source-site inventory contains a duplicate expression")]
    DuplicateSite,
}
