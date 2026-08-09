//! Position-aware semantic signature queries over one accepted HIR revision.

#[cfg(test)]
use std::cell::Cell;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use arcweft_lang_hir::{
    identity::ExprId,
    module::HirModule,
    source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQueryError, HirSourceSite},
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocument, SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use crate::{
    callable::{
        CallableFamily, CallableQueryLimitError, PRODUCTION_CALLABLE_LIMITS,
        PRODUCTION_SIGNATURE_LIMITS, ResolveCallError, SemanticSignatureError,
        SemanticSignatureHelp, SignatureLimitExceeded, SignatureQueryLimits,
        SignatureQueryWorkMeter, SignatureWorkKind,
    },
    final_analysis::FinalSemanticAnalysis,
    registration::RegisteredSemanticWorld,
};

mod project;
mod surface;

pub(crate) use surface::FocusedCallSite;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureQueryStep {
    SurfaceTraversal,
}

pub(crate) trait SignatureQueryStepControl {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError>;
}

/// Immutable inputs for one native semantic signature query.
pub struct SignatureQuery<'a> {
    document: &'a SourceDocument,
    hir: &'a HirModule,
    analysis: &'a FinalSemanticAnalysis,
    byte_offset: usize,
    limits: SignatureQueryLimits,
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
        analysis: &'a FinalSemanticAnalysis,
        byte_offset: usize,
        limits: SignatureQueryLimits,
        control: SignatureQueryControl<'a>,
    ) -> Result<Self, SignatureQueryError> {
        let source_bytes = u64::try_from(document.text().len()).map_err(|_| {
            SignatureQueryError::ArithmeticOverflow {
                counter: SignatureWorkKind::SourceBytes,
            }
        })?;
        if source_bytes > limits.source_bytes() {
            return Err(SignatureLimitExceeded {
                kind: crate::callable::SignatureLimitKind::SourceBytes,
                observed: source_bytes,
                maximum: limits.source_bytes(),
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
        let hir_identity = hir.provenance().source_identity();
        if hir_identity != document.identity() {
            return Err(SignatureSemanticStale::HirDocumentIdentity {
                expected: document.identity().clone(),
                actual: hir_identity.clone(),
            }
            .into());
        }
        let module_path = hir.key().path();
        let Some(world_identity) = world.symbols().source_identity(module_path) else {
            return Err(SignatureSemanticUnavailable::MissingProjectModule {
                module: module_path.clone(),
            }
            .into());
        };
        if world_identity != document.identity() {
            return Err(SignatureSemanticStale::WorldDocumentIdentity {
                module: module_path.clone(),
                expected: document.identity().clone(),
                actual: world_identity.clone(),
            }
            .into());
        }
        analysis
            .validate_module_generation(hir, world.symbols())
            .map_err(|_| SignatureSemanticStale::AnalysisGeneration {
                module: module_path.clone(),
            })?;
        Ok(Self {
            document,
            hir,
            analysis,
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
        analysis: &'a FinalSemanticAnalysis,
        byte_offset: usize,
        control: SignatureQueryControl<'a>,
    ) -> Result<Self, SignatureQueryError> {
        Self::try_new(
            world,
            document,
            hir,
            analysis,
            byte_offset,
            PRODUCTION_SIGNATURE_LIMITS,
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
    #[cfg(test)]
    remaining_steps: Option<&'a Cell<usize>>,
    #[cfg(test)]
    deadline_step: Option<SignatureQueryStep>,
    #[cfg(test)]
    cancellation_step: Option<SignatureQueryStep>,
    #[cfg(test)]
    cancellation_step_delay: Option<&'a Cell<usize>>,
}

impl<'a> SignatureQueryControl<'a> {
    pub const fn new(cancelled: &'a AtomicBool, deadline: Option<Instant>) -> Self {
        Self {
            cancelled,
            deadline,
            #[cfg(test)]
            remaining_steps: None,
            #[cfg(test)]
            deadline_step: None,
            #[cfg(test)]
            cancellation_step: None,
            #[cfg(test)]
            cancellation_step_delay: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_remaining_steps(mut self, remaining_steps: &'a Cell<usize>) -> Self {
        self.remaining_steps = Some(remaining_steps);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_deadline_step(mut self, deadline_step: SignatureQueryStep) -> Self {
        self.deadline_step = Some(deadline_step);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_cancellation_step_after(
        mut self,
        cancellation_step: SignatureQueryStep,
        prior_occurrences: &'a Cell<usize>,
    ) -> Self {
        self.cancellation_step = Some(cancellation_step);
        self.cancellation_step_delay = Some(prior_occurrences);
        self
    }

    #[allow(
        clippy::result_large_err,
        reason = "the query uses one typed error surface for control and semantic failures"
    )]
    fn check(self) -> Result<(), SignatureQueryError> {
        #[cfg(test)]
        if let Some(remaining) = self.remaining_steps {
            let current = remaining.get();
            if current == 0 {
                return Err(SignatureQueryError::DeadlineExceeded);
            }
            remaining.set(current - 1);
        }
        if self.cancelled.load(Ordering::Acquire) {
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

impl SignatureQueryStepControl for SignatureQueryControl<'_> {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError> {
        #[cfg(test)]
        if self.deadline_step == Some(step) {
            return Err(ResolveCallError::DeadlineExceeded);
        }
        #[cfg(test)]
        if self.cancellation_step == Some(step) {
            let remaining = self
                .cancellation_step_delay
                .expect("cancellation-step fixture owns its delay");
            if remaining.get() == 0 {
                self.cancelled.store(true, Ordering::Release);
                return Err(ResolveCallError::Cancelled);
            }
            remaining.set(remaining.get() - 1);
        }
        #[cfg(not(test))]
        let _ = step;
        match (*self).check() {
            Ok(()) => Ok(()),
            Err(SignatureQueryError::Cancelled) => Err(ResolveCallError::Cancelled),
            Err(SignatureQueryError::DeadlineExceeded) => Err(ResolveCallError::DeadlineExceeded),
            Err(_) => unreachable!("query control checks only cancellation and deadline"),
        }
    }
}

/// Runs one native signature query without parsing, lowering, or name fallback.
#[allow(
    clippy::result_large_err,
    reason = "the error preserves exact typed evidence"
)]
pub fn query_signature(
    request: SignatureQuery<'_>,
) -> Result<SignatureQueryOutcome, SignatureQueryError> {
    request.execute()
}

impl SignatureQuery<'_> {
    #[allow(
        clippy::result_large_err,
        reason = "the one-shot query retains exact typed error evidence"
    )]
    fn execute(self) -> Result<SignatureQueryOutcome, SignatureQueryError> {
        self.check_control()?;
        let mut signature_work = SignatureQueryWorkMeter::new(self.limits);
        let selection = surface::select_signature_surface(
            self.hir,
            self.document,
            self.byte_offset,
            self.control,
            &mut signature_work,
        )?;
        let Some(site) = selection.site else {
            return if selection.unsupported_surface {
                Ok(SignatureQueryOutcome::NotApplicable(
                    SignatureNotApplicable::UnsupportedSurface,
                ))
            } else {
                Ok(SignatureQueryOutcome::NotApplicable(
                    SignatureNotApplicable::CursorOutsideArgumentList,
                ))
            };
        };
        self.check_control()?;
        let facts = self.analysis.call(site.expression()).ok_or_else(|| {
            SignatureSemanticUnavailable::MissingCallableFacts {
                call: Box::new(site.call().clone()),
            }
        })?;
        let source = self
            .hir
            .source_site(self.document.identity(), facts.source_query())
            .map_err(|error| SignatureSemanticUnavailable::SourceQuery {
                owner: facts.expression(),
                error: Box::new(error),
            })?;
        let HirSourcePresence::Present(HirSourceSite::Span(fact_span)) = source.presence() else {
            return Err(SignatureSemanticUnavailable::MissingSourceComponent {
                owner: facts.expression(),
                role: HirExprSourceRole::Whole,
            }
            .into());
        };
        if fact_span != site.call() {
            return Err(SignatureSemanticUnavailable::MissingCallableFacts {
                call: Box::new(site.call().clone()),
            }
            .into());
        }
        project::project_signature_help(project::SignatureProjection {
            document: self.document,
            control: self.control,
            site: &site,
            facts,
            checked: self.analysis.checked_callables(),
            callable_limits: &PRODUCTION_CALLABLE_LIMITS,
            signature_limits: &self.limits,
            signature_work: &mut signature_work,
        })
    }
}

pub(super) fn map_signature_accounting_error(
    error: crate::callable::SignatureAccountingError,
) -> SignatureQueryError {
    match error {
        crate::callable::SignatureAccountingError::Limit(error) => error.into(),
        crate::callable::SignatureAccountingError::Arithmetic { counter } => {
            SignatureQueryError::ArithmeticOverflow { counter }
        }
    }
}

/// Result of one accepted native query.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the frozen public query contract returns semantic help by value and has no compatibility indirection"
)]
pub enum SignatureQueryOutcome {
    Help(SemanticSignatureHelp),
    NotApplicable(SignatureNotApplicable),
}

/// Typed reasons for returning no signature surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureNotApplicable {
    CursorOutsideArgumentList,
    UnknownCallee,
    UnsupportedSurface,
    NonCallableCallee,
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
    Stale(Box<SignatureSemanticStale>),
    #[error(transparent)]
    InvalidPosition(#[from] SignaturePositionError),
    #[error(transparent)]
    SemanticUnavailable(#[from] SignatureSemanticUnavailable),
    #[error(transparent)]
    CallableLimitExceeded(#[from] CallableQueryLimitError),
    #[error(transparent)]
    LimitExceeded(#[from] SignatureLimitExceeded),
    #[error("signature query counter overflowed")]
    ArithmeticOverflow { counter: SignatureWorkKind },
    #[error(transparent)]
    InvalidSignature(#[from] SemanticSignatureError),
    #[error(transparent)]
    Resolve(ResolveCallError),
    #[error("signature query was cancelled")]
    Cancelled,
    #[error("signature query deadline elapsed")]
    DeadlineExceeded,
}

impl From<SignatureSemanticStale> for SignatureQueryError {
    fn from(value: SignatureSemanticStale) -> Self {
        Self::Stale(Box::new(value))
    }
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
    #[error("signature semantic world maps the module to another source revision")]
    WorldDocumentIdentity {
        module: CanonicalModulePath,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("signature semantic analysis does not own the accepted generation for {module}")]
    AnalysisGeneration { module: CanonicalModulePath },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SignatureSemanticUnavailable {
    #[error("signature semantic world has no module {module}")]
    MissingProjectModule { module: CanonicalModulePath },
    #[error("signature cursor selects equally specific typed call ranges")]
    AmbiguousCallRange {
        document: Box<SourceDocumentIdentity>,
        byte_offset: usize,
    },
    #[error("signature query has no checked facts for {call:?}")]
    MissingCallableFacts { call: Box<SourceSpan> },
    #[error("signature query has no accepted checked authority for {candidate:?}")]
    MissingCallableAuthority {
        candidate: Box<crate::callable::CallableCandidateId>,
    },
    #[error("signature HIR has no required source component {role:?} for {owner:?}")]
    MissingSourceComponent {
        owner: ExprId,
        role: HirExprSourceRole,
    },
    #[error("signature HIR source query failed for {owner:?}")]
    SourceQuery {
        owner: ExprId,
        #[source]
        error: Box<HirSourceQueryError>,
    },
}
