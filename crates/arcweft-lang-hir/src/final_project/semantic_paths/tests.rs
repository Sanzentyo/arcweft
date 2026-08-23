use core::num::{NonZeroU32, NonZeroU64};

use super::*;
use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirLinePlan,
    HirLinePlanItem,
};
use crate::expr::{
    HirChoiceBody, HirChoiceExpr, HirChoiceItem, HirChoiceOptionBody, HirChoiceOptionField,
    HirChoiceOptionFor, HirExprKind, HirExpressionOwnedBodyRole, HirLinePlanStatementRole,
    HirNestedExpressionPathSegment,
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
fn expression_walk_rejects_cycle_before_duplicate() {
    let (module, declaration, owner) = fixture();
    let mut builder = PathBuilder::new(&module, declaration);
    builder.expressions.insert(owner, Box::new([]));
    builder.active_expressions.insert(owner);
    assert_eq!(
        builder.walk_expression(owner, &[], &[]),
        Err(HirSemanticPathError::CyclicPath)
    );
}

#[test]
fn expression_walk_rejects_duplicate_before_resolution() {
    let (module, declaration, owner) = fixture();
    let mut builder = PathBuilder::new(&module, declaration);
    builder.expressions.insert(owner, Box::new([]));
    assert_eq!(
        builder.walk_expression(owner, &[], &[]),
        Err(HirSemanticPathError::DuplicatePath)
    );
}

#[test]
fn expression_walk_rejects_an_unresolved_owner() {
    let (module, declaration, owner) = fixture();
    let mut builder = PathBuilder::new(&module, declaration);
    assert_eq!(
        builder.walk_expression(owner, &[], &[]),
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
    let (module, declaration, _) = fixture();
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

    let mut builder = PathBuilder::new(&module, declaration);
    builder
        .walk_expression_owned_edge(
            &edge,
            &[HirSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FunctionBody,
            )],
        )
        .expect("builder consumes nested owned edge");
    let path = builder
        .statements
        .get(&statement)
        .expect("nested statement path");
    assert!(matches!(
        path.as_ref(),
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
    let (module, declaration, _) = fixture();
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

    let mut builder = PathBuilder::new(&module, declaration);
    builder
        .walk_expression_owned_edge(
            &edge,
            &[HirSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FunctionBody,
            )],
        )
        .expect("builder consumes Choice owned edge");
    let path = builder
        .statements
        .get(&statement)
        .expect("Choice statement path");
    assert!(matches!(
        path.as_ref(),
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
