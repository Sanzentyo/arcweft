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
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocument, SourceDocumentIdentity};
use lsp_server::{ErrorCode, RequestId};
use lsp_types::Position;
use thiserror::Error;

use crate::{
    documents::DocumentSnapshot,
    profiles::{
        accepted_project::{AcceptedHirLookupError, AcceptedModuleKey, AcceptedProjectSnapshot},
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
#[allow(
    dead_code,
    reason = "AW-AH-009.3.1 authored-call integration is the explicit Cut 6 gate"
)]
pub(crate) struct AcceptedDocumentHirLease {
    environment: Arc<AcceptedProfileEnvironment>,
    document: Arc<SourceDocument>,
    uri: LspUriKey,
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
    #[allow(
        dead_code,
        reason = "AW-AH-009.3.1 authored-call integration is the explicit Cut 6 gate"
    )]
    lease: AcceptedDocumentHirLease,
    stamp: SignatureRequestStamp,
    active: ActiveRequest,
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

#[allow(
    dead_code,
    clippy::result_large_err,
    reason = "AW-AH-009.3.1 authored-call integration is the explicit Cut 6 gate"
)]
impl AcceptedDocumentHirLease {
    pub(crate) fn new(
        environment: Arc<AcceptedProfileEnvironment>,
        document: Arc<SourceDocument>,
        uri: LspUriKey,
        module: AcceptedModuleKey,
    ) -> Self {
        Self {
            environment,
            document,
            uri,
            module,
        }
    }

    pub(crate) fn document(&self) -> &SourceDocument {
        self.document.as_ref()
    }

    pub(crate) const fn module(&self) -> &CanonicalModulePath {
        self.module.module()
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

    pub(crate) const fn uri(&self) -> &LspUriKey {
        &self.uri
    }

    pub(crate) const fn module_key(&self) -> &AcceptedModuleKey {
        &self.module
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

    #[allow(
        dead_code,
        reason = "AW-AH-009.3.1 authored-call integration is the explicit Cut 6 gate"
    )]
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

impl SignatureAcquireError {
    pub(crate) fn is_not_applicable(&self) -> bool {
        matches!(
            self,
            Self::ProfileNotMapped { .. }
                | Self::NoAcceptedEnvironment
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
            Self::Admission(_) => ErrorCode::RequestFailed as i32,
            Self::SourceDigestCollision { .. }
            | Self::MissingHirModule { .. }
            | Self::HirIdentityMismatch { .. } => ErrorCode::InternalError as i32,
            Self::DocumentNotOpen { .. }
            | Self::ProfileKeyMismatch
            | Self::OverlayNotAccepted { .. }
            | Self::OverlayVersionNotAccepted { .. }
            | Self::DocumentNotAccepted { .. } => ErrorCode::ContentModified as i32,
            Self::ProfileNotMapped { .. }
            | Self::NoAcceptedEnvironment
            | Self::UriNotAccepted { .. }
            | Self::SourceHasNoHirModule { .. } => unreachable!("handled above"),
        })
    }
}

impl SignatureRequestStale {
    pub(crate) const fn lsp_code(&self) -> i32 {
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
}
