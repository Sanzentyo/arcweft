use std::sync::Arc;

use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use crate::arena::HirArenaError;
use crate::expr::HirExprKind;
use crate::identity::{
    ExprId, HirIdKind, HirTypedId, IdResolveError, ItemId, LocalId, PatternId, StmtId,
};
use crate::item::{HirFunctionBody, HirItemKind};
use crate::module::HirModule;
use crate::slot::{HirOrigin, HirSlotError};
use crate::stmt::HirStmtKind;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LetIdentity {
    statement: StmtId,
    pattern: PatternId,
    initializer: ExprId,
    local: LocalId,
    syntax: arcweft_lang_syntax::attachment::SyntaxNodeId,
}

fn parse_initial(document_id: &str, source: &str) -> (SyntaxDatabase, ParsedSource) {
    let name = SourceName::path(format!("proof/expression-lowering/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/{document_id}.arcw"
            ))
            .expect("identity fixture document ID"),
            name.clone(),
            source,
        )
        .expect("identity fixture source"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("identity fixture syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("identity fixture initial parse");
    (syntax, parsed)
}

fn reparse_whole(
    syntax: &mut SyntaxDatabase,
    previous: &ParsedSource,
    replacement: &str,
) -> ParsedSource {
    syntax
        .reparse(
            previous,
            &[SourceEdit::new(
                previous
                    .document()
                    .span(SourceRange::new(0, previous.document().text().len()))
                    .expect("whole-document identity edit"),
                replacement,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("identity fixture reparse")
}

fn publish_expressions(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
) -> (Arc<HirModule>, Vec<ExprId>) {
    let attached = attached_expressions(parsed);
    let mut transaction = stage(database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    let roots = attached
        .iter()
        .map(|expression| {
            transaction
                .lower_attached_expression(expression, scope)
                .expect("identity fixture expression lowering")
        })
        .collect();
    let module = transaction
        .finish(database)
        .expect("identity fixture HIR publication")
        .into_module();
    (module, roots)
}

fn publish_items(database: &mut HirDatabase, parsed: &ParsedSource) -> Arc<HirModule> {
    let mut transaction = stage(database, parsed);
    transaction
        .lower_parsed_source_items(parsed)
        .expect("identity fixture item lowering");
    transaction
        .finish(database)
        .expect("identity fixture item publication")
        .into_module()
}

fn block_let(module: &HirModule, block: ExprId, ordinal: usize) -> LetIdentity {
    let HirExprKind::Block(block) = expression(module, block).kind() else {
        panic!("identity fixture initializer must remain a Block")
    };
    let statement = block.statements()[ordinal];
    let payload = module
        .arenas()
        .statements()
        .resolve(module.slots(), statement)
        .expect("identity fixture Let statement");
    let HirStmtKind::Let {
        pattern,
        initializer,
        locals,
        ..
    } = payload.kind()
    else {
        panic!("identity fixture statement must remain a Let")
    };
    let [local] = locals.as_ref() else {
        panic!("identity fixture Let must publish one local")
    };
    let metadata = module
        .slots()
        .resolve(statement)
        .expect("identity fixture statement metadata");
    let HirOrigin::Source(source) = metadata.origin() else {
        panic!("authored Let must retain a source-backed HIR origin")
    };
    LetIdentity {
        statement,
        pattern: *pattern,
        initializer: *initializer,
        local: *local,
        syntax: source.syntax(),
    }
}

fn assert_retired(module: &HirModule, statement: StmtId) {
    assert!(matches!(
        module
            .arenas()
            .statements()
            .resolve(module.slots(), statement),
        Err(HirArenaError::Slot(HirSlotError::Resolve(
            IdResolveError::Retired {
                snapshot,
                retired_at,
                ..
            }
        ))) if snapshot == module.snapshot_id()
            && retired_at == module.snapshot_id().revision()
    ));
}

fn assert_not_yet_live(module: &HirModule, statement: StmtId) {
    assert!(matches!(
        module
            .arenas()
            .statements()
            .resolve(module.slots(), statement),
        Err(HirArenaError::Slot(HirSlotError::Resolve(
            IdResolveError::NotYetLive { snapshot, born, .. }
        ))) if snapshot == module.snapshot_id()
            && born > module.snapshot_id().revision()
    ));
}

fn item_syntax(module: &HirModule, item: ItemId) -> arcweft_lang_syntax::attachment::SyntaxNodeId {
    let metadata = module
        .slots()
        .resolve(item)
        .expect("identity fixture item metadata");
    let HirOrigin::Source(source) = metadata.origin() else {
        panic!("authored item must retain a source-backed HIR origin")
    };
    source.syntax()
}

fn assert_item_not_yet_live(module: &HirModule, item: ItemId) {
    assert!(matches!(
        module.arenas().items().resolve(module.slots(), item),
        Err(HirArenaError::Slot(HirSlotError::Resolve(
            IdResolveError::NotYetLive { snapshot, born, .. }
        ))) if snapshot == module.snapshot_id()
            && born > module.snapshot_id().revision()
    ));
}

fn function_statements(module: &HirModule) -> Vec<StmtId> {
    let item = module
        .resolve_item(module.source_ordered_items()[0])
        .expect("identity fixture function item");
    let HirItemKind::Function(function) = item.kind() else {
        panic!("identity fixture must lower one ordinary Function")
    };
    let HirFunctionBody::Block { statements, .. } = function.body() else {
        panic!("identity fixture Function must retain a block body")
    };
    statements.to_vec()
}

#[test]
fn same_parent_reorder_preserves_hir_ids() {
    const INITIAL: &str = concat!(
        "fn reorder() {\n",
        "    let alpha = 1;\n",
        "    let beta = 2;\n",
        "    alpha\n",
        "}\n",
    );
    const REORDERED: &str = concat!(
        "fn reorder() {\n",
        "    let beta = 2;\n",
        "    let alpha = 1;\n",
        "    alpha\n",
        "}\n",
    );

    let (mut syntax, initial) = parse_initial("same-parent-hir-reorder", INITIAL);
    let mut database = HirDatabase::try_new().expect("identity fixture HIR database");
    let first = publish_items(&mut database, &initial);
    let first_statements = function_statements(&first);
    assert_eq!(first_statements.len(), 2);

    let revised = reparse_whole(&mut syntax, &initial, REORDERED);
    let second = publish_items(&mut database, &revised);
    let second_statements = function_statements(&second);
    assert_eq!(
        second_statements,
        [first_statements[1], first_statements[0]]
    );
    for statement in &first_statements {
        assert!(first.resolve_stmt(*statement).is_ok());
        assert!(second.resolve_stmt(*statement).is_ok());
    }
}

#[test]
fn changed_source_kind_retires_old_slot_and_allocates_new_kind() {
    const INITIAL: &str = "fn change_kind() { source }\n";
    const REVISED: &str = "fn change_kind(source: I32) { () }\n";

    let (mut syntax, initial) = parse_initial("changed-source-hir-kind", INITIAL);
    let mut database = HirDatabase::try_new().expect("identity fixture HIR database");
    let first = publish_items(&mut database, &initial);
    let first_item = first
        .resolve_item(first.source_ordered_items()[0])
        .expect("initial Function item");
    let HirItemKind::Function(first_function) = first_item.kind() else {
        panic!("identity fixture must lower one ordinary Function")
    };
    let HirFunctionBody::Block {
        tail: old_expression,
        ..
    } = first_function.body()
    else {
        panic!("initial Function must retain its authored tail")
    };
    let old_expression = *old_expression;

    let revised = reparse_whole(&mut syntax, &initial, REVISED);
    let second = publish_items(&mut database, &revised);
    let second_item = second
        .resolve_item(second.source_ordered_items()[0])
        .expect("revised Function item");
    let HirItemKind::Function(second_function) = second_item.kind() else {
        panic!("identity fixture must retain one ordinary Function")
    };
    let [group] = second_function.parameter_groups() else {
        panic!("revised Function must retain one parameter group")
    };
    let [parameter] = group.parameters() else {
        panic!("revised Function must retain one parameter")
    };
    let new_pattern = parameter.pattern();

    assert_eq!(old_expression.raw().kind(), HirIdKind::Expr);
    assert_eq!(new_pattern.raw().kind(), HirIdKind::Pattern);
    assert_ne!(old_expression.raw().slot(), new_pattern.raw().slot());
    assert!(first.resolve_expr(old_expression).is_ok());
    assert!(matches!(
        second.resolve_expr(old_expression),
        Err(IdResolveError::Retired { .. })
    ));
    assert!(second.resolve_pattern(new_pattern).is_ok());
    assert!(matches!(
        first.resolve_pattern(new_pattern),
        Err(IdResolveError::NotYetLive { .. })
    ));
}

#[test]
fn cross_parent_move_retires_and_reallocates_hir_ids() {
    const INITIAL: &str = concat!(
        "fn lower_expressions() {\n",
        "    let value_0 = { let moved = 1; moved };\n",
        "    let value_1 = { 2 };\n",
        "    let value_2 = { let stable = 3; stable };\n",
        "}\n",
    );
    const MOVED: &str = concat!(
        "fn lower_expressions() {\n",
        "    let value_0 = { 0 };\n",
        "    let value_1 = { let moved = 1; moved };\n",
        "    let value_2 = { let stable = 3; stable };\n",
        "}\n",
    );

    let (mut syntax, initial) = parse_initial("cross-parent-hir-identity", INITIAL);
    let mut database = HirDatabase::try_new().expect("identity fixture HIR database");
    let (first, first_roots) = publish_expressions(&mut database, &initial);
    let moved_before = block_let(&first, first_roots[0], 0);
    let stable_before = block_let(&first, first_roots[2], 0);

    let revised = reparse_whole(&mut syntax, &initial, MOVED);
    let (second, second_roots) = publish_expressions(&mut database, &revised);
    let moved_after = block_let(&second, second_roots[1], 0);
    let stable_after = block_let(&second, second_roots[2], 0);

    assert_ne!(moved_before.syntax, moved_after.syntax);
    assert_ne!(moved_before.statement, moved_after.statement);
    assert_ne!(moved_before.pattern, moved_after.pattern);
    assert_ne!(moved_before.initializer, moved_after.initializer);
    assert_ne!(moved_before.local, moved_after.local);
    assert_eq!(stable_before, stable_after);

    assert!(
        first
            .arenas()
            .statements()
            .resolve(first.slots(), moved_before.statement)
            .is_ok()
    );
    assert_retired(&second, moved_before.statement);
    assert_not_yet_live(&first, moved_after.statement);
}

#[test]
fn copied_source_node_gets_fresh_hir_ids() {
    const INITIAL: &str = "use crate.alpha.value\n";
    const COPIED: &str = "use crate.alpha.value\nuse crate.alpha.value\n";

    let (mut syntax, initial) = parse_initial("copied-hir-identity", INITIAL);
    let mut database = HirDatabase::try_new().expect("identity fixture HIR database");
    let first = publish_items(&mut database, &initial);
    let [original] = first.source_ordered_items() else {
        panic!("initial copy fixture must publish one item")
    };
    let original_syntax = item_syntax(&first, *original);

    let revised = reparse_whole(&mut syntax, &initial, COPIED);
    let second = publish_items(&mut database, &revised);
    let [retained, fresh] = second.source_ordered_items() else {
        panic!("copied fixture must publish two source-ordered items")
    };

    assert_eq!(*retained, *original);
    assert_eq!(item_syntax(&second, *retained), original_syntax);
    assert_ne!(*fresh, *original);
    assert_ne!(item_syntax(&second, *fresh), original_syntax);
    assert_item_not_yet_live(&first, *fresh);
}
