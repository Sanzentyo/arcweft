use core::num::{NonZeroU32, NonZeroU64};

use super::super::{HirItem, HirItemKind, HirItemPoisonState};
use super::*;
use crate::identity::{HirDatabaseId, HirTypedId, ItemId, RawHirId};
use crate::leaf::HirIdRef;
use crate::leaf::{
    HirEntityReference, HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirNameInvariantError,
};

fn module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::new(slot).unwrap(),
    )
}

fn typed_id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn resolved_id(value: &str) -> HirIdRefValue {
    HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new(value.into()).unwrap(),
    ))
}

fn recovered_id() -> HirIdRefValue {
    HirIdRefValue::Recovered(HirIdRefRecovery::new(
        HirIdRefShape::Missing,
        HirIdRefIssue::Missing,
    ))
}

fn style_name(value: &str) -> HirStyleName {
    HirStyleName::try_new(value.into()).unwrap()
}

fn selector_sequence(
    relation_to_previous: Option<HirStyleCombinator>,
    element: Option<&str>,
    part: Option<&str>,
    predicates: &[&str],
) -> HirStyleSelectorSequence {
    HirStyleSelectorSequence::new(
        relation_to_previous,
        element.map(style_name),
        part.map(style_name),
        predicates
            .iter()
            .map(|predicate| style_name(predicate))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn empty_prefix() -> HirItemPrefix {
    HirItemPrefix::new(None, Box::new([]), None)
}

#[test]
fn style_names_admit_native_hyphens_without_becoming_ordinary_hir_names() {
    let name = HirStyleName::try_new("background-color".into()).unwrap();

    assert_eq!(name.as_str(), Some("background-color"));
    assert_eq!(name.recovery_issue(), None);
    assert!(!name.has_recovery());
    assert_eq!(
        HirName::try_new("background-color".into()),
        Err(HirNameInvariantError::InvalidIdentifier)
    );
    assert_eq!(
        HirStyleName::try_new("background--color".into()),
        Err(HirStyleNameIssue::Invalid)
    );
    assert_eq!(
        HirStyleName::try_new("-background".into()),
        Err(HirStyleNameIssue::Invalid)
    );
    assert_eq!(
        HirStyleName::try_new("".into()),
        Err(HirStyleNameIssue::Missing)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one clean Style fixture proves typed IDs, source order, and nested-body ownership together"
)]
fn clean_style_model_preserves_typed_ids_source_order_and_nested_body() {
    let local = module(1, 1);
    let token_type = typed_id::<TypeId>(local, 1);
    let token_value = typed_id::<ExprId>(local, 2);
    let declaration_value = typed_id::<ExprId>(local, 3);
    let condition_value = typed_id::<ExprId>(local, 4);
    let nested_value = typed_id::<ExprId>(local, 5);
    let token = HirStyleToken::try_new(
        local,
        resolved_id("style.dialogue.spacing"),
        Some(token_type),
        token_value,
        None,
    )
    .unwrap();
    let selector = HirStyleSelector::try_new(
        vec![
            selector_sequence(None, Some("dialogue-line"), None, &["active"]),
            selector_sequence(
                Some(HirStyleCombinator::Child),
                None,
                Some("speaker-name"),
                &[],
            ),
        ]
        .into_boxed_slice(),
    )
    .unwrap();
    let declaration = HirStyleDeclaration::try_new(
        local,
        style_name("background-color"),
        declaration_value,
        HirStyleAssignOperation::Append,
    )
    .unwrap();
    let rule = HirStyleRule::try_new(local, selector, Box::new([declaration])).unwrap();
    let clause = HirStyleEnvironmentClause::try_new(
        local,
        HirStyleEnvironmentField::ColorScheme,
        HirStyleEnvironmentComparison::Equal,
        condition_value,
    )
    .unwrap();
    let nested_rule = HirStyleRule::try_new(
        local,
        HirStyleSelector::try_new(Box::new([selector_sequence(
            None,
            Some("choice-list"),
            None,
            &[],
        )]))
        .unwrap(),
        Box::new([HirStyleDeclaration::try_new(
            local,
            style_name("text-scale"),
            nested_value,
            HirStyleAssignOperation::Replace,
        )
        .unwrap()]),
    )
    .unwrap();
    let environment = HirStyleEnvironment::try_new(
        local,
        Box::new([clause]),
        Box::new([HirStyleBodyItem::Rule(nested_rule)]),
    )
    .unwrap();
    let style = HirStyleItem::try_new(
        local,
        resolved_id("style.dialogue"),
        Box::new([token]),
        Box::new([
            HirStyleBodyItem::Rule(rule),
            HirStyleBodyItem::Environment(environment),
        ]),
    )
    .unwrap();

    assert!(matches!(style.id(), HirIdRefValue::Resolved(_)));
    assert_eq!(style.tokens().len(), 1);
    assert!(matches!(style.tokens()[0].id(), HirIdRefValue::Resolved(_)));
    assert_eq!(style.tokens()[0].value_type(), Some(token_type));
    assert_eq!(style.tokens()[0].value(), token_value);
    assert_eq!(style.tokens()[0].recovery_issue(), None);
    let HirStyleBodyItem::Rule(rule) = &style.body()[0] else {
        panic!("first Style body member must remain the authored rule");
    };
    assert_eq!(rule.selector().sequences().len(), 2);
    assert_eq!(
        rule.selector().sequences()[1].relation_to_previous(),
        Some(HirStyleCombinator::Child)
    );
    assert_eq!(
        rule.selector().sequences()[1]
            .part()
            .and_then(HirStyleName::as_str),
        Some("speaker-name")
    );
    assert_eq!(rule.declarations()[0].value(), declaration_value);
    assert_eq!(
        rule.declarations()[0].operation(),
        HirStyleAssignOperation::Append
    );
    let HirStyleBodyItem::Environment(environment) = &style.body()[1] else {
        panic!("second Style body member must remain the authored environment");
    };
    assert_eq!(
        environment.clauses()[0].field(),
        HirStyleEnvironmentField::ColorScheme
    );
    assert_eq!(
        environment.clauses()[0].comparison(),
        HirStyleEnvironmentComparison::Equal
    );
    assert_eq!(environment.clauses()[0].value(), condition_value);
    assert!(matches!(environment.body()[0], HirStyleBodyItem::Rule(_)));
    assert!(!style.has_recovery());
    assert!(
        HirItem::try_new(
            typed_id(local, 20),
            typed_id(local, 21),
            empty_prefix(),
            HirItemKind::Style(style),
            Box::new([]),
        )
        .is_ok()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one recovery matrix proves every typed Style role poisons its owning item"
)]
fn every_typed_style_recovery_role_poison_propagates_to_the_item() {
    let local = module(2, 1);
    let token = HirStyleToken::try_new(
        local,
        recovered_id(),
        None,
        typed_id(local, 1),
        Some(HirStyleTokenIssue::MissingAssignment),
    )
    .unwrap();
    let recovered_name = HirStyleName::recovered(HirStyleNameIssue::Invalid);
    let selector = HirStyleSelector::recovered(
        Box::new([HirStyleSelectorSequence::new(
            None,
            Some(recovered_name.clone()),
            None,
            Box::new([]),
        )]),
        HirStyleSelectorIssue::InvalidComponent,
    );
    let declaration = HirStyleDeclaration::try_new(
        local,
        recovered_name,
        typed_id(local, 2),
        HirStyleAssignOperation::Recovered(HirStyleAssignOperationIssue::Missing),
    )
    .unwrap();
    let rule = HirStyleRule::try_new(local, selector, Box::new([declaration])).unwrap();
    let clause = HirStyleEnvironmentClause::try_new(
        local,
        HirStyleEnvironmentField::Recovered(HirStyleEnvironmentFieldIssue::Unknown),
        HirStyleEnvironmentComparison::Recovered(HirStyleEnvironmentComparisonIssue::Invalid),
        typed_id(local, 3),
    )
    .unwrap();
    let environment = HirStyleEnvironment::try_new(
        local,
        Box::new([clause]),
        Box::new([HirStyleBodyItem::Recovered(HirStyleBodyIssue::Malformed)]),
    )
    .unwrap();
    let style = HirStyleItem::try_new(
        local,
        recovered_id(),
        Box::new([token]),
        Box::new([
            HirStyleBodyItem::Rule(rule),
            HirStyleBodyItem::Environment(environment),
        ]),
    )
    .unwrap();

    assert!(style.has_recovery());
    assert_eq!(
        style.tokens()[0].recovery_issue(),
        Some(HirStyleTokenIssue::MissingAssignment)
    );
    let HirStyleBodyItem::Rule(rule) = &style.body()[0] else {
        panic!("expected recovered rule");
    };
    assert_eq!(
        rule.selector().recovery_issue(),
        Some(HirStyleSelectorIssue::InvalidComponent)
    );
    assert_eq!(
        rule.declarations()[0].property().recovery_issue(),
        Some(HirStyleNameIssue::Invalid)
    );
    assert_eq!(
        rule.declarations()[0].operation().recovery_issue(),
        Some(HirStyleAssignOperationIssue::Missing)
    );
    let HirStyleBodyItem::Environment(environment) = &style.body()[1] else {
        panic!("expected recovered environment");
    };
    assert_eq!(
        environment.clauses()[0].field().recovery_issue(),
        Some(HirStyleEnvironmentFieldIssue::Unknown)
    );
    assert_eq!(
        environment.clauses()[0].comparison().recovery_issue(),
        Some(HirStyleEnvironmentComparisonIssue::Invalid)
    );
    assert_eq!(
        environment.body()[0].recovery_issue(),
        Some(HirStyleBodyIssue::Malformed)
    );

    let owner = typed_id::<ItemId>(local, 10);
    let scope = typed_id::<ScopeId>(local, 11);
    assert_eq!(
        HirItem::try_new(
            owner,
            scope,
            empty_prefix(),
            HirItemKind::Style(style.clone()),
            Box::new([]),
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
    let item = HirItem::try_new_with_state(
        owner,
        scope,
        empty_prefix(),
        HirItemKind::Style(style),
        Box::new([]),
        HirItemPoisonState::Poisoned(HirItemIssue::Recovery),
    )
    .unwrap();
    assert!(item.is_poisoned());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one nested recovery matrix proves every Style role marks the item recovered"
)]
fn each_nested_style_recovery_role_marks_the_item_recovered() {
    let local = module(5, 1);
    let value = typed_id::<ExprId>(local, 1);
    let clean_selector = || {
        HirStyleSelector::try_new(Box::new([selector_sequence(
            None,
            Some("dialogue-line"),
            None,
            &[],
        )]))
        .unwrap()
    };
    let style_with_token = |token| {
        HirStyleItem::try_new(
            local,
            resolved_id("style.dialogue"),
            Box::new([token]),
            Box::new([]),
        )
        .unwrap()
    };
    let style_with_rule = |declaration| {
        HirStyleItem::try_new(
            local,
            resolved_id("style.dialogue"),
            Box::new([]),
            Box::new([HirStyleBodyItem::Rule(
                HirStyleRule::try_new(local, clean_selector(), Box::new([declaration])).unwrap(),
            )]),
        )
        .unwrap()
    };
    let style_with_clause = |clause| {
        HirStyleItem::try_new(
            local,
            resolved_id("style.dialogue"),
            Box::new([]),
            Box::new([HirStyleBodyItem::Environment(
                HirStyleEnvironment::try_new(local, Box::new([clause]), Box::new([])).unwrap(),
            )]),
        )
        .unwrap()
    };

    let cases = [
        HirStyleItem::try_new(local, recovered_id(), Box::new([]), Box::new([])).unwrap(),
        style_with_token(HirStyleToken::try_new(local, recovered_id(), None, value, None).unwrap()),
        style_with_token(
            HirStyleToken::try_new(
                local,
                resolved_id("style.dialogue.spacing"),
                None,
                value,
                Some(HirStyleTokenIssue::MalformedAssignment),
            )
            .unwrap(),
        ),
        HirStyleItem::try_new(
            local,
            resolved_id("style.dialogue"),
            Box::new([]),
            Box::new([HirStyleBodyItem::Rule(
                HirStyleRule::try_new(
                    local,
                    HirStyleSelector::recovered(
                        Box::new([HirStyleSelectorSequence::new(
                            None,
                            Some(HirStyleName::recovered(HirStyleNameIssue::Invalid)),
                            None,
                            Box::new([]),
                        )]),
                        HirStyleSelectorIssue::InvalidComponent,
                    ),
                    Box::new([]),
                )
                .unwrap(),
            )]),
        )
        .unwrap(),
        style_with_rule(
            HirStyleDeclaration::try_new(
                local,
                HirStyleName::recovered(HirStyleNameIssue::Missing),
                value,
                HirStyleAssignOperation::Replace,
            )
            .unwrap(),
        ),
        style_with_rule(
            HirStyleDeclaration::try_new(
                local,
                style_name("color"),
                value,
                HirStyleAssignOperation::Recovered(HirStyleAssignOperationIssue::Invalid),
            )
            .unwrap(),
        ),
        style_with_clause(
            HirStyleEnvironmentClause::try_new(
                local,
                HirStyleEnvironmentField::Recovered(HirStyleEnvironmentFieldIssue::Missing),
                HirStyleEnvironmentComparison::Equal,
                value,
            )
            .unwrap(),
        ),
        style_with_clause(
            HirStyleEnvironmentClause::try_new(
                local,
                HirStyleEnvironmentField::Contrast,
                HirStyleEnvironmentComparison::Recovered(
                    HirStyleEnvironmentComparisonIssue::Missing,
                ),
                value,
            )
            .unwrap(),
        ),
        HirStyleItem::try_new(
            local,
            resolved_id("style.dialogue"),
            Box::new([]),
            Box::new([HirStyleBodyItem::Recovered(HirStyleBodyIssue::Missing)]),
        )
        .unwrap(),
    ];

    assert!(cases.iter().all(HirStyleItem::has_recovery));
}

#[test]
fn resolved_style_selector_enforces_sequence_structure() {
    assert_eq!(
        HirStyleSelector::try_new(Box::new([])),
        Err(HirStyleSelectorIssue::MissingSequence)
    );
    assert_eq!(
        HirStyleSelector::try_new(Box::new([selector_sequence(
            Some(HirStyleCombinator::Descendant),
            Some("dialogue-line"),
            None,
            &[],
        )])),
        Err(HirStyleSelectorIssue::InvalidRelation)
    );
    assert_eq!(
        HirStyleSelector::try_new(Box::new([
            selector_sequence(None, Some("dialogue-line"), None, &[]),
            selector_sequence(None, None, Some("speaker-name"), &[]),
        ])),
        Err(HirStyleSelectorIssue::InvalidRelation)
    );
    assert_eq!(
        HirStyleSelector::try_new(Box::new([HirStyleSelectorSequence::new(
            None,
            None,
            None,
            Box::new([]),
        )])),
        Err(HirStyleSelectorIssue::MissingComponent)
    );
    assert_eq!(
        HirStyleSelector::try_new(Box::new([HirStyleSelectorSequence::new(
            None,
            Some(HirStyleName::recovered(HirStyleNameIssue::Invalid)),
            None,
            Box::new([]),
        )])),
        Err(HirStyleSelectorIssue::InvalidComponent)
    );
}

#[test]
fn style_model_rejects_foreign_type_expression_and_nested_environment_ids() {
    let local = module(3, 1);
    let foreign = module(4, 1);
    let foreign_type = typed_id::<TypeId>(foreign, 1);
    let foreign_expression = typed_id::<ExprId>(foreign, 2);
    let local_expression = typed_id::<ExprId>(local, 3);

    assert_eq!(
        HirStyleToken::try_new(
            local,
            resolved_id("style.token"),
            Some(foreign_type),
            local_expression,
            None,
        ),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );
    assert_eq!(
        HirStyleDeclaration::try_new(
            local,
            style_name("color"),
            foreign_expression,
            HirStyleAssignOperation::Replace,
        ),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );

    let foreign_clause = HirStyleEnvironmentClause {
        field: HirStyleEnvironmentField::TextScale,
        comparison: HirStyleEnvironmentComparison::GreaterOrEqual,
        value: foreign_expression,
    };
    let environment = HirStyleEnvironment {
        clauses: Box::new([foreign_clause]),
        body: Box::new([]),
    };
    assert_eq!(
        HirStyleItem::try_new(
            local,
            resolved_id("style.dialogue"),
            Box::new([]),
            Box::new([HirStyleBodyItem::Environment(environment)]),
        ),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );
}
