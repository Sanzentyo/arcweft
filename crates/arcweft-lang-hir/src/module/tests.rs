use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::node::FunctionBodyKind;
use arcweft_lang_syntax::attachment::{DeclarationBodyNode, StatementNode, SyntaxNodeId};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::arena::{ArenaSnapshot, StagedArena};
use crate::diagnostic::{HirDiagnostic, HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{
    HirExpr, HirExprError, HirExprKind, HirGenericExprIssue, HirPoisonState, HirRecoveryIssue,
    HirTupleExpr,
};
use crate::identity::{
    ExprId, HirDatabaseId, HirLimit, HirModuleId, HirRevision, HirSnapshotId, ScopeId, StmtId,
    SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::item::HirDeclarationMemberIndexBuilder;
use crate::lowering::{HirInvariantFailure, HirLimitError, HirLowerFailure, HirModuleKey};
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::{SlotSnapshot, StagedSlotTransaction};
use crate::source_index::{
    HirExprSourceRole, HirInsertionPoint, HirSourceIndex, HirSourceQuery, HirSourceSite,
};
use crate::stmt::{HirStmt, HirStmtKind};
use crate::symbol::CallablePackageId;

use super::{
    HirModule, HirModuleArenaParts, HirModuleArenas, HirModuleStatus, validate_diagnostic_limit,
};

fn parsed(document_id: &str, source: &str) -> ParsedSource {
    let name = SourceName::path("proof/module.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(document_id).unwrap(),
            name.clone(),
            source,
        )
        .unwrap(),
    );
    let mut database = SyntaxDatabase::try_new().unwrap();
    database
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap()
}

fn module(value: u64) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(value).unwrap()),
        NonZeroU32::MIN,
    )
}

fn snapshot(module: HirModuleId) -> HirSnapshotId {
    HirSnapshotId::new(module, HirRevision::INITIAL)
}

fn key(document: &SourceDocumentId) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-module-tests").unwrap(),
        CanonicalModulePath::crate_root(),
        document.clone(),
    )
}

fn empty_arenas(slots: &crate::slot::SlotSnapshot) -> HirModuleArenas {
    HirModuleArenas::empty(slots)
}

fn diagnostics(parsed: &ParsedSource) -> Arc<[HirDiagnostic]> {
    parsed
        .diagnostics()
        .iter()
        .cloned()
        .map(HirDiagnostic::Syntax)
        .collect::<Vec<_>>()
        .into()
}

fn build(parsed: &ParsedSource, module: HirModuleId) -> Result<HirModule, HirLowerFailure> {
    let snapshot = snapshot(module);
    let slots = StagedSlotTransaction::new(module, HirRevision::INITIAL)
        .prepare()
        .unwrap();
    HirModule::try_new(
        snapshot,
        key(parsed.document().identity().id()),
        parsed,
        diagnostics(parsed),
        Arc::clone(slots.snapshot()),
        empty_arenas(slots.snapshot()),
        Box::new([]),
        HirDeclarationMemberIndexBuilder::new(module).freeze(),
        HirSourceIndex::empty(parsed.document().identity().clone(), slots.snapshot()),
        NonZeroU64::MIN,
    )
}

struct RecoveryModuleFixture {
    parsed: ParsedSource,
    owner: HirModuleId,
    slots: Arc<SlotSnapshot>,
    arenas: HirModuleArenas,
    source_components: HirSourceIndex,
    diagnostics: Arc<[HirDiagnostic]>,
    root: ExprId,
    scope: ScopeId,
    children: Vec<ExprId>,
    child_sites: Vec<HirSourceSite>,
    root_site: HirSourceSite,
}

impl RecoveryModuleFixture {
    fn build(self) -> Result<HirModule, HirLowerFailure> {
        let Self {
            parsed,
            owner,
            slots,
            arenas,
            source_components,
            diagnostics,
            ..
        } = self;
        HirModule::try_new(
            snapshot(owner),
            key(parsed.document().identity().id()),
            &parsed,
            diagnostics,
            slots,
            arenas,
            Box::new([]),
            HirDeclarationMemberIndexBuilder::new(owner).freeze(),
            source_components,
            NonZeroU64::MIN,
        )
    }

    fn with_diagnostics(mut self, diagnostics: Vec<HirDiagnostic>) -> Self {
        self.diagnostics = diagnostics.into();
        self
    }
}

fn recovery_fixture(
    owner: HirModuleId,
    child_count: usize,
    add_statement_child: bool,
) -> RecoveryModuleFixture {
    recovery_fixture_with_parent(
        owner,
        child_count,
        add_statement_child,
        RecoveryParentEvidence::Exact,
    )
}

#[derive(Clone, Copy)]
enum RecoveryParentEvidence {
    Exact,
    Orphan,
    OneOver,
    Clean,
}

#[allow(
    clippy::too_many_lines,
    reason = "the fixture atomically constructs the complete synthetic recovery-owner evidence matrix"
)]
fn recovery_fixture_with_parent(
    owner: HirModuleId,
    child_count: usize,
    add_statement_child: bool,
    parent_evidence: RecoveryParentEvidence,
) -> RecoveryModuleFixture {
    assert!(child_count <= HirLimit::SyntheticDescendantsPerOwner.maximum());
    let parsed = parsed(
        &format!("arcweft-test://proof/module-recovery-{}", owner.slot()),
        if add_statement_child {
            "fn recovery() { value; }\n"
        } else {
            "\n"
        },
    );
    assert!(parsed.diagnostics().is_empty());
    let (root_syntax, scope_site, root_site, child_site) = recovery_source_sites(&parsed);

    let mut slots = StagedSlotTransaction::new(owner, HirRevision::INITIAL);
    let mut scopes = StagedArena::<HirScope, ScopeId>::new();
    let scope = scopes
        .allocate_source(
            &mut slots,
            root_syntax,
            scope_site,
            HirScope::try_new(
                owner,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(owner),
                Box::new([]),
                Box::new([]),
            )
            .expect("module scope has no foreign children"),
        )
        .expect("module scope allocation");

    let mut expressions = StagedArena::<HirExpr, ExprId>::new();
    let root_key = SyntheticKey::try_new(
        SyntheticOwner::Scope(scope),
        SyntheticRole::MissingRequiredTail,
        0,
    )
    .expect("one missing required tail belongs to the recovery scope");
    let root_reservation = expressions
        .reserve_synthetic(&mut slots, root_key, root_site.clone())
        .expect("synthetic missing-tail recovery parent reservation");
    let root = root_reservation.id();

    let (mut children, mut child_sites) = allocate_recovery_children(
        &mut slots,
        &mut expressions,
        root,
        scope,
        &child_site,
        child_count,
        u32::from(matches!(parent_evidence, RecoveryParentEvidence::OneOver)),
    );
    let parent_children = if matches!(parent_evidence, RecoveryParentEvidence::Orphan) {
        Box::default()
    } else {
        children.clone().into_boxed_slice()
    };
    let parent_state = match (parent_evidence, child_count) {
        (RecoveryParentEvidence::Clean, _) => HirPoisonState::Clean,
        (_, 0) => HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
        (_, _) => HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Recovery,
        }),
    };
    expressions
        .finalize(
            &mut slots,
            root_reservation,
            HirExpr::try_new(
                scope,
                HirExprKind::Tuple(HirTupleExpr::new(parent_children)),
                parent_state,
            )
            .expect("recovery parent retains its configured ordered child payload"),
        )
        .expect("synthetic missing-tail recovery parent finalization");

    let mut statements = StagedArena::<HirStmt, StmtId>::new();
    if add_statement_child {
        let (child, diagnostic_site) = allocate_attached_statement_recovery_child(
            &parsed,
            &mut slots,
            &mut statements,
            &mut expressions,
            &child_site,
            scope,
        );
        children.push(child);
        child_sites.push(diagnostic_site);
    }

    let scopes = scopes
        .into_snapshot(&mut slots)
        .expect("scope arena freeze");
    let expressions = expressions
        .into_snapshot(&mut slots)
        .expect("expression arena freeze");
    let statements = statements
        .into_snapshot(&mut slots)
        .expect("statement arena freeze");
    let prepared = slots.prepare().expect("slot transaction prepare");
    let slots = Arc::clone(prepared.snapshot());
    let arenas = HirModuleArenas::try_new(
        &slots,
        HirModuleArenaParts {
            items: ArenaSnapshot::empty(&slots),
            scopes,
            locals: ArenaSnapshot::empty(&slots),
            expressions,
            statements,
            types: ArenaSnapshot::empty(&slots),
            patterns: ArenaSnapshot::empty(&slots),
            captures: ArenaSnapshot::empty(&slots),
        },
    )
    .expect("all recovery slots have exact arena payload coverage");
    let source_components = HirSourceIndex::empty(parsed.document().identity().clone(), &slots);
    let diagnostics = recovery_diagnostics(&children, &child_sites);

    RecoveryModuleFixture {
        parsed,
        owner,
        slots,
        arenas,
        source_components,
        diagnostics,
        root,
        scope,
        children,
        child_sites,
        root_site,
    }
}

fn first_statement(parsed: &ParsedSource) -> StatementNode {
    let item = parsed
        .items()
        .expect("recovery fixture item inventory")
        .into_iter()
        .next()
        .expect("recovery fixture function");
    let Some(DeclarationBodyNode::Body(body)) = item.body().expect("function body access") else {
        panic!("recovery fixture function must have a body");
    };
    body.cast::<FunctionBodyKind>()
        .expect("ordinary function body")
        .block()
        .expect("ordinary function block")
        .statements()
        .expect("ordinary function statements")
        .into_iter()
        .next()
        .expect("recovery fixture statement")
}

fn allocate_attached_statement_recovery_child(
    parsed: &ParsedSource,
    slots: &mut StagedSlotTransaction,
    statements: &mut StagedArena<HirStmt, StmtId>,
    expressions: &mut StagedArena<HirExpr, ExprId>,
    child_site: &HirSourceSite,
    scope: ScopeId,
) -> (ExprId, HirSourceSite) {
    let attached = first_statement(parsed);
    let statement_site = HirSourceSite::Span(attached.source_span());
    let child = allocate_statement_recovery_child(
        slots,
        statements,
        expressions,
        attached.id(),
        &statement_site,
        child_site,
        scope,
    );
    (child, child_site.clone())
}

fn recovery_source_sites(
    parsed: &ParsedSource,
) -> (SyntaxNodeId, HirSourceSite, HirSourceSite, HirSourceSite) {
    let attached_root = parsed.root_syntax();
    let scope = HirSourceSite::Span(attached_root.source_span().clone());
    let root = HirSourceSite::Insertion(
        HirInsertionPoint::try_new(parsed.document(), 0)
            .expect("source owns its leading missing-tail insertion"),
    );
    let child = HirSourceSite::Insertion(
        HirInsertionPoint::try_new(parsed.document(), parsed.document().text().len())
            .expect("source owns its trailing recovery insertion"),
    );
    (attached_root.id(), scope, root, child)
}

fn allocate_recovery_children(
    slots: &mut StagedSlotTransaction,
    expressions: &mut StagedArena<HirExpr, ExprId>,
    root: ExprId,
    scope: ScopeId,
    child_site: &HirSourceSite,
    count: usize,
    first_ordinal: u32,
) -> (Vec<ExprId>, Vec<HirSourceSite>) {
    let mut children = Vec::with_capacity(count);
    let mut sites = Vec::with_capacity(count);
    for ordinal in 0..count {
        let ordinal = first_ordinal
            .checked_add(u32::try_from(ordinal).expect("diagnostic boundary fits u32"))
            .expect("recovery fixture ordinal stays within u32");
        let key = SyntheticKey::try_new(
            SyntheticOwner::Expr(root),
            SyntheticRole::RecoveryOperand,
            ordinal,
        )
        .expect("recovery operand ordinal is within its accepted boundary");
        let child = expressions
            .allocate_synthetic(
                slots,
                key,
                child_site.clone(),
                poisoned_expression(
                    scope,
                    HirRecoveryIssue::MissingOperand {
                        role: HirExprSourceRole::Recovery,
                    },
                ),
            )
            .expect("synthetic recovery child allocation");
        children.push(child);
        sites.push(child_site.clone());
    }
    (children, sites)
}

fn allocate_statement_recovery_child(
    slots: &mut StagedSlotTransaction,
    statements: &mut StagedArena<HirStmt, StmtId>,
    expressions: &mut StagedArena<HirExpr, ExprId>,
    syntax: SyntaxNodeId,
    root_site: &HirSourceSite,
    child_site: &HirSourceSite,
    scope: ScopeId,
) -> ExprId {
    let statement = statements
        .allocate_source(
            slots,
            syntax,
            root_site.clone(),
            HirStmt::try_new(scope, HirStmtKind::Error)
                .expect("error statement stays in its module scope"),
        )
        .expect("source-backed recovery statement allocation");
    let key = SyntheticKey::try_new(
        SyntheticOwner::Stmt(statement),
        SyntheticRole::RecoveryOperand,
        0,
    )
    .expect("statement recovery operand zero is admitted");
    expressions
        .allocate_synthetic(
            slots,
            key,
            child_site.clone(),
            poisoned_expression(
                scope,
                HirRecoveryIssue::MissingOperand {
                    role: HirExprSourceRole::Recovery,
                },
            ),
        )
        .expect("statement recovery child allocation")
}

fn poisoned_expression(scope: ScopeId, issue: HirRecoveryIssue) -> HirExpr {
    HirExpr::try_new(
        scope,
        HirExprKind::Error(HirExprError::new(
            HirGenericExprIssue::TransactionalChildFailure,
        )),
        HirPoisonState::Poisoned(issue),
    )
    .expect("typed recovery expression is poisoned")
}

fn recovery_diagnostics(children: &[ExprId], sites: &[HirSourceSite]) -> Arc<[HirDiagnostic]> {
    children
        .iter()
        .copied()
        .zip(sites.iter().cloned())
        .map(|(owner, site)| {
            HirDiagnostic::Recovery(HirRecoveryDiagnostic::new(
                SyntheticOwner::Expr(owner),
                HirRecoveryPrimary::query(HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::Whole,
                }),
                site,
            ))
        })
        .collect::<Vec<_>>()
        .into()
}

#[test]
fn clean_module_retains_the_exact_source_and_all_eight_empty_arenas() {
    let parsed = parsed("arcweft-test://proof/module-clean", "fn main() {}\n");
    let module = build(&parsed, module(1)).unwrap();

    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(module.is_executable());
    assert!(module.is_cache_eligible());
    assert!(Arc::ptr_eq(
        module.provenance().document(),
        parsed.document_lease()
    ));
    assert_eq!(module.provenance().syntax_snapshot(), parsed.snapshot_id());
    assert_eq!(
        module.provenance().source_snapshot(),
        parsed.source_snapshot_id()
    );
    assert!(module.diagnostics().is_empty());
}

#[test]
fn parser_recovery_and_its_exact_diagnostics_make_the_module_non_executable() {
    let parsed = parsed("arcweft-test://proof/module-recovered", "fn {\n");
    assert!(!parsed.diagnostics().is_empty());
    let module = build(&parsed, module(2)).unwrap();

    assert_eq!(module.status(), HirModuleStatus::Recovered);
    assert!(!module.is_executable());
    assert!(!module.is_cache_eligible());
    assert_eq!(module.diagnostics().len(), parsed.diagnostics().len());
}

#[test]
fn diagnostic_limit_accepts_exact_and_rejects_one_over() {
    let limit = HirLimit::Diagnostics;
    let maximum = limit.maximum();
    assert_eq!(validate_diagnostic_limit(maximum), Ok(()));
    assert_eq!(
        validate_diagnostic_limit(maximum + 1),
        Err(HirLowerFailure::Limit(HirLimitError::with_maximum(
            limit,
            maximum + 1,
            maximum,
        )))
    );
}

#[test]
fn missing_parser_diagnostics_are_rejected_before_module_publication() {
    let parsed = parsed("arcweft-test://proof/module-diagnostic", "fn {\n");
    let owner = module(3);
    let snapshot = snapshot(owner);
    let slots = StagedSlotTransaction::new(owner, HirRevision::INITIAL)
        .prepare()
        .unwrap();
    let result = HirModule::try_new(
        snapshot,
        key(parsed.document().identity().id()),
        &parsed,
        Arc::from([]),
        Arc::clone(slots.snapshot()),
        empty_arenas(slots.snapshot()),
        Box::new([]),
        HirDeclarationMemberIndexBuilder::new(owner).freeze(),
        HirSourceIndex::empty(parsed.document().identity().clone(), slots.snapshot()),
        NonZeroU64::MIN,
    );

    assert!(matches!(
        result,
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
}

#[test]
fn foreign_document_key_is_rejected_as_provenance_not_rebased() {
    let parsed = parsed("arcweft-test://proof/module-source", "fn main() {}\n");
    let owner = module(4);
    let snapshot = snapshot(owner);
    let slots = StagedSlotTransaction::new(owner, HirRevision::INITIAL)
        .prepare()
        .unwrap();
    let foreign = SourceDocumentId::try_new("arcweft-test://proof/foreign").unwrap();
    let result = HirModule::try_new(
        snapshot,
        key(&foreign),
        &parsed,
        diagnostics(&parsed),
        Arc::clone(slots.snapshot()),
        empty_arenas(slots.snapshot()),
        Box::new([]),
        HirDeclarationMemberIndexBuilder::new(owner).freeze(),
        HirSourceIndex::empty(parsed.document().identity().clone(), slots.snapshot()),
        NonZeroU64::MIN,
    );

    assert!(matches!(
        result,
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleProvenance
        ))
    ));
}

#[test]
fn arena_bundle_rejects_an_equal_snapshot_frozen_by_another_transaction() {
    let owner = module(5);
    let first = StagedSlotTransaction::new(owner, HirRevision::INITIAL)
        .prepare()
        .unwrap();
    let second = StagedSlotTransaction::new(owner, HirRevision::INITIAL)
        .prepare()
        .unwrap();
    let result = HirModuleArenas::try_new(
        second.snapshot(),
        HirModuleArenaParts {
            items: ArenaSnapshot::empty(first.snapshot()),
            scopes: ArenaSnapshot::empty(first.snapshot()),
            locals: ArenaSnapshot::empty(first.snapshot()),
            expressions: ArenaSnapshot::empty(first.snapshot()),
            statements: ArenaSnapshot::empty(first.snapshot()),
            types: ArenaSnapshot::empty(first.snapshot()),
            patterns: ArenaSnapshot::empty(first.snapshot()),
            captures: ArenaSnapshot::empty(first.snapshot()),
        },
    );

    assert!(matches!(
        result,
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
}

#[test]
fn arena_bundle_requires_payload_coverage_for_every_prepared_live_slot() {
    let parsed = parsed(
        "arcweft-test://proof/module-arena-coverage",
        "fn main() {}\n",
    );
    let owner = module(6);
    let mut transaction = StagedSlotTransaction::new(owner, HirRevision::INITIAL);
    transaction
        .reserve_source::<ExprId>(
            parsed.root_syntax().id(),
            HirSourceSite::Span(parsed.document().span(SourceRange::new(0, 0)).unwrap()),
            false,
        )
        .unwrap();
    let slots = transaction.prepare().unwrap();

    let result = HirModuleArenas::try_new(
        slots.snapshot(),
        HirModuleArenaParts {
            items: ArenaSnapshot::empty(slots.snapshot()),
            scopes: ArenaSnapshot::empty(slots.snapshot()),
            locals: ArenaSnapshot::empty(slots.snapshot()),
            expressions: ArenaSnapshot::empty(slots.snapshot()),
            statements: ArenaSnapshot::empty(slots.snapshot()),
            types: ArenaSnapshot::empty(slots.snapshot()),
            patterns: ArenaSnapshot::empty(slots.snapshot()),
            captures: ArenaSnapshot::empty(slots.snapshot()),
        },
    );
    assert!(matches!(
        result,
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleArenaSnapshot
        ))
    ));
}

#[test]
fn recovery_child_is_the_only_diagnostic_owner_for_its_poisoned_parent_event() {
    let fixture = recovery_fixture(module(7), 1, false);
    assert_eq!(fixture.diagnostics.len(), 1);

    let module = fixture.build().expect("one terminal recovery diagnostic");
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    assert_eq!(module.diagnostics().len(), 1);
    assert!(!module.is_executable());
    assert!(!module.is_cache_eligible());
}

#[test]
fn recovery_operand_requires_exact_poisoned_parent_payload_reachability() {
    for (owner, evidence) in [
        (module(17), RecoveryParentEvidence::Orphan),
        (module(18), RecoveryParentEvidence::OneOver),
        (module(19), RecoveryParentEvidence::Clean),
    ] {
        assert!(matches!(
            recovery_fixture_with_parent(owner, 1, false, evidence).build(),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ));
    }
}

#[test]
fn multiple_recovery_children_each_require_one_diagnostic() {
    let fixture = recovery_fixture(module(8), 2, false);
    assert_eq!(fixture.diagnostics.len(), 2);
    let accepted_module = fixture
        .build()
        .expect("each terminal recovery child has one diagnostic");
    assert_eq!(accepted_module.diagnostics().len(), 2);

    let fixture = recovery_fixture(module(9), 2, false);
    let diagnostics = fixture
        .diagnostics
        .iter()
        .take(1)
        .cloned()
        .collect::<Vec<_>>();
    assert!(matches!(
        fixture.with_diagnostics(diagnostics).build(),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
}

#[test]
fn duplicate_or_clean_owner_recovery_diagnostics_are_rejected() {
    let fixture = recovery_fixture(module(10), 1, false);
    let diagnostic = fixture.diagnostics[0].clone();
    assert!(matches!(
        fixture
            .with_diagnostics(vec![diagnostic.clone(), diagnostic])
            .build(),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));

    let fixture = recovery_fixture(module(11), 1, false);
    let clean = HirDiagnostic::Recovery(HirRecoveryDiagnostic::new(
        SyntheticOwner::Scope(fixture.scope),
        HirRecoveryPrimary::owner_whole(SyntheticOwner::Scope(fixture.scope)),
        fixture.root_site.clone(),
    ));
    assert!(matches!(
        fixture.with_diagnostics(vec![clean]).build(),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
}

#[test]
fn recovery_diagnostic_rejects_wrong_typed_primary_owner_role_and_site() {
    let fixture = recovery_fixture(module(12), 1, false);
    let child = fixture.children[0];
    let wrong_owner = HirDiagnostic::Recovery(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(child),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner: fixture.root,
            role: HirExprSourceRole::Whole,
        }),
        fixture.child_sites[0].clone(),
    ));
    assert!(matches!(
        fixture.with_diagnostics(vec![wrong_owner]).build(),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));

    let fixture = recovery_fixture(module(13), 1, false);
    let child = fixture.children[0];
    let wrong_role = HirDiagnostic::Recovery(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(child),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner: child,
            role: HirExprSourceRole::Recovery,
        }),
        fixture.child_sites[0].clone(),
    ));
    assert!(matches!(
        fixture.with_diagnostics(vec![wrong_role]).build(),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));

    let fixture = recovery_fixture(module(14), 1, false);
    let child = fixture.children[0];
    let wrong_site = HirDiagnostic::Recovery(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(child),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner: child,
            role: HirExprSourceRole::Whole,
        }),
        fixture.root_site.clone(),
    ));
    assert!(matches!(
        fixture.with_diagnostics(vec![wrong_site]).build(),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
}

#[test]
fn hir_diagnostic_limit_is_inclusive_and_atomic() {
    let maximum = HirLimit::Diagnostics.maximum();
    let fixture = recovery_fixture(module(15), maximum, false);
    let slots = Arc::clone(&fixture.slots);
    let accepted_module = fixture
        .build()
        .expect("the exact diagnostic limit freezes one complete module");
    assert_eq!(accepted_module.diagnostics().len(), maximum);
    assert_eq!(slots.committed_slot_count(), 0);

    let fixture = recovery_fixture(module(16), maximum, true);
    let slots = Arc::clone(&fixture.slots);
    assert_eq!(fixture.diagnostics.len(), maximum + 1);
    let Err(error) = fixture.build() else {
        panic!("one-over must reject module freeze");
    };
    let HirLowerFailure::Limit(error) = error else {
        panic!("expected diagnostic limit failure, got {error:?}");
    };
    assert_eq!(error.limit(), HirLimit::Diagnostics);
    assert_eq!(error.observed(), maximum + 1);
    assert_eq!(error.maximum(), maximum);
    assert_eq!(slots.committed_slot_count(), 0);
}
