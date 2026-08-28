use core::num::{NonZeroU32, NonZeroU64};

use super::*;
use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirLinePlan,
    HirLinePlanItem,
};
use crate::expr::{
    HirChoiceBody, HirChoiceExpr, HirChoiceItem, HirChoiceOptionBody, HirChoiceOptionField,
    HirChoiceOptionFor, HirExprKind, HirExpressionChildOwnership, HirExpressionChildRole,
    HirExpressionOwnedBodyRole, HirLinePlanStatementRole, HirNestedExpressionPathSegment,
};
use crate::identity::{HirDatabaseId, HirModuleId, HirTypedId, RawHirId};
use crate::symbol::{CallableDeclarationId, CallableDeclarationOwner};

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
fn item_root_path_index_keeps_recovery_coordinate_typed_and_item_owned() {
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
            child: HirDeclarationBodyRootChild::Expression(expression),
        })
        .expect("recovery item root");
    let paths = builder
        .path_index_since(
            HirSemanticPathRoot::Item {
                item,
                entry_ordinal: 0,
                role: HirItemEvaluationEntryRole::Item,
            },
            &checkpoint,
        )
        .expect("item recovery path index");
    assert!(matches!(
        paths.root(),
        HirSemanticPathRoot::Item {
            item: actual,
            entry_ordinal: 0,
            role: HirItemEvaluationEntryRole::Item,
        } if *actual == item
    ));
    assert!(matches!(
        paths
            .expression(expression)
            .and_then(|path| path.steps().first()),
        Some(HirSemanticPathStep::DeclarationItem(
            HirDeclarationItemRootRole::Recovery {
                owner: HirItemRecoveryRootOwner::Item
            }
        ))
    ));
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
    let content =
        HirDialogueContent::try_new(HirDialogueContentId::new(owner), Box::new([]), Box::new([]))
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
