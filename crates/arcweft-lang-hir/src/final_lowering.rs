//! Private all-or-nothing construction of accepted final-HIR project revisions.
//!
//! Attached syntax lowerers stage every module inside one project transaction.
//! No public reader is exposed until all arenas, declaration members, source
//! components, diagnostics, semantic Proof-return facts, and slot lifetimes
//! have frozen and every project module can publish atomically.

mod capture_lowering;
mod expression_lowering;
pub(crate) mod id_ref_projection;
mod item_lowering;
pub(crate) mod literal_projection;
pub(crate) mod name_projection;
mod path_projection;
pub(crate) mod pattern_lowering;
mod statement_lowering;
mod thread_body_lowering;
mod type_ref_lowering;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_syntax::text::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS, MAX_RICH_TEXT_TAG_ARGUMENTS,
    MAX_RICH_TEXT_TAG_KEY_BYTES, MAX_RICH_TEXT_TAG_VALUE_BYTES,
};

use crate::arena::{HirArenaError, StagedArena};
use crate::database::{HirDatabase, HirLowerOutput, PreparedHirModuleCommit, StagedModuleCommit};
use crate::diagnostic::{HirDiagnostic, HirRecoveryDiagnostic};
use crate::dialogue_application::{
    HirDialogueExpressionExpectation, HirDialogueTransactionContext,
    HirDialogueTransactionRequirement, HirRichTextCharge,
};
use crate::expr::{HirExpr, HirExprKind};
use crate::identity::{
    CaptureId, ExprId, HirLimit, HirSnapshotId, IdResolveError, ItemId, LocalGeneration, LocalId,
    PatternId, ScopeId, StmtId, SyntheticOwner, TypeId,
};
use crate::item::{
    HirDeclarationMemberArena, HirDeclarationMemberIndexBuilder, HirItem, HirItemKind,
};
use crate::lowering::{
    HirInvariantFailure, HirLimitError, HirLowerFailure, HirLoweringCheckpoint, HirLoweringControl,
    LoweringRequest,
};
use crate::module::{HirModule, HirModuleArenaParts, HirModuleArenas};
use crate::pattern::{HirPattern, HirPatternResolver};
use crate::proof_return::{
    HirProofReturnCallableHeaderRef, HirProofReturnHeader, HirProofReturnHeaderItemRef,
    HirProofReturnHeaderModuleView, HirProofReturnHeaderProjectView, HirProofReturnModuleLease,
    HirProofReturnProjectGeneration, HirProofReturnProjectTransaction, HirProofReturnSemanticClass,
    HirProofReturnSemanticFactSet,
};
use crate::scope::{HirCapture, HirLocal, HirScope};
use crate::slot::{HirOrigin, HirSlotError, StagedSlotTransaction};
use crate::source_index::{
    HirCallableSourceOwner, HirCallableSourceRole, HirDeclarationSourceRole, HirItemSourceRole,
    HirSourceCommitInvariantError, HirSourcePresence, HirSourceQuery, HirSourceSite,
    StagedHirSourceIndex,
};
use crate::stmt::HirStmt;
use crate::type_ref::{HirType, HirTypeResolver};

use capture_lowering::ClosureCaptureFrame;
use item_lowering::PendingProofDeclaration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StagedProofReturnHeader {
    pub(crate) item: ItemId,
    pub(crate) return_type: TypeId,
    pub(crate) source: arcweft_source::SourceSpan,
    pub(crate) declaration_source: arcweft_source::SourceSpan,
    pub(crate) name_source: arcweft_source::SourceSpan,
    pub(crate) name: crate::item::HirRequiredName,
    pub(crate) prefix: crate::item::HirItemPrefix,
    pub(crate) generic_parameters: Box<[crate::item::HirGenericParameter]>,
}

/// One exact current module retained by an incremental project transaction.
///
/// Retained modules are already frozen in the same `HirDatabase`. Their
/// authored Proof-return headers are projected once from the final typed item
/// and source owners so registration and semantic classification still see one
/// complete project generation.
pub(crate) struct RetainedProofReturnModule {
    module: Arc<HirModule>,
    authored_proof_return_headers: Box<[StagedProofReturnHeader]>,
}

/// Per-module owner inside one complete Proof-return project transaction.
#[allow(
    clippy::large_enum_variant,
    reason = "the transaction keeps staged and retained module ownership inline until atomic publication"
)]
pub(crate) enum ProofReturnProjectModuleTransaction<'source> {
    Staged(StagedHirModuleTransaction<'source>),
    Retained(RetainedProofReturnModule),
}

impl RetainedProofReturnModule {
    fn try_new(module: Arc<HirModule>) -> Result<Self, HirLowerFailure> {
        if !module.is_cache_eligible() {
            return Err(HirLowerFailure::RetainedModuleNotCacheEligible {
                module: module.key().path().clone(),
            });
        }

        let mut headers = Vec::new();
        for item in module.source_ordered_items().iter().copied() {
            let payload = module.resolve_item(item)?;
            let HirItemKind::Proof(proof) = payload.kind() else {
                continue;
            };
            let result_query = HirSourceQuery::Item {
                owner: item,
                role: HirItemSourceRole::Callable(HirCallableSourceRole::Result {
                    owner: HirCallableSourceOwner::Item,
                }),
            };
            let result = match module
                .source_site(module.provenance().source_identity(), result_query)
                .map_err(|_| HirInvariantFailure::InvalidSourceIndex)?
                .presence()
            {
                HirSourcePresence::AbsentOptional => continue,
                HirSourcePresence::Present(HirSourceSite::Span(span)) => span.clone(),
                HirSourcePresence::Present(HirSourceSite::Insertion(_)) => {
                    return Err(HirInvariantFailure::InvalidSourceIndex.into());
                }
            };
            let declaration_source =
                retained_item_span(&module, item, HirDeclarationSourceRole::Whole)?;
            let name_source = retained_item_span(&module, item, HirDeclarationSourceRole::Name)?;
            headers.push(StagedProofReturnHeader {
                item,
                return_type: proof.return_type(),
                source: result,
                declaration_source,
                name_source,
                name: proof.name().clone(),
                prefix: payload.prefix().clone(),
                generic_parameters: proof.generic_parameters().into(),
            });
        }

        Ok(Self {
            module,
            authored_proof_return_headers: headers.into_boxed_slice(),
        })
    }
}

fn retained_item_span(
    module: &HirModule,
    owner: ItemId,
    role: HirDeclarationSourceRole,
) -> Result<arcweft_source::SourceSpan, HirLowerFailure> {
    let query = HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Declaration(role),
    };
    match module
        .source_site(module.provenance().source_identity(), query)
        .map_err(|_| HirInvariantFailure::InvalidSourceIndex)?
        .presence()
    {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => Err(HirInvariantFailure::InvalidSourceIndex.into()),
    }
}

impl ProofReturnProjectModuleTransaction<'_> {
    fn key(&self) -> &crate::lowering::HirModuleKey {
        match self {
            Self::Staged(module) => module.request.key(),
            Self::Retained(module) => module.module.key(),
        }
    }

    fn snapshot_id(&self) -> HirSnapshotId {
        match self {
            Self::Staged(module) => module.plan.snapshot_id(),
            Self::Retained(module) => module.module.snapshot_id(),
        }
    }

    fn syntax_snapshot(&self) -> &arcweft_lang_syntax::attachment::SyntaxSnapshotId {
        match self {
            Self::Staged(module) => module.request.source().snapshot_id(),
            Self::Retained(module) => module.module.provenance().syntax_snapshot(),
        }
    }

    fn source_identity(&self) -> &arcweft_source::SourceDocumentIdentity {
        match self {
            Self::Staged(module) => module.request.source().document().identity(),
            Self::Retained(module) => module.module.provenance().source_identity(),
        }
    }

    fn authored_proof_return_headers(&self) -> &[StagedProofReturnHeader] {
        match self {
            Self::Staged(module) => &module.staged_proof_return_headers,
            Self::Retained(module) => &module.authored_proof_return_headers,
        }
    }
}

fn require_limit(limit: HirLimit, observed: usize) -> Result<(), HirLowerFailure> {
    let maximum = limit.maximum();
    if observed <= maximum {
        Ok(())
    } else {
        Err(HirLimitError::with_maximum(limit, observed, maximum).into())
    }
}

/// The eight mutable typed arenas owned by one final-HIR transaction.
pub(crate) struct StagedHirModuleArenas {
    items: StagedArena<HirItem, ItemId>,
    scopes: StagedArena<HirScope, ScopeId>,
    locals: StagedArena<HirLocal, LocalId>,
    expressions: StagedArena<HirExpr, ExprId>,
    statements: StagedArena<HirStmt, StmtId>,
    types: StagedArena<HirType, TypeId>,
    patterns: StagedArena<HirPattern, PatternId>,
    captures: StagedArena<HirCapture, CaptureId>,
}

impl StagedHirModuleArenas {
    fn from_previous(previous: Option<&HirModule>) -> Self {
        let Some(previous) = previous else {
            return Self {
                items: StagedArena::new(),
                scopes: StagedArena::new(),
                locals: StagedArena::new(),
                expressions: StagedArena::new(),
                statements: StagedArena::new(),
                types: StagedArena::new(),
                patterns: StagedArena::new(),
                captures: StagedArena::new(),
            };
        };
        let arenas = previous.arenas();
        Self {
            items: StagedArena::from_snapshot(arenas.items()),
            scopes: StagedArena::from_snapshot(arenas.scopes()),
            locals: StagedArena::from_snapshot(arenas.locals()),
            expressions: StagedArena::from_snapshot(arenas.expressions()),
            statements: StagedArena::from_snapshot(arenas.statements()),
            types: StagedArena::from_snapshot(arenas.types()),
            patterns: StagedArena::from_snapshot(arenas.patterns()),
            captures: StagedArena::from_snapshot(arenas.captures()),
        }
    }

    pub(crate) const fn items(&mut self) -> &mut StagedArena<HirItem, ItemId> {
        &mut self.items
    }

    pub(crate) const fn scopes(&mut self) -> &mut StagedArena<HirScope, ScopeId> {
        &mut self.scopes
    }

    pub(crate) const fn locals(&mut self) -> &mut StagedArena<HirLocal, LocalId> {
        &mut self.locals
    }

    pub(crate) const fn expressions(&mut self) -> &mut StagedArena<HirExpr, ExprId> {
        &mut self.expressions
    }

    pub(crate) const fn statements(&mut self) -> &mut StagedArena<HirStmt, StmtId> {
        &mut self.statements
    }

    pub(crate) const fn types(&mut self) -> &mut StagedArena<HirType, TypeId> {
        &mut self.types
    }

    pub(crate) const fn patterns(&mut self) -> &mut StagedArena<HirPattern, PatternId> {
        &mut self.patterns
    }

    pub(crate) const fn captures(&mut self) -> &mut StagedArena<HirCapture, CaptureId> {
        &mut self.captures
    }

    fn freeze(
        self,
        slots: &mut StagedSlotTransaction,
    ) -> Result<HirModuleArenaParts, HirLowerFailure> {
        Ok(HirModuleArenaParts {
            items: self.items.into_snapshot(slots)?,
            scopes: self.scopes.into_snapshot(slots)?,
            locals: self.locals.into_snapshot(slots)?,
            expressions: self.expressions.into_snapshot(slots)?,
            statements: self.statements.into_snapshot(slots)?,
            types: self.types.into_snapshot(slots)?,
            patterns: self.patterns.into_snapshot(slots)?,
            captures: self.captures.into_snapshot(slots)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalGenerationLedgerEntry {
    local: LocalId,
    generation: LocalGeneration,
    binding_name_start: usize,
}

impl LocalGenerationLedgerEntry {
    const fn new(local: LocalId, generation: LocalGeneration, binding_name_start: usize) -> Self {
        Self {
            local,
            generation,
            binding_name_start,
        }
    }
}

/// Source-ordered publication history for one `(scope, name)` binding key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LocalPublicationTimeline {
    entries: Vec<LocalGenerationLedgerEntry>,
}

impl LocalPublicationTimeline {
    fn last(&self) -> Option<LocalGenerationLedgerEntry> {
        self.entries.last().copied()
    }

    fn visible_at(
        &self,
        use_start: usize,
    ) -> impl Iterator<Item = LocalGenerationLedgerEntry> + '_ {
        self.entries
            .iter()
            .rev()
            .copied()
            .filter(move |entry| entry.binding_name_start < use_start)
    }

    fn publish(&mut self, entry: LocalGenerationLedgerEntry) -> Result<(), HirLowerFailure> {
        if self.entries.last().is_some_and(|previous| {
            previous.binding_name_start >= entry.binding_name_start
                || previous.generation >= entry.generation
        }) {
            return Err(HirInvariantFailure::InvalidLocalTimeline.into());
        }
        self.entries.push(entry);
        Ok(())
    }

    #[cfg(test)]
    fn entries(&self) -> &[LocalGenerationLedgerEntry] {
        &self.entries
    }
}

/// Private owner of every fallible input to one final-HIR publication.
pub(crate) struct StagedHirModuleTransaction<'source> {
    control: HirLoweringControl,
    request: LoweringRequest<'source>,
    plan: StagedModuleCommit,
    slots: StagedSlotTransaction,
    arenas: StagedHirModuleArenas,
    declaration_members: HirDeclarationMemberIndexBuilder,
    source_components: StagedHirSourceIndex,
    source_ordered_items: Vec<ItemId>,
    diagnostics: Vec<HirDiagnostic>,
    local_timelines: BTreeMap<(ScopeId, crate::leaf::HirName), LocalPublicationTimeline>,
    pattern_locals: BTreeMap<PatternId, Box<[LocalId]>>,
    closure_capture_frames: Vec<ClosureCaptureFrame>,
    pending_proofs: Vec<PendingProofDeclaration>,
    staged_proof_return_headers: Vec<StagedProofReturnHeader>,
    proof_return_facts: Option<Arc<HirProofReturnSemanticFactSet>>,
    #[cfg(test)]
    reverse_pattern_child_insertion: bool,
}

impl HirDatabase {
    /// Stages a complete module-preserving project through the Proof return
    /// header barrier without publishing a module, reserving an observable
    /// revision, or allocating any authored Proof body. The supplied symbol
    /// source set is the complete semantic revision; staged HIR module leases
    /// must be exact members of that potentially broader inventory.
    pub fn stage_proof_return_project<'source, 'symbol>(
        &self,
        requests: impl IntoIterator<Item = LoweringRequest<'source>>,
        world: crate::symbol::ProjectSymbolWorldId,
        revision: crate::symbol::ProjectSymbolRevision,
        symbol_sources: impl IntoIterator<Item = &'symbol arcweft_source::SourceDocumentIdentity>,
        control: HirLoweringControl,
    ) -> Result<HirProofReturnProjectTransaction<'source>, HirLowerFailure> {
        self.stage_proof_return_project_with_retained(
            requests,
            std::iter::empty(),
            world,
            revision,
            symbol_sources,
            control,
        )
    }

    /// Stages changed modules and retains exact current cache-hit modules in
    /// one complete Proof-return generation. Retained leases must come from
    /// this database and remain its current clean module `Arc`.
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic preflight validates and stages the complete changed-plus-retained project generation"
    )]
    pub fn stage_proof_return_project_with_retained<'source, 'symbol>(
        &self,
        requests: impl IntoIterator<Item = LoweringRequest<'source>>,
        retained: impl IntoIterator<Item = Arc<HirModule>>,
        world: crate::symbol::ProjectSymbolWorldId,
        revision: crate::symbol::ProjectSymbolRevision,
        symbol_sources: impl IntoIterator<Item = &'symbol arcweft_source::SourceDocumentIdentity>,
        control: HirLoweringControl,
    ) -> Result<HirProofReturnProjectTransaction<'source>, HirLowerFailure> {
        control.checkpoint(HirLoweringCheckpoint::BeforePreflight)?;
        let mut requests = requests.into_iter().collect::<Vec<_>>();
        let mut retained = retained.into_iter().collect::<Vec<_>>();
        if requests.is_empty() && retained.is_empty() {
            return Err(HirLowerFailure::EmptyProjectTransaction);
        }
        requests.sort_by(|left, right| left.key().cmp(right.key()));
        retained.sort_by(|left, right| left.key().cmp(right.key()));

        let mut keys = BTreeSet::new();
        for request in &requests {
            if !keys.insert(request.key().clone()) {
                return Err(HirLowerFailure::DuplicateModuleRequest {
                    module: request.key().path().clone(),
                });
            }
        }
        for module in &retained {
            let actual = module.module_id().database();
            if actual != self.database_id() {
                return Err(HirLowerFailure::RetainedModuleWrongDatabase {
                    expected: self.database_id(),
                    actual,
                });
            }
            if !keys.insert(module.key().clone()) {
                return Err(HirLowerFailure::DuplicateModuleRequest {
                    module: module.key().path().clone(),
                });
            }
            let current = self.current(module.key()).ok_or_else(|| {
                HirLowerFailure::RetainedModuleNotCurrent {
                    module: module.key().path().clone(),
                    snapshot: module.snapshot_id(),
                }
            })?;
            if !Arc::ptr_eq(&current, module) {
                return Err(HirLowerFailure::RetainedModuleNotCurrent {
                    module: module.key().path().clone(),
                    snapshot: module.snapshot_id(),
                });
            }
        }

        // An exact accepted syntax/source request is a HIR no-op. Select the
        // current immutable module here, before allocating a revision or
        // staging any arena, so callers cannot accidentally republish an
        // equal module through the changed-module path. Recovered modules are
        // deliberately rebuilt because they are not executable cache leases.
        let mut changed_requests = Vec::with_capacity(requests.len());
        for request in requests {
            let exact_current = self.current(request.key()).filter(|module| {
                module.is_cache_eligible()
                    && module.provenance().syntax_snapshot() == request.source().snapshot_id()
                    && module.provenance().source_snapshot()
                        == request.source().source_snapshot_id()
                    && module.provenance().source_identity()
                        == request.source().document().identity()
            });
            if let Some(module) = exact_current {
                retained.push(module);
            } else {
                changed_requests.push(request);
            }
        }
        requests = changed_requests;
        retained.sort_by(|left, right| left.key().cmp(right.key()));

        let plans = if requests.is_empty() {
            Vec::new()
        } else {
            self.stage_project_modules(requests.iter().map(|request| request.key().clone()))?
        };
        let mut modules = Vec::with_capacity(requests.len() + retained.len());
        for (request, plan) in requests.into_iter().zip(plans) {
            control.checkpoint(HirLoweringCheckpoint::BeforeModuleStaging)?;
            debug_assert_eq!(request.key(), plan.key());
            let source = request.source().clone();
            let mut transaction =
                StagedHirModuleTransaction::stage_with_plan(request, plan, control.clone())?;
            transaction.lower_parsed_source_items(&source)?;
            modules.push(ProofReturnProjectModuleTransaction::Staged(transaction));
        }
        for module in retained {
            modules.push(ProofReturnProjectModuleTransaction::Retained(
                RetainedProofReturnModule::try_new(module)?,
            ));
        }
        modules.sort_by(|left, right| left.key().cmp(right.key()));
        let generation = HirProofReturnProjectGeneration::try_new(
            self.database_id(),
            world,
            revision,
            symbol_sources,
            modules.iter().map(|module| {
                HirProofReturnModuleLease::new(
                    module.key().package().clone(),
                    module.key().path().clone(),
                    module.snapshot_id(),
                    module.syntax_snapshot().clone(),
                    module.source_identity().clone(),
                )
            }),
        )?;
        let mut headers = modules
            .iter()
            .flat_map(|module| {
                module.authored_proof_return_headers().iter().map(|header| {
                    HirProofReturnHeader::try_new(
                        Arc::clone(&generation),
                        module.key().path().clone(),
                        header.item,
                        header.return_type,
                        header.source.clone(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        headers.sort_by_key(HirProofReturnHeader::item);
        Ok(HirProofReturnProjectTransaction {
            control,
            generation,
            headers: headers.into_boxed_slice(),
            modules,
        })
    }
}

/// Exposes the unpublished module primitive only to crate-internal invariant
/// tests. Production consumers must enter through the complete project
/// transaction and cannot publish this value directly.
#[cfg(test)]
pub(crate) fn stage_unpublished_module_for_invariant_test<'source>(
    database: &HirDatabase,
    request: LoweringRequest<'source>,
    control: HirLoweringControl,
) -> Result<StagedHirModuleTransaction<'source>, HirLowerFailure> {
    StagedHirModuleTransaction::stage(database, request, control)
}

impl HirProofReturnProjectTransaction<'_> {
    /// Installs one complete fact set, resumes every paused Proof body, freezes
    /// changed modules, and publishes the project only after every fallible
    /// check has succeeded. Exact no-op modules remain accepted by the same
    /// `Arc` and therefore produce no invalidation output.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "publication deliberately consumes the caller's accepted semantic-fact lease"
    )]
    pub fn publish_with_semantic_facts(
        self,
        database: &mut HirDatabase,
        facts: Arc<HirProofReturnSemanticFactSet>,
    ) -> Result<Vec<HirLowerOutput>, HirLowerFailure> {
        let (outputs, _retained) = self.publish_parts(database, &facts)?;
        Ok(outputs)
    }

    /// Publishes changed modules and returns the complete exact project module
    /// lease set, including read-only cache-hit modules retained by the same
    /// transaction.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "publication deliberately consumes the caller's accepted semantic-fact lease"
    )]
    pub fn publish_modules_with_semantic_facts(
        self,
        database: &mut HirDatabase,
        facts: Arc<HirProofReturnSemanticFactSet>,
    ) -> Result<Vec<Arc<HirModule>>, HirLowerFailure> {
        let (outputs, retained) = self.publish_parts(database, &facts)?;
        let mut modules = outputs
            .into_iter()
            .map(HirLowerOutput::into_module)
            .chain(retained)
            .collect::<Vec<_>>();
        modules.sort_by(|left, right| left.key().cmp(right.key()));
        Ok(modules)
    }

    fn publish_parts(
        self,
        database: &mut HirDatabase,
        facts: &Arc<HirProofReturnSemanticFactSet>,
    ) -> Result<(Vec<HirLowerOutput>, Vec<Arc<HirModule>>), HirLowerFailure> {
        self.control
            .checkpoint(HirLoweringCheckpoint::BeforeSemanticResume)?;
        if !Arc::ptr_eq(facts.generation(), &self.generation) {
            return Err(
                crate::proof_return::HirProofReturnAuthorityError::ForeignGeneration.into(),
            );
        }
        for header in &self.headers {
            let _ = facts.class_for(header)?;
        }

        let mut prepared = Vec::with_capacity(self.modules.len());
        let mut retained = Vec::new();
        for module in self.modules {
            match module {
                ProofReturnProjectModuleTransaction::Staged(mut module) => {
                    self.generation.validate_module_transaction(
                        module.request.key().package(),
                        module.request.key().path(),
                        module.plan.snapshot_id(),
                        module.request.source().snapshot_id(),
                        module.request.source().document().identity(),
                    )?;
                    module.proof_return_facts = Some(Arc::clone(facts));
                    module.resume_pending_proof_declarations()?;
                    prepared.push(module.prepare()?);
                }
                ProofReturnProjectModuleTransaction::Retained(module) => {
                    self.generation.validate_module_transaction(
                        module.module.key().package(),
                        module.module.key().path(),
                        module.module.snapshot_id(),
                        module.module.provenance().syntax_snapshot(),
                        module.module.provenance().source_identity(),
                    )?;
                    let current = database.current(module.module.key()).ok_or_else(|| {
                        HirLowerFailure::RetainedModuleNotCurrent {
                            module: module.module.key().path().clone(),
                            snapshot: module.module.snapshot_id(),
                        }
                    })?;
                    if !Arc::ptr_eq(&current, &module.module) {
                        return Err(HirLowerFailure::RetainedModuleNotCurrent {
                            module: module.module.key().path().clone(),
                            snapshot: module.module.snapshot_id(),
                        });
                    }
                    for header in &module.authored_proof_return_headers {
                        let class = facts.class_for(
                            self.headers
                                .iter()
                                .find(|candidate| candidate.item() == header.item)
                                .ok_or(
                                    crate::proof_return::HirProofReturnAuthorityError::MissingFact {
                                        item: header.item,
                                    },
                                )?,
                        )?;
                        let item = module.module.resolve_item(header.item)?;
                        let HirItemKind::Proof(proof) = item.kind() else {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        };
                        if proof.return_semantic_class() != class {
                            return Err(HirLowerFailure::RetainedProofReturnSemanticMismatch {
                                item: header.item,
                            });
                        }
                    }
                    retained.push(module.module);
                }
            }
            self.control
                .checkpoint(HirLoweringCheckpoint::ModulePrepared)?;
        }
        self.control
            .checkpoint(HirLoweringCheckpoint::BeforeCommit)?;
        let outputs = if prepared.is_empty() {
            Vec::new()
        } else {
            database.publish_project_modules(prepared)?
        };
        Ok((outputs, retained))
    }
}

impl<'source> HirProofReturnProjectTransaction<'source> {
    pub fn header_view(&self) -> HirProofReturnHeaderProjectView<'_, 'source> {
        HirProofReturnHeaderProjectView {
            modules: &self.modules,
        }
    }
}

impl<'transaction, 'source> HirProofReturnHeaderProjectView<'transaction, 'source> {
    pub fn modules(
        self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'transaction arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
            HirProofReturnHeaderModuleView<'transaction, 'source>,
        ),
    > {
        self.modules.iter().map(|module| {
            (
                module.key().path(),
                HirProofReturnHeaderModuleView { module },
            )
        })
    }

    pub fn module(
        self,
        path: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
    ) -> Option<HirProofReturnHeaderModuleView<'transaction, 'source>> {
        self.modules
            .iter()
            .find(|module| module.key().path() == path)
            .map(|module| HirProofReturnHeaderModuleView { module })
    }

    pub fn items(self) -> impl Iterator<Item = HirProofReturnHeaderItemRef<'transaction, 'source>> {
        self.modules().flat_map(|(_, module)| module.items())
    }

    pub fn authored_proof_returns(
        self,
    ) -> impl Iterator<Item = HirProofReturnCallableHeaderRef<'transaction, 'source>> {
        self.modules()
            .flat_map(|(_, module)| module.authored_proof_returns())
    }
}

impl<'transaction, 'source> HirProofReturnHeaderModuleView<'transaction, 'source> {
    pub fn same_transaction(self, other: Self) -> bool {
        core::ptr::eq(self.module, other.module)
    }

    pub fn snapshot_id(self) -> HirSnapshotId {
        self.module.snapshot_id()
    }

    pub fn syntax_snapshot(
        self,
    ) -> &'transaction arcweft_lang_syntax::attachment::SyntaxSnapshotId {
        self.module.syntax_snapshot()
    }

    pub fn key(self) -> &'transaction crate::lowering::HirModuleKey {
        self.module.key()
    }

    pub fn source_identity(self) -> &'transaction arcweft_source::SourceDocumentIdentity {
        self.module.source_identity()
    }

    pub fn document(self) -> &'transaction arcweft_source::SourceDocument {
        match self.module {
            ProofReturnProjectModuleTransaction::Staged(module) => {
                module.request.source().document()
            }
            ProofReturnProjectModuleTransaction::Retained(module) => {
                module.module.provenance().document()
            }
        }
    }

    pub fn resolve_type(self, owner: TypeId) -> Result<&'transaction HirType, IdResolveError> {
        match self.module {
            ProofReturnProjectModuleTransaction::Staged(module) => {
                match module.arenas.types.resolve_staged(&module.slots, owner) {
                    Ok(ty) => Ok(ty),
                    Err(HirArenaError::Slot(HirSlotError::Resolve(error))) => Err(error),
                    Err(error) => unreachable!(
                        "validated paused Proof return header type failed staged resolution: {error}"
                    ),
                }
            }
            ProofReturnProjectModuleTransaction::Retained(module) => {
                module.module.resolve_type(owner)
            }
        }
    }

    pub fn type_source_site(
        self,
        owner: TypeId,
        role: crate::source_index::HirTypeSourceRole,
    ) -> Option<&'transaction HirSourceSite> {
        let query = crate::source_index::HirSourceQuery::Type { owner, role };
        match self.module {
            ProofReturnProjectModuleTransaction::Staged(module) => {
                if query.is_slot_whole() {
                    return module
                        .slots
                        .resolve_staged(owner)
                        .ok()
                        .map(crate::slot::HirSlotMetadata::source_site);
                }
                match module.source_components.component_presence(&query) {
                    Some(crate::source_index::HirSourcePresence::Present(site)) => Some(site),
                    Some(crate::source_index::HirSourcePresence::AbsentOptional) | None => None,
                }
            }
            ProofReturnProjectModuleTransaction::Retained(module) => module
                .module
                .source_site(module.module.provenance().source_identity(), query)
                .ok()
                .and_then(|lookup| match lookup.presence() {
                    HirSourcePresence::Present(site) => Some(site),
                    HirSourcePresence::AbsentOptional => None,
                }),
        }
    }

    pub fn item_source_site(
        self,
        owner: ItemId,
        role: crate::source_index::HirItemSourceRole,
    ) -> Option<&'transaction HirSourceSite> {
        let query = crate::source_index::HirSourceQuery::Item { owner, role };
        match self.module {
            ProofReturnProjectModuleTransaction::Staged(module) => {
                if query.is_slot_whole() {
                    return module
                        .slots
                        .resolve_staged(owner)
                        .ok()
                        .map(crate::slot::HirSlotMetadata::source_site);
                }
                match module.source_components.component_presence(&query) {
                    Some(crate::source_index::HirSourcePresence::Present(site)) => Some(site),
                    Some(crate::source_index::HirSourcePresence::AbsentOptional) | None => None,
                }
            }
            ProofReturnProjectModuleTransaction::Retained(module) => module
                .module
                .source_site(module.module.provenance().source_identity(), query)
                .ok()
                .and_then(|lookup| match lookup.presence() {
                    HirSourcePresence::Present(site) => Some(site),
                    HirSourcePresence::AbsentOptional => None,
                }),
        }
    }

    fn resolve_item(self, owner: ItemId) -> Result<&'transaction HirItem, IdResolveError> {
        match self.module {
            ProofReturnProjectModuleTransaction::Staged(module) => {
                match module.arenas.items.resolve_staged(&module.slots, owner) {
                    Ok(item) => Ok(item),
                    Err(HirArenaError::Slot(HirSlotError::Resolve(error))) => Err(error),
                    Err(error) => unreachable!(
                        "validated paused Proof return item failed staged resolution: {error}"
                    ),
                }
            }
            ProofReturnProjectModuleTransaction::Retained(module) => {
                module.module.resolve_item(owner)
            }
        }
    }

    pub fn items(self) -> impl Iterator<Item = HirProofReturnHeaderItemRef<'transaction, 'source>> {
        let items: &'transaction [ItemId] = match self.module {
            ProofReturnProjectModuleTransaction::Staged(module) => &module.source_ordered_items,
            ProofReturnProjectModuleTransaction::Retained(module) => {
                module.module.source_ordered_items()
            }
        };
        items.iter().filter_map(move |id| {
            self.resolve_item(*id)
                .ok()
                .map(|item| HirProofReturnHeaderItemRef {
                    module: self,
                    id: *id,
                    item,
                })
        })
    }

    pub fn authored_proof_returns(
        self,
    ) -> impl ExactSizeIterator<Item = HirProofReturnCallableHeaderRef<'transaction, 'source>> {
        self.module
            .authored_proof_return_headers()
            .iter()
            .map(move |header| HirProofReturnCallableHeaderRef {
                module: self,
                header,
            })
    }
}

impl<'transaction, 'source> HirProofReturnHeaderItemRef<'transaction, 'source> {
    pub const fn module(self) -> HirProofReturnHeaderModuleView<'transaction, 'source> {
        self.module
    }

    pub const fn id(self) -> ItemId {
        self.id
    }

    pub const fn item(self) -> &'transaction HirItem {
        self.item
    }
}

impl<'transaction, 'source> HirProofReturnCallableHeaderRef<'transaction, 'source> {
    pub const fn module(self) -> HirProofReturnHeaderModuleView<'transaction, 'source> {
        self.module
    }

    pub const fn item(self) -> ItemId {
        self.header.item
    }

    pub const fn return_type(self) -> TypeId {
        self.header.return_type
    }

    pub const fn name(self) -> &'transaction crate::item::HirRequiredName {
        &self.header.name
    }

    pub const fn prefix(self) -> &'transaction crate::item::HirItemPrefix {
        &self.header.prefix
    }

    pub fn generic_parameters(self) -> &'transaction [crate::item::HirGenericParameter] {
        &self.header.generic_parameters
    }

    pub const fn declaration_source(self) -> &'transaction arcweft_source::SourceSpan {
        &self.header.declaration_source
    }

    pub const fn name_source(self) -> &'transaction arcweft_source::SourceSpan {
        &self.header.name_source
    }
}

impl<'source> StagedHirModuleTransaction<'source> {
    #[cfg(test)]
    fn stage(
        database: &HirDatabase,
        request: LoweringRequest<'source>,
        control: HirLoweringControl,
    ) -> Result<Self, HirLowerFailure> {
        let plan = database.stage_module(request.key())?;
        Self::stage_inner(request, plan, None, control)
    }

    fn stage_with_plan(
        request: LoweringRequest<'source>,
        plan: StagedModuleCommit,
        control: HirLoweringControl,
    ) -> Result<Self, HirLowerFailure> {
        Self::stage_inner(request, plan, None, control)
    }

    fn stage_inner(
        request: LoweringRequest<'source>,
        plan: StagedModuleCommit,
        proof_return_facts: Option<Arc<HirProofReturnSemanticFactSet>>,
        control: HirLoweringControl,
    ) -> Result<Self, HirLowerFailure> {
        if let Some(previous) = plan.previous() {
            let accepted_snapshot = previous.provenance().syntax_snapshot();
            let supplied_snapshot = request.source().snapshot_id();
            let accepted = accepted_snapshot.lineage();
            let supplied = supplied_snapshot.lineage();
            if accepted.database() != supplied.database() {
                return Err(HirLowerFailure::WrongSyntaxDatabase {
                    expected: accepted.database(),
                    actual: supplied.database(),
                });
            }
            if accepted != supplied {
                return Err(HirLowerFailure::WrongSyntaxLineage {
                    expected: accepted,
                    actual: supplied,
                });
            }
            if supplied_snapshot.source().generation() < accepted_snapshot.source().generation() {
                return Err(HirLowerFailure::StaleSource {
                    current: accepted_snapshot.clone(),
                    supplied: supplied_snapshot.clone(),
                });
            }
        }

        let slots = match plan.previous() {
            Some(previous) => {
                StagedSlotTransaction::from_snapshot(previous.slots(), plan.revision())
            }
            None => StagedSlotTransaction::new(plan.module_id(), plan.revision()),
        };
        let arenas = StagedHirModuleArenas::from_previous(plan.previous().map(Arc::as_ref));
        let declaration_members = HirDeclarationMemberIndexBuilder::new(plan.module_id());
        let source_components =
            StagedHirSourceIndex::new(request.source().document().identity().clone(), &slots);
        let diagnostics = request
            .source()
            .diagnostics()
            .iter()
            .cloned()
            .map(HirDiagnostic::Syntax)
            .collect();

        if let Some(facts) = proof_return_facts.as_ref() {
            facts.generation().validate_module_transaction(
                request.key().package(),
                request.key().path(),
                plan.snapshot_id(),
                request.source().snapshot_id(),
                request.source().document().identity(),
            )?;
        }

        Ok(Self {
            control,
            request,
            plan,
            slots,
            arenas,
            declaration_members,
            source_components,
            source_ordered_items: Vec::new(),
            diagnostics,
            local_timelines: BTreeMap::new(),
            pattern_locals: BTreeMap::new(),
            closure_capture_frames: Vec::new(),
            pending_proofs: Vec::new(),
            staged_proof_return_headers: Vec::new(),
            proof_return_facts,
            #[cfg(test)]
            reverse_pattern_child_insertion: false,
        })
    }

    pub(super) fn authored_proof_return_semantic_class(
        &self,
        item: ItemId,
        return_type: TypeId,
        source: arcweft_source::SourceSpan,
    ) -> Result<HirProofReturnSemanticClass, HirLowerFailure> {
        let facts = self
            .proof_return_facts
            .as_ref()
            .ok_or(HirLowerFailure::MissingProofReturnSemanticFacts { item })?;
        let header = crate::proof_return::HirProofReturnHeader::try_new(
            Arc::clone(facts.generation()),
            self.request.key().path().clone(),
            item,
            return_type,
            source,
        )?;
        facts.class_for(&header).map_err(Into::into)
    }

    fn stage_proof_return_header(&mut self, header: StagedProofReturnHeader) {
        self.staged_proof_return_headers.push(header);
    }

    fn next_sequential_local_generation(
        &self,
        scope: ScopeId,
        name: &crate::leaf::HirName,
        binding_name_start: usize,
    ) -> Result<LocalGeneration, HirLowerFailure> {
        let Some(previous) = self
            .local_timelines
            .get(&(scope, name.clone()))
            .and_then(LocalPublicationTimeline::last)
        else {
            return Ok(LocalGeneration::FIRST);
        };
        if binding_name_start <= previous.binding_name_start {
            return Err(HirLowerFailure::LocalBindingSourceOrderViolation {
                scope,
                name: name.clone(),
                previous_start: previous.binding_name_start,
                attempted_start: binding_name_start,
            });
        }
        previous.generation.checked_next().ok_or_else(|| {
            HirLowerFailure::LocalGenerationExhausted {
                scope,
                name: name.clone(),
            }
        })
    }

    pub(crate) const fn snapshot_id(&self) -> HirSnapshotId {
        self.plan.snapshot_id()
    }

    /// Grants attached lowerers simultaneous access to the one slot ledger and
    /// its eight typed payload arenas. Neither value can outlive this owner.
    pub(crate) fn storage_mut(
        &mut self,
    ) -> (&mut StagedSlotTransaction, &mut StagedHirModuleArenas) {
        (&mut self.slots, &mut self.arenas)
    }

    pub(crate) fn stage_declaration_members(
        &mut self,
        owner: ItemId,
        item: &HirItem,
        arena: HirDeclarationMemberArena,
    ) -> Result<(), HirLowerFailure> {
        require_limit(HirLimit::DeclarationMembers, arena.members().len())?;
        self.declaration_members
            .stage(owner, item, arena)
            .map_err(|_| HirInvariantFailure::InvalidDeclarationMemberIndex.into())
    }

    #[cfg(test)]
    pub(crate) const fn source_components(&mut self) -> &mut StagedHirSourceIndex {
        &mut self.source_components
    }

    pub(crate) fn stage_recovery_diagnostic(&mut self, diagnostic: HirRecoveryDiagnostic) {
        self.diagnostics.push(HirDiagnostic::Recovery(diagnostic));
    }

    /// Resolves the latest binding published before one exact source use.
    ///
    /// Lookup walks lexical parents. A later same-scope shadow does not hide an
    /// earlier ancestor at a use that precedes the shadow's publication.
    pub(super) fn visible_local(
        &self,
        scope: ScopeId,
        name: &crate::leaf::HirName,
        use_start: usize,
    ) -> Result<Option<LocalId>, HirLowerFailure> {
        let mut current = Some(scope);
        let mut visited = BTreeSet::new();
        while let Some(scope) = current {
            if !visited.insert(scope) {
                return Err(HirInvariantFailure::InvalidScopeParent.into());
            }
            if let Some(timeline) = self.local_timelines.get(&(scope, name.clone()))
                && let Some(entry) = timeline.visible_at(use_start).next()
            {
                let local = self
                    .arenas
                    .locals
                    .resolve_staged(&self.slots, entry.local)?;
                if local.scope() != scope
                    || local.name() != name
                    || local.generation() != entry.generation
                {
                    return Err(HirInvariantFailure::InvalidLocalTimeline.into());
                }
                if !local.is_poisoned() {
                    return Ok(Some(entry.local));
                }
                return Ok(None);
            }
            current = self
                .arenas
                .scopes
                .resolve_staged(&self.slots, scope)?
                .parent();
        }
        Ok(None)
    }

    /// Records one source-backed top-level item in attached source order.
    /// Final publication validates complete, non-duplicated source-item
    /// coverage once against the immutable item arena and slot ledger.
    pub(crate) fn stage_source_ordered_item(&mut self, item: ItemId) {
        self.source_ordered_items.push(item);
    }

    #[cfg(test)]
    pub(crate) fn staged_source_ordered_items(&self) -> &[ItemId] {
        &self.source_ordered_items
    }

    #[cfg(test)]
    pub(crate) fn staged_item(&self, item: ItemId) -> Result<&HirItem, HirLowerFailure> {
        self.arenas
            .items
            .resolve_staged(&self.slots, item)
            .map_err(HirLowerFailure::from)
    }

    /// Test-only single-module publication used by lowering invariant tests.
    #[cfg(test)]
    pub(crate) fn finish(
        self,
        database: &mut HirDatabase,
    ) -> Result<HirLowerOutput, HirLowerFailure> {
        self.control
            .checkpoint(HirLoweringCheckpoint::BeforeSemanticResume)?;
        let control = self.control.clone();
        let prepared = self.prepare()?;
        control.checkpoint(HirLoweringCheckpoint::ModulePrepared)?;
        control.checkpoint(HirLoweringCheckpoint::BeforeCommit)?;
        database.publish_prepared_module(prepared)
    }

    fn prepare(mut self) -> Result<PreparedHirModuleCommit, HirLowerFailure> {
        if !self.closure_capture_frames.is_empty() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        self.slots.retire_untouched()?;

        let arena_parts = self.arenas.freeze(&mut self.slots)?;
        self.control
            .checkpoint(HirLoweringCheckpoint::SourceFreeze)?;
        let source_components = self.source_components.commit()?;
        let declaration_members = self.declaration_members.freeze();

        let syntax_diagnostic_count = self.request.source().diagnostics().len();
        self.diagnostics[syntax_diagnostic_count..].sort_by(HirDiagnostic::compare_for_publication);
        let diagnostics = Arc::from(self.diagnostics);

        let prepared_slots = self.slots.prepare()?;
        let arenas = HirModuleArenas::try_new(prepared_slots.snapshot(), arena_parts)?;
        let module = Arc::new(HirModule::try_new(
            self.plan.snapshot_id(),
            self.plan.key().clone(),
            self.request.source(),
            diagnostics,
            Arc::clone(prepared_slots.snapshot()),
            arenas,
            self.source_ordered_items.into_boxed_slice(),
            declaration_members,
            source_components,
            self.plan.invalidation_epoch(),
        )?);
        Ok(PreparedHirModuleCommit::new(
            self.plan,
            prepared_slots,
            module,
        ))
    }
}

impl HirTypeResolver for StagedHirModuleTransaction<'_> {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        self.arenas
            .scopes
            .resolve_staged(&self.slots, scope)
            .is_ok()
    }

    fn resolve_type(&self, scope: ScopeId, ty: TypeId) -> Option<&HirType> {
        HirTypeResolver::scope_is_live(self, scope)
            .then(|| self.arenas.types.resolve_staged(&self.slots, ty).ok())
            .flatten()
            .filter(|ty| ty.scope() == scope)
    }
}

impl HirPatternResolver for StagedHirModuleTransaction<'_> {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        HirTypeResolver::scope_is_live(self, scope)
    }

    fn local_is_visible(&self, scope: ScopeId, local: LocalId) -> bool {
        HirPatternResolver::scope_is_live(self, scope)
            && self
                .arenas
                .locals
                .resolve_staged(&self.slots, local)
                .is_ok_and(|local| local.scope() == scope)
    }

    fn resolve_type_state(
        &self,
        scope: ScopeId,
        ty: TypeId,
    ) -> Option<&crate::expr::HirPoisonState> {
        HirTypeResolver::resolve_type(self, scope, ty).map(HirType::state)
    }

    fn resolve_pattern(&self, scope: ScopeId, pattern: PatternId) -> Option<&HirPattern> {
        HirPatternResolver::scope_is_live(self, scope)
            .then(|| {
                self.arenas
                    .patterns
                    .resolve_staged(&self.slots, pattern)
                    .ok()
            })
            .flatten()
            .filter(|pattern| pattern.scope() == scope)
    }
}

impl HirDialogueTransactionContext for StagedHirModuleTransaction<'_> {
    type Error = HirLowerFailure;

    fn require(
        &mut self,
        requirement: HirDialogueTransactionRequirement,
    ) -> Result<(), Self::Error> {
        let valid = match requirement {
            HirDialogueTransactionRequirement::Expression { id, expected } => {
                let metadata = self.slots.resolve_staged(id)?;
                let expression = self.arenas.expressions.resolve_staged(&self.slots, id)?;
                let scope_is_live = HirTypeResolver::scope_is_live(self, expression.scope());
                let kind_matches = match expected {
                    HirDialogueExpressionExpectation::Any => true,
                    HirDialogueExpressionExpectation::Call => {
                        matches!(expression.kind(), HirExprKind::Call(_))
                    }
                    HirDialogueExpressionExpectation::PostfixIndexCandidate {
                        owner,
                        role,
                        target,
                    } => {
                        matches!(
                            (metadata.origin(), expression.kind()),
                            (
                                HirOrigin::Synthetic(key),
                                HirExprKind::Index(index)
                            ) if key.owner() == SyntheticOwner::Expr(owner)
                                && key.role() == role
                                && index.target() == target
                        )
                    }
                    HirDialogueExpressionExpectation::DialogueContentCandidate {
                        owner,
                        role,
                        target,
                    } => {
                        matches!(
                            (metadata.origin(), expression.kind()),
                            (
                                HirOrigin::Synthetic(key),
                                HirExprKind::DialogueContentApplication(application)
                            ) if key.owner() == SyntheticOwner::Expr(owner)
                                && key.role() == role
                                && application.target() == target
                        )
                    }
                };
                scope_is_live && kind_matches
            }
            HirDialogueTransactionRequirement::Statement(id) => self
                .arenas
                .statements
                .resolve_staged(&self.slots, id)
                .is_ok_and(|statement| HirTypeResolver::scope_is_live(self, statement.scope())),
            HirDialogueTransactionRequirement::Pattern(id) => self
                .arenas
                .patterns
                .resolve_staged(&self.slots, id)
                .is_ok_and(|pattern| HirPatternResolver::scope_is_live(self, pattern.scope())),
            HirDialogueTransactionRequirement::Scope(id) => {
                HirTypeResolver::scope_is_live(self, id)
            }
            HirDialogueTransactionRequirement::Item(id) => self
                .arenas
                .items
                .resolve_staged(&self.slots, id)
                .is_ok_and(|item| HirTypeResolver::scope_is_live(self, item.scope())),
            HirDialogueTransactionRequirement::RichTextCharge(charge) => match charge {
                HirRichTextCharge::ContentTags { observed } => {
                    observed <= MAX_RICH_TEXT_CONTENT_TAGS
                }
                HirRichTextCharge::ContentArguments { observed } => {
                    observed <= MAX_RICH_TEXT_CONTENT_ARGUMENTS
                }
                HirRichTextCharge::TagArguments { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_ARGUMENTS
                }
                HirRichTextCharge::ArgumentKeyBytes { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_KEY_BYTES
                }
                HirRichTextCharge::ArgumentValueDecodedBytes { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_VALUE_BYTES
                }
            },
        };
        if valid {
            Ok(())
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }
}

impl From<HirSlotError> for HirLowerFailure {
    fn from(error: HirSlotError) -> Self {
        match error {
            HirSlotError::Resolve(error) => Self::IdResolve(error),
            HirSlotError::Limit(error) => Self::Limit(error),
            HirSlotError::SlotIdentityExhausted { module, kind } => {
                Self::SlotIdentityExhausted { module, kind }
            }
            HirSlotError::OwnerNotReserved { .. }
            | HirSlotError::ConflictingSlotView { .. }
            | HirSlotError::MetadataMismatch { .. }
            | HirSlotError::InvalidKeyOnlyRole { .. }
            | HirSlotError::InvalidRetirement
            | HirSlotError::CommitConflict
            | HirSlotError::TransactionPoisoned => {
                Self::Invariant(HirInvariantFailure::InvalidSlotCommit)
            }
        }
    }
}

impl From<HirArenaError> for HirLowerFailure {
    fn from(error: HirArenaError) -> Self {
        match error {
            HirArenaError::Slot(error) => error.into(),
            HirArenaError::Limit(error) => Self::Limit(error),
            HirArenaError::InvalidReservation { .. }
            | HirArenaError::UnfinalizedReservations { .. }
            | HirArenaError::CoverageMismatch { .. }
            | HirArenaError::SnapshotMismatch { .. }
            | HirArenaError::TransactionMismatch { .. }
            | HirArenaError::MissingPayload { .. }
            | HirArenaError::BaseSnapshotMismatch { .. }
            | HirArenaError::TransactionPoisoned => {
                Self::Invariant(HirInvariantFailure::InvalidArenaCommit)
            }
        }
    }
}

impl From<HirSourceCommitInvariantError> for HirLowerFailure {
    fn from(_: HirSourceCommitInvariantError) -> Self {
        Self::Invariant(HirInvariantFailure::InvalidSourceIndex)
    }
}

#[cfg(test)]
#[path = "final_lowering/tests.rs"]
mod tests;
