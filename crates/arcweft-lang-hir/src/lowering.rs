//! Bound whole-source input and failure vocabulary for final HIR lowering.

use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::{
    SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotId,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::patterns::PatternOrBindingIssue;
use arcweft_source::{SourceDocumentId, SourceDocumentIdentity};
use thiserror::Error;

use crate::identity::{
    HirDatabaseId, HirIdKind, HirLimit, HirModuleId, HirSnapshotId, IdResolveError, ItemId, ScopeId,
};
use crate::leaf::HirName;
use crate::proof_return::{HirProofReturnAuthorityError, HirProofReturnGenerationError};
use crate::symbol::CallablePackageId;

/// Shared cancellation authority for one final-HIR project transaction.
///
/// The control is revision-independent and carries no syntax or HIR payload.
/// Clones observe the same monotonic cancellation state, so a caller may retain
/// one clone while the accepted transaction owns another through publication.
#[derive(Clone, Debug)]
pub struct HirLoweringControl {
    cancelled: Arc<AtomicBool>,
    #[cfg(test)]
    script: Option<Arc<HirLoweringTestScript>>,
}

impl Default for HirLoweringControl {
    fn default() -> Self {
        Self::new()
    }
}

impl HirLoweringControl {
    /// Creates one live cancellation authority for a fresh transaction.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            script: None,
        }
    }

    /// Monotonically cancels every staging or publication step sharing this
    /// authority.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Reports whether cancellation has been published to this transaction.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn checkpoint(
        &self,
        checkpoint: HirLoweringCheckpoint,
    ) -> Result<(), HirLowerFailure> {
        #[cfg(test)]
        if let Some(script) = self.script.as_ref()
            && script.checkpoint == checkpoint
        {
            let hit = script.hits.fetch_add(1, Ordering::AcqRel);
            if hit == 0 {
                match script.action {
                    HirLoweringTestAction::Cancel => self.cancel(),
                    HirLoweringTestAction::Panic => {
                        panic!("injected final-HIR transaction panic at {checkpoint:?}")
                    }
                }
            }
        }
        #[cfg(not(test))]
        let _ = checkpoint;

        if self.is_cancelled() {
            Err(HirLowerFailure::Cancelled)
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel_at_for_test(checkpoint: HirLoweringCheckpoint) -> Self {
        Self::scripted(checkpoint, HirLoweringTestAction::Cancel)
    }

    #[cfg(test)]
    pub(crate) fn panic_at_for_test(checkpoint: HirLoweringCheckpoint) -> Self {
        Self::scripted(checkpoint, HirLoweringTestAction::Panic)
    }

    #[cfg(test)]
    fn scripted(checkpoint: HirLoweringCheckpoint, action: HirLoweringTestAction) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            script: Some(Arc::new(HirLoweringTestScript {
                checkpoint,
                action,
                hits: AtomicUsize::new(0),
            })),
        }
    }

    #[cfg(test)]
    pub(crate) fn hit_count_for_test(&self) -> usize {
        self.script
            .as_ref()
            .map_or(0, |script| script.hits.load(Ordering::Acquire))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HirLoweringCheckpoint {
    BeforePreflight,
    BeforeModuleStaging,
    BeforeRootScopeReservation,
    ItemReserved,
    FlowScopesReserved,
    ChildReserved,
    FlowSourceStaged,
    SourceFreeze,
    BeforeSemanticResume,
    ModulePrepared,
    BeforeCommit,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HirLoweringTestAction {
    Cancel,
    Panic,
}

#[cfg(test)]
#[derive(Debug)]
struct HirLoweringTestScript {
    checkpoint: HirLoweringCheckpoint,
    action: HirLoweringTestAction,
    hits: AtomicUsize,
}

/// Package-qualified owner of one source-backed HIR module.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
    document: SourceDocumentId,
}

impl HirModuleKey {
    /// Binds a package, canonical module path, and logical source document.
    pub fn new(
        package: CallablePackageId,
        path: CanonicalModulePath,
        document: SourceDocumentId,
    ) -> Self {
        Self {
            package,
            path,
            document,
        }
    }

    /// Package that owns this module.
    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    /// Canonical path of this module within its package.
    pub const fn path(&self) -> &CanonicalModulePath {
        &self.path
    }

    /// Logical source document admitted for this module.
    pub const fn document(&self) -> &SourceDocumentId {
        &self.document
    }
}

/// Exact attached whole-source snapshot admitted to one HIR transaction.
pub struct LoweringRequest<'source> {
    key: HirModuleKey,
    source: &'source ParsedSource,
}

impl<'source> LoweringRequest<'source> {
    /// Validates document and revision identity before any HIR allocation.
    pub fn try_new(
        key: HirModuleKey,
        source: &'source ParsedSource,
    ) -> Result<Self, HirLowerFailure> {
        let actual_document = source.document().identity().id();
        if key.document() != actual_document {
            return Err(HirLowerFailure::SourceDocumentMismatch {
                expected: key.document().clone(),
                actual: actual_document.clone(),
            });
        }

        // `ParsedSource` is the whole-source owner: attached fragments have a
        // distinct type and cannot enter this boundary. Revalidate its retained
        // root against the same immutable snapshot and document identity.
        let root = source.root_syntax();
        if root.snapshot_id() != source.snapshot_id() {
            return Err(HirLowerFailure::StaleSource {
                current: source.snapshot_id().clone(),
                supplied: root.snapshot_id().clone(),
            });
        }

        let expected_identity = source.document().identity();
        let actual_identity = root.source_span().source().clone();
        if &actual_identity != expected_identity {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: expected_identity.clone(),
                actual: actual_identity,
            });
        }

        let observed = source.document().text().len();
        let maximum = HirLimit::SourceDocumentBytes.maximum();
        if observed > maximum {
            return Err(HirLimitError::with_maximum(
                HirLimit::SourceDocumentBytes,
                observed,
                maximum,
            )
            .into());
        }

        Ok(Self { key, source })
    }

    /// Package/module/document owner accepted for this transaction.
    pub const fn key(&self) -> &HirModuleKey {
        &self.key
    }

    /// Bound whole-source syntax snapshot accepted for this transaction.
    pub const fn source(&self) -> &'source ParsedSource {
        self.source
    }
}

/// Fatal lowering failure that publishes no HIR snapshot or allocation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirLowerFailure {
    #[error("final-HIR project transaction was cancelled")]
    Cancelled,
    #[error("parsed source belongs to another syntax database")]
    WrongSyntaxDatabase {
        expected: SyntaxDatabaseId,
        actual: SyntaxDatabaseId,
    },
    #[error("parsed source belongs to another syntax lineage")]
    WrongSyntaxLineage {
        expected: SyntaxLineageId,
        actual: SyntaxLineageId,
    },
    #[error("attached source root belongs to snapshot {supplied:?}, expected {current:?}")]
    StaleSource {
        current: SyntaxSnapshotId,
        supplied: SyntaxSnapshotId,
    },
    #[error("attached source identity {actual:?} does not match retained identity {expected:?}")]
    SourceIdentityMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("parsed source document {actual} does not match HIR module document {expected}")]
    SourceDocumentMismatch {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    #[error(transparent)]
    Limit(#[from] HirLimitError),
    #[error("HIR module identity allocation is exhausted")]
    ModuleIdentityExhausted,
    #[error("project HIR transaction contains no modules")]
    EmptyProjectTransaction,
    #[error("project HIR transaction contains duplicate module `{module}`")]
    DuplicateModuleRequest { module: CanonicalModulePath },
    #[error("retained HIR module belongs to database {actual:?}, expected {expected:?}")]
    RetainedModuleWrongDatabase {
        expected: HirDatabaseId,
        actual: HirDatabaseId,
    },
    #[error("retained HIR module `{module}` is not the current accepted snapshot {snapshot:?}")]
    RetainedModuleNotCurrent {
        module: CanonicalModulePath,
        snapshot: HirSnapshotId,
    },
    #[error("retained HIR module `{module}` is recovered and cannot enter an executable cache")]
    RetainedModuleNotCacheEligible { module: CanonicalModulePath },
    #[error("retained Proof return semantic class changed for item {item:?}")]
    RetainedProofReturnSemanticMismatch { item: ItemId },
    #[error("HIR revision allocation is exhausted for module {module:?}")]
    RevisionExhausted { module: HirModuleId },
    #[error("HIR {kind:?} slot identity is exhausted for module {module:?}")]
    SlotIdentityExhausted {
        module: HirModuleId,
        kind: HirIdKind,
    },
    #[error("HIR cache epoch allocation is exhausted for module {module:?}")]
    CacheEpochExhausted { module: HirModuleId },
    #[error("local generation is exhausted for {name:?} in scope {scope:?}")]
    LocalGenerationExhausted { scope: ScopeId, name: HirName },
    #[error("or-pattern {owner:?} has inconsistent alternative bindings: {issue:?}")]
    OrAlternativeBindingsMismatch {
        owner: SyntaxNodeId,
        issue: PatternOrBindingIssue,
    },
    #[error(
        "local binding {name:?} in scope {scope:?} starts at {attempted_start}, not after the previously lowered binding at {previous_start}"
    )]
    LocalBindingSourceOrderViolation {
        scope: ScopeId,
        name: HirName,
        previous_start: usize,
        attempted_start: usize,
    },
    #[error("authored Proof return for item {item:?} has no staged semantic fact authority")]
    MissingProofReturnSemanticFacts { item: ItemId },
    #[error(transparent)]
    ProofReturnAuthority(#[from] HirProofReturnAuthorityError),
    #[error(transparent)]
    ProofReturnGeneration(#[from] HirProofReturnGenerationError),
    #[error(transparent)]
    IdResolve(#[from] IdResolveError),
    #[error(transparent)]
    Invariant(#[from] HirInvariantFailure),
}

/// Exact hard-limit failure produced before any HIR publication.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("HIR limit {limit:?} allows {maximum} entries, but the transaction observed {observed}")]
pub struct HirLimitError {
    limit: HirLimit,
    observed: usize,
    maximum: usize,
}

impl HirLimitError {
    pub(crate) const fn with_maximum(limit: HirLimit, observed: usize, maximum: usize) -> Self {
        Self {
            limit,
            observed,
            maximum,
        }
    }

    /// Limit family whose deterministic preflight failed.
    pub const fn limit(self) -> HirLimit {
        self.limit
    }

    /// Checked observed amount, or `usize::MAX` after conversion/addition overflow.
    pub const fn observed(self) -> usize {
        self.observed
    }

    /// Inclusive maximum applied by the failing lowering context.
    pub const fn maximum(self) -> usize {
        self.maximum
    }
}

/// Fatal HIR storage invariant detected before publication.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirInvariantFailure {
    #[error("HIR arena and ID kinds disagree")]
    ArenaKindMismatch,
    #[error("a HIR slot has an invalid live interval")]
    InvalidLiveInterval,
    #[error("a HIR scope has an invalid parent")]
    InvalidScopeParent,
    #[error("a HIR local has an invalid generation timeline")]
    InvalidLocalTimeline,
    #[error("a HIR capture has an invalid owner")]
    InvalidCaptureOwner,
    #[error("a HIR slot has an invalid source span")]
    InvalidSourceSpan,
    #[error("a staged HIR module commit does not match current database state")]
    InvalidModuleCommit,
    #[error("a staged HIR module has inconsistent source or syntax provenance")]
    InvalidModuleProvenance,
    #[error("a staged HIR module arena belongs to another snapshot")]
    InvalidModuleArenaSnapshot,
    #[error("a staged HIR module has invalid recoverable diagnostics")]
    InvalidModuleDiagnostics,
    #[error("a staged HIR module has an invalid execution status")]
    InvalidModuleStatus,
    #[error("a staged HIR module has incomplete or foreign declaration members")]
    InvalidDeclarationMemberIndex,
    #[error("a staged HIR module has an invalid source-ordered top-level item inventory")]
    InvalidSourceOrderedItems,
    #[error("a staged typed arena failed final coverage or payload validation")]
    InvalidArenaCommit,
    #[error("a staged slot ledger failed final liveness validation")]
    InvalidSlotCommit,
    #[error("a staged typed source-component index failed final validation")]
    InvalidSourceIndex,
}
