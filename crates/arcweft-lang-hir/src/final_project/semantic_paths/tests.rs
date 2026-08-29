use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use super::*;
use crate::database::HirDatabase;
use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirLinePlan,
    HirLinePlanItem,
};
use crate::expr::{
    HirChoiceBody, HirChoiceExpr, HirChoiceItem, HirChoiceOptionBody, HirChoiceOptionField,
    HirChoiceOptionFor, HirExprKind, HirExpressionChildOwnership, HirExpressionChildRole,
    HirExpressionOwnedBodyRole, HirLinePlanStatementRole, HirNestedExpressionPathSegment,
};
use crate::final_lowering::stage_unpublished_module_for_invariant_test;
use crate::identity::{HirDatabaseId, HirModuleId, HirTypedId, RawHirId};
use crate::lowering::{HirLoweringControl, HirModuleKey, LoweringRequest};
use crate::symbol::{CallableDeclarationId, CallableDeclarationOwner};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn lower_control_transfer_fixture(source: &str) -> Arc<HirModule> {
    let package =
        crate::symbol::CallablePackageId::try_new("control-transfer-tests").expect("test package");
    let path = CanonicalModulePath::crate_root();
    let source = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://control-transfer").expect("source ID"),
            SourceName::path("control-transfer.arcw"),
            source,
        )
        .expect("source document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(SourceName::path("control-transfer.arcw")),
            source,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("parse fixture");
    let mut database = HirDatabase::try_new().expect("HIR database");
    let key = HirModuleKey::new(package, path, parsed.document().identity().clone());
    let mut transaction = stage_unpublished_module_for_invariant_test(
        &database,
        LoweringRequest::try_new(key, &parsed).expect("lowering request"),
        HirLoweringControl::new(),
    )
    .expect("stage fixture");
    transaction
        .lower_parsed_source_items(&parsed)
        .expect("lower fixture");
    transaction
        .finish(&mut database)
        .expect("finish fixture")
        .into_module()
}

fn loop_expression_owner(module: &HirModule, scope: crate::identity::ScopeId) -> Option<ExprId> {
    let scope = module.resolve_scope(scope).ok()?;
    match scope.owner() {
        crate::scope::HirScopeOwner::Expr(owner)
            if matches!(
                module.resolve_expr(*owner).ok()?.kind(),
                HirExprKind::Loop(_)
            ) =>
        {
            Some(*owner)
        }
        _ => None,
    }
}

fn nearest_loop_expression(module: &HirModule, statement: StmtId) -> Option<ExprId> {
    let mut scope = Some(module.resolve_stmt(statement).ok()?.scope());
    while let Some(scope_id) = scope {
        let value = module.resolve_scope(scope_id).ok()?;
        if let Some(owner) = loop_expression_owner(module, scope_id) {
            return Some(owner);
        }
        scope = value.parent();
    }
    None
}

fn fixture() -> (std::sync::Arc<HirModule>, CallableDeclarationKey, ExprId) {
    let (_, package, module_path, module) =
        crate::project::tests::root_module_fixture("semantic-path-errors");
    let declaration = CallableDeclarationKey::Existing(
        CallableDeclarationId::try_new(
            package,
            module_path,
            CallableDeclarationOwner::Function,
            "accepted",
        )
        .expect("fixture declaration"),
    );
    let foreign_module = HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(2).unwrap()),
        NonZeroU32::MIN,
    );
    let missing = ExprId::from_raw(RawHirId::new(foreign_module, NonZeroU32::MIN, ExprId::KIND));
    (module, declaration, missing)
}

#[test]
fn loop_control_transfers_resolve_the_nearest_loop_expression() {
    let module = lower_control_transfer_fixture(
        "fn accepted() {\n    let result = loop {\n        loop {\n            break\n        }\n        break\n    }\n}\n",
    );
    let loops = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Loop(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert_eq!(loops.len(), 2, "nested loop fixture must retain both loops");
    let breaks = module
        .statements()
        .filter_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Break { .. }).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        breaks.len(),
        2,
        "nested loop fixture must retain both breaks"
    );

    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    let root = [HirSemanticPathStep::DeclarationBody(
        HirDeclarationBodyRootRole::FunctionBody,
    )];
    let outer = loops
        .iter()
        .copied()
        .find(|owner| {
            module
                .resolve_expr(*owner)
                .ok()
                .and_then(|expression| match expression.kind() {
                    HirExprKind::Loop(loop_expression) => module
                        .resolve_scope(loop_expression.scope())
                        .ok()
                        .and_then(crate::scope::HirScope::parent)
                        .and_then(|parent| loop_expression_owner(&module, parent)),
                    _ => None,
                })
                .is_none()
        })
        .expect("outer loop expression");
    builder
        .walk_expression(outer, &root, &[], None, CaptureAccess::Read)
        .expect("nested loop topology");

    assert_eq!(builder.control_transfers.len(), 2);
    for statement in breaks {
        let expected = nearest_loop_expression(&module, statement).expect("nearest loop");
        let row = builder
            .control_transfers
            .get(&statement)
            .expect("break transfer row");
        assert_eq!(row.kind(), HirControlTransferKind::Break);
        assert_eq!(
            row.target(),
            &HirControlTransferTarget::Loop {
                family: HirLoopTargetFamily::LoopExpression,
                body_owner: HirSemanticBodyOwner::direct_expression(expected),
            }
        );
    }
}

#[test]
fn break_value_rejects_statement_loop_targets() {
    let module = lower_control_transfer_fixture(
        "flow accepted() {\n    while true {\n        break 1\n    }\n}\n",
    );
    let statement = module
        .statements()
        .find_map(|(owner, value)| {
            matches!(value.kind(), HirStmtKind::Break { value: Some(_), .. }).then_some(owner)
        })
        .expect("break value statement");
    let kind = module
        .resolve_stmt(statement)
        .expect("break statement")
        .kind();
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    assert_eq!(
        builder.record_control_transfer(statement, kind),
        Err(HirSemanticPathError::ControlTransfer(
            HirControlTransferResolutionError::BreakValueRequiresLoopExpression { statement },
        ))
    );
}

#[test]
fn statement_loop_control_transfers_resolve_each_statement_loop_family() {
    let module = lower_control_transfer_fixture(
        "flow accepted() {\n    while ready {\n        continue\n    }\n    while let value = source {\n        break\n    }\n    for value in source {\n        continue\n    }\n}\n",
    );
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    let expected = [
        (
            HirControlTransferKind::Continue,
            HirLoopTargetFamily::WhileStatement,
            HirStatementBodyRole::While,
        ),
        (
            HirControlTransferKind::Break,
            HirLoopTargetFamily::WhileLetStatement,
            HirStatementBodyRole::WhileLet,
        ),
        (
            HirControlTransferKind::Continue,
            HirLoopTargetFamily::ForStatement,
            HirStatementBodyRole::For,
        ),
    ];
    let mut transfers = module
        .statements()
        .filter_map(|(owner, value)| match value.kind() {
            HirStmtKind::Break { .. } => Some((owner, HirControlTransferKind::Break)),
            HirStmtKind::Continue { .. } => Some((owner, HirControlTransferKind::Continue)),
            _ => None,
        })
        .collect::<Vec<_>>();
    transfers.sort_by_key(|(owner, _)| *owner);
    assert_eq!(transfers.len(), expected.len());
    for ((statement, kind), (_, family, role)) in transfers.into_iter().zip(expected) {
        let statement_kind = module.resolve_stmt(statement).expect("transfer").kind();
        builder
            .record_control_transfer(statement, statement_kind)
            .expect("statement-loop transfer");
        let HirControlTransferTarget::Loop {
            family: actual_family,
            body_owner,
        } = builder
            .control_transfers
            .get(&statement)
            .expect("transfer row")
            .target()
        else {
            panic!("statement loop must resolve to a loop target")
        };
        assert_eq!(*actual_family, family);
        assert_eq!(
            body_owner.semantic_role(),
            HirSemanticBodyOwnerRole::Statement(role)
        );
        assert_eq!(
            builder
                .control_transfers
                .get(&statement)
                .expect("row")
                .kind(),
            kind
        );
    }
}

#[test]
fn output_control_transfers_require_a_line_plan_and_reject_labels() {
    let module = lower_control_transfer_fixture(
        "pub character alice { display_name = \"Alice\" }\nflow accepted() -> String {\n    let (_, cue) = alice(voice=auto)[聞いて。[p]]\n    with:\n        out cue\n    return \"done\"\n}\n",
    );
    let application = module
        .expressions()
        .find_map(|(owner, value)| match value.kind() {
            HirExprKind::DialogueContentApplication(application)
                if application.plan().is_some() =>
            {
                Some(owner)
            }
            _ => None,
        })
        .expect("dialogue application with line plan");
    let output = module
        .statements()
        .find_map(|(owner, value)| matches!(value.kind(), HirStmtKind::Out { .. }).then_some(owner))
        .expect("line-plan out statement");
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    builder
        .walk_expression(
            application,
            &[HirSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FlowBody,
            )],
            &[],
            None,
            CaptureAccess::Read,
        )
        .expect("line-plan topology");
    assert_eq!(
        builder.control_transfers.get(&output),
        Some(&HirControlTransferRow::new(
            output,
            HirControlTransferKind::Out,
            HirControlTransferTarget::Output { application },
        ))
    );

    let outside = lower_control_transfer_fixture("flow accepted() { out 1 }\n");
    let output = outside
        .statements()
        .find_map(|(owner, value)| matches!(value.kind(), HirStmtKind::Out { .. }).then_some(owner))
        .expect("outside out statement");
    let kind = outside.resolve_stmt(output).expect("out statement").kind();
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&outside);
    assert_eq!(
        builder.record_control_transfer(output, kind),
        Err(HirSemanticPathError::ControlTransfer(
            HirControlTransferResolutionError::UnresolvedTarget {
                statement: output,
                kind: HirControlTransferKind::Out,
            },
        ))
    );

    let labeled = lower_control_transfer_fixture(
        "flow accepted() {\n    while true {\n        continue 'events\n    }\n}\n",
    );
    let statement = labeled
        .statements()
        .find_map(|(owner, value)| {
            matches!(value.kind(), HirStmtKind::Continue { .. }).then_some(owner)
        })
        .expect("labeled continue statement");
    let kind = labeled
        .resolve_stmt(statement)
        .expect("continue statement")
        .kind();
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&labeled);
    assert_eq!(
        builder.record_control_transfer(statement, kind),
        Err(HirSemanticPathError::ControlTransfer(
            HirControlTransferResolutionError::UnresolvedTarget {
                statement,
                kind: HirControlTransferKind::Continue,
            },
        ))
    );
}

#[test]
fn postfix_candidate_targets_are_references_only_at_the_immediate_candidate_boundary() {
    let (module, _, owner) = fixture();
    for candidate_role in [
        HirExpressionChildRole::PostfixIndexCandidate,
        HirExpressionChildRole::PostfixDialogueCandidate,
    ] {
        let immediate_candidate = [HirSemanticPathStep::Expression(candidate_role.clone())];
        assert_eq!(
            expression_edge_ownership(
                &module,
                owner,
                None,
                &immediate_candidate,
                &HirExpressionChildRole::Target,
                owner,
            )
            .unwrap(),
            HirExpressionChildOwnership::ReferenceOnly
        );
        assert_eq!(
            expression_edge_ownership(
                &module,
                owner,
                None,
                &immediate_candidate,
                &HirExpressionChildRole::DialogueTarget,
                owner,
            )
            .unwrap(),
            HirExpressionChildOwnership::ReferenceOnly
        );

        let descendant_postfix = [
            HirSemanticPathStep::Expression(candidate_role),
            HirSemanticPathStep::Expression(HirExpressionChildRole::Index),
        ];
        assert_eq!(
            expression_edge_ownership(
                &module,
                owner,
                None,
                &descendant_postfix,
                &HirExpressionChildRole::Target,
                owner,
            )
            .unwrap(),
            HirExpressionChildOwnership::Owning
        );
        assert_eq!(
            expression_edge_ownership(
                &module,
                owner,
                None,
                &descendant_postfix,
                &HirExpressionChildRole::DialogueTarget,
                owner,
            )
            .unwrap(),
            HirExpressionChildOwnership::Owning
        );
    }
}

#[test]
fn expression_walk_rejects_cycle_before_duplicate() {
    let (module, _, owner) = fixture();
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    builder
        .expressions
        .insert(owner, HirSemanticOwnerPath::new(Box::new([]), Box::new([])));
    builder.active_expressions.insert(owner);
    assert_eq!(
        builder.walk_expression(owner, &[], &[], None, CaptureAccess::Read),
        Err(HirSemanticPathError::CyclicPath {
            owner: owner.into()
        })
    );
}

#[test]
fn item_root_path_index_rejects_recovery_body_publication() {
    let (module, _, _) = fixture();
    let item = module.items().next().expect("fixture item").0;
    let expression = module.expressions().next().expect("fixture expression").0;
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    let checkpoint = builder.path_checkpoint();
    builder
        .walk_item_root(&HirItemEvaluationRoot {
            role: HirDeclarationItemRootRole::Recovery {
                owner: HirItemRecoveryRootOwner::Item,
            },
            projection: HirBodyProjection::expression(expression),
        })
        .expect("recovery item root");
    assert_eq!(
        builder.path_index_since(
            HirSemanticPathRoot::Item {
                item,
                entry_ordinal: 0,
                role: HirItemEvaluationEntryRole::Item,
            },
            &checkpoint,
        ),
        Err(HirSemanticPathError::InvalidBodyRow {
            owner: HirSemanticBodyOwner::item(HirDeclarationItemRootRole::Recovery {
                owner: HirItemRecoveryRootOwner::Item,
            }),
        })
    );
}

#[test]
fn expression_walk_rejects_duplicate_before_resolution() {
    let (module, _, owner) = fixture();
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    builder
        .expressions
        .insert(owner, HirSemanticOwnerPath::new(Box::new([]), Box::new([])));
    assert_eq!(
        builder.walk_expression(owner, &[], &[], None, CaptureAccess::Read),
        Err(HirSemanticPathError::DuplicatePath {
            owner: owner.into()
        })
    );
}

#[test]
fn insert_unique_rejects_every_second_owning_coordinate() {
    let (_, _, owner) = fixture();
    let mut rows = BTreeMap::new();
    insert_unique(&mut rows, owner, &[], &[]).expect("first owning coordinate");
    assert_eq!(
        insert_unique(
            &mut rows,
            owner,
            &[HirSemanticPathStep::DeclarationResult],
            &[],
        ),
        Err(HirSemanticPathError::DuplicatePath {
            owner: owner.into()
        })
    );
}

#[test]
fn project_lookup_rejects_every_second_location_with_the_exact_owner() {
    let (module, declaration, _) = fixture();
    let owner = ExprId::from_raw(RawHirId::new(
        module.snapshot_id().module(),
        NonZeroU32::MIN,
        ExprId::KIND,
    ));
    let path = HirSemanticOwnerPath::new(
        Box::new([HirSemanticPathStep::DeclarationResult]),
        Box::new([]),
    );
    let index = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(owner, path)]),
        statements: BTreeMap::new(),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: Box::new([]),
        control_transfers: Box::new([]),
    };
    let owner = HirSemanticPathOwnerId::Expression(owner);
    let mut found = None;
    record_semantic_path_location(&mut found, owner, &index).expect("first owner location");
    assert_eq!(found.expect("stored location").owner(), owner);
    assert_eq!(
        record_semantic_path_location(&mut found, owner, &index),
        Err(HirSemanticPathLookupError::DuplicateOwner { owner })
    );
}

#[test]
fn control_transfer_lookup_reports_missing_duplicate_and_module_mismatch() {
    let (module, declaration, application) = fixture();
    let statement = module.statements().next().expect("fixture statement").0;
    let path = HirSemanticOwnerPath::new(
        Box::new([HirSemanticPathStep::DeclarationResult]),
        Box::new([]),
    );
    let row = HirControlTransferRow::new(
        statement,
        HirControlTransferKind::Out,
        HirControlTransferTarget::Output { application },
    );
    let index = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(application, path.clone())]),
        statements: BTreeMap::from([(statement, path)]),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: Box::new([]),
        control_transfers: Box::new([row.clone()]),
    };
    let mut found = None;
    record_control_transfer_location(&mut found, statement, &index)
        .expect("first transfer location");
    assert_eq!(found.expect("stored transfer location").row(), &row);
    assert_eq!(
        record_control_transfer_location(&mut found, statement, &index),
        Err(HirControlTransferLookupError::Duplicate { statement })
    );

    let missing = StmtId::from_raw(RawHirId::new(
        module.module_id(),
        NonZeroU32::new(99).expect("missing statement slot"),
        StmtId::KIND,
    ));
    let mut missing_found = None;
    assert_eq!(
        record_control_transfer_location(&mut missing_found, missing, &index),
        Ok(())
    );
    assert_eq!(
        HirProjectEvaluationTopology {
            generation: Arc::new(super::super::AcceptedHirProjectGeneration {
                symbol_world: crate::symbol::ProjectSymbolWorldId::try_new(
                    crate::symbol::CallablePackageId::try_new("lookup-tests").unwrap(),
                    SourceDocumentId::try_new("lookup").unwrap(),
                    "default",
                )
                .unwrap(),
                symbol_revision: crate::symbol::ProjectSymbolRevision::try_for_documents([])
                    .unwrap(),
                modules: Box::new([]),
            }),
            modules: Box::new([]),
        }
        .control_transfer(missing),
        Err(HirControlTransferLookupError::Missing { statement: missing })
    );

    let foreign_module = HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(2).unwrap()),
        NonZeroU32::MIN,
    );
    let foreign = StmtId::from_raw(RawHirId::new(foreign_module, NonZeroU32::MIN, StmtId::KIND));
    let mut foreign_found = None;
    assert_eq!(
        record_control_transfer_location(&mut foreign_found, foreign, &index),
        Err(HirControlTransferLookupError::ModuleMismatch {
            statement: foreign,
            snapshot: module.snapshot_id(),
        })
    );
}

#[test]
fn body_rows_allow_distinct_typed_owners_at_one_structural_path() {
    let (module, declaration, _) = fixture();
    let expression = module.expressions().next().expect("fixture expression").0;
    let path = HirSemanticOwnerPath::new(
        Box::new([HirSemanticPathStep::DeclarationBody(
            HirDeclarationBodyRootRole::ViewValue { ordinal: 0 },
        )]),
        Box::new([]),
    );
    let declaration_row = HirSemanticBodyRow::try_new(
        HirSemanticBodyOwner::declaration(HirDeclarationBodyRootRole::ViewValue { ordinal: 0 }),
        path.clone(),
        HirBodyProjection::try_new(
            HirBodyKind::Expression,
            vec![HirBodyChildEdge::new(
                HirBodyChild::Expression(expression),
                HirBodyChildRole::Expression,
            )],
        )
        .unwrap(),
    )
    .unwrap();
    let expression_row = HirSemanticBodyRow::try_new(
        HirSemanticBodyOwner::direct_expression(expression),
        path.clone(),
        HirBodyProjection::try_new(HirBodyKind::Thread, Vec::new()).unwrap(),
    )
    .unwrap();
    let index = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(expression, path)]),
        statements: BTreeMap::new(),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: vec![declaration_row, expression_row].into_boxed_slice(),
        control_transfers: Box::new([]),
    };

    index.validate_root_paths().unwrap();
    assert_eq!(index.body_rows().len(), 2);
    assert_eq!(index.body_rows()[0].path(), index.body_rows()[1].path());
}

#[test]
fn body_rows_reject_duplicate_typed_owner_and_wrong_child_join() {
    let (module, declaration, _) = fixture();
    let expression = module.expressions().next().expect("fixture expression").0;
    let root_role = HirDeclarationBodyRootRole::ViewValue { ordinal: 0 };
    let path = HirSemanticOwnerPath::new(
        Box::new([HirSemanticPathStep::DeclarationBody(root_role)]),
        Box::new([]),
    );
    let row = || {
        HirSemanticBodyRow::try_new(
            HirSemanticBodyOwner::declaration(root_role),
            path.clone(),
            HirBodyProjection::try_new(
                HirBodyKind::Expression,
                vec![HirBodyChildEdge::new(
                    HirBodyChild::Expression(expression),
                    HirBodyChildRole::Expression,
                )],
            )
            .unwrap(),
        )
        .unwrap()
    };
    let duplicate = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration.clone()),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(expression, path.clone())]),
        statements: BTreeMap::new(),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: vec![row(), row()].into_boxed_slice(),
        control_transfers: Box::new([]),
    };
    assert_eq!(
        duplicate.validate_root_paths(),
        Err(HirSemanticPathError::DuplicateBodyOwner {
            owner: HirSemanticBodyOwner::declaration(root_role),
        })
    );

    let wrong_path = HirSemanticOwnerPath::new(
        Box::new([HirSemanticPathStep::DeclarationBody(
            HirDeclarationBodyRootRole::ViewValue { ordinal: 1 },
        )]),
        Box::new([]),
    );
    let mismatched = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(expression, wrong_path)]),
        statements: BTreeMap::new(),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: vec![row()].into_boxed_slice(),
        control_transfers: Box::new([]),
    };
    assert_eq!(
        mismatched.validate_root_paths(),
        Err(HirSemanticPathError::InvalidBodyRow {
            owner: HirSemanticBodyOwner::declaration(root_role),
        })
    );
}

#[test]
fn body_owner_and_kind_pairing_is_closed() {
    let (module, _, _) = fixture();
    let expression = module.expressions().next().expect("fixture expression").0;
    let non_body_role = HirExpressionOwnedBodyRole::ClosureParameterPattern { parameter: 0 };
    assert_eq!(
        HirSemanticBodyOwner::try_expression_owned(expression, non_body_role.clone()),
        Err(
            HirSemanticBodyOwnerError::NonBodyBearingExpressionOwnedRole {
                role: non_body_role,
            }
        )
    );

    let item = module.items().next().expect("fixture item").0;
    let root = HirSemanticPathRoot::Item {
        item,
        entry_ordinal: 0,
        role: HirItemEvaluationEntryRole::Item,
    };
    let owner = HirSemanticBodyOwner::item(HirDeclarationItemRootRole::TestBody);
    let row = HirSemanticBodyRow::try_new(
        owner.clone(),
        HirSemanticOwnerPath::new(
            Box::new([HirSemanticPathStep::DeclarationItem(
                HirDeclarationItemRootRole::TestBody,
            )]),
            Box::new([]),
        ),
        HirBodyProjection::try_new(HirBodyKind::Thread, Vec::new()).unwrap(),
    )
    .unwrap();
    let index = HirSemanticPathIndex {
        root,
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::new(),
        statements: BTreeMap::new(),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: Box::new([row]),
        control_transfers: Box::new([]),
    };
    assert_eq!(
        index.validate_root_paths(),
        Err(HirSemanticPathError::InvalidBodyRow { owner })
    );
}

#[test]
fn path_index_rejects_cross_family_structural_aliases() {
    let (module, declaration, _) = fixture();
    let module_id = module.snapshot_id().module();
    let expression = ExprId::from_raw(RawHirId::new(
        module_id,
        NonZeroU32::new(1).unwrap(),
        ExprId::KIND,
    ));
    let statement = StmtId::from_raw(RawHirId::new(
        module_id,
        NonZeroU32::new(2).unwrap(),
        StmtId::KIND,
    ));
    let path = || {
        HirSemanticOwnerPath::new(
            Box::new([HirSemanticPathStep::DeclarationResult]),
            Box::new([]),
        )
    };
    let index = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(expression, path())]),
        statements: BTreeMap::from([(statement, path())]),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: Box::new([]),
        control_transfers: Box::new([]),
    };
    assert_eq!(
        index.validate_root_paths(),
        Err(HirSemanticPathError::DuplicateStructuralPath {
            first: expression.into(),
            second: statement.into(),
        })
    );
}

#[test]
fn path_index_rejects_a_foreign_owner_before_issuing_a_location() {
    let (module, declaration, foreign_owner) = fixture();
    let index = HirSemanticPathIndex {
        root: HirSemanticPathRoot::Declaration(declaration),
        snapshot: module.snapshot_id(),
        expressions: BTreeMap::from([(
            foreign_owner,
            HirSemanticOwnerPath::new(
                Box::new([HirSemanticPathStep::DeclarationResult]),
                Box::new([]),
            ),
        )]),
        statements: BTreeMap::new(),
        patterns: BTreeMap::new(),
        locals: BTreeMap::new(),
        body_rows: Box::new([]),
        control_transfers: Box::new([]),
    };
    assert_eq!(
        index.validate_root_paths(),
        Err(HirSemanticPathError::OwnerModuleMismatch {
            owner: foreign_owner.into(),
            snapshot: module.snapshot_id(),
        })
    );
    let mut found = None;
    assert_eq!(
        record_semantic_path_location(&mut found, foreign_owner.into(), &index),
        Err(HirSemanticPathLookupError::OwnerModuleMismatch {
            owner: foreign_owner.into(),
            snapshot: module.snapshot_id(),
        })
    );
}

#[test]
fn expression_walk_rejects_an_unresolved_owner() {
    let (module, _, owner) = fixture();
    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    assert_eq!(
        builder.walk_expression(owner, &[], &[], None, CaptureAccess::Read),
        Err(HirSemanticPathError::UnresolvedOwner)
    );
}

#[test]
fn semantic_path_ordinal_conversion_is_checked_at_the_u32_boundary() {
    let exact = usize::try_from(u32::MAX).expect("test platform supports u32 indices");
    assert_eq!(checked_ordinal(exact), Ok(u32::MAX));
    if let Ok(one_over) = usize::try_from(u64::from(u32::MAX) + 1) {
        assert_eq!(
            checked_ordinal(one_over),
            Err(HirSemanticPathError::OrdinalOverflow)
        );
    }
}

#[test]
fn path_builder_consumes_nested_start_and_together_owned_edge() {
    let (module, _, _) = fixture();
    let statement = module.statements().next().expect("fixture statement").0;
    let target = module.expressions().next().expect("fixture expression").0;
    let scope = module.scopes().next().expect("fixture scope").0;
    let owner = ExprId::from_raw(RawHirId::new(
        module.module_id(),
        NonZeroU32::new(u32::MAX).unwrap(),
        ExprId::KIND,
    ));
    let plan = HirLinePlan::try_new(
        scope,
        None,
        Box::new([HirLinePlanItem::StartGroup(Box::new([
            HirLinePlanItem::TogetherGroup(Box::new([HirLinePlanItem::Thread(statement)])),
        ]))]),
    )
    .expect("nested line plan");
    let content = HirDialogueContent::try_new(
        HirDialogueContentId::new(owner),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    )
    .expect("empty dialogue content");
    let dialogue = HirExprKind::DialogueContentApplication(
        HirDialogueContentApplication::try_new(owner, target, content, Some(plan), Box::new([]))
            .expect("dialogue application"),
    );
    let edge = dialogue
        .expression_owned_child_edges()
        .expect("owned topology")
        .into_iter()
        .find(|edge| {
            matches!(
                edge.role(),
                HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                    role: HirLinePlanStatementRole::Thread,
                    ..
                }
            )
        })
        .expect("nested Thread edge");

    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    builder
        .walk_expression_owned_edge(
            target,
            &edge,
            &[HirSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FunctionBody,
            )],
            &[],
            CaptureAccess::Read,
        )
        .expect("builder consumes nested owned edge");
    let path = builder
        .statements
        .get(&statement)
        .expect("nested statement path");
    assert!(matches!(
        path.steps(),
        [
            HirSemanticPathStep::DeclarationBody(HirDeclarationBodyRootRole::FunctionBody),
            HirSemanticPathStep::ExpressionOwned(
                HirExpressionOwnedBodyRole::DialogueLinePlanStatement { path, role: HirLinePlanStatementRole::Thread }
            )
        ] if path.segments() == [
            HirNestedExpressionPathSegment::LinePlanItem { ordinal: 0 },
            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 0 },
            HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal: 0 },
        ]
    ));
}

#[test]
fn path_builder_preserves_a_multisegment_choice_owned_statement_path() {
    let (module, _, _) = fixture();
    let statement = module.statements().next().expect("fixture statement").0;
    let pattern = module.patterns().next().expect("fixture pattern").0;
    let source = module.expressions().next().expect("fixture expression").0;
    let scope = module.scopes().next().expect("fixture scope").0;
    let choice = HirExprKind::Choice(HirChoiceExpr::new(
        None,
        HirChoiceBody::new(
            scope,
            Box::new([HirChoiceItem::OptionFor(HirChoiceOptionFor::new(
                pattern,
                source,
                HirChoiceOptionBody::new(scope, Box::new([HirChoiceOptionField::Let(statement)])),
                Box::new([]),
            ))]),
        ),
        None,
    ));
    let edge = choice
        .expression_owned_child_edges()
        .expect("owned topology")
        .into_iter()
        .find(|edge| {
            matches!(
                edge.role(),
                HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { .. }
            )
        })
        .expect("Choice option Let edge");

    let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(&module);
    builder
        .walk_expression_owned_edge(
            source,
            &edge,
            &[HirSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FunctionBody,
            )],
            &[],
            CaptureAccess::Read,
        )
        .expect("builder consumes Choice owned edge");
    let path = builder
        .statements
        .get(&statement)
        .expect("Choice statement path");
    assert!(matches!(
        path.steps(),
        [
            HirSemanticPathStep::DeclarationBody(HirDeclarationBodyRootRole::FunctionBody),
            HirSemanticPathStep::ExpressionOwned(
                HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { path, field: 0 }
            )
        ] if path.segments() == [
            HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            HirNestedExpressionPathSegment::ChoiceOptionBody,
            HirNestedExpressionPathSegment::ChoiceOptionField { ordinal: 0 },
        ]
    ));
}
