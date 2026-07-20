//! Immutable accepted-document/HIR lease and exact signature-request freshness stamp.

use std::sync::Arc;

use arcweft_lang_hir::{
    model::HirModule,
    project::HirProject,
    symbol::{ProjectSymbolRevision, ProjectSymbolWorldId},
};
use arcweft_lang_sema::registration::{
    CharacterInventoryDigest, CharacterInventoryRevision, RegisteredSemanticWorld,
};
use arcweft_lang_sema::{
    callable::{CallableQueryLimitError, ResolveCallError, SemanticSignatureError},
    signature::{SignatureQueryError, SignatureQueryOutcome},
};
use arcweft_source::{SourceDocument, SourceDocumentIdentity};
use lsp_server::{ErrorCode, RequestId, Response, ResponseError};
use lsp_types::{Position, SignatureHelp};
use thiserror::Error;

use crate::{
    documents::DocumentSnapshot,
    features::signature::SignatureProjectionError,
    positions::CheckedPositionError,
    profiles::{
        accepted_project::{AcceptedHirLookupError, AcceptedModuleKey, AcceptedProjectSnapshot},
        caches::SignatureCacheKey,
        state::{
            AcceptedEnvironmentGeneration, AcceptedProfileEnvironment, AcceptedProfileKey,
            LspProfileState,
        },
    },
    uri_key::LspUriKey,
};

use super::{ActiveRequest, RequestAdmissionError, RequestControl, SignatureCancellationReason};

/// Exact accepted source and canonical HIR module retained for one request.
#[derive(Debug)]
pub(crate) struct AcceptedDocumentHirLease {
    environment: Arc<AcceptedProfileEnvironment>,
    document: Arc<SourceDocument>,
    module: AcceptedModuleKey,
}

/// Immutable evidence used for every pre-cache and publication freshness check.
#[derive(Debug)]
pub(crate) struct SignatureRequestStamp {
    profile_state: Arc<LspProfileState>,
    accepted: Arc<AcceptedProfileEnvironment>,
    project: Arc<AcceptedProjectSnapshot>,
    hir_project: Arc<HirProject>,
    world: Arc<RegisteredSemanticWorld>,
    accepted_document: Arc<SourceDocument>,
    profile: AcceptedProfileKey,
    generation: AcceptedEnvironmentGeneration,
    world_id: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    character_digest: CharacterInventoryDigest,
    character_revision: CharacterInventoryRevision,
    uri: LspUriKey,
    protocol_document: SourceDocumentIdentity,
    accepted_document_identity: SourceDocumentIdentity,
    lsp_version: i32,
    module: AcceptedModuleKey,
}

/// Fully acquired request that owns every source borrowed by later worker execution.
#[derive(Debug)]
pub(crate) struct PreparedSignatureRequest {
    request_id: RequestId,
    position: Position,
    snapshot: DocumentSnapshot,
    lease: AcceptedDocumentHirLease,
    stamp: SignatureRequestStamp,
    active: ActiveRequest,
}

/// Semantic result retained until the final stamp gate decides cache publication.
#[derive(Debug)]
pub(crate) struct SignatureRequestResult {
    key: SignatureCacheKey,
    outcome: Arc<SignatureQueryOutcome>,
    cache: SignatureCacheDisposition,
}

/// Pre-work cache disposition after exact session/profile/gate validation.
#[derive(Debug)]
pub(crate) enum SignatureRequestWork {
    Hit(SignatureRequestResult),
    Miss(SignatureCacheKey),
}

/// Whether this request resolved from the exact accepted-generation cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureCacheDisposition {
    Hit,
    Miss,
}

/// Signature request could not acquire one exact accepted URI/source/module/HIR chain.
#[derive(Debug)]
pub(crate) enum SignatureAcquireError {
    Admission(RequestAdmissionError),
    DocumentNotOpen {
        uri: LspUriKey,
    },
    ProfileNotMapped {
        uri: LspUriKey,
    },
    ProfileClosing,
    NoAcceptedEnvironment,
    ProfileKeyMismatch,
    UriNotAccepted {
        uri: LspUriKey,
    },
    OverlayNotAccepted {
        uri: LspUriKey,
    },
    OverlayVersionNotAccepted {
        uri: LspUriKey,
        expected: i32,
        actual: i32,
    },
    DocumentNotAccepted {
        uri: LspUriKey,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    SourceDigestCollision {
        source: SourceDocumentIdentity,
    },
    SourceHasNoHirModule {
        source: SourceDocumentIdentity,
    },
    MissingHirModule {
        module: AcceptedModuleKey,
    },
    HirIdentityMismatch {
        module: AcceptedModuleKey,
    },
}

impl std::fmt::Display for SignatureAcquireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(error) => std::fmt::Display::fmt(error, formatter),
            Self::DocumentNotOpen { uri } => write!(formatter, "document is not open: {uri}"),
            Self::ProfileNotMapped { uri } => {
                write!(formatter, "document has no Arcweft profile mapping: {uri}")
            }
            Self::ProfileClosing => formatter.write_str("profile request admission is closed"),
            Self::NoAcceptedEnvironment => {
                formatter.write_str("profile has no accepted environment")
            }
            Self::ProfileKeyMismatch => {
                formatter.write_str("accepted profile key differs from the mapped profile")
            }
            Self::UriNotAccepted { uri } => {
                write!(formatter, "URI is not present in accepted sources: {uri}")
            }
            Self::OverlayNotAccepted { uri } => {
                write!(
                    formatter,
                    "open URI is absent from accepted overlays: {uri}"
                )
            }
            Self::OverlayVersionNotAccepted {
                uri,
                expected,
                actual,
            } => write!(
                formatter,
                "open document version for {uri} is {actual}, accepted version is {expected}"
            ),
            Self::DocumentNotAccepted {
                uri,
                expected,
                actual,
            } => write!(
                formatter,
                "open bytes for {uri} are {actual:?}, accepted identity is {expected:?}"
            ),
            Self::SourceDigestCollision { source } => {
                write!(
                    formatter,
                    "equal source identity has unequal text: {source:?}"
                )
            }
            Self::SourceHasNoHirModule { source } => {
                write!(
                    formatter,
                    "accepted source has no canonical HIR module: {source:?}"
                )
            }
            Self::MissingHirModule { module } => {
                write!(formatter, "accepted HIR module is absent: {module:?}")
            }
            Self::HirIdentityMismatch { module } => {
                write!(
                    formatter,
                    "accepted HIR module identity changed: {module:?}"
                )
            }
        }
    }
}

impl std::error::Error for SignatureAcquireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RequestAdmissionError> for SignatureAcquireError {
    fn from(error: RequestAdmissionError) -> Self {
        Self::Admission(error)
    }
}

/// Exact reason a previously acquired request is no longer publishable.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum SignatureRequestStale {
    #[error("signature session admission is closing")]
    SessionClosing,
    #[error("signature profile admission is closing")]
    ProfileClosing,
    #[error("signature document was closed")]
    DocumentClosed { uri: LspUriKey },
    #[error("signature document bytes changed")]
    DocumentChanged {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("signature document version changed")]
    DocumentVersionChanged { expected: i32, actual: i32 },
    #[error("signature URI was remapped to another profile")]
    ProfileRemapped {
        expected: AcceptedProfileKey,
        actual: Option<AcceptedProfileKey>,
    },
    #[error("signature profile state was replaced")]
    ProfileStateReplaced,
    #[error("signature accepted environment was replaced")]
    AcceptedReplaced,
    #[error("signature generation changed")]
    GenerationChanged {
        expected: AcceptedEnvironmentGeneration,
        actual: AcceptedEnvironmentGeneration,
    },
    #[error("signature accepted profile key changed")]
    ProfileKeyChanged {
        expected: AcceptedProfileKey,
        actual: AcceptedProfileKey,
    },
    #[error("signature registered world allocation changed")]
    WorldArcChanged,
    #[error("signature world identity changed")]
    WorldIdentityChanged {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    #[error("signature symbol revision changed")]
    SymbolRevisionChanged {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("signature character digest changed")]
    CharacterDigestChanged {
        expected: CharacterInventoryDigest,
        actual: CharacterInventoryDigest,
    },
    #[error("signature character revision changed")]
    CharacterRevisionChanged {
        expected: CharacterInventoryRevision,
        actual: CharacterInventoryRevision,
    },
    #[error("signature accepted project wrapper changed")]
    ProjectArcChanged,
    #[error("signature URI maps to another accepted document")]
    UriRemapped {
        expected: SourceDocumentIdentity,
        actual: Option<SourceDocumentIdentity>,
    },
    #[error("signature accepted document changed")]
    AcceptedDocumentChanged {
        expected: SourceDocumentIdentity,
        actual: Option<SourceDocumentIdentity>,
    },
    #[error("signature source maps to another module")]
    ModuleChanged {
        expected: AcceptedModuleKey,
        actual: Option<AcceptedModuleKey>,
    },
    #[error("signature HIR project/module instance changed")]
    HirChanged { module: AcceptedModuleKey },
    #[error("signature request was cancelled")]
    Cancelled { reason: SignatureCancellationReason },
    #[error("signature request deadline elapsed")]
    DeadlineExceeded { deadline: std::time::Instant },
}

/// Typed failure from checked position conversion through final publication.
#[derive(Debug, Error)]
pub(crate) enum SignatureRequestError {
    #[error(transparent)]
    Acquire(#[from] SignatureAcquireError),
    #[error(transparent)]
    InvalidLspPosition(#[from] CheckedPositionError),
    #[error(transparent)]
    Query(#[from] SignatureQueryError),
    #[error(transparent)]
    Projection(#[from] SignatureProjectionError),
    #[error(transparent)]
    Stale(#[from] SignatureRequestStale),
}

#[allow(clippy::result_large_err)]
impl AcceptedDocumentHirLease {
    pub(crate) fn new(
        environment: Arc<AcceptedProfileEnvironment>,
        document: Arc<SourceDocument>,
        module: AcceptedModuleKey,
    ) -> Self {
        Self {
            environment,
            document,
            module,
        }
    }

    pub(crate) fn document(&self) -> &SourceDocument {
        self.document.as_ref()
    }

    pub(crate) fn world(&self) -> &RegisteredSemanticWorld {
        self.environment.world().as_ref()
    }

    pub(crate) fn hir(&self) -> Result<&HirModule, SignatureAcquireError> {
        self.environment
            .project()
            .hir(&self.module)
            .map_err(|error| match error {
                AcceptedHirLookupError::MissingModule { key } => {
                    SignatureAcquireError::MissingHirModule { module: key }
                }
                AcceptedHirLookupError::SourceIdentityMismatch { key, .. }
                | AcceptedHirLookupError::MissingSourceDocument { key }
                | AcceptedHirLookupError::SourceDocumentMismatch { key, .. } => {
                    SignatureAcquireError::HirIdentityMismatch { module: key }
                }
            })
    }
}

impl SignatureRequestStamp {
    #[allow(
        clippy::too_many_arguments,
        reason = "the stamp deliberately retains every independent freshness authority"
    )]
    pub(crate) fn new(
        profile_state: Arc<LspProfileState>,
        accepted: Arc<AcceptedProfileEnvironment>,
        accepted_document: Arc<SourceDocument>,
        uri: LspUriKey,
        protocol_document: SourceDocumentIdentity,
        lsp_version: i32,
        module: AcceptedModuleKey,
    ) -> Self {
        let project = Arc::clone(accepted.project());
        let hir_project = Arc::clone(project.hir_project());
        let world = Arc::clone(accepted.world());
        let environment = world.environment();
        Self {
            profile_state,
            profile: accepted.profile().clone(),
            generation: accepted.generation(),
            world_id: world.symbols().world().clone(),
            symbol_revision: *world.symbols().revision(),
            character_digest: environment.character_digest(),
            character_revision: environment.character_revision(),
            accepted_document_identity: accepted_document.identity().clone(),
            accepted,
            project,
            hir_project,
            world,
            accepted_document,
            uri,
            protocol_document,
            lsp_version,
            module,
        }
    }

    pub(crate) const fn profile_state(&self) -> &Arc<LspProfileState> {
        &self.profile_state
    }

    pub(crate) const fn accepted(&self) -> &Arc<AcceptedProfileEnvironment> {
        &self.accepted
    }

    pub(crate) const fn project(&self) -> &Arc<AcceptedProjectSnapshot> {
        &self.project
    }

    pub(crate) const fn hir_project(&self) -> &Arc<HirProject> {
        &self.hir_project
    }

    pub(crate) const fn world(&self) -> &Arc<RegisteredSemanticWorld> {
        &self.world
    }

    pub(crate) const fn accepted_document(&self) -> &Arc<SourceDocument> {
        &self.accepted_document
    }

    pub(crate) const fn profile(&self) -> &AcceptedProfileKey {
        &self.profile
    }

    pub(crate) const fn generation(&self) -> AcceptedEnvironmentGeneration {
        self.generation
    }

    pub(crate) const fn world_id(&self) -> &ProjectSymbolWorldId {
        &self.world_id
    }

    pub(crate) const fn symbol_revision(&self) -> ProjectSymbolRevision {
        self.symbol_revision
    }

    pub(crate) const fn character_digest(&self) -> CharacterInventoryDigest {
        self.character_digest
    }

    pub(crate) const fn character_revision(&self) -> CharacterInventoryRevision {
        self.character_revision
    }

    pub(crate) const fn uri(&self) -> &LspUriKey {
        &self.uri
    }

    pub(crate) const fn protocol_document(&self) -> &SourceDocumentIdentity {
        &self.protocol_document
    }

    pub(crate) const fn accepted_document_identity(&self) -> &SourceDocumentIdentity {
        &self.accepted_document_identity
    }

    pub(crate) const fn lsp_version(&self) -> i32 {
        self.lsp_version
    }

    pub(crate) const fn module(&self) -> &AcceptedModuleKey {
        &self.module
    }

    /// Projects the one existing request stamp into its semantic cache identity.
    pub(crate) fn cache_key(&self, byte_offset: usize) -> SignatureCacheKey {
        SignatureCacheKey::new(
            self.generation,
            self.world_id.clone(),
            self.symbol_revision,
            self.character_revision,
            self.character_digest,
            self.accepted_document_identity.clone(),
            Some(self.lsp_version),
            byte_offset,
        )
    }
}

impl PreparedSignatureRequest {
    #[allow(
        clippy::too_many_arguments,
        reason = "the prepared carrier owns the complete admitted request evidence"
    )]
    pub(crate) fn new(
        request_id: RequestId,
        position: Position,
        snapshot: DocumentSnapshot,
        lease: AcceptedDocumentHirLease,
        stamp: SignatureRequestStamp,
        active: ActiveRequest,
    ) -> Self {
        Self {
            request_id,
            position,
            snapshot,
            lease,
            stamp,
            active,
        }
    }

    pub(crate) const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    pub(crate) const fn position(&self) -> Position {
        self.position
    }

    pub(crate) const fn snapshot(&self) -> &DocumentSnapshot {
        &self.snapshot
    }

    pub(crate) const fn lease(&self) -> &AcceptedDocumentHirLease {
        &self.lease
    }

    pub(crate) const fn stamp(&self) -> &SignatureRequestStamp {
        &self.stamp
    }

    pub(crate) fn control(&self) -> &RequestControl {
        self.active.control().as_ref()
    }

    pub(crate) fn control_arc(&self) -> Arc<RequestControl> {
        Arc::clone(self.active.control())
    }
}

impl SignatureRequestResult {
    pub(crate) fn new(
        key: SignatureCacheKey,
        outcome: Arc<SignatureQueryOutcome>,
        cache: SignatureCacheDisposition,
    ) -> Self {
        Self {
            key,
            outcome,
            cache,
        }
    }

    pub(crate) const fn outcome(&self) -> &Arc<SignatureQueryOutcome> {
        &self.outcome
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        SignatureCacheKey,
        Arc<SignatureQueryOutcome>,
        SignatureCacheDisposition,
    ) {
        (self.key, self.outcome, self.cache)
    }
}

impl SignatureAcquireError {
    pub(crate) fn is_not_applicable(&self) -> bool {
        matches!(
            self,
            Self::ProfileNotMapped { .. }
                | Self::UriNotAccepted { .. }
                | Self::SourceHasNoHirModule { .. }
        )
    }

    pub(crate) fn lsp_code(&self) -> Option<i32> {
        if self.is_not_applicable() {
            return None;
        }
        Some(match self {
            Self::Admission(
                RequestAdmissionError::AdmissionClosed
                | RequestAdmissionError::ProfileClosing
                | RequestAdmissionError::QueueClosed,
            )
            | Self::ProfileClosing => ErrorCode::ServerCancelled as i32,
            Self::Admission(_) | Self::NoAcceptedEnvironment => ErrorCode::RequestFailed as i32,
            Self::SourceDigestCollision { .. }
            | Self::MissingHirModule { .. }
            | Self::HirIdentityMismatch { .. } => ErrorCode::InternalError as i32,
            Self::DocumentNotOpen { .. }
            | Self::ProfileKeyMismatch
            | Self::OverlayNotAccepted { .. }
            | Self::OverlayVersionNotAccepted { .. }
            | Self::DocumentNotAccepted { .. } => ErrorCode::ContentModified as i32,
            Self::ProfileNotMapped { .. }
            | Self::UriNotAccepted { .. }
            | Self::SourceHasNoHirModule { .. } => unreachable!("handled above"),
        })
    }

    pub(crate) const fn stable_code(&self) -> &'static str {
        match self {
            Self::Admission(_) => "aw.signature.acquire.admission",
            Self::DocumentNotOpen { .. } => "aw.signature.acquire.document_not_open",
            Self::ProfileNotMapped { .. } => "aw.signature.acquire.profile_not_mapped",
            Self::ProfileClosing => "aw.signature.acquire.profile_closing",
            Self::NoAcceptedEnvironment => "aw.signature.acquire.no_accepted_environment",
            Self::ProfileKeyMismatch => "aw.signature.acquire.profile_key_mismatch",
            Self::UriNotAccepted { .. } => "aw.signature.acquire.uri_not_accepted",
            Self::OverlayNotAccepted { .. } => "aw.signature.acquire.overlay_not_accepted",
            Self::OverlayVersionNotAccepted { .. } => {
                "aw.signature.acquire.overlay_version_not_accepted"
            }
            Self::DocumentNotAccepted { .. } => "aw.signature.acquire.document_not_accepted",
            Self::SourceDigestCollision { .. } => "aw.signature.acquire.source_digest_collision",
            Self::SourceHasNoHirModule { .. } => "aw.signature.acquire.source_has_no_hir_module",
            Self::MissingHirModule { .. } => "aw.signature.acquire.missing_hir_module",
            Self::HirIdentityMismatch { .. } => "aw.signature.acquire.hir_identity_mismatch",
        }
    }

    pub(crate) fn into_response(self, id: RequestId) -> Response {
        match self.lsp_code() {
            None => Response::new_ok(id, Option::<SignatureHelp>::None),
            Some(code) => error_response(id, code, self.to_string(), self.stable_code()),
        }
    }
}

impl SignatureRequestStale {
    pub(crate) fn lsp_code(&self) -> i32 {
        match self {
            Self::Cancelled {
                reason: SignatureCancellationReason::ClientCancelled,
            } => ErrorCode::RequestCanceled as i32,
            Self::SessionClosing
            | Self::ProfileClosing
            | Self::DeadlineExceeded { .. }
            | Self::Cancelled {
                reason:
                    SignatureCancellationReason::DeadlineExceeded
                    | SignatureCancellationReason::ProfileClosing
                    | SignatureCancellationReason::WorkspaceRemoved
                    | SignatureCancellationReason::SessionShutdown,
            } => ErrorCode::ServerCancelled as i32,
            _ => ErrorCode::ContentModified as i32,
        }
    }

    pub(crate) const fn stable_code(&self) -> &'static str {
        match self {
            Self::SessionClosing => "aw.signature.stale.session_closing",
            Self::ProfileClosing => "aw.signature.stale.profile_closing",
            Self::DocumentClosed { .. } => "aw.signature.stale.document_closed",
            Self::DocumentChanged { .. } => "aw.signature.stale.document_changed",
            Self::DocumentVersionChanged { .. } => "aw.signature.stale.document_version_changed",
            Self::ProfileRemapped { .. } => "aw.signature.stale.profile_remapped",
            Self::ProfileStateReplaced => "aw.signature.stale.profile_state_replaced",
            Self::AcceptedReplaced => "aw.signature.stale.accepted_replaced",
            Self::GenerationChanged { .. } => "aw.signature.stale.generation_changed",
            Self::ProfileKeyChanged { .. } => "aw.signature.stale.profile_key_changed",
            Self::WorldArcChanged => "aw.signature.stale.world_arc_changed",
            Self::WorldIdentityChanged { .. } => "aw.signature.stale.world_identity_changed",
            Self::SymbolRevisionChanged { .. } => "aw.signature.stale.symbol_revision_changed",
            Self::CharacterDigestChanged { .. } => "aw.signature.stale.character_digest_changed",
            Self::CharacterRevisionChanged { .. } => {
                "aw.signature.stale.character_revision_changed"
            }
            Self::ProjectArcChanged => "aw.signature.stale.project_arc_changed",
            Self::UriRemapped { .. } => "aw.signature.stale.uri_remapped",
            Self::AcceptedDocumentChanged { .. } => "aw.signature.stale.accepted_document_changed",
            Self::ModuleChanged { .. } => "aw.signature.stale.module_changed",
            Self::HirChanged { .. } => "aw.signature.stale.hir_changed",
            Self::Cancelled { .. } => "aw.signature.stale.cancelled",
            Self::DeadlineExceeded { .. } => "aw.signature.stale.deadline_exceeded",
        }
    }
}

impl SignatureRequestError {
    pub(crate) fn lsp_code(&self) -> i32 {
        match self {
            Self::Acquire(error) => match error.lsp_code() {
                Some(code) => code,
                None => ErrorCode::RequestFailed as i32,
            },
            Self::InvalidLspPosition(_) => ErrorCode::InvalidParams as i32,
            Self::Stale(error) => error.lsp_code(),
            Self::Projection(_) => ErrorCode::RequestFailed as i32,
            Self::Query(error) => query_lsp_code(error),
        }
    }

    pub(crate) const fn stable_code(&self) -> &'static str {
        match self {
            Self::Acquire(error) => error.stable_code(),
            Self::InvalidLspPosition(_) => "aw.signature.request.invalid_lsp_position",
            Self::Stale(error) => error.stable_code(),
            Self::Projection(SignatureProjectionError::LabelOffsetOverflow) => {
                "aw.signature.projection.label_offset_overflow"
            }
            Self::Projection(SignatureProjectionError::ActiveSignatureOverflow) => {
                "aw.signature.projection.active_signature_overflow"
            }
            Self::Projection(SignatureProjectionError::ActiveParameterOverflow) => {
                "aw.signature.projection.active_parameter_overflow"
            }
            Self::Projection(SignatureProjectionError::ActiveParameterMissing) => {
                "aw.signature.projection.active_parameter_missing"
            }
            Self::Query(error) => query_stable_code(error),
        }
    }

    pub(crate) fn into_response(self, id: RequestId) -> Response {
        error_response(id, self.lsp_code(), self.to_string(), self.stable_code())
    }
}

const fn query_lsp_code(error: &SignatureQueryError) -> i32 {
    match error {
        SignatureQueryError::Stale(_) => ErrorCode::ContentModified as i32,
        SignatureQueryError::InvalidPosition(_) => ErrorCode::InvalidParams as i32,
        SignatureQueryError::LimitExceeded(CallableQueryLimitError::ArithmeticOverflow)
        | SignatureQueryError::InvalidSignature(SemanticSignatureError::Limit(
            CallableQueryLimitError::ArithmeticOverflow,
        ))
        | SignatureQueryError::Resolve(ResolveCallError::Work(
            CallableQueryLimitError::ArithmeticOverflow,
        )) => ErrorCode::RequestFailed as i32,
        SignatureQueryError::LimitExceeded(_)
        | SignatureQueryError::InvalidSignature(SemanticSignatureError::Limit(_))
        | SignatureQueryError::Resolve(ResolveCallError::Work(_))
        | SignatureQueryError::DeadlineExceeded => ErrorCode::ServerCancelled as i32,
        SignatureQueryError::Cancelled
        | SignatureQueryError::Resolve(ResolveCallError::Cancelled) => {
            ErrorCode::RequestCanceled as i32
        }
        SignatureQueryError::SemanticUnavailable(_)
        | SignatureQueryError::InvalidSignature(_)
        | SignatureQueryError::Resolve(_) => ErrorCode::RequestFailed as i32,
    }
}

const fn query_stable_code(error: &SignatureQueryError) -> &'static str {
    match error {
        SignatureQueryError::Stale(_) => "aw.signature.query.stale",
        SignatureQueryError::InvalidPosition(_) => "aw.signature.query.invalid_position",
        SignatureQueryError::SemanticUnavailable(_) => "aw.signature.query.semantic_unavailable",
        SignatureQueryError::LimitExceeded(CallableQueryLimitError::ArithmeticOverflow)
        | SignatureQueryError::InvalidSignature(SemanticSignatureError::Limit(
            CallableQueryLimitError::ArithmeticOverflow,
        ))
        | SignatureQueryError::Resolve(ResolveCallError::Work(
            CallableQueryLimitError::ArithmeticOverflow,
        )) => "aw.signature.query.arithmetic_overflow",
        SignatureQueryError::LimitExceeded(_)
        | SignatureQueryError::InvalidSignature(SemanticSignatureError::Limit(_))
        | SignatureQueryError::Resolve(ResolveCallError::Work(_)) => {
            "aw.signature.query.limit_exceeded"
        }
        SignatureQueryError::InvalidSignature(_) => "aw.signature.query.invalid_signature",
        SignatureQueryError::Resolve(_) => "aw.signature.query.resolve",
        SignatureQueryError::Cancelled => "aw.signature.query.cancelled",
        SignatureQueryError::DeadlineExceeded => "aw.signature.query.deadline_exceeded",
    }
}

fn error_response(
    id: RequestId,
    code: i32,
    message: String,
    stable_code: &'static str,
) -> Response {
    Response {
        id,
        result: None,
        error: Some(ResponseError {
            code,
            message,
            data: Some(serde_json::json!({ "code": stable_code })),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_accepted_world_is_a_typed_request_failure() {
        let response =
            SignatureAcquireError::NoAcceptedEnvironment.into_response(RequestId::from(7));
        let error = response.error.expect("request error");

        assert_eq!(error.code, ErrorCode::RequestFailed as i32);
        assert_eq!(
            error.data,
            Some(serde_json::json!({
                "code": "aw.signature.acquire.no_accepted_environment"
            }))
        );
    }

    #[test]
    fn stale_failures_preserve_content_and_cancellation_semantics() {
        for (request_error, expected_lsp_code, expected_stable_code) in [
            (
                SignatureRequestError::from(SignatureRequestStale::DocumentVersionChanged {
                    expected: 1,
                    actual: 2,
                }),
                ErrorCode::ContentModified as i32,
                "aw.signature.stale.document_version_changed",
            ),
            (
                SignatureRequestError::from(SignatureRequestStale::Cancelled {
                    reason: SignatureCancellationReason::ClientCancelled,
                }),
                ErrorCode::RequestCanceled as i32,
                "aw.signature.stale.cancelled",
            ),
            (
                SignatureRequestError::from(SignatureRequestStale::DeadlineExceeded {
                    deadline: std::time::Instant::now(),
                }),
                ErrorCode::ServerCancelled as i32,
                "aw.signature.stale.deadline_exceeded",
            ),
        ] {
            let response = request_error.into_response(RequestId::from(8));
            let error = response.error.expect("typed stale request error");
            assert_eq!(error.code, expected_lsp_code);
            assert_eq!(
                error.data,
                Some(serde_json::json!({ "code": expected_stable_code }))
            );
        }
    }

    #[test]
    fn query_resource_and_arithmetic_failures_remain_distinct() {
        for (query_error, expected_lsp_code, expected_stable_code) in [
            (
                SignatureQueryError::Cancelled,
                ErrorCode::RequestCanceled as i32,
                "aw.signature.query.cancelled",
            ),
            (
                SignatureQueryError::DeadlineExceeded,
                ErrorCode::ServerCancelled as i32,
                "aw.signature.query.deadline_exceeded",
            ),
            (
                SignatureQueryError::LimitExceeded(CallableQueryLimitError::ArithmeticOverflow),
                ErrorCode::RequestFailed as i32,
                "aw.signature.query.arithmetic_overflow",
            ),
        ] {
            let response =
                SignatureRequestError::from(query_error).into_response(RequestId::from(9));
            let error = response.error.expect("typed query request error");
            assert_eq!(error.code, expected_lsp_code);
            assert_eq!(
                error.data,
                Some(serde_json::json!({ "code": expected_stable_code }))
            );
        }
    }
}
