use super::{module_key, parse, stage};

use crate::database::HirDatabase;
use crate::identity::{HirModuleId, ItemId};
use crate::item::{
    HirItem, HirItemKind, HirStyleAssignOperation, HirStyleBodyItem, HirStyleCombinator,
    HirStyleDeclaration, HirStyleEnvironment, HirStyleEnvironmentClause,
    HirStyleEnvironmentComparison, HirStyleEnvironmentField, HirStyleItem, HirStyleRule,
    HirStyleSelector, HirStyleSelectorSequence, HirStyleToken,
};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{
    HirItemSourceRole, HirSourceQuery, HirStyleSourceRole, HirStyleTokenSourcePart,
    StagedHirSourceIndex,
};

const STYLE_FREEZE_SOURCE: &str = concat!(
    "style freeze {\n",
    "    token primary: Color = white\n",
    "    token secondary: Length = 1\n",
    "    Panel > Button {\n",
    "        opacity = 2\n",
    "        append shadow = 3\n",
    "    }\n",
    "    when environment(text-scale >= 1) {\n",
    "        Label { opacity = 4 }\n",
    "    }\n",
    "}\n",
);

fn assert_style_payload_tamper_rejected(
    document_id: &str,
    mutate: impl FnOnce(HirModuleId, &HirStyleItem) -> HirStyleItem,
) {
    let parsed = parse(document_id, STYLE_FREEZE_SOURCE);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("Style freeze database");
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .expect("Style lowers before payload tamper");
    let [owner] = transaction.staged_source_ordered_items() else {
        panic!("one source-ordered Style item")
    };
    let owner = *owner;
    let retained = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .items()
            .resolve_staged(slots, owner)
            .expect("staged Style item")
            .clone()
    };
    let HirItemKind::Style(style) = retained.kind() else {
        panic!("staged item must remain Style")
    };
    let replacement_style = mutate(owner.module(), style);
    let replacement = HirItem::try_new_with_state(
        owner,
        retained.scope(),
        retained.prefix().clone(),
        HirItemKind::Style(replacement_style),
        retained.members().to_vec().into_boxed_slice(),
        *retained.state(),
    )
    .expect("same-module Style payload substitution");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .expect("test-only Style payload substitution");
    }

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

fn assert_style_manifest_tamper_rejected(
    document_id: &str,
    mutate: impl FnOnce(&mut StagedHirSourceIndex, ItemId),
) {
    let parsed = parse(document_id, STYLE_FREEZE_SOURCE);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("Style manifest freeze database");
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .expect("Style lowers before manifest tamper");
    let [owner] = transaction.staged_source_ordered_items() else {
        panic!("one source-ordered Style item")
    };
    let owner = *owner;
    mutate(transaction.source_components(), owner);

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

fn rebuild_style(
    module: HirModuleId,
    retained: &HirStyleItem,
    tokens: Vec<HirStyleToken>,
    body: Vec<HirStyleBodyItem>,
) -> HirStyleItem {
    HirStyleItem::try_new(
        module,
        retained.id().clone(),
        tokens.into_boxed_slice(),
        body.into_boxed_slice(),
    )
    .expect("same-module Style reconstruction")
}

#[test]
fn style_freeze_rejects_selector_relation_substitution() {
    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-selector",
        |module, style| {
            let mut body = style.body().to_vec();
            let HirStyleBodyItem::Rule(rule) = &body[0] else {
                panic!("first Style body item must remain a rule")
            };
            let mut sequences = rule.selector().sequences().to_vec();
            let second = &sequences[1];
            sequences[1] = HirStyleSelectorSequence::new(
                Some(HirStyleCombinator::Descendant),
                second.element().cloned(),
                second.part().cloned(),
                second.predicates().to_vec().into_boxed_slice(),
            );
            let selector = HirStyleSelector::try_new(sequences.into_boxed_slice())
                .expect("substituted selector remains structurally valid");
            body[0] = HirStyleBodyItem::Rule(
                HirStyleRule::try_new(
                    module,
                    selector,
                    rule.declarations().to_vec().into_boxed_slice(),
                )
                .expect("substituted Style rule"),
            );
            HirStyleItem::try_new(
                module,
                style.id().clone(),
                style.tokens().to_vec().into_boxed_slice(),
                body.into_boxed_slice(),
            )
            .expect("substituted Style item")
        },
    );
}

#[test]
fn style_freeze_rejects_operation_and_environment_substitutions() {
    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-operation",
        |module, style| {
            let mut body = style.body().to_vec();
            let HirStyleBodyItem::Rule(rule) = &body[0] else {
                panic!("first Style body item must remain a rule")
            };
            let mut declarations = rule.declarations().to_vec();
            let first = &declarations[0];
            declarations[0] = HirStyleDeclaration::try_new(
                module,
                first.property().clone(),
                first.value(),
                HirStyleAssignOperation::Append,
            )
            .expect("operation-substituted declaration");
            body[0] = HirStyleBodyItem::Rule(
                HirStyleRule::try_new(
                    module,
                    rule.selector().clone(),
                    declarations.into_boxed_slice(),
                )
                .expect("operation-substituted rule"),
            );
            rebuild_style(module, style, style.tokens().to_vec(), body)
        },
    );

    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-environment-field",
        |module, style| {
            let mut body = style.body().to_vec();
            let HirStyleBodyItem::Environment(environment) = &body[1] else {
                panic!("second Style body item must remain an environment")
            };
            let mut clauses = environment.clauses().to_vec();
            clauses[0] = HirStyleEnvironmentClause::try_new(
                module,
                HirStyleEnvironmentField::Contrast,
                HirStyleEnvironmentComparison::GreaterOrEqual,
                clauses[0].value(),
            )
            .expect("field-substituted clause");
            body[1] = HirStyleBodyItem::Environment(
                HirStyleEnvironment::try_new(
                    module,
                    clauses.into_boxed_slice(),
                    environment.body().to_vec().into_boxed_slice(),
                )
                .expect("field-substituted environment"),
            );
            rebuild_style(module, style, style.tokens().to_vec(), body)
        },
    );

    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-environment-comparison",
        |module, style| {
            let mut body = style.body().to_vec();
            let HirStyleBodyItem::Environment(environment) = &body[1] else {
                panic!("second Style body item must remain an environment")
            };
            let mut clauses = environment.clauses().to_vec();
            clauses[0] = HirStyleEnvironmentClause::try_new(
                module,
                HirStyleEnvironmentField::TextScale,
                HirStyleEnvironmentComparison::Equal,
                clauses[0].value(),
            )
            .expect("comparison-substituted clause");
            body[1] = HirStyleBodyItem::Environment(
                HirStyleEnvironment::try_new(
                    module,
                    clauses.into_boxed_slice(),
                    environment.body().to_vec().into_boxed_slice(),
                )
                .expect("comparison-substituted environment"),
            );
            rebuild_style(module, style, style.tokens().to_vec(), body)
        },
    );
}

#[test]
fn style_freeze_rejects_body_reorder_and_duplicate_declaration() {
    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-body-order",
        |module, style| {
            let mut body = style.body().to_vec();
            body.swap(0, 1);
            rebuild_style(module, style, style.tokens().to_vec(), body)
        },
    );

    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-duplicate-declaration",
        |module, style| {
            let mut body = style.body().to_vec();
            let HirStyleBodyItem::Rule(rule) = &body[0] else {
                panic!("first Style body item must remain a rule")
            };
            let mut declarations = rule.declarations().to_vec();
            declarations[1] = declarations[0].clone();
            body[0] = HirStyleBodyItem::Rule(
                HirStyleRule::try_new(
                    module,
                    rule.selector().clone(),
                    declarations.into_boxed_slice(),
                )
                .expect("duplicate-child Style rule"),
            );
            rebuild_style(module, style, style.tokens().to_vec(), body)
        },
    );
}

#[test]
fn style_freeze_rejects_substituted_expression_and_type_children() {
    assert_style_payload_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-foreign-children",
        |module, style| {
            let mut tokens = style.tokens().to_vec();
            let first = tokens[0].clone();
            let second = tokens[1].clone();
            tokens[0] = HirStyleToken::try_new(
                module,
                first.id().clone(),
                second.value_type(),
                second.value(),
                first.recovery_issue(),
            )
            .expect("first token with substituted source children");
            tokens[1] = HirStyleToken::try_new(
                module,
                second.id().clone(),
                first.value_type(),
                first.value(),
                second.recovery_issue(),
            )
            .expect("second token with substituted source children");
            rebuild_style(module, style, tokens, style.body().to_vec())
        },
    );
}

#[test]
fn style_freeze_rejects_missing_and_extra_manifest_rows() {
    assert_style_manifest_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-missing-row",
        |index, owner| {
            let query = HirSourceQuery::Item {
                owner,
                role: HirItemSourceRole::Style(HirStyleSourceRole::Token {
                    ordinal: 0,
                    part: HirStyleTokenSourcePart::Whole,
                }),
            };
            assert!(index.remove_staged_query(&query));
        },
    );

    assert_style_manifest_tamper_rejected(
        "arcweft-test://proof/final-hir-style-freeze-extra-row",
        |index, owner| {
            let query = HirSourceQuery::Item {
                owner,
                role: HirItemSourceRole::Style(HirStyleSourceRole::Token {
                    ordinal: u32::MAX,
                    part: HirStyleTokenSourcePart::Whole,
                }),
            };
            index
                .stage_absent_optional_query(&query)
                .expect("test-only extra optional Style row");
        },
    );
}
