//! Position-aware semantic signature queries over one accepted HIR revision.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use arcweft_lang_hir::model::HirModule;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocument, SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use crate::{
    callable::{
        CallTargetFactError, CallableFamily, CallableLimits, CallableQueryLimitError,
        NonCallableSource, PRODUCTION_CALLABLE_LIMITS, ResolveCallError, ResolverWork,
        SemanticSignatureError, SemanticSignatureHelp,
    },
    checker::module::analyze_registered_project_types_for_signature_cursor,
    registration::RegisteredSemanticWorld,
    types::TypeKind,
};

mod project;

/// Immutable inputs for one native semantic signature query.
pub struct SignatureQuery<'a> {
    world: &'a RegisteredSemanticWorld,
    document: &'a SourceDocument,
    hir: &'a HirModule,
    byte_offset: usize,
    limits: CallableLimits,
    control: SignatureQueryControl<'a>,
}

impl<'a> SignatureQuery<'a> {
    /// Validates one exact accepted document/HIR/world tuple before semantic work.
    #[allow(
        clippy::result_large_err,
        reason = "the public error retains exact stale-world and resolver evidence"
    )]
    pub fn try_new(
        world: &'a RegisteredSemanticWorld,
        document: &'a SourceDocument,
        hir: &'a HirModule,
        byte_offset: usize,
        limits: CallableLimits,
        control: SignatureQueryControl<'a>,
    ) -> Result<Self, SignatureQueryError> {
        if document.text().len() > limits.max_source_bytes() {
            return Err(CallableQueryLimitError::SourceBytes {
                actual: document.text().len(),
                limit: limits.max_source_bytes(),
            }
            .into());
        }
        if byte_offset > document.text().len() {
            return Err(SignaturePositionError::OutOfBounds {
                byte_offset,
                source_len: document.text().len(),
            }
            .into());
        }
        if !document.text().is_char_boundary(byte_offset) {
            return Err(SignaturePositionError::NotUtf8Boundary { byte_offset }.into());
        }
        let Some(hir_identity) = hir.source_identity() else {
            return Err(SignatureSemanticUnavailable::MissingSourceIdentity.into());
        };
        if hir_identity != document.identity() {
            return Err(SignatureSemanticStale::HirDocumentIdentity {
                expected: document.identity().clone(),
                actual: hir_identity.clone(),
            }
            .into());
        }
        let Some(hir_document) = hir.source_document() else {
            return Err(SignatureSemanticUnavailable::MissingSourceDocument.into());
        };
        if hir_document.identity() != document.identity() || hir_document.text() != document.text()
        {
            return Err(SignatureSemanticStale::HirDocumentBytes {
                document: document.identity().clone(),
            }
            .into());
        }
        let Some(world_identity) = world.symbols().source_identity(hir.module_path()) else {
            return Err(SignatureSemanticUnavailable::MissingProjectModule {
                module: hir.module_path().clone(),
            }
            .into());
        };
        if world_identity != document.identity() {
            return Err(SignatureSemanticStale::WorldDocumentIdentity {
                module: hir.module_path().clone(),
                expected: document.identity().clone(),
                actual: world_identity.clone(),
            }
            .into());
        }
        Ok(Self {
            world,
            document,
            hir,
            byte_offset,
            limits,
            control,
        })
    }

    /// Uses the fixed production resource policy.
    #[allow(
        clippy::result_large_err,
        reason = "the public error retains exact stale-world and resolver evidence"
    )]
    pub fn production(
        world: &'a RegisteredSemanticWorld,
        document: &'a SourceDocument,
        hir: &'a HirModule,
        byte_offset: usize,
        control: SignatureQueryControl<'a>,
    ) -> Result<Self, SignatureQueryError> {
        Self::try_new(
            world,
            document,
            hir,
            byte_offset,
            PRODUCTION_CALLABLE_LIMITS,
            control,
        )
    }

    #[allow(
        clippy::result_large_err,
        reason = "the query uses one typed error surface for control and semantic failures"
    )]
    fn check_control(&self) -> Result<(), SignatureQueryError> {
        self.control.check()
    }
}

/// Borrowed cancellation and deadline authorities for one query.
#[derive(Clone, Copy)]
pub struct SignatureQueryControl<'a> {
    cancelled: &'a AtomicBool,
    deadline: Option<Instant>,
}

impl<'a> SignatureQueryControl<'a> {
    pub const fn new(cancelled: &'a AtomicBool, deadline: Option<Instant>) -> Self {
        Self {
            cancelled,
            deadline,
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "the query uses one typed error surface for control and semantic failures"
    )]
    fn check(self) -> Result<(), SignatureQueryError> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(SignatureQueryError::Cancelled);
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(SignatureQueryError::DeadlineExceeded);
        }
        Ok(())
    }
}

/// Runs one native signature query without parsing, lowering, or name fallback.
#[allow(
    clippy::needless_pass_by_value,
    clippy::result_large_err,
    reason = "the validated query is a one-shot request and the error preserves exact typed evidence"
)]
pub fn query_signature(
    request: SignatureQuery<'_>,
) -> Result<SignatureQueryOutcome, SignatureQueryError> {
    request.check_control()?;
    let mut work = ResolverWork::new(request.limits.max_query_work());
    let checked = analyze_registered_project_types_for_signature_cursor(
        request.hir,
        request.world,
        request.document.identity().clone(),
        request.byte_offset,
        request.control.cancelled,
        &mut work,
    )
    .map_err(map_focused_error)?;
    request.check_control()?;

    let site = checked.signature_call_site().map_err(map_focused_error)?;
    let Some(site) = site else {
        return if checked
            .unsupported_signature_surface()
            .map_err(map_focused_error)?
        {
            Ok(SignatureQueryOutcome::NotApplicable(
                SignatureNotApplicable::UnsupportedSurface,
            ))
        } else {
            Ok(SignatureQueryOutcome::NotApplicable(
                SignatureNotApplicable::CursorOutsideArgumentList,
            ))
        };
    };
    let Some(facts) = checked
        .signature_call_target_facts()
        .map_err(map_focused_error)?
    else {
        return Err(SignatureSemanticUnavailable::MissingCallableFacts {
            call: site.call().clone(),
        }
        .into());
    };
    if facts.call_span() != site.call() {
        return Err(SignatureSemanticUnavailable::MissingCallableFacts {
            call: site.call().clone(),
        }
        .into());
    }
    project::project_signature_help(
        request.world,
        request.document,
        request.control,
        site,
        facts,
        &mut work,
        &request.limits,
    )
}

fn map_focused_error(error: CallTargetFactError) -> SignatureQueryError {
    match error {
        CallTargetFactError::AmbiguousCallRange {
            document,
            byte_offset,
        } => SignatureSemanticUnavailable::AmbiguousCallRange {
            document,
            byte_offset,
        }
        .into(),
        CallTargetFactError::Resolve { reason, .. } => match *reason {
            ResolveCallError::Cancelled => SignatureQueryError::Cancelled,
            ResolveCallError::Work(error) => error.into(),
            reason => SignatureQueryError::Resolve(reason),
        },
        CallTargetFactError::Unavailable { reason, .. } => reason.into(),
        CallTargetFactError::FocusedSourceUnavailable { document } => {
            SignatureSemanticUnavailable::SourceOutsideAcceptedProject { document }.into()
        }
        #[cfg(test)]
        CallTargetFactError::FocusedTargetMissing { call }
        | CallTargetFactError::FocusedTargetDuplicate { call } => {
            SignatureSemanticUnavailable::MissingCallableFacts { call }.into()
        }
        CallTargetFactError::FocusedModeRequired => {
            SignatureSemanticUnavailable::FocusedModeMismatch.into()
        }
    }
}

/// Result of one accepted native query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureQueryOutcome {
    Help(SemanticSignatureHelp),
    NotApplicable(SignatureNotApplicable),
}

/// Typed reasons for returning no signature surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureNotApplicable {
    CursorOutsideArgumentList,
    UnsupportedSurface,
    NonCallableCallee {
        source: NonCallableSource,
        ty: TypeKind,
    },
}

/// Recovery retained from the exact parser-owned argument list.
pub use crate::callable::SemanticSignatureRecovery as SignatureRecovery;

/// Native-query implementation status for a production callable family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureFamilySupport {
    NativeFacts,
}

/// Returns the explicit native-query status for every production family.
pub const fn signature_family_support(_family: CallableFamily) -> SignatureFamilySupport {
    SignatureFamilySupport::NativeFacts
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SignatureQueryError {
    #[error(transparent)]
    Stale(#[from] SignatureSemanticStale),
    #[error(transparent)]
    InvalidPosition(#[from] SignaturePositionError),
    #[error(transparent)]
    SemanticUnavailable(#[from] SignatureSemanticUnavailable),
    #[error(transparent)]
    LimitExceeded(#[from] CallableQueryLimitError),
    #[error(transparent)]
    InvalidSignature(#[from] SemanticSignatureError),
    #[error(transparent)]
    Resolve(ResolveCallError),
    #[error("signature query was cancelled")]
    Cancelled,
    #[error("signature query deadline elapsed")]
    DeadlineExceeded,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SignaturePositionError {
    #[error("signature byte offset {byte_offset} exceeds source length {source_len}")]
    OutOfBounds {
        byte_offset: usize,
        source_len: usize,
    },
    #[error("signature byte offset {byte_offset} is not a UTF-8 boundary")]
    NotUtf8Boundary { byte_offset: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SignatureSemanticStale {
    #[error("signature HIR was lowered from another source revision")]
    HirDocumentIdentity {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("signature HIR retained unequal bytes for the same document identity")]
    HirDocumentBytes { document: SourceDocumentIdentity },
    #[error("signature semantic world maps the module to another source revision")]
    WorldDocumentIdentity {
        module: CanonicalModulePath,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SignatureSemanticUnavailable {
    #[error("signature HIR has no revision-bound source identity")]
    MissingSourceIdentity,
    #[error("signature HIR has no retained source document")]
    MissingSourceDocument,
    #[error("signature semantic world has no module {module}")]
    MissingProjectModule { module: CanonicalModulePath },
    #[error("signature cursor selects equally specific typed call ranges")]
    AmbiguousCallRange {
        document: SourceDocumentIdentity,
        byte_offset: usize,
    },
    #[error("signature source is outside the accepted project")]
    SourceOutsideAcceptedProject { document: SourceDocumentIdentity },
    #[error("signature query has no checked facts for {call:?}")]
    MissingCallableFacts { call: SourceSpan },
    #[error("signature query invoked the focused checker in a non-focused mode")]
    FocusedModeMismatch,
}

#[cfg(test)]
mod tests;
