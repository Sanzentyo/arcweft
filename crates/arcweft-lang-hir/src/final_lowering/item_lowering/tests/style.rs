use super::*;

use std::fmt::Write as _;

use arcweft_lang_syntax::attachment::{
    AttachedStyleExpression, AttachedStyleMember, AttachedStyleToken, SyntaxNodeId,
};

use crate::expr::{
    HirExprError, HirExprKind, HirGenericExprIssue, HirPoisonState, HirRecoveryIssue,
};
use crate::identity::{ExprId, ItemId, TypeId};
use crate::item::{
    HirStyleAssignOperation, HirStyleAssignOperationIssue, HirStyleBodyIssue, HirStyleBodyItem,
    HirStyleCombinator, HirStyleEnvironmentComparison, HirStyleEnvironmentComparisonIssue,
    HirStyleEnvironmentField, HirStyleEnvironmentFieldIssue, HirStyleItem, HirStyleSelectorIssue,
};
use crate::leaf::{HirIdRefInvariantError, HirIdRefIssue, HirIdRefShape};
use crate::source_index::{HirExprSourceRole, HirSourceQuery};

use super::super::style::{preflight_style_member_count, preflight_style_nesting_depth};

fn style_item(module: &HirModule, ordinal: usize) -> (ItemId, &HirItem, &HirStyleItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Style(style) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Style")
    };
    (owner, item, style)
}

fn style_expression_syntax(attached: &AttachedStyleExpression) -> SyntaxNodeId {
    match attached {
        AttachedStyleExpression::Authored(attached) => attached.syntax().id(),
        AttachedStyleExpression::Missing(attached) => attached.id(),
    }
}

#[derive(Debug)]
struct CleanStyleSyntax {
    token_type: SyntaxNodeId,
    token_value: SyntaxNodeId,
    rule_values: Vec<SyntaxNodeId>,
    clause_values: Vec<SyntaxNodeId>,
    nested_rule_value: SyntaxNodeId,
}

fn clean_style_syntax(parsed: &ParsedSource) -> CleanStyleSyntax {
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => Some(style.semantics().unwrap()),
            _ => None,
        })
        .expect("typed Style attachment");
    let [
        AttachedStyleMember::Token(token),
        AttachedStyleMember::Rule(rule),
        AttachedStyleMember::Environment(environment),
    ] = attached.body().members()
    else {
        panic!("clean Style source order")
    };
    let [AttachedStyleMember::Rule(nested_rule)] = environment.body().members() else {
        panic!("clean nested Style rule")
    };
    CleanStyleSyntax {
        token_type: token
            .type_annotation()
            .expect("typed Style token")
            .value()
            .syntax()
            .id(),
        token_value: style_expression_syntax(token.value()),
        rule_values: rule
            .body()
            .declarations()
            .iter()
            .map(|declaration| style_expression_syntax(declaration.value()))
            .collect(),
        clause_values: environment
            .condition()
            .clauses()
            .iter()
            .map(|clause| style_expression_syntax(clause.value()))
            .collect(),
        nested_rule_value: style_expression_syntax(nested_rule.body().declarations()[0].value()),
    }
}

#[test]
fn canonical_style_lowers_typed_payloads_and_exact_child_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-style-clean",
        concat!(
            "/// Main native theme.\n",
            "#[preview]\n",
            "pub style theme {\n",
            "    token color.text: Color = white\n",
            "    Panel Button.primary:hover > .label:active {\n",
            "        background-color = color.text\n",
            "        append shadow-list = shadow\n",
            "    }\n",
            "    when environment(color-scheme == dark, text-scale >= 1) {\n",
            "        Panel { opacity = 1 }\n",
            "    }\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let syntax = clean_style_syntax(&parsed);

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, style) = style_item(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert!(style.id().as_resolved().is_some());
    let [token] = style.tokens() else {
        panic!("one retained Style token")
    };
    assert!(token.id().as_resolved().is_some());
    let token_type = token.value_type().expect("retained Style token type");
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<TypeId>(syntax.token_type),
        Some(token_type)
    );
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.token_value),
        Some(token.value())
    );

    let [
        HirStyleBodyItem::Rule(rule),
        HirStyleBodyItem::Environment(environment),
    ] = style.body()
    else {
        panic!("Style tokens are split from the ordered rule/environment payload")
    };
    let sequences = rule.selector().sequences();
    assert_eq!(sequences.len(), 3);
    assert_eq!(sequences[0].relation_to_previous(), None);
    assert_eq!(
        sequences[1].relation_to_previous(),
        Some(HirStyleCombinator::Descendant)
    );
    assert_eq!(
        sequences[2].relation_to_previous(),
        Some(HirStyleCombinator::Child)
    );
    assert_eq!(rule.declarations().len(), 2);
    assert_eq!(
        rule.declarations()[0].operation(),
        HirStyleAssignOperation::Replace
    );
    assert_eq!(
        rule.declarations()[1].operation(),
        HirStyleAssignOperation::Append
    );
    for (syntax, declaration) in syntax.rule_values.iter().zip(rule.declarations()) {
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(*syntax),
            Some(declaration.value())
        );
    }

    assert_eq!(environment.clauses().len(), 2);
    assert_eq!(
        environment.clauses()[0].field(),
        HirStyleEnvironmentField::ColorScheme
    );
    assert_eq!(
        environment.clauses()[0].comparison(),
        HirStyleEnvironmentComparison::Equal
    );
    assert_eq!(
        environment.clauses()[1].field(),
        HirStyleEnvironmentField::TextScale
    );
    assert_eq!(
        environment.clauses()[1].comparison(),
        HirStyleEnvironmentComparison::GreaterOrEqual
    );
    for (syntax, clause) in syntax.clause_values.iter().zip(environment.clauses()) {
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(*syntax),
            Some(clause.value())
        );
    }
    let [HirStyleBodyItem::Rule(nested_rule)] = environment.body() else {
        panic!("retained nested Style rule")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.nested_rule_value),
        Some(nested_rule.declarations()[0].value())
    );
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);
}

fn token_child_syntax(token: &AttachedStyleToken) -> (Option<SyntaxNodeId>, SyntaxNodeId) {
    (
        token
            .type_annotation()
            .map(|annotation| annotation.value().syntax().id()),
        style_expression_syntax(token.value()),
    )
}

fn assert_missing_style_value(module: &HirModule, syntax: SyntaxNodeId, owner: ExprId) {
    assert_eq!(
        module.slots().prepared_source_owner::<ExprId>(syntax),
        Some(owner)
    );
    let metadata = module.slots().resolve(owner).unwrap();
    assert!(matches!(metadata.origin(), HirOrigin::Source(origin) if origin.syntax() == syntax));
    assert!(matches!(
        metadata.source_site(),
        HirSourceSite::Insertion(_)
    ));
    let expression = module
        .arenas()
        .expressions()
        .resolve(module.slots(), owner)
        .unwrap();
    assert_eq!(
        expression.kind(),
        &HirExprKind::Error(HirExprError::new(
            HirGenericExprIssue::TransactionalChildFailure
        ))
    );
    assert_eq!(
        expression.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Whole,
        })
    );
    let diagnostics = module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(diagnostic)
                if diagnostic.owner() == SyntheticOwner::Expr(owner) =>
            {
                Some(diagnostic)
            }
            HirDiagnostic::Syntax(_) | HirDiagnostic::Recovery(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].primary_role(),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::Whole,
        })
    );
    assert_eq!(diagnostics[0].primary(), metadata.source_site());
}

#[test]
fn missing_style_values_keep_their_parser_owned_source_identity() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-style-missing-values",
        concat!(
            "style @style.missing_values {\n",
            "    token color.text =\n",
            "    Panel { opacity = }\n",
            "    when environment(text-scale >= ) {\n",
            "        Label { opacity = 1 }\n",
            "    }\n",
            "}\n",
        ),
    );
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => Some(style.semantics().unwrap()),
            _ => None,
        })
        .expect("typed recovered Style attachment");
    let [
        AttachedStyleMember::Token(token),
        AttachedStyleMember::Rule(rule),
        AttachedStyleMember::Environment(environment),
    ] = attached.body().members()
    else {
        panic!("three retained Style members")
    };
    let token_syntax = token.value().missing().expect("missing token value").id();
    let property_syntax = rule.body().declarations()[0]
        .value()
        .missing()
        .expect("missing property value")
        .id();
    let clause_syntax = environment.condition().clauses()[0]
        .value()
        .missing()
        .expect("missing environment value")
        .id();

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, style) = style_item(&module, 0);
    assert!(item.state().is_poisoned());
    let [token] = style.tokens() else {
        panic!("one retained Style token")
    };
    let [
        HirStyleBodyItem::Rule(rule),
        HirStyleBodyItem::Environment(environment),
    ] = style.body()
    else {
        panic!("retained Style rule and environment")
    };

    assert_missing_style_value(&module, token_syntax, token.value());
    assert_missing_style_value(&module, property_syntax, rule.declarations()[0].value());
    assert_missing_style_value(&module, clause_syntax, environment.clauses()[0].value());
}

#[test]
fn whole_recovered_environment_and_nested_token_allocate_no_unowned_children() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-style-recovery-allocation",
        concat!(
            "style @style.recover {\n",
            "    token top: Color = white\n",
            "    when environment() {\n",
            "        token skipped_empty: Color = red\n",
            "        Panel { opacity = 1 }\n",
            "    }\n",
            "    when environment(color-scheme == dark) {\n",
            "        token skipped_nested: Color = blue\n",
            "        Panel { opacity = 2 }\n",
            "    }\n",
            "}\n",
        ),
    );
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => Some(style.semantics().unwrap()),
            _ => None,
        })
        .expect("typed recovered Style attachment");
    let [
        AttachedStyleMember::Token(top),
        AttachedStyleMember::Environment(empty),
        AttachedStyleMember::Environment(retained),
    ] = attached.body().members()
    else {
        panic!("recovered Style source order")
    };
    let [
        AttachedStyleMember::Token(empty_token),
        AttachedStyleMember::Rule(empty_rule),
    ] = empty.body().members()
    else {
        panic!("empty-condition environment body")
    };
    let [
        AttachedStyleMember::Token(nested_token),
        AttachedStyleMember::Rule(nested_rule),
    ] = retained.body().members()
    else {
        panic!("retained environment body")
    };
    let top_children = token_child_syntax(top);
    let skipped_children = [
        token_child_syntax(empty_token),
        token_child_syntax(nested_token),
    ];
    let skipped_empty_property =
        style_expression_syntax(empty_rule.body().declarations()[0].value());
    let retained_clause = style_expression_syntax(retained.condition().clauses()[0].value());
    let retained_property = style_expression_syntax(nested_rule.body().declarations()[0].value());

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, style) = style_item(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    let [top] = style.tokens() else {
        panic!("only the sheet-level token is retained")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<TypeId>(top_children.0.unwrap()),
        top.value_type()
    );
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(top_children.1),
        Some(top.value())
    );
    let [
        HirStyleBodyItem::Recovered(HirStyleBodyIssue::Missing),
        HirStyleBodyItem::Environment(environment),
    ] = style.body()
    else {
        panic!("whole-recovered and retained environment rows")
    };
    let [
        HirStyleBodyItem::Recovered(HirStyleBodyIssue::Unexpected),
        HirStyleBodyItem::Rule(rule),
    ] = environment.body()
    else {
        panic!("nested token must become one recovered body row")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(retained_clause),
        Some(environment.clauses()[0].value())
    );
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(retained_property),
        Some(rule.declarations()[0].value())
    );
    for (ty, value) in skipped_children {
        assert_eq!(
            ty.and_then(|syntax| module.slots().prepared_source_owner::<TypeId>(syntax)),
            None,
            "whole-recovered or unexpected tokens must not allocate TypeIds"
        );
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(value),
            None,
            "whole-recovered or unexpected tokens must not allocate ExprIds"
        );
    }
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(skipped_empty_property),
        None,
        "a whole-recovered environment subtree has no executable child owner"
    );
    assert_item_owner_whole_recovery(&module, module.source_ordered_items()[0]);
}

#[test]
fn whole_recovered_delimiters_drop_children_but_outer_close_recovery_retains_them() {
    let rule_missing = parse(
        "arcweft-test://proof/final-hir-style-rule-missing-close",
        "style @style.rule_missing {\nPanel { opacity = 1\n",
    );
    let rule_attached = rule_missing
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => Some(style.semantics().unwrap()),
            _ => None,
        })
        .unwrap();
    let [AttachedStyleMember::Rule(rule)] = rule_attached.body().members() else {
        panic!("whole-recovered Style rule")
    };
    let dropped_rule_value = style_expression_syntax(rule.body().declarations()[0].value());
    let rule_key = module_key(&rule_missing);
    let mut rule_database = HirDatabase::try_new().unwrap();
    let rule_module = lower(&mut rule_database, &rule_missing, &rule_key);
    let (_, _, rule_style) = style_item(&rule_module, 0);
    assert!(matches!(
        rule_style.body(),
        [HirStyleBodyItem::Recovered(HirStyleBodyIssue::Malformed)]
    ));
    assert_eq!(
        rule_module
            .slots()
            .prepared_source_owner::<ExprId>(dropped_rule_value),
        None
    );

    let environment_missing = parse(
        "arcweft-test://proof/final-hir-style-environment-missing-body",
        "style @style.environment_missing {\nwhen environment(color-scheme == dark)\n",
    );
    let environment_attached = environment_missing
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => Some(style.semantics().unwrap()),
            _ => None,
        })
        .unwrap();
    let [AttachedStyleMember::Environment(environment)] = environment_attached.body().members()
    else {
        panic!("whole-recovered Style environment")
    };
    let dropped_clause_value =
        style_expression_syntax(environment.condition().clauses()[0].value());
    let environment_key = module_key(&environment_missing);
    let mut environment_database = HirDatabase::try_new().unwrap();
    let environment_module = lower(
        &mut environment_database,
        &environment_missing,
        &environment_key,
    );
    let (_, _, environment_style) = style_item(&environment_module, 0);
    assert!(matches!(
        environment_style.body(),
        [HirStyleBodyItem::Recovered(HirStyleBodyIssue::Missing)]
    ));
    assert_eq!(
        environment_module
            .slots()
            .prepared_source_owner::<ExprId>(dropped_clause_value),
        None
    );

    let outer_missing = parse(
        "arcweft-test://proof/final-hir-style-outer-missing-close",
        "style @style.outer_missing {\nPanel { opacity = 1 }\n",
    );
    let outer_attached = outer_missing
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => Some(style.semantics().unwrap()),
            _ => None,
        })
        .unwrap();
    let [AttachedStyleMember::Rule(rule)] = outer_attached.body().members() else {
        panic!("retained rule below an outer-close recovery")
    };
    let retained_value = style_expression_syntax(rule.body().declarations()[0].value());
    let outer_key = module_key(&outer_missing);
    let mut outer_database = HirDatabase::try_new().unwrap();
    let outer_module = lower(&mut outer_database, &outer_missing, &outer_key);
    let (_, outer_item, outer_style) = style_item(&outer_module, 0);
    assert_eq!(
        outer_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let [HirStyleBodyItem::Rule(rule)] = outer_style.body() else {
        panic!("outer-close recovery retains the complete rule")
    };
    assert_eq!(
        outer_module
            .slots()
            .prepared_source_owner::<ExprId>(retained_value),
        Some(rule.declarations()[0].value())
    );
}

#[test]
fn style_id_recovery_keeps_parser_owned_shape_without_reparsing() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-style-id-recovery",
        concat!(
            "style broken. {}\n",
            "style {}\n",
            "style @view.foreign {}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, malformed, broken) = style_item(&module, 0);
    assert_eq!(
        malformed.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    let recovery = broken.id().recovery().expect("invalid dotted Style ID");
    assert_eq!(
        recovery.shape(),
        HirIdRefShape::Relative {
            parent_depth: 0,
            suffix_segment_count: 2,
        }
    );
    assert_eq!(
        recovery.issue(),
        HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix)
    );

    let (_, missing_item, missing) = style_item(&module, 1);
    assert_eq!(
        missing_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingId)
    );
    assert_eq!(
        missing.id().recovery().expect("missing Style ID").shape(),
        HirIdRefShape::Missing
    );

    let (_, wrong_family_item, wrong_family) = style_item(&module, 2);
    assert_eq!(
        wrong_family_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    assert!(wrong_family.id().as_resolved().is_some());
}

#[test]
fn style_hir_limits_accept_exact_and_reject_one_over() {
    let members = HirLimit::DeclarationMembers.maximum();
    preflight_style_member_count(members).unwrap();
    let Err(HirLowerFailure::Limit(error)) = preflight_style_member_count(members + 1) else {
        panic!("one-over Style aggregate member limit")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), members + 1);
    assert_eq!(error.maximum(), members);

    let depth = HirLimit::StyleNestingDepth.maximum();
    preflight_style_nesting_depth(depth).unwrap();
    let Err(HirLowerFailure::Limit(error)) = preflight_style_nesting_depth(depth + 1) else {
        panic!("one-over Style nesting limit")
    };
    assert_eq!(error.limit(), HirLimit::StyleNestingDepth);
    assert_eq!(error.observed(), depth + 1);
    assert_eq!(error.maximum(), depth);
}

#[test]
fn mixed_style_aggregate_accepts_the_exact_hir_member_limit() {
    // Fixed cost is twelve: one token; one rule plus two selector sequences,
    // two predicates, and one declaration; one environment plus one clause;
    // and its nested rule, selector sequence, and declaration.
    let maximum = HirLimit::DeclarationMembers.maximum();
    let fixed_cost = 12_usize;
    let mut source = String::from(
        "style exact_budget {\n\
             token seed = 0\n\
             Panel:hover > Button:active { opacity = 1 }\n\
             when environment(text-scale >= 1) {\n\
                 Label { opacity = 1 }\n\
             }\n",
    );
    for ordinal in 0..maximum - fixed_cost {
        writeln!(source, "token filler_{ordinal} = 0").unwrap();
    }
    source.push_str("}\n");

    let parsed = parse("arcweft-test://proof/final-hir-style-exact-budget", &source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, style) = style_item(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(style.tokens().len(), maximum - fixed_cost + 1);
    assert_eq!(style.body().len(), 2);
}

#[test]
fn style_recovery_matrix_keeps_typed_selector_property_environment_and_body_rows() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-style-recovery-matrix",
        concat!(
            "style @style.missing_selector {\n",
            "    { opacity = 1 }\n",
            "}\n",
            "style @style.invalid_relation {\n",
            "    > Panel { opacity = 1 }\n",
            "}\n",
            "style @style.invalid_component {\n",
            "    . { opacity = 1 }\n",
            "}\n",
            "style @style.assignment_recovery {\n",
            "    Panel {\n",
            "        opacity 1\n",
            "        size += 2\n",
            "    }\n",
            "}\n",
            "style @style.environment_recovery {\n",
            "    when environment(\n",
            "        text_scale == 1,\n",
            "        == 2,\n",
            "        contrast high,\n",
            "        contrast = high\n",
            "    ) {}\n",
            "}\n",
            "style @style.missing_body\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    for ordinal in 0..5 {
        let (_, item, _) = style_item(&module, ordinal);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
        );
    }

    let (_, _, missing_selector) = style_item(&module, 0);
    let [HirStyleBodyItem::Rule(rule)] = missing_selector.body() else {
        panic!("missing selector retains one typed rule")
    };
    assert_eq!(
        rule.selector().recovery_issue(),
        Some(HirStyleSelectorIssue::MissingSequence)
    );

    let (_, _, invalid_relation) = style_item(&module, 1);
    let [HirStyleBodyItem::Rule(rule)] = invalid_relation.body() else {
        panic!("invalid selector relation retains one typed rule")
    };
    assert_eq!(
        rule.selector().recovery_issue(),
        Some(HirStyleSelectorIssue::InvalidRelation)
    );

    let (_, _, invalid_component) = style_item(&module, 2);
    let [HirStyleBodyItem::Rule(rule)] = invalid_component.body() else {
        panic!("invalid selector component retains one typed rule")
    };
    assert_eq!(
        rule.selector().recovery_issue(),
        Some(HirStyleSelectorIssue::InvalidComponent)
    );

    let (_, _, assignment_recovery) = style_item(&module, 3);
    let [HirStyleBodyItem::Rule(rule)] = assignment_recovery.body() else {
        panic!("assignment recovery retains one typed rule")
    };
    assert_eq!(
        rule.declarations()
            .iter()
            .map(|declaration| declaration.operation())
            .collect::<Vec<_>>(),
        [
            HirStyleAssignOperation::Recovered(HirStyleAssignOperationIssue::Missing),
            HirStyleAssignOperation::Recovered(HirStyleAssignOperationIssue::Invalid),
        ]
    );

    let (_, _, environment_recovery) = style_item(&module, 4);
    let [HirStyleBodyItem::Environment(environment)] = environment_recovery.body() else {
        panic!("field and comparison recovery retains one typed environment")
    };
    assert_eq!(environment.clauses().len(), 4);
    assert_eq!(
        environment.clauses()[0].field(),
        HirStyleEnvironmentField::Recovered(HirStyleEnvironmentFieldIssue::Unknown)
    );
    assert_eq!(
        environment.clauses()[0].comparison(),
        HirStyleEnvironmentComparison::Equal
    );
    assert_eq!(
        environment.clauses()[1].field(),
        HirStyleEnvironmentField::Recovered(HirStyleEnvironmentFieldIssue::Missing)
    );
    assert_eq!(
        environment.clauses()[1].comparison(),
        HirStyleEnvironmentComparison::Equal
    );
    assert_eq!(
        environment.clauses()[2].field(),
        HirStyleEnvironmentField::Contrast
    );
    assert_eq!(
        environment.clauses()[2].comparison(),
        HirStyleEnvironmentComparison::Recovered(HirStyleEnvironmentComparisonIssue::Missing)
    );
    assert_eq!(
        environment.clauses()[3].field(),
        HirStyleEnvironmentField::Contrast
    );
    assert_eq!(
        environment.clauses()[3].comparison(),
        HirStyleEnvironmentComparison::Recovered(HirStyleEnvironmentComparisonIssue::Invalid)
    );

    let (_, missing_body_item, missing_body) = style_item(&module, 5);
    assert_eq!(
        missing_body_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert!(missing_body.tokens().is_empty());
    assert!(missing_body.body().is_empty());
}

#[test]
fn style_name_bytes_accept_exact_and_reject_one_over_atomically() {
    let maximum = HirLimit::NameBytes.maximum();
    let exact_name = "a".repeat(maximum);
    let exact = parse(
        "arcweft-test://proof/final-hir-style-name-exact",
        &format!("style name_exact {{\n    {exact_name} {{ opacity = 1 }}\n}}\n"),
    );
    let exact_key = module_key(&exact);
    let mut exact_database = HirDatabase::try_new().unwrap();
    let exact_module = lower(&mut exact_database, &exact, &exact_key);
    let (_, exact_item, _) = style_item(&exact_module, 0);
    assert_eq!(exact_item.state(), &HirItemPoisonState::Clean);

    let one_over_name = "a".repeat(maximum + 1);
    let one_over = parse(
        "arcweft-test://proof/final-hir-style-name-one-over",
        &format!("style name_one_over {{\n    {one_over_name} {{ opacity = 1 }}\n}}\n"),
    );
    let one_over_key = module_key(&one_over);
    let mut one_over_database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&one_over_database, &one_over, &one_over_key);
    let error = transaction
        .lower_attached_source_file_items(&one_over.tree())
        .expect_err("one-over Style name must fail final-HIR preflight");
    let HirLowerFailure::Limit(error) = error else {
        panic!("one-over Style name must report its typed HIR limit")
    };
    assert_eq!(error.limit(), HirLimit::NameBytes);
    assert_eq!(error.observed(), maximum + 1);
    assert_eq!(error.maximum(), maximum);
    assert!(transaction.finish(&mut one_over_database).is_err());
    assert!(one_over_database.current(&one_over_key).is_none());
}
