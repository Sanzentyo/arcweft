//! Session-local typed identity vocabulary for runtime assertions.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::effect::{
    RuntimeArtifactFingerprint, RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionGuardId,
    RuntimeAssertionProfile,
};
use arcweft_lang_hir::{
    identity::StmtId,
    symbol::{CallableDeclarationId, CallablePackageId},
};
use arcweft_lang_syntax::{assertion::AssertionMode, ast::module_path::CanonicalModulePath};
use arcweft_source::SourceSpan;
use thiserror::Error;

/// Derives the artifact-stable guard for one typed runtime assertion condition.
///
/// The identity uses canonical declaration identities and authored ordinals;
/// source text, condition labels, and runtime messages never participate.
pub fn derive_runtime_assertion_guard(
    package: &CallablePackageId,
    module: &CanonicalModulePath,
    callable: &CallableDeclarationId,
    assertion_ordinal: u32,
    condition: AssertionConditionIndex,
    profile: RuntimeAssertionProfile,
) -> RuntimeAssertionGuardId {
    crate::assertion_lower::derive_runtime_assertion_guard(
        package,
        module,
        callable,
        assertion_ordinal,
        condition,
        profile,
    )
}

/// Runtime-capable assertion mode retained by a fresh compilation session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAssertionMode {
    /// Release-visible runtime assertion.
    Check,
    /// Debug-profile-only runtime assertion.
    Debug,
}

impl RuntimeAssertionMode {
    /// Stable presentation spelling shared by runtime diagnostic adapters.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Debug => "debug",
        }
    }

    /// Converts the typed source/HIR mode without inventing a runtime form for
    /// proof-only assertions.
    pub fn try_from_assertion_mode(mode: AssertionMode) -> Result<Self, RuntimeAssertionModeError> {
        match mode {
            AssertionMode::Check => Ok(Self::Check),
            AssertionMode::Debug => Ok(Self::Debug),
            AssertionMode::Prove => Err(RuntimeAssertionModeError::ProveHasNoRuntimeRepresentation),
        }
    }
}

/// Failure to convert a source assertion mode into runtime identity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeAssertionModeError {
    /// `assert.prove` is verification-only and never owns a runtime guard.
    #[error("proof assertions have no runtime representation")]
    ProveHasNoRuntimeRepresentation,
}

/// Zero-based authored position of one condition in an assertion statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssertionConditionIndex(u8);

impl AssertionConditionIndex {
    /// Constructs an index after validating both the authored list size and
    /// the selected position against the language's 64-condition limit.
    pub fn try_new(
        index: usize,
        condition_count: usize,
    ) -> Result<Self, AssertionConditionIndexError> {
        if !(1..=64).contains(&condition_count) {
            return Err(AssertionConditionIndexError::InvalidConditionCount {
                count: condition_count,
            });
        }
        if index >= condition_count {
            return Err(AssertionConditionIndexError::OutOfBounds {
                index,
                count: condition_count,
            });
        }
        let narrowed =
            u8::try_from(index).map_err(|_| AssertionConditionIndexError::OutOfBounds {
                index,
                count: condition_count,
            })?;
        Ok(Self(narrowed))
    }

    /// Returns the zero-based authored condition position.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Invalid assertion condition position or authored list size.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AssertionConditionIndexError {
    /// Runtime assertion statements must carry between one and 64 conditions.
    #[error("assertion condition count must be in 1..=64")]
    InvalidConditionCount { count: usize },
    /// The requested zero-based position is not present in the authored list.
    #[error("assertion condition index is outside the authored condition list")]
    OutOfBounds { index: usize, count: usize },
}

/// Source presentation retained separately from executable assertion identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssertionPresentation {
    statement_span: SourceSpan,
    condition_label: Arc<str>,
}

impl AssertionPresentation {
    pub(crate) fn new(statement_span: SourceSpan, condition_label: Arc<str>) -> Self {
        Self {
            statement_span,
            condition_label,
        }
    }

    pub const fn statement_span(&self) -> &SourceSpan {
        &self.statement_span
    }

    pub fn condition_label(&self) -> &str {
        &self.condition_label
    }
}

/// Exact fresh-session owner for one executable assertion condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionSite {
    guard: RuntimeAssertionGuardId,
    statement: StmtId,
    condition: AssertionConditionIndex,
    mode: RuntimeAssertionMode,
    condition_span: SourceSpan,
    presentation: AssertionPresentation,
}

impl RuntimeAssertionSite {
    pub(crate) fn new(
        guard: RuntimeAssertionGuardId,
        statement: StmtId,
        condition: AssertionConditionIndex,
        mode: RuntimeAssertionMode,
        condition_span: SourceSpan,
        presentation: AssertionPresentation,
    ) -> Self {
        Self {
            guard,
            statement,
            condition,
            mode,
            condition_span,
            presentation,
        }
    }

    pub const fn guard(&self) -> RuntimeAssertionGuardId {
        self.guard
    }

    pub const fn statement(&self) -> StmtId {
        self.statement
    }

    pub const fn condition(&self) -> AssertionConditionIndex {
        self.condition
    }

    pub const fn mode(&self) -> RuntimeAssertionMode {
        self.mode
    }

    pub const fn condition_span(&self) -> &SourceSpan {
        &self.condition_span
    }

    pub const fn presentation(&self) -> &AssertionPresentation {
        &self.presentation
    }
}

/// Fresh-session assertion inventory bound to one exact persisted artifact.
#[derive(Clone)]
pub struct RuntimeAssertionInventory {
    artifact: RuntimeArtifactFingerprint,
    sites: BTreeMap<RuntimeAssertionGuardId, RuntimeAssertionSite>,
}

impl RuntimeAssertionInventory {
    pub(crate) fn try_new(
        artifact: RuntimeArtifactFingerprint,
        sites: impl IntoIterator<Item = RuntimeAssertionSite>,
    ) -> Result<Self, RuntimeAssertionInventoryError> {
        let mut indexed = BTreeMap::new();
        for site in sites {
            let guard = site.guard();
            if indexed.insert(guard, site).is_some() {
                return Err(RuntimeAssertionInventoryError::DuplicateGuard { guard });
            }
        }
        Ok(Self {
            artifact,
            sites: indexed,
        })
    }

    pub const fn artifact(&self) -> RuntimeArtifactFingerprint {
        self.artifact
    }

    pub fn site(&self, guard: RuntimeAssertionGuardId) -> Option<&RuntimeAssertionSite> {
        self.sites.get(&guard)
    }

    pub fn project_failure(
        &self,
        artifact: RuntimeArtifactFingerprint,
        failure: RuntimeAssertionFailure,
    ) -> Result<RuntimeAssertionFault, RuntimeAssertionProjectionError> {
        crate::assertion_projection::project_failure(self, artifact, failure)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeAssertionInventoryError {
    #[error("runtime assertion inventory contains duplicate guard {guard:?}")]
    DuplicateGuard { guard: RuntimeAssertionGuardId },
}

/// Exact HIR identity projected from a fresh-session assertion inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionFaultIdentity {
    statement: StmtId,
    condition: AssertionConditionIndex,
    mode: RuntimeAssertionMode,
    span: SourceSpan,
}

impl RuntimeAssertionFaultIdentity {
    pub(crate) fn new(
        statement: StmtId,
        condition: AssertionConditionIndex,
        mode: RuntimeAssertionMode,
        span: SourceSpan,
    ) -> Self {
        Self {
            statement,
            condition,
            mode,
            span,
        }
    }

    pub const fn statement(&self) -> StmtId {
        self.statement
    }

    pub const fn condition(&self) -> AssertionConditionIndex {
        self.condition
    }

    pub const fn mode(&self) -> RuntimeAssertionMode {
        self.mode
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

/// Runtime assertion failure associated with exact fresh-session HIR identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionFault {
    identity: RuntimeAssertionFaultIdentity,
    guard: RuntimeAssertionGuardId,
    presentation: AssertionPresentation,
    observed: RuntimeAssertion,
}

impl RuntimeAssertionFault {
    pub(crate) fn new(
        identity: RuntimeAssertionFaultIdentity,
        guard: RuntimeAssertionGuardId,
        presentation: AssertionPresentation,
        observed: RuntimeAssertion,
    ) -> Self {
        Self {
            identity,
            guard,
            presentation,
            observed,
        }
    }

    pub const fn identity(&self) -> &RuntimeAssertionFaultIdentity {
        &self.identity
    }

    pub const fn guard(&self) -> RuntimeAssertionGuardId {
        self.guard
    }

    pub const fn presentation(&self) -> &AssertionPresentation {
        &self.presentation
    }

    pub const fn observed(&self) -> &RuntimeAssertion {
        &self.observed
    }
}

/// Typed rejection when persisted failure data cannot join a fresh inventory.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeAssertionProjectionError {
    #[error("runtime assertion artifact does not match the fresh inventory")]
    ArtifactMismatch {
        expected: RuntimeArtifactFingerprint,
        actual: RuntimeArtifactFingerprint,
    },
    #[error("runtime assertion guard is absent from the fresh inventory")]
    UnknownGuard { guard: RuntimeAssertionGuardId },
    #[error("runtime assertion profile does not match the fresh assertion mode")]
    ProfileModeMismatch {
        guard: RuntimeAssertionGuardId,
        profile: RuntimeAssertionProfile,
        mode: RuntimeAssertionMode,
    },
    #[error(transparent)]
    InvalidConditionIndex(#[from] AssertionConditionIndexError),
}

#[cfg(test)]
mod tests {
    use super::{
        AssertionConditionIndex, AssertionPresentation, RuntimeAssertionInventory,
        RuntimeAssertionInventoryError, RuntimeAssertionMode, RuntimeAssertionModeError,
        RuntimeAssertionSite,
    };
    use arcweft_core::effect::{RuntimeArtifactFingerprint, RuntimeAssertionGuardId};
    use arcweft_lang_hir::{
        database::HirDatabase,
        expr::HirThreadFlowItem,
        identity::StmtId,
        item::HirItemKind,
        lowering::{HirModuleKey, LoweringRequest},
        proof_return::HirProofReturnSemanticFactSet,
        symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId},
    };
    use arcweft_lang_syntax::{
        assertion::AssertionMode, ast::module_path::CanonicalModulePath,
        incremental::SyntaxDatabase, parser::ParseOptions,
    };
    use arcweft_source::{
        SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
    };
    use std::sync::Arc;

    fn statement() -> arcweft_lang_hir::identity::StmtId {
        let package = CallablePackageId::try_new("runtime-assertion-inventory-test")
            .expect("fixture package");
        let path = CanonicalModulePath::crate_root();
        let source_name = SourceName::path("runtime-assertion-inventory-test.arcw");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://runtime-assertion-inventory")
                    .expect("fixture document ID"),
                source_name.clone(),
                "flow checks { assert.check(true) }\n",
            )
            .expect("fixture document"),
        );
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(source_name),
                document,
                ParseOptions::default(),
            )
            .expect("attached fixture parse");
        let key = HirModuleKey::new(package.clone(), path, parsed.document().identity().clone());
        let mut database = HirDatabase::try_new().expect("HIR database");
        let world = ProjectSymbolWorldId::try_new(
            package,
            parsed.document().identity().id().clone(),
            "runtime-assertion-inventory-test",
        )
        .expect("fixture symbol world");
        let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
            .expect("fixture symbol revision");
        let transaction = database
            .stage_proof_return_project(
                [LoweringRequest::try_new(key, &parsed).expect("lower request")],
                world,
                revision,
                [parsed.document().identity()],
                arcweft_lang_hir::lowering::HirLoweringControl::new(),
            )
            .expect("final HIR project stages");
        let facts = HirProofReturnSemanticFactSet::try_new(
            Arc::clone(transaction.generation()),
            transaction.headers().cloned(),
            [],
        )
        .expect("assertion fixture has no authored Proof return headers");
        let mut outputs = transaction
            .publish_with_semantic_facts(&mut database, facts)
            .expect("final HIR project publishes");
        let module = outputs
            .pop()
            .expect("one assertion fixture module")
            .into_module();
        assert!(outputs.is_empty());
        let flow = module
            .source_ordered_items()
            .iter()
            .find_map(|owner| match module.resolve_item(*owner).ok()?.kind() {
                HirItemKind::Flow(flow) => Some(flow),
                _ => None,
            })
            .expect("Flow item");
        match flow.body().items().first().expect("assertion statement") {
            HirThreadFlowItem::Statement(owner)
            | HirThreadFlowItem::Choice(owner)
            | HirThreadFlowItem::If(owner)
            | HirThreadFlowItem::IfLet(owner)
            | HirThreadFlowItem::Match(owner)
            | HirThreadFlowItem::Loop(owner)
            | HirThreadFlowItem::While(owner)
            | HirThreadFlowItem::WhileLet(owner)
            | HirThreadFlowItem::For(owner)
            | HirThreadFlowItem::Select(owner)
            | HirThreadFlowItem::SourceLocale(owner)
            | HirThreadFlowItem::Scope(owner)
            | HirThreadFlowItem::Include(owner)
            | HirThreadFlowItem::AwaitWith(owner)
            | HirThreadFlowItem::Error(owner) => *owner,
            HirThreadFlowItem::DialogueApplication(_) => panic!("assertion lowered as dialogue"),
        }
    }

    fn site(guard_byte: u8, mode: RuntimeAssertionMode, statement: StmtId) -> RuntimeAssertionSite {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("assertion-test").expect("document ID"),
            SourceName::Memory,
            "condition",
        )
        .expect("source document");
        let span = document
            .span(SourceRange::new(0, document.text().len()))
            .expect("condition span");
        RuntimeAssertionSite::new(
            RuntimeAssertionGuardId::try_from_bytes([guard_byte; 16]).expect("nonzero guard"),
            statement,
            AssertionConditionIndex::try_new(0, 1).expect("condition index"),
            mode,
            span.clone(),
            AssertionPresentation::new(span, Arc::from("condition")),
        )
    }

    #[test]
    fn only_runtime_capable_typed_modes_convert() {
        assert_eq!(
            RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Check),
            Ok(RuntimeAssertionMode::Check)
        );
        assert_eq!(
            RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Debug),
            Ok(RuntimeAssertionMode::Debug)
        );
        assert_eq!(
            RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Prove),
            Err(RuntimeAssertionModeError::ProveHasNoRuntimeRepresentation)
        );
    }

    #[test]
    fn inventory_rejects_duplicate_guards_before_publication() {
        let statement = statement();
        let Err(error) = RuntimeAssertionInventory::try_new(
            RuntimeArtifactFingerprint::try_from_bytes([3; 32]).expect("artifact"),
            [
                site(7, RuntimeAssertionMode::Check, statement),
                site(7, RuntimeAssertionMode::Check, statement),
            ],
        ) else {
            panic!("duplicate guard must be terminal");
        };
        assert_eq!(
            error,
            RuntimeAssertionInventoryError::DuplicateGuard {
                guard: RuntimeAssertionGuardId::try_from_bytes([7; 16]).expect("guard"),
            }
        );
    }
}
