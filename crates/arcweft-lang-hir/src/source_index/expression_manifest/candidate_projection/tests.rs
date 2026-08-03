use super::candidate_role_map;

use std::sync::Arc;

use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::node::{FunctionBodyKind, LetStatementKind};
use arcweft_lang_syntax::attachment::{
    AttachedExpressionNode, DeclarationBodyNode, LetInitializerNode,
};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::database::HirDatabase;
use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::HirExprKind;
use crate::final_lowering::StagedHirModuleTransaction;
use crate::identity::{ExprId, LocalGeneration, LocalId, ScopeId};
use crate::leaf::HirName;
use crate::lower::{HirInvariantFailure, HirLowerFailure, HirModuleKey, LoweringRequest};
use crate::scope::{HirLocal, HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::HirSourceSite;
use crate::stmt::{HirStmt, HirStmtKind};
use crate::symbol::CallablePackageId;

#[test]
fn candidate_role_map_preserves_distinct_roles() {
    let roles =
        candidate_role_map([(2_u8, "right"), (1_u8, "left")]).expect("distinct candidate roles");

    assert_eq!(roles.get(&1), Some(&"left"));
    assert_eq!(roles.get(&2), Some(&"right"));
}

#[test]
fn candidate_role_map_rejects_duplicate_roles_without_overwrite() {
    assert!(candidate_role_map([(1_u8, "first"), (1_u8, "second")]).is_none());
}

#[test]
fn candidate_freeze_rejects_closure_local_name_substitution() {
    assert_candidate_local_freeze_rejects("candidate-local-name", |payload| {
        HirLocal::try_new(
            payload.scope(),
            payload.kind(),
            HirName::try_new("renamed".into()).expect("test Local name"),
            payload.generation(),
            payload.pattern(),
            payload.annotation(),
            payload.is_mutable_binding(),
            payload.is_poisoned(),
        )
        .expect("same-module Local replacement")
    });
}

#[test]
fn candidate_freeze_rejects_closure_local_generation_substitution() {
    assert_candidate_local_freeze_rejects("candidate-local-generation", |payload| {
        HirLocal::try_new(
            payload.scope(),
            payload.kind(),
            payload.name().clone(),
            LocalGeneration::try_new(2).expect("second Local generation"),
            payload.pattern(),
            payload.annotation(),
            payload.is_mutable_binding(),
            payload.is_poisoned(),
        )
        .expect("same-module Local replacement")
    });
}

#[test]
fn candidate_freeze_rejects_assertion_condition_reordering() {
    let parsed = parsed_source_with_expression(
        "candidate-assertion-condition-order",
        "items[{ assert.check(first, second); marker }]",
    );
    let attached = attached_expression(&parsed);
    let mut database = HirDatabase::try_new().expect("candidate freeze database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("candidate assertion lowers before tamper");
    let statement = candidate_block_statement(&mut transaction, root);
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .resolve_staged(slots, statement)
            .expect("candidate assertion payload")
            .clone()
    };
    let HirStmtKind::Assertion { mode, conditions } = retained.kind() else {
        panic!("candidate statement must remain an assertion");
    };
    let [first, second] = conditions.as_ref() else {
        panic!("candidate assertion must retain two conditions");
    };
    let replacement = HirStmt::try_new_with_state(
        retained.scope(),
        HirStmtKind::Assertion {
            mode: *mode,
            conditions: Box::new([*second, *first]),
        },
        retained.state().clone(),
    )
    .expect("same-module reordered assertion");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .revise_finalized(slots, statement, replacement)
            .expect("test-only candidate assertion substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn candidate_freeze_rejects_assignment_operand_reordering() {
    let parsed = parsed_source_with_expression(
        "candidate-assignment-operand-order",
        "items[{ marker; target = value; marker }]",
    );
    let attached = attached_expression(&parsed);
    let mut database = HirDatabase::try_new().expect("candidate freeze database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("candidate assignment lowers before tamper");
    let statement = candidate_block_statement_at(&mut transaction, root, 1);
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .resolve_staged(slots, statement)
            .expect("candidate assignment payload")
            .clone()
    };
    let HirStmtKind::Assign { target, value } = retained.kind() else {
        panic!("candidate statement must remain an assignment");
    };
    let replacement = HirStmt::try_new_with_state(
        retained.scope(),
        HirStmtKind::Assign {
            target: *value,
            value: *target,
        },
        retained.state().clone(),
    )
    .expect("same-module reordered assignment");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .revise_finalized(slots, statement, replacement)
            .expect("test-only candidate assignment substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn candidate_freeze_rejects_required_operand_family_substitution() {
    let parsed = parsed_source_with_expression(
        "candidate-required-operand-family",
        "items[{ marker; return value; marker }]",
    );
    let attached = attached_expression(&parsed);
    let mut database = HirDatabase::try_new().expect("candidate freeze database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("candidate Return lowers before tamper");
    let statement = candidate_block_statement_at(&mut transaction, root, 1);
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .resolve_staged(slots, statement)
            .expect("candidate Return payload")
            .clone()
    };
    let HirStmtKind::Return { value } = retained.kind() else {
        panic!("candidate statement must remain Return");
    };
    let replacement = HirStmt::try_new_with_state(
        retained.scope(),
        HirStmtKind::Yield { expression: *value },
        retained.state().clone(),
    )
    .expect("same-module required-operand family substitution");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .revise_finalized(slots, statement, replacement)
            .expect("test-only required-operand family substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn candidate_freeze_rejects_signal_operand_reordering() {
    assert_candidate_keyword_freeze_rejects(
        "candidate-signal-operand-order",
        "items[{ signal target <- value; marker }]",
        |retained| {
            let HirStmtKind::Signal { target, value } = retained.kind() else {
                panic!("candidate statement must remain Signal");
            };
            HirStmt::try_new_with_state(
                retained.scope(),
                HirStmtKind::Signal {
                    target: *value,
                    value: *target,
                },
                retained.state().clone(),
            )
            .expect("same-module reordered candidate Signal")
        },
    );
}

#[test]
fn candidate_freeze_rejects_defer_outcome_substitution() {
    assert_candidate_keyword_freeze_rejects(
        "candidate-defer-outcome",
        "items[{ defer cleanup(); marker }]",
        |retained| {
            let HirStmtKind::Defer { expression, .. } = retained.kind() else {
                panic!("candidate statement must remain Defer");
            };
            HirStmt::try_new_with_state(
                retained.scope(),
                HirStmtKind::Defer {
                    outcome: DeferOutcome::Completed,
                    expression: *expression,
                },
                retained.state().clone(),
            )
            .expect("same-module candidate Defer outcome substitution")
        },
    );
}

fn assert_candidate_keyword_freeze_rejects(
    document_id: &str,
    expression: &str,
    replacement: impl FnOnce(&HirStmt) -> HirStmt,
) {
    let parsed = parsed_source_with_expression(document_id, expression);
    let attached = attached_expression(&parsed);
    let mut database = HirDatabase::try_new().expect("candidate freeze database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("candidate keyword statement lowers before tamper");
    let statement = candidate_block_statement(&mut transaction, root);
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .resolve_staged(slots, statement)
            .expect("candidate keyword statement payload")
            .clone()
    };
    let replacement = replacement(&retained);
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .revise_finalized(slots, statement, replacement)
            .expect("test-only candidate keyword substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

fn assert_candidate_local_freeze_rejects(
    document_id: &str,
    replacement: impl FnOnce(&HirLocal) -> HirLocal,
) {
    let parsed = parsed_source(document_id);
    let attached = attached_expression(&parsed);
    let mut database = HirDatabase::try_new().expect("candidate freeze database");
    let mut transaction = stage(&database, &parsed);
    let module_scope = allocate_module_scope(&mut transaction, &parsed);
    let root = transaction
        .lower_attached_expression(&attached, module_scope)
        .expect("candidate closure lowers before tamper");
    let local = candidate_closure_local(&mut transaction, root);
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .locals()
            .resolve_staged(slots, local)
            .expect("candidate Local payload")
            .clone()
    };
    let replacement = replacement(&retained);
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .locals()
            .revise_finalized(slots, local, replacement)
            .expect("test-only candidate Local substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());
}

fn candidate_closure_local(
    transaction: &mut StagedHirModuleTransaction<'_>,
    root: ExprId,
) -> LocalId {
    let (slots, arenas) = transaction.storage_mut();
    let index = {
        let root = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("candidate postfix root");
        let HirExprKind::PostfixBracket(postfix) = root.kind() else {
            panic!("fixture must retain ambiguous E34 postfix")
        };
        let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
            panic!("fixture must retain index interpretation")
        };
        *index
    };
    let closure = {
        let index = arenas
            .expressions()
            .resolve_staged(slots, index)
            .expect("candidate Index root");
        let HirExprKind::Index(index) = index.kind() else {
            panic!("ordinary interpretation must be Index")
        };
        index.index()
    };
    let closure_scope = {
        let closure = arenas
            .expressions()
            .resolve_staged(slots, closure)
            .expect("candidate Closure expression");
        let HirExprKind::Closure(closure) = closure.kind() else {
            panic!("candidate primary must be Closure")
        };
        closure.scope()
    };
    let scope = arenas
        .scopes()
        .resolve_staged(slots, closure_scope)
        .expect("candidate Closure scope");
    let [local] = scope.locals() else {
        panic!("candidate Closure must retain one Local")
    };
    *local
}

fn candidate_block_statement(
    transaction: &mut StagedHirModuleTransaction<'_>,
    root: ExprId,
) -> crate::identity::StmtId {
    candidate_block_statement_at(transaction, root, 0)
}

fn candidate_block_statement_at(
    transaction: &mut StagedHirModuleTransaction<'_>,
    root: ExprId,
    ordinal: usize,
) -> crate::identity::StmtId {
    let (slots, arenas) = transaction.storage_mut();
    let index = {
        let root = arenas
            .expressions()
            .resolve_staged(slots, root)
            .expect("candidate postfix root");
        let HirExprKind::PostfixBracket(postfix) = root.kind() else {
            panic!("fixture must retain ambiguous E34 postfix")
        };
        let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
            panic!("fixture must retain index interpretation")
        };
        *index
    };
    let block = {
        let index = arenas
            .expressions()
            .resolve_staged(slots, index)
            .expect("candidate Index root");
        let HirExprKind::Index(index) = index.kind() else {
            panic!("ordinary interpretation must be Index")
        };
        index.index()
    };
    let block = arenas
        .expressions()
        .resolve_staged(slots, block)
        .expect("candidate Block expression");
    let HirExprKind::Block(block) = block.kind() else {
        panic!("candidate primary must be a Block")
    };
    *block
        .statements()
        .get(ordinal)
        .expect("candidate Block statement ordinal")
}

fn parsed_source(document_id: &str) -> ParsedSource {
    parsed_source_with_expression(document_id, "items[|value: Pair| value]")
}

fn parsed_source_with_expression(document_id: &str, expression: &str) -> ParsedSource {
    let name = SourceName::path(format!("proof/candidate-freeze/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/candidate-freeze/{document_id}.arcw"
            ))
            .expect("candidate freeze document ID"),
            name.clone(),
            format!("fn candidate_freeze() {{\n    let value = {expression};\n}}\n"),
        )
        .expect("candidate freeze source"),
    );
    SyntaxDatabase::try_new()
        .expect("candidate freeze syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("candidate freeze source parses")
}

fn attached_expression(parsed: &ParsedSource) -> AttachedExpressionNode {
    let item = parsed
        .tree()
        .items()
        .expect("source item inventory")
        .into_iter()
        .next()
        .expect("candidate freeze function");
    let Some(DeclarationBodyNode::Body(body)) = item.body().expect("function body") else {
        panic!("candidate freeze function must have a body")
    };
    let statement = body
        .cast::<FunctionBodyKind>()
        .expect("function body family")
        .block()
        .expect("function computation block")
        .statements()
        .expect("function statements")
        .into_iter()
        .next()
        .expect("candidate freeze Let");
    let initializer = match statement
        .cast::<LetStatementKind>()
        .expect("Let statement")
        .initializer()
        .expect("initializer access")
        .expect("authored initializer")
    {
        LetInitializerNode::Expression(expression) => expression,
        LetInitializerNode::Missing(_) => panic!("candidate fixture initializer is authored"),
    };
    initializer
        .semantic()
        .expect("attached candidate expression")
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("candidate-freeze-tests").expect("package ID"),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().id().clone(),
    )
}

fn stage<'source>(
    database: &HirDatabase,
    parsed: &'source ParsedSource,
) -> StagedHirModuleTransaction<'source> {
    database
        .stage_final_hir(
            LoweringRequest::try_new(module_key(parsed), parsed).expect("lowering request"),
        )
        .expect("staged candidate module")
}

fn allocate_module_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    parsed: &ParsedSource,
) -> ScopeId {
    let module = transaction.snapshot_id().module();
    let root = parsed.root_syntax();
    let site = HirSourceSite::Span(root.source_span().clone());
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .allocate_source(
            slots,
            root.id(),
            site,
            HirScope::try_new(
                module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(module),
                Box::new([]),
                Box::new([]),
            )
            .expect("module scope"),
        )
        .expect("module scope allocation")
}
