use std::collections::HashMap;
use std::fmt::Write as _;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::{
    AstNode, AttachedFlowContractMode, AttachedFlowContractOperands, AttachedFlowIdSyntax,
    AttachedFlowIdentity, AttachedFlowReturnSyntax, AttachedFlowSignatureRecovery,
    AttachedRequiredFlowBody, FlowItemKind,
};
use crate::attachment::node::{LetChoiceStatementKind, OnStatementKind, ThreadExpressionKind};
use crate::attachment::{
    AttachedChoiceCompactAction, AttachedChoiceItem, AttachedChoiceMatchArmBody,
    AttachedChoiceOptionField, AttachedChoicePlanItem, AttachedChoiceSuiteSource,
    AttachedExpressionNode, AttachedRequiredChoiceBody, AttachedRequiredChoiceMatchBody,
    AttachedRequiredChoiceOptionBody, AttachedRequiredChoicePlanBody,
    AttachedRequiredChoiceViewBody, AttachedRequiredIncludeTarget,
    AttachedRequiredNestedThreadFlowBody, AttachedRequiredThreadExpressionBody,
    AttachedSelectBindingName, AttachedSelectBranch, AttachedSelectStatementForm,
    AttachedSourceLocaleValue, AttachedThreadFlowItem, AttachedThreadFlowItemFamily,
    AttachedTriggerPattern, GrammarIdentityMap, RequiredStatementExpressionNode, SyntaxDatabaseId,
    SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::id_ref::SyntaxIdRefPart;
use crate::name::SyntaxNameIssue;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/flow-contract-attachment-test").unwrap(),
            SourceName::path("flow-contract-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(211).unwrap());
    let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
    let snapshot = SyntaxSnapshotId::new(
        lineage,
        SourceSnapshotId::initial(document.display_name().clone()),
    );
    let identities = build
        .index()
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                entry.path().clone(),
                SyntaxNodeId::new(
                    lineage,
                    NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    attach_typed_tree(
        &build,
        &GrammarIdentityMap::new(identities),
        snapshot,
        document,
    )
    .unwrap()
}

fn flow(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<FlowItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::FlowItem)
        .unwrap()
        .cast()
        .unwrap()
}

#[test]
fn flow_contract_attachment_preserves_heterogeneous_order_modes_and_keyword_sites() {
    let source = concat!(
        "flow contract_matrix(state: State)\n",
        "requires prove state.ready\n",
        "effects { asset.read }\n",
        "ensures check state.ok\n",
        "reads { state.value }\n",
        "invariant debug state.valid\n",
        "ensures no_effect network.request\n",
        "modifies { state.value }\n",
        "assume external_ok\n",
        "decreases state.remaining\n",
        "{}\n",
    );
    let snapshot = attach(source);
    let clauses = flow(&snapshot).contract_clauses().unwrap();

    assert_eq!(
        clauses
            .iter()
            .map(super::AttachedFlowContractClause::kind)
            .collect::<Vec<_>>(),
        [
            SyntaxKind::RequiresClause,
            SyntaxKind::EffectsClause,
            SyntaxKind::EnsuresClause,
            SyntaxKind::ReadsClause,
            SyntaxKind::InvariantClause,
            SyntaxKind::NoEffectClause,
            SyntaxKind::ModifiesClause,
            SyntaxKind::AssumeClause,
            SyntaxKind::DecreasesClause,
        ]
    );
    assert_eq!(
        clauses
            .iter()
            .map(super::AttachedFlowContractClause::source_ordinal)
            .collect::<Vec<_>>(),
        (0_u16..9).collect::<Vec<_>>()
    );
    assert!(matches!(
        clauses[0].mode(),
        Some(AttachedFlowContractMode::Prove(_))
    ));
    assert!(matches!(
        clauses[2].mode(),
        Some(AttachedFlowContractMode::Check(_))
    ));
    assert!(matches!(
        clauses[4].mode(),
        Some(AttachedFlowContractMode::Debug(_))
    ));

    let no_effect = &clauses[5];
    let ensures_start = source.find("ensures no_effect").unwrap();
    let no_effect_start = ensures_start + "ensures ".len();
    assert_eq!(
        no_effect.keyword().range(),
        SourceRange::new(ensures_start, ensures_start + "ensures".len())
    );
    assert_eq!(
        no_effect.no_effect_keyword().unwrap().range(),
        SourceRange::new(no_effect_start, no_effect_start + "no_effect".len())
    );
    let AttachedFlowContractOperands::One(no_effect_operand) = no_effect.operands() else {
        panic!("ensures no_effect must retain one scalar operand");
    };
    let operand_start = source.find("network.request").unwrap();
    assert_eq!(
        no_effect_operand.whole_source_span().range(),
        SourceRange::new(operand_start, operand_start + "network.request".len())
    );

    let effects = clauses[1].list().unwrap();
    assert!(effects.is_braced());
    assert_eq!(effects.operands().len(), 1);
    assert!(effects.close_state().is_some());
    assert!(matches!(
        clauses[7].operands(),
        AttachedFlowContractOperands::One(_)
    ));
}

#[test]
fn flow_contract_attachment_distinguishes_empty_braced_and_missing_unbraced_lists() {
    let snapshot = attach(concat!(
        "flow list_recovery()\n",
        "effects {}\n",
        "reads\n",
        "{}\n",
    ));
    let clauses = flow(&snapshot).contract_clauses().unwrap();

    let empty = clauses[0].list().unwrap();
    assert!(empty.is_braced());
    assert!(empty.operands().is_empty());
    assert!(!empty.has_recovery());

    let missing = clauses[1].list().unwrap();
    assert!(!missing.is_braced());
    assert_eq!(missing.operands().len(), 1);
    assert!(missing.has_recovery());
}

#[test]
fn flow_contract_attachment_retains_unclosed_list_without_losing_following_clause() {
    let snapshot = attach(concat!(
        "flow list_close_recovery()\n",
        "effects { asset.read\n",
        "requires state.ready\n",
        "{}\n",
    ));
    let clauses = flow(&snapshot).contract_clauses().unwrap();

    assert_eq!(clauses.len(), 2);
    let list = clauses[0].list().unwrap();
    assert!(list.is_braced());
    assert_eq!(list.operands().len(), 1);
    assert!(matches!(
        list.close_state(),
        Some(crate::attachment::source_file::AttachedDelimiterState::Missing(_))
    ));
    assert_eq!(clauses[1].kind(), SyntaxKind::RequiresClause);
}

#[test]
fn flow_attachment_owns_the_four_identity_states_without_source_reconstruction() {
    let cases = [
        ("flow opening {}", "name"),
        ("flow @flow.opening {}", "public_id"),
        ("flow @flow.opening opening {}", "both"),
        ("flow {}", "missing"),
    ];

    for (source, expected) in cases {
        let snapshot = attach(source);
        let declaration = flow(&snapshot).semantics().unwrap();
        match (expected, declaration.identity()) {
            ("name", AttachedFlowIdentity::Name { name }) => {
                assert_eq!(name.value().as_str(), "opening");
            }
            ("public_id", AttachedFlowIdentity::PublicId { public_id }) => {
                assert!(public_id.is_canonical_flow_family());
                assert!(matches!(
                    public_id.value(),
                    AttachedFlowIdSyntax::Authored(_)
                ));
            }
            ("both", AttachedFlowIdentity::PublicIdAndName { public_id, name }) => {
                assert!(public_id.is_canonical_flow_family());
                assert_eq!(name.value().as_str(), "opening");
            }
            ("missing", AttachedFlowIdentity::Missing { insertion, .. }) => {
                assert!(insertion.range().is_empty());
            }
            _ => panic!("unexpected Flow identity state for {source}"),
        }
    }
}

#[test]
fn flow_attachment_retains_empty_markers_and_wrong_family_poison_as_typed_states() {
    let snapshot = attach("flow @flow:. opening {}");
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedFlowIdentity::PublicIdAndName { public_id, name } = declaration.identity() else {
        panic!("empty Flow marker with a following name must retain both components");
    };
    assert!(matches!(
        public_id.value(),
        AttachedFlowIdSyntax::DerivedFromEmptyMarker {
            marker_family: Some(family)
        } if family.as_str() == "flow"
    ));
    assert_eq!(name.value().as_str(), "opening");
    assert_eq!(
        public_id
            .components()
            .iter()
            .map(super::AttachedFlowIdComponent::part)
            .collect::<Vec<_>>(),
        [
            SyntaxIdRefPart::Whole,
            SyntaxIdRefPart::Family,
            SyntaxIdRefPart::FamilySeparator,
            SyntaxIdRefPart::SuffixSegment { ordinal: 0 },
        ]
    );
    assert!(
        public_id
            .components()
            .last()
            .unwrap()
            .source_span()
            .range()
            .is_empty()
    );
    assert!(public_id.syntax().range().end() <= name.syntax().range().start());

    let snapshot = attach("flow @. {}");
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedFlowIdentity::Missing {
        attempted_public_id: Some(public_id),
        ..
    } = declaration.identity()
    else {
        panic!("missing marker suffix name must retain attempted public-ID evidence");
    };
    assert!(matches!(
        public_id.value(),
        AttachedFlowIdSyntax::DerivedFromEmptyMarker {
            marker_family: None
        }
    ));

    let snapshot = attach("flow @view.opening {}");
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedFlowIdentity::PublicId { public_id } = declaration.identity() else {
        panic!("wrong-family public ID must not fall back to a name");
    };
    assert!(!public_id.is_canonical_flow_family());
    assert!(public_id.has_recovery());
}

#[test]
fn flow_attachment_aggregates_shared_signature_and_statement_only_body_owners() {
    let source = "flow render<T>(value: T) -> T where T: Display { value }";
    let snapshot = attach(source);
    let declaration = flow(&snapshot).semantics().unwrap();
    let signature = declaration.signature();
    assert!(signature.generics().is_some());
    assert_eq!(signature.parameters().unwrap().parameters().len(), 1);
    let parameter = &signature.parameters().unwrap().parameters()[0];
    let colon_start = source.find(':').unwrap();
    assert_eq!(
        parameter.colon().source_span().range(),
        SourceRange::new(colon_start, colon_start + 1)
    );
    let AttachedFlowReturnSyntax::Authored(result) = signature.result() else {
        panic!("authored return must retain the shared callable return owner");
    };
    let arrow_start = source.find("->").unwrap();
    assert_eq!(
        result.arrow().source_span().range(),
        SourceRange::new(arrow_start, arrow_start + 2)
    );
    assert!(signature.where_clause().is_some());
    assert!(signature.end().range().is_empty());

    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("authored Flow body must be present");
    };
    assert_eq!(body.items().len(), 1);
    assert_eq!(body.items()[0].kind(), SyntaxKind::ExpressionStatement);
    assert!(!body.open().range().is_empty());
    assert!(!body.close().range().is_empty());

    let snapshot = attach("flow omitted_return {}");
    let declaration = flow(&snapshot).semantics().unwrap();
    assert!(matches!(
        declaration.signature().result(),
        AttachedFlowReturnSyntax::Omitted
    ));
}

#[test]
fn flow_missing_required_body_retains_the_typed_insertion_owner() {
    let source = "flow unfinished";
    let snapshot = attach(source);
    let declaration = flow(&snapshot).semantics().unwrap();

    assert!(declaration.has_recovery());
    let AttachedRequiredFlowBody::Missing {
        syntax,
        missing,
        insertion,
    } = declaration.body()
    else {
        panic!("recognized Flow without a body must retain typed MissingBody recovery");
    };
    assert_eq!(syntax.kind(), SyntaxKind::FlowBody);
    assert_eq!(missing.kind(), SyntaxKind::MissingBody);
    assert!(missing.range().is_empty());
    assert_eq!(missing.source_span(), insertion.clone());
    assert_eq!(
        insertion.range(),
        SourceRange::new(source.len(), source.len())
    );
}

#[test]
fn flow_parameter_retains_the_parser_owned_missing_colon_insertion() {
    let source = "flow broken(value) {}";
    let snapshot = attach(source);
    let declaration = flow(&snapshot).semantics().unwrap();
    let parameter = &declaration.signature().parameters().unwrap().parameters()[0];
    let insertion = source.find(')').unwrap();

    assert!(parameter.colon().is_missing());
    assert_eq!(
        parameter.colon().source_span().range(),
        SourceRange::new(insertion, insertion)
    );
    assert!(parameter.has_recovery());
}

#[test]
fn line_plan_at_indentation_is_the_same_callback_expression_as_braces_and_let() {
    let snapshot = attach(concat!(
        "flow line_plan_at_surface {\n",
        "    let (_, line) = alice[こんにちは。]\n",
        "    with:\n",
        "        at(0.42s) {\n",
        "            alice.stage.look(.smile)\n",
        "        }\n",
        "        at(0.84s):\n",
        "            alice.stage.look(.worried)\n",
        "        let cue = at(1.2s):\n",
        "            alice.stage.look(.surprised)\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("line-plan fixture requires a present Flow body");
    };
    let AttachedThreadFlowItem::Statement(line) = &body.items()[0] else {
        panic!("the first flow item must remain the dialogue binding");
    };
    let children = line.syntax().children_with_role(SyntaxRole::Initializer);
    let [dialogue] = children.as_slice() else {
        panic!("the dialogue binding must own one initializer expression");
    };
    let application = AttachedExpressionNode::from_syntax(dialogue.clone()).unwrap();
    let plan = application
        .dialogue_line_plan()
        .unwrap()
        .expect("dialogue application must own the line plan");
    let items = plan.body().items();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].kind(), SyntaxKind::ExpressionStatement);
    assert_eq!(items[1].kind(), SyntaxKind::ExpressionStatement);
    assert_eq!(items[2].kind(), SyntaxKind::LetStatement);

    for item in &items[..2] {
        let children = item.syntax().children_with_role(SyntaxRole::Initializer);
        let [expression] = children.as_slice() else {
            panic!("bare line-plan callback must own one initializer expression");
        };
        let expression = AttachedExpressionNode::from_syntax(expression.clone()).unwrap();
        assert!(matches!(
            expression.projection(),
            crate::expressions::ExpressionProjection::Call(
                crate::expressions::SyntaxCallProjection::CallbackBlock(_)
            )
        ));
    }

    let children = items[2]
        .syntax()
        .children_with_role(SyntaxRole::Initializer);
    let [initializer] = children.as_slice() else {
        panic!("callback let must own one initializer expression");
    };
    let initializer = AttachedExpressionNode::from_syntax(initializer.clone()).unwrap();
    assert!(matches!(
        initializer.projection(),
        crate::expressions::ExpressionProjection::Call(
            crate::expressions::SyntaxCallProjection::CallbackBlock(_)
        )
    ));
}

fn thread_flow_matrix_body() -> &'static str {
    concat!(
        "    return unit\n",
        "    alice[こんにちは。]\n",
        "    choice {}\n",
        "    if ready {}\n",
        "    if let value = source {}\n",
        "    match value { _ => {} }\n",
        "    loop {}\n",
        "    while ready {}\n",
        "    while let value = source {}\n",
        "    for value in source {}\n",
        "    select {}\n",
        "    source locale en-US {}\n",
        "    scope local {}\n",
        "    include @flow.shared\n",
        "    await task with {}\n",
        "    ???\n",
    )
}

fn expected_thread_flow_families() -> [AttachedThreadFlowItemFamily; 16] {
    [
        AttachedThreadFlowItemFamily::Statement,
        AttachedThreadFlowItemFamily::DialogueApplication,
        AttachedThreadFlowItemFamily::Choice,
        AttachedThreadFlowItemFamily::If,
        AttachedThreadFlowItemFamily::IfLet,
        AttachedThreadFlowItemFamily::Match,
        AttachedThreadFlowItemFamily::Statement,
        AttachedThreadFlowItemFamily::While,
        AttachedThreadFlowItemFamily::WhileLet,
        AttachedThreadFlowItemFamily::For,
        AttachedThreadFlowItemFamily::Select,
        AttachedThreadFlowItemFamily::SourceLocale,
        AttachedThreadFlowItemFamily::Scope,
        AttachedThreadFlowItemFamily::Include,
        AttachedThreadFlowItemFamily::Statement,
        AttachedThreadFlowItemFamily::Error,
    ]
}

#[test]
fn flow_statement_only_body_projects_all_sixteen_families_in_source_order() {
    let source = format!("flow matrix {{\n{}}}\n", thread_flow_matrix_body());
    let snapshot = attach(&source);
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("matrix Flow body must be present");
    };

    let observed = body
        .items()
        .iter()
        .map(crate::attachment::AttachedThreadFlowItem::family)
        .collect::<Vec<_>>();
    let inventory = snapshot
        .nodes()
        .map(|node| (node.kind(), node.role(), node.source_text().to_owned()))
        .collect::<Vec<_>>();
    assert_eq!(observed, expected_thread_flow_families(), "{inventory:#?}");
    for (ordinal, item) in body.items().iter().enumerate() {
        assert_eq!(
            item.syntax().role(),
            crate::grammar::kinds::SyntaxRole::ThreadFlowItem(u32::try_from(ordinal).unwrap())
        );
    }
}

#[test]
fn thread_expression_uses_the_same_sixteen_family_statement_only_body() {
    let source = format!(
        "flow host {{\nlet worker = thread {{\n{}}}\n}}\n",
        thread_flow_matrix_body()
    );
    let snapshot = attach(&source);
    let thread = snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::ThreadExpression)
        .expect("Thread expression owner")
        .cast::<ThreadExpressionKind>()
        .unwrap();
    let AttachedRequiredThreadExpressionBody::Present(body) = thread.statement_body().unwrap()
    else {
        panic!("Thread expression body must be present");
    };

    assert_eq!(
        body.items()
            .iter()
            .map(crate::attachment::AttachedThreadFlowItem::family)
            .collect::<Vec<_>>(),
        expected_thread_flow_families()
    );
    assert!(
        body.has_recovery(),
        "the explicit Error row remains typed recovery"
    );
}

#[test]
fn dedicated_thread_flow_statements_expose_typed_values_and_nested_bodies() {
    let snapshot = attach(concat!(
        "flow typed_statements {\n",
        "    source locale en-US { include @flow.shared }\n",
        "    scope local { include @flow.shared }\n",
        "    scope { include @flow.shared }\n",
        "    include @flow.shared\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("typed statement fixture requires a Flow body");
    };

    let AttachedThreadFlowItem::SourceLocale(source_locale) = &body.items()[0] else {
        panic!("first item must be SourceLocale");
    };
    let source_locale = source_locale.semantics().unwrap();
    assert!(matches!(
        source_locale.locale(),
        AttachedSourceLocaleValue::Authored { value: Ok(locale), .. }
            if locale.as_str() == "en-US"
    ));
    let AttachedRequiredNestedThreadFlowBody::Present(locale_body) = source_locale.body() else {
        panic!("SourceLocale body must remain present");
    };
    assert_eq!(locale_body.items().len(), 1);
    assert!(matches!(
        locale_body.items()[0],
        AttachedThreadFlowItem::Include(_)
    ));

    let AttachedThreadFlowItem::Scope(named) = &body.items()[1] else {
        panic!("second item must be named Scope");
    };
    let named = named.semantics().unwrap();
    assert_eq!(named.name().unwrap().value().unwrap().as_str(), "local");
    assert!(matches!(
        named.body(),
        AttachedRequiredNestedThreadFlowBody::Present(nested) if nested.items().len() == 1
    ));

    let AttachedThreadFlowItem::Scope(anonymous) = &body.items()[2] else {
        panic!("third item must be anonymous Scope");
    };
    assert!(anonymous.semantics().unwrap().name().is_none());

    let AttachedThreadFlowItem::Include(include) = &body.items()[3] else {
        panic!("fourth item must be Include");
    };
    let include = include.semantics().unwrap();
    let AttachedRequiredIncludeTarget::Reference(target) = include.target() else {
        panic!("Include target must remain a typed entity reference");
    };
    assert!(target.value().value().is_ok());
    assert!(target.value().shape().has_absolute_marker());
    assert_eq!(target.value().shape().segment_count(), 2);
    assert!(!include.has_recovery());
}

#[test]
fn dedicated_thread_flow_statement_recovery_remains_typed() {
    let snapshot = attach(concat!(
        "flow recovered_statements {\n",
        "    source locale zh-hant-tw {}\n",
        "    source locale {}\n",
        "    scope 123 extra {}\n",
        "    include\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("recovery fixture requires a Flow body");
    };

    let AttachedThreadFlowItem::SourceLocale(noncanonical) = &body.items()[0] else {
        panic!("first recovery item must be SourceLocale");
    };
    assert!(matches!(
        noncanonical.semantics().unwrap().locale(),
        AttachedSourceLocaleValue::Authored { value: Err(_), .. }
    ));

    let AttachedThreadFlowItem::SourceLocale(missing) = &body.items()[1] else {
        panic!("second recovery item must be SourceLocale");
    };
    assert!(matches!(
        missing.semantics().unwrap().locale(),
        AttachedSourceLocaleValue::Missing(node) if node.range().is_empty()
    ));

    let AttachedThreadFlowItem::Scope(scope) = &body.items()[2] else {
        panic!("third recovery item must be Scope");
    };
    let scope = scope.semantics().unwrap();
    assert!(scope.name().unwrap().value().is_err());
    assert!(scope.header_recovery().is_some());
    assert!(scope.has_recovery());

    let AttachedThreadFlowItem::Include(include) = &body.items()[3] else {
        panic!("fourth recovery item must be Include");
    };
    let include = include.semantics().unwrap();
    assert!(matches!(
        include.target(),
        AttachedRequiredIncludeTarget::Missing(node) if node.range().is_empty()
    ));
    assert!(include.has_recovery());
}

#[test]
fn select_statement_preserves_unary_and_typed_branch_forms() {
    let snapshot = attach(concat!(
        "flow select_forms {\n",
        "    select selected\n",
        "    select {\n",
        "        frame frame => { include @flow.frame }\n",
        "        event .Back => { goto @flow.back }\n",
        "        value = source? => { out value }\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Select fixture requires a Flow body");
    };

    let AttachedThreadFlowItem::Select(unary) = &body.items()[0] else {
        panic!("first item must be unary Select");
    };
    let unary = unary.semantics().unwrap();
    assert!(matches!(
        unary.form(),
        AttachedSelectStatementForm::Operand(RequiredStatementExpressionNode::Expression(value))
            if value.source_text() == "selected"
    ));

    let AttachedThreadFlowItem::Select(branches) = &body.items()[1] else {
        panic!("second item must be branch Select");
    };
    let branches = branches.semantics().unwrap();
    let AttachedSelectStatementForm::Branches(branches) = branches.form() else {
        panic!("braced Select must own a typed branch block");
    };
    assert_eq!(branches.branches().len(), 3);
    assert!(matches!(
        &branches.branches()[0],
        AttachedSelectBranch::Frame { pattern, body, .. }
            if pattern.syntax().source_text() == "frame"
                && matches!(body, AttachedRequiredNestedThreadFlowBody::Present(nested)
                    if matches!(nested.items()[0], AttachedThreadFlowItem::Include(_)))
    ));
    assert!(matches!(
        &branches.branches()[1],
        AttachedSelectBranch::Event { pattern, body, .. }
            if pattern.syntax().source_text() == ".Back"
                && matches!(body, AttachedRequiredNestedThreadFlowBody::Present(nested)
                    if nested.items()[0].kind() == SyntaxKind::GotoStatement)
    ));
    assert!(matches!(
        &branches.branches()[2],
        AttachedSelectBranch::Bind {
            name:
                AttachedSelectBindingName::Authored {
                    value: Ok(name), ..
                },
            source: RequiredStatementExpressionNode::Expression(source),
            body,
            ..
        } if name.as_str() == "value"
            && source.source_text() == "source?"
            && matches!(body, AttachedRequiredNestedThreadFlowBody::Present(nested)
                if nested.items()[0].kind() == SyntaxKind::OutStatement)
    ));
    assert!(!branches.has_recovery());
}

#[test]
fn select_branch_recovery_retains_unknown_head_and_missing_body() {
    let snapshot = attach(concat!(
        "flow select_recovery {\n",
        "    select {\n",
        "        unknown head\n",
        "        frame frame =>\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Select recovery fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Select(select) = &body.items()[0] else {
        panic!("fixture must remain a Select statement");
    };
    let select = select.semantics().unwrap();
    let AttachedSelectStatementForm::Branches(branches) = select.form() else {
        panic!("recovered Select retains its branch block");
    };
    assert_eq!(branches.branches().len(), 2);
    assert!(matches!(
        &branches.branches()[0],
        AttachedSelectBranch::Recovered {
            body: AttachedRequiredNestedThreadFlowBody::Missing(missing),
            ..
        } if missing.range().is_empty()
    ));
    assert!(matches!(
        &branches.branches()[1],
        AttachedSelectBranch::Frame {
            body: AttachedRequiredNestedThreadFlowBody::Missing(missing),
            ..
        } if missing.range().is_empty()
    ));
    assert!(select.has_recovery());
}

#[test]
fn select_binding_name_retains_missing_and_invalid_authored_owners() {
    let snapshot = attach(concat!(
        "flow select_binding_name_recovery {\n",
        "    select {\n",
        "        = source => { out source }\n",
        "        1bad = other => { out other }\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Select binding-name fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Select(select) = &body.items()[0] else {
        panic!("fixture must retain one Select statement");
    };
    let select = select.semantics().unwrap();
    let AttachedSelectStatementForm::Branches(branches) = select.form() else {
        panic!("fixture must retain one Select branch block");
    };

    assert!(matches!(
        &branches.branches()[0],
        AttachedSelectBranch::Bind {
            name: AttachedSelectBindingName::Missing(syntax),
            source: RequiredStatementExpressionNode::Expression(source),
            ..
        } if syntax.range().is_empty() && source.source_text() == "source"
    ));
    assert!(matches!(
        &branches.branches()[1],
        AttachedSelectBranch::Bind {
            name: AttachedSelectBindingName::Authored {
                syntax,
                value: Err(SyntaxNameIssue::InvalidStart { spelling }),
            },
            source: RequiredStatementExpressionNode::Expression(source),
            ..
        } if syntax.source_text() == "1bad"
            && spelling.as_ref() == "1bad"
            && source.source_text() == "other"
    ));
    assert!(select.has_recovery());
}

#[test]
fn direct_and_let_choice_share_one_choice_expression_owner() {
    let snapshot = attach(concat!(
        "flow choice_owners {\n",
        "    choice @choice.direct {\n",
        "        @.only \"Only\" => unit\n",
        "    }\n",
        "    let selected = choice @choice.bound:\n",
        "        @.bound \"Bound\" => unit\n",
        "    include next_flow\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Choice owner fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 3);

    let AttachedThreadFlowItem::Choice(direct) = &body.items()[0] else {
        panic!("direct Choice must retain the dedicated flow category");
    };
    let direct_children = direct.syntax().children();
    assert_eq!(direct_children.len(), 1);
    assert_eq!(direct_children[0].kind(), SyntaxKind::ChoiceExpression);
    assert_eq!(
        direct_children[0].role(),
        crate::grammar::SyntaxRole::Initializer
    );
    let direct_expression = direct.semantics().unwrap().expression().clone();
    assert_eq!(
        direct_expression.syntax().kind(),
        SyntaxKind::ChoiceExpression
    );

    let AttachedThreadFlowItem::Statement(binding) = &body.items()[1] else {
        panic!("LetChoice must remain an ordinary Statement flow category");
    };
    assert_eq!(binding.kind(), SyntaxKind::LetChoiceStatement);
    let binding = binding.cast::<LetChoiceStatementKind>().unwrap();
    let binding = binding.semantics().unwrap();
    assert_eq!(binding.pattern().syntax().source_text(), "selected");
    let bound_expression = binding.expression();
    assert_eq!(
        bound_expression.syntax().kind(),
        SyntaxKind::ChoiceExpression
    );
    assert!(matches!(
        bound_expression.body(),
        AttachedRequiredChoiceBody::Present(_)
    ));

    assert!(matches!(
        body.items()[2],
        AttachedThreadFlowItem::Include(_)
    ));
    assert!(
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::ChoiceExpression)
            .count()
            == 2
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed Choice option/action ownership matrix is easier to audit together"
)]
fn choice_statement_owns_closed_candidate_option_and_action_families() {
    let snapshot = attach(concat!(
        "flow choice_matrix {\n",
        "    choice @choice.opening {\n",
        "        let allowed = true\n",
        "        if allowed {\n",
        "            @.listen \"Listen\" -> @flow.listen\n",
        "        } else {\n",
        "            @.quiet \"Quiet\" => unit\n",
        "        }\n",
        "        for route in routes {\n",
        "            option route.id {\n",
        "                label = route.label\n",
        "                value = route\n",
        "                visible = true\n",
        "                enabled = route.enabled\n",
        "                order = route.order\n",
        "                hotkey = route.hotkey\n",
        "                view { badge = route.badge }\n",
        "                select { goto route.target }\n",
        "                let local = route\n",
        "            }\n",
        "        }\n",
        "        match state {\n",
        "            .Ready when allowed => { @.ready \"Ready\" -> @flow.ready }\n",
        "            _ => @.wait \"Wait\" => unit\n",
        "        }\n",
        "        option @.full {\n",
        "            label(id = @text.choice.full) = \"Full\"\n",
        "            value = 1\n",
        "            select { out 1 }\n",
        "        }\n",
        "        option item in items {\n",
        "            id = item.id\n",
        "            label = item.label\n",
        "            select { out item }\n",
        "        }\n",
        "        @.compact \"Compact\" if allowed -> @flow.compact\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Choice fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain a Choice statement");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    assert_eq!(
        choice
            .id()
            .unwrap()
            .value()
            .value()
            .unwrap()
            .segments()
            .iter()
            .map(crate::id_ref::AuthoredIdSegment::as_str)
            .collect::<Vec<_>>(),
        ["choice", "opening"]
    );
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 7);
    assert!(matches!(choice_body.items()[0], AttachedChoiceItem::Let(_)));

    let AttachedChoiceItem::If(branch) = &choice_body.items()[1] else {
        panic!("second Choice item must be If");
    };
    assert!(
        matches!(branch.branches()[0].then_body(), AttachedRequiredChoiceBody::Present(body)
        if matches!(body.items()[0], AttachedChoiceItem::CompactArm(_)))
    );
    assert!(
        matches!(branch.else_body(), Some(AttachedRequiredChoiceBody::Present(body))
        if matches!(body.items()[0], AttachedChoiceItem::CompactArm(_)))
    );

    let AttachedChoiceItem::For(loop_item) = &choice_body.items()[2] else {
        panic!("third Choice item must be For");
    };
    assert_eq!(loop_item.pattern().syntax().source_text(), "route");
    let AttachedRequiredChoiceBody::Present(loop_body) = loop_item.body() else {
        panic!("Choice For body must be present");
    };
    let AttachedChoiceItem::Option(dynamic) = &loop_body.items()[0] else {
        panic!("Choice For body must own one full option");
    };
    let AttachedRequiredChoiceOptionBody::Present(dynamic_body) = dynamic.body() else {
        panic!("dynamic option body must be present");
    };
    assert_eq!(dynamic_body.fields().len(), 9);
    assert!(matches!(
        dynamic_body.fields()[0],
        AttachedChoiceOptionField::Label { .. }
    ));
    let AttachedChoiceOptionField::View(view) = &dynamic_body.fields()[6] else {
        panic!("seventh option field must be View");
    };
    assert!(
        matches!(view.body(), AttachedRequiredChoiceViewBody::Present(body)
        if body.fields().len() == 1)
    );
    let AttachedChoiceOptionField::Select(select) = &dynamic_body.fields()[7] else {
        panic!("eighth option field must be Select");
    };
    assert!(
        matches!(select.body(), AttachedRequiredNestedThreadFlowBody::Present(body)
        if body.items()[0].kind() == SyntaxKind::GotoStatement)
    );
    assert!(matches!(
        dynamic_body.fields()[8],
        AttachedChoiceOptionField::Let(_)
    ));

    let AttachedChoiceItem::Match(matched) = &choice_body.items()[3] else {
        panic!("fourth Choice item must be Match");
    };
    let AttachedRequiredChoiceMatchBody::Present(match_body) = matched.body() else {
        panic!("Choice Match body must be present");
    };
    assert_eq!(match_body.arms().len(), 2);
    assert!(matches!(
        match_body.arms()[0].body(),
        AttachedChoiceMatchArmBody::Block(_)
    ));
    assert!(matches!(
        match_body.arms()[1].body(),
        AttachedChoiceMatchArmBody::Single(_)
    ));

    let AttachedChoiceItem::Option(full) = &choice_body.items()[4] else {
        panic!("fifth Choice item must be a full option");
    };
    let AttachedRequiredChoiceOptionBody::Present(full_body) = full.body() else {
        panic!("full option body must be present");
    };
    let AttachedChoiceOptionField::Label { text_key, .. } = &full_body.fields()[0] else {
        panic!("localized label must remain a label field");
    };
    assert!(text_key.is_some());

    assert!(matches!(
        choice_body.items()[5],
        AttachedChoiceItem::OptionFor(_)
    ));
    let AttachedChoiceItem::CompactArm(compact) = &choice_body.items()[6] else {
        panic!("seventh Choice item must be a compact arm");
    };
    assert!(compact.condition().is_some());
    assert!(matches!(
        compact.action(),
        AttachedChoiceCompactAction::Goto { .. }
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn compact_choice_arm_accepts_the_shared_raw_string_literal_family() {
    let snapshot = attach(concat!(
        "flow raw_label {\n",
        "    choice {\n",
        "        @.raw r#\"Raw label\"# => unit\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    let [AttachedChoiceItem::CompactArm(arm)] = choice_body.items() else {
        panic!("raw-string fixture must retain one compact arm");
    };
    let RequiredStatementExpressionNode::Expression(label) = arm.label() else {
        panic!("raw string label must remain an authored expression");
    };
    assert_eq!(label.syntax().source_text(), "r#\"Raw label\"#");
    assert!(!choice.has_recovery());
}

#[test]
fn choice_recovery_keeps_missing_body_unknown_item_and_compact_slots_typed() {
    let snapshot = attach(concat!(
        "flow choice_recovery {\n",
        "    choice @choice.missing\n",
        "    choice {\n",
        "        unknown item\n",
        "        option @.missing_body\n",
        "        @.compact =>\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Choice recovery fixture requires a Flow body");
    };

    let AttachedThreadFlowItem::Choice(missing) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let missing = missing.semantics().unwrap();
    let missing = missing.expression();
    assert!(matches!(
        missing.body(),
        AttachedRequiredChoiceBody::Missing(node) if node.range().is_empty()
    ));

    let AttachedThreadFlowItem::Choice(recovered) = &body.items()[1] else {
        panic!("second item must remain Choice");
    };
    let recovered = recovered.semantics().unwrap();
    let recovered = recovered.expression();
    let AttachedRequiredChoiceBody::Present(recovered_body) = recovered.body() else {
        panic!("recovered Choice body remains present");
    };
    assert_eq!(recovered_body.items().len(), 3);
    assert!(matches!(
        recovered_body.items()[0],
        AttachedChoiceItem::Recovered(_)
    ));
    assert!(matches!(
        recovered_body.items()[1],
        AttachedChoiceItem::Option(ref option)
            if matches!(option.body(), AttachedRequiredChoiceOptionBody::Missing(node)
                if node.range().is_empty())
    ));
    let AttachedChoiceItem::CompactArm(compact) = &recovered_body.items()[2] else {
        panic!("malformed compact arm retains its typed root");
    };
    assert!(matches!(
        compact.label(),
        RequiredStatementExpressionNode::Missing(_)
    ));
    assert!(matches!(
        compact.action(),
        AttachedChoiceCompactAction::Out {
            value: RequiredStatementExpressionNode::Missing(_),
            ..
        }
    ));
    assert!(recovered.has_recovery());
}

#[test]
fn missing_compact_action_stops_before_comment_and_valid_sibling() {
    let snapshot = attach(concat!(
        "flow compact_boundary {\n",
        "    choice:\n",
        "        @.first \"First\"\n",
        "        // remains between candidate owners\n",
        "        @.second \"Second\" => unit\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 2);
    assert!(matches!(
        choice_body.items()[0],
        AttachedChoiceItem::CompactArm(ref arm)
            if matches!(arm.action(), AttachedChoiceCompactAction::Missing(_))
    ));
    assert!(matches!(
        choice_body.items()[1],
        AttachedChoiceItem::CompactArm(ref arm)
            if matches!(arm.action(), AttachedChoiceCompactAction::Out { .. })
    ));
    assert!(choice.has_recovery());
}

#[test]
fn missing_option_field_value_stops_before_comment_and_valid_field() {
    let snapshot = attach(concat!(
        "flow field_boundary {\n",
        "    choice:\n",
        "        option @.only:\n",
        "            label =\n",
        "            // remains between field owners\n",
        "            value = 1\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    let [AttachedChoiceItem::Option(option)] = choice_body.items() else {
        panic!("fixture must retain one full option");
    };
    let AttachedRequiredChoiceOptionBody::Present(option_body) = option.body() else {
        panic!("option body must remain present");
    };
    assert_eq!(option_body.fields().len(), 2);
    assert!(matches!(
        option_body.fields()[0],
        AttachedChoiceOptionField::Label {
            value: RequiredStatementExpressionNode::Missing(_),
            ..
        }
    ));
    assert!(matches!(
        option_body.fields()[1],
        AttachedChoiceOptionField::Value { .. }
    ));
    assert!(choice.has_recovery());
}

#[test]
fn choice_lifecycle_plan_is_one_typed_source_ordered_owner() {
    let snapshot = attach(concat!(
        "flow planned_choice {\n",
        "    choice @choice.opening {\n",
        "        @.listen \"Listen\" -> @flow.listen\n",
        "    }\n",
        "    with {\n",
        "        window = @choice_window.main\n",
        "        timeout 10s { select @choice.opening.listen }\n",
        "        cancel on input(.BackToTitle) { goto @flow.title }\n",
        "        on select selected { log.info(selected.id) }\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Choice-plan fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 1);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("Choice and its plan must remain one Flow item");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice
        .plan()
        .expect("authored `with` plan must be attached");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("Choice plan body must remain present");
    };
    assert_eq!(plan_body.items().len(), 4);

    let AttachedChoicePlanItem::Assignment(window) = &plan_body.items()[0] else {
        panic!("first plan item must be an assignment");
    };
    assert_eq!(window.key().value().unwrap().as_str(), "window");
    assert!(!window.equals().range().is_empty());

    let AttachedChoicePlanItem::Timeout(timeout) = &plan_body.items()[1] else {
        panic!("second plan item must be timeout");
    };
    assert!(
        matches!(timeout.body(), AttachedRequiredNestedThreadFlowBody::Present(body)
        if body.items()[0].kind() == SyntaxKind::SelectStatement)
    );

    let AttachedChoicePlanItem::Cancel(cancel) = &plan_body.items()[2] else {
        panic!("third plan item must be cancel-on");
    };
    assert!(matches!(
        cancel.trigger(),
        AttachedTriggerPattern::Input(trigger)
            if trigger.pattern().syntax().source_text() == ".BackToTitle"
    ));
    assert!(
        matches!(cancel.body(), AttachedRequiredNestedThreadFlowBody::Present(body)
        if body.items()[0].kind() == SyntaxKind::GotoStatement)
    );

    let AttachedChoicePlanItem::OnSelect(on_select) = &plan_body.items()[3] else {
        panic!("fourth plan item must be on-select");
    };
    assert_eq!(on_select.pattern().syntax().source_text(), "selected");
    assert!(
        matches!(on_select.body(), AttachedRequiredNestedThreadFlowBody::Present(body)
        if body.items()[0].kind() == SyntaxKind::ExpressionStatement)
    );
    assert!(!choice.has_recovery());
}

#[test]
fn choice_lifecycle_mark_trigger_owns_typed_selector() {
    let snapshot = attach(concat!(
        "flow planned_mark_choice {\n",
        "    choice @choice.opening {\n",
        "        @.listen \"Listen\" -> @flow.listen\n",
        "    }\n",
        "    with {\n",
        "        cancel on mark(@.checkpoint) { goto @flow.listen }\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("mark-choice fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("Choice and its plan must remain one Flow item");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice
        .plan()
        .expect("authored `with` plan must be attached");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("Choice plan body must remain present");
    };
    let AttachedChoicePlanItem::Cancel(cancel) = &plan_body.items()[0] else {
        panic!("first plan item must be cancel-on");
    };
    assert!(matches!(
        cancel.trigger(),
        AttachedTriggerPattern::Mark(trigger)
            if trigger
                .selector()
                .name()
                .is_some_and(|name| name.as_str() == "checkpoint")
                && !trigger.has_recovery()
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn on_statement_attachment_owns_typed_trigger_and_body() {
    let snapshot = attach(concat!(
        "flow on_mark {\n",
        "    on mark(@.checkpoint) => goto @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("On fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Statement(statement) = &body.items()[0] else {
        panic!("On fixture must remain a statement item");
    };
    let statement = statement.cast::<OnStatementKind>().unwrap();
    let on = statement.semantics().unwrap();
    assert!(matches!(
        on.trigger(),
        AttachedTriggerPattern::Mark(trigger)
            if trigger.selector().name().is_some_and(|name| name.as_str() == "checkpoint")
    ));
    assert_eq!(on.body().kind(), SyntaxKind::GotoStatement);
}

#[test]
fn choice_lifecycle_plan_recovery_stays_inside_typed_plan_items() {
    let snapshot = attach(concat!(
        "flow recovered_plan {\n",
        "    choice { @.only \"Only\" => unit }\n",
        "    with {\n",
        "        window @choice_window.main\n",
        "        timeout { select @.only }\n",
        "        cancel input(.Back) { continue }\n",
        "        on selected { continue }\n",
        "        42\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("Choice-plan recovery fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 1);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("recovered lifecycle plan must stay attached to Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice
        .plan()
        .expect("recovered plan must retain its typed owner");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("recovered plan body must remain present");
    };
    assert_eq!(plan_body.items().len(), 5);
    assert!(matches!(
        plan_body.items()[0],
        AttachedChoicePlanItem::Assignment(ref assignment)
            if assignment.equals().range().is_empty()
    ));
    assert!(matches!(
        plan_body.items()[1],
        AttachedChoicePlanItem::Timeout(ref timeout)
            if matches!(timeout.duration(), RequiredStatementExpressionNode::Missing(_))
    ));
    assert!(matches!(
        plan_body.items()[2],
        AttachedChoicePlanItem::Cancel(ref cancel) if cancel.header_recovery().is_some()
    ));
    assert!(matches!(
        plan_body.items()[3],
        AttachedChoicePlanItem::OnSelect(ref handler) if handler.header_recovery().is_some()
    ));
    assert!(matches!(
        plan_body.items()[4],
        AttachedChoicePlanItem::Recovered(_)
    ));
    assert!(choice.has_recovery());
}

#[test]
fn choice_lifecycle_plan_same_line_uses_the_same_owner_in_thread() {
    let snapshot = attach(concat!(
        "flow host {\n",
        "    let worker = thread {\n",
        "        choice { @.only \"Only\" => unit } with { window = @choice_window.main }\n",
        "    }\n",
        "}\n",
    ));
    let thread = snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::ThreadExpression)
        .expect("Thread expression owner")
        .cast::<ThreadExpressionKind>()
        .unwrap();
    let AttachedRequiredThreadExpressionBody::Present(body) = thread.statement_body().unwrap()
    else {
        panic!("Thread expression body must be present");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("same-line Choice plan must retain the shared Choice owner");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice.plan().expect("same-line plan must be attached");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("same-line plan body must be present");
    };
    assert_eq!(plan_body.items().len(), 1);
    assert!(!choice.has_recovery());
}

#[test]
fn next_line_bare_with_is_a_typed_missing_choice_plan_body() {
    let snapshot = attach(concat!(
        "flow missing_plan {\n",
        "    choice { @.only \"Only\" => unit }\n",
        "    with\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("missing-plan fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 1);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("bare `with` must stay attached to Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice.plan().expect("bare `with` must retain a plan owner");
    assert!(matches!(
        plan.body(),
        AttachedRequiredChoicePlanBody::Missing(node) if node.range().is_empty()
    ));
    assert!(choice.has_recovery());
}

#[test]
fn missing_plan_body_stops_before_comment_and_outer_sibling() {
    let snapshot = attach(concat!(
        "flow missing_plan_boundary {\n",
        "    choice { @.only \"Only\" => unit }\n",
        "    with\n",
        "    // remains in the outer Flow body\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice.plan().expect("bare `with` must retain a plan owner");
    assert!(matches!(
        plan.body(),
        AttachedRequiredChoicePlanBody::Missing(node) if node.range().is_empty()
    ));
    assert!(
        !plan
            .syntax()
            .syntax()
            .source_text()
            .contains("remains in the outer")
    );
    assert!(matches!(
        body.items()[1],
        AttachedThreadFlowItem::Include(_)
    ));
}

#[test]
fn unclosed_choice_plan_action_retains_a_present_body_with_missing_close() {
    let snapshot = attach(concat!(
        "flow unclosed_action {\n",
        "    choice { @.only \"Only\" => unit }\n",
        "    with {\n",
        "        timeout 10s { select @.only\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("unclosed-action fixture requires a present Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("unclosed action must stay attached to Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice.plan().expect("unclosed plan must retain its owner");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("authored plan open must retain a present body");
    };
    let AttachedChoicePlanItem::Timeout(timeout) = &plan_body.items()[0] else {
        panic!("timeout must retain its typed item owner");
    };
    assert!(matches!(
        timeout.body(),
        AttachedRequiredNestedThreadFlowBody::Present(body)
            if matches!(
                body.close_state(),
                crate::attachment::source_file::AttachedDelimiterState::Missing(_)
            )
    ));
    assert!(choice.has_recovery());
}

#[test]
fn choice_plan_trailing_tokens_are_typed_recovery() {
    let snapshot = attach(concat!(
        "flow trailing_plan {\n",
        "    choice { @.only \"Only\" => unit } with { window = @choice_window.main } stray\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("trailing-plan fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("trailing tokens must not split the Choice owner");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice.plan().expect("plan must remain attached");
    assert_eq!(plan.trailing_recovery().unwrap().source_text(), "stray");
    assert!(choice.has_recovery());
}

#[test]
fn choice_plan_action_suffix_is_typed_recovery_after_a_present_body() {
    let snapshot = attach(concat!(
        "flow action_suffix {\n",
        "    choice { @.only \"Only\" => unit } with {\n",
        "        timeout 10s { select @.only } stray\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("action-suffix fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("action suffix must stay under Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice.plan().expect("plan must remain attached");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("plan body must remain present");
    };
    let AttachedChoicePlanItem::Timeout(timeout) = &plan_body.items()[0] else {
        panic!("timeout must retain its typed owner");
    };
    assert!(matches!(
        timeout.body(),
        AttachedRequiredNestedThreadFlowBody::Present(_)
    ));
    assert_eq!(timeout.trailing_recovery().unwrap().source_text(), "stray");
    assert!(choice.has_recovery());
}

#[test]
fn missing_choice_body_and_present_plan_share_one_choice_owner() {
    let snapshot = attach(concat!(
        "flow missing_choice_body {\n",
        "    choice @choice.only with { window = @choice_window.main }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("missing-choice-body fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 1);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("plan brace must not be stolen as the Choice body");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    assert!(matches!(
        choice.body(),
        AttachedRequiredChoiceBody::Missing(node) if node.range().is_empty()
    ));
    let plan = choice.plan().expect("present plan must remain attached");
    assert!(matches!(
        plan.body(),
        AttachedRequiredChoicePlanBody::Present(body) if body.items().len() == 1
    ));
    assert!(choice.has_recovery());
}

#[test]
fn indented_choice_body_and_plan_stop_before_the_next_flow_item() {
    let snapshot = attach(concat!(
        "flow indented_choice {\n",
        "    choice @choice.menu:\n",
        "        @.yes \"Yes\" => unit\n",
        "    with:\n",
        "        window = @choice_window.main\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("indented Choice fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("authored colon must retain a present Choice body");
    };
    assert!(matches!(
        choice_body.source(),
        AttachedChoiceSuiteSource::Indented { colon } if colon.source_text() == ":"
    ));
    assert_eq!(choice_body.items().len(), 1);

    let plan = choice.plan().expect("same-indent `with:` must attach");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("authored plan colon must retain a present body");
    };
    assert!(matches!(
        plan_body.source(),
        AttachedChoiceSuiteSource::Indented { colon } if colon.source_text() == ":"
    ));
    assert_eq!(plan_body.items().len(), 1);
    assert_eq!(body.items()[1].kind(), SyntaxKind::IncludeStatement);
    assert!(!choice.has_recovery());
}

#[test]
fn indented_choice_for_and_both_option_forms_share_the_typed_suite_owner() {
    let snapshot = attach(concat!(
        "flow indented_choice_for {\n",
        "    choice @choice.menu:\n",
        "        for item in items:\n",
        "            option item.id:\n",
        "                label = item.label\n",
        "                select { out item }\n",
        "        option item in items:\n",
        "            id = item.id\n",
        "            label = item.label\n",
        "            select { out item }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 2);

    let AttachedChoiceItem::For(loop_item) = &choice_body.items()[0] else {
        panic!("first Choice item must remain `for`");
    };
    let AttachedRequiredChoiceBody::Present(loop_body) = loop_item.body() else {
        panic!("Choice `for` colon must retain its body");
    };
    assert!(matches!(
        loop_body.source(),
        AttachedChoiceSuiteSource::Indented { .. }
    ));
    let AttachedChoiceItem::Option(option) = &loop_body.items()[0] else {
        panic!("nested item must remain a full option");
    };
    assert!(matches!(
        option.body(),
        AttachedRequiredChoiceOptionBody::Present(body)
            if matches!(body.source(), AttachedChoiceSuiteSource::Indented { .. })
    ));

    let AttachedChoiceItem::OptionFor(option_for) = &choice_body.items()[1] else {
        panic!("second Choice item must remain option-for sugar");
    };
    assert!(matches!(
        option_for.body(),
        AttachedRequiredChoiceOptionBody::Present(body)
            if matches!(body.source(), AttachedChoiceSuiteSource::Indented { .. })
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn intermediate_option_indent_is_a_recovered_field_not_a_sibling() {
    let snapshot = attach(concat!(
        "flow recovered_indent {\n",
        "    choice:\n",
        "        option @.only:\n",
        "            label = \"Only\"\n",
        "          label = \"Wrong indent\"\n",
        "            value = 1\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    let AttachedChoiceItem::Option(option) = &choice_body.items()[0] else {
        panic!("Choice body must retain one option");
    };
    let AttachedRequiredChoiceOptionBody::Present(option_body) = option.body() else {
        panic!("option body must remain present");
    };
    assert_eq!(option_body.fields().len(), 3);
    assert!(matches!(
        option_body.fields()[0],
        AttachedChoiceOptionField::Label { .. }
    ));
    assert!(matches!(
        option_body.fields()[1],
        AttachedChoiceOptionField::Recovered(_)
    ));
    assert!(matches!(
        option_body.fields()[2],
        AttachedChoiceOptionField::Value { .. }
    ));
    assert!(choice.has_recovery());
}

#[test]
fn comment_only_indented_choice_recovers_without_consuming_the_dedent() {
    let snapshot = attach(concat!(
        "flow empty_indented {\n",
        "    choice:\n",
        "        // no authored candidate\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("authored colon must retain a present body owner");
    };
    assert!(matches!(
        choice_body.source(),
        AttachedChoiceSuiteSource::Indented { .. }
    ));
    assert!(choice_body.items().is_empty());
    assert_eq!(choice_body.recovery().len(), 1);
    assert_eq!(body.items()[1].kind(), SyntaxKind::IncludeStatement);
    assert!(choice.has_recovery());
}

#[test]
fn nonempty_option_does_not_consume_dedented_comment_or_outer_sibling() {
    let snapshot = attach(concat!(
        "flow option_dedent {\n",
        "    choice:\n",
        "        option @.yes:\n",
        "            label = \"Yes\"\n",
        "    // belongs to the outer Flow body\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    assert!(
        !choice
            .syntax()
            .source_text()
            .contains("belongs to the outer")
    );
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    let [AttachedChoiceItem::Option(option)] = choice_body.items() else {
        panic!("Choice body must retain one option");
    };
    let AttachedRequiredChoiceOptionBody::Present(option_body) = option.body() else {
        panic!("option body must remain present");
    };
    assert!(
        !option_body
            .syntax()
            .syntax()
            .source_text()
            .contains("belongs to the outer")
    );
    assert!(matches!(
        body.items()[1],
        AttachedThreadFlowItem::Include(_)
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn crlf_thread_choice_uses_the_same_indented_source_owner() {
    let snapshot = attach(concat!(
        "flow host {\r\n",
        "    let worker = thread {\r\n",
        "        choice:\r\n",
        "            @.yes \"Yes\" => unit\r\n",
        "    }\r\n",
        "}\r\n",
    ));
    let thread = snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::ThreadExpression)
        .expect("Thread expression owner")
        .cast::<ThreadExpressionKind>()
        .unwrap();
    let AttachedRequiredThreadExpressionBody::Present(body) = thread.statement_body().unwrap()
    else {
        panic!("Thread expression body must be present");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("Thread body item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    assert!(matches!(
        choice.body(),
        AttachedRequiredChoiceBody::Present(body)
            if matches!(body.source(), AttachedChoiceSuiteSource::Indented { .. })
                && body.items().len() == 1
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn same_line_braced_choice_can_open_an_indented_plan_at_choice_indent() {
    let snapshot = attach(concat!(
        "flow same_line_plan {\n",
        "    choice { @.yes \"Yes\" => unit } with:\n",
        "        window = @choice_window.main\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let plan = choice
        .plan()
        .expect("same-line `with:` must remain attached");
    assert!(matches!(
        plan.body(),
        AttachedRequiredChoicePlanBody::Present(body)
            if matches!(body.source(), AttachedChoiceSuiteSource::Indented { .. })
                && body.items().len() == 1
    ));
    assert_eq!(body.items()[1].kind(), SyntaxKind::IncludeStatement);
    assert!(!choice.has_recovery());
}

#[test]
fn indented_choice_if_keeps_owner_aligned_else_as_one_item() {
    let snapshot = attach(concat!(
        "flow indented_if {\n",
        "    choice:\n",
        "        if allowed {\n",
        "            @.yes \"Yes\" => unit\n",
        "        }\n",
        "        else {\n",
        "            @.no \"No\" => unit\n",
        "        }\n",
        "        @.later \"Later\" => unit\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 2);
    let AttachedChoiceItem::If(branch) = &choice_body.items()[0] else {
        panic!("if/else must remain one Choice item");
    };
    assert!(matches!(
        branch.else_body(),
        Some(AttachedRequiredChoiceBody::Present(body))
            if body.items().len() == 1
    ));
    assert!(matches!(
        choice_body.items()[1],
        AttachedChoiceItem::CompactArm(_)
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn braced_choice_if_keeps_next_line_else_as_one_item() {
    let snapshot = attach(concat!(
        "flow braced_if {\n",
        "    choice {\n",
        "        if allowed {\n",
        "            @.yes \"Yes\" => unit\n",
        "        }\n",
        "        else {\n",
        "            @.no \"No\" => unit\n",
        "        }\n",
        "        @.later \"Later\" => unit\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 2);
    let AttachedChoiceItem::If(branch) = &choice_body.items()[0] else {
        panic!("if/else must remain one Choice item");
    };
    assert!(matches!(
        branch.else_body(),
        Some(AttachedRequiredChoiceBody::Present(body))
            if body.items().len() == 1
    ));
    assert!(matches!(
        choice_body.items()[1],
        AttachedChoiceItem::CompactArm(_)
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn choice_if_else_if_chain_is_one_flat_typed_item() {
    let snapshot = attach(concat!(
        "flow choice_if_chain {\n",
        "    choice {\n",
        "        if first {\n",
        "            @.first \"First\" => unit\n",
        "        }\n",
        "        else if second {\n",
        "            @.second \"Second\" => unit\n",
        "        }\n",
        "        else {\n",
        "            @.fallback \"Fallback\" => unit\n",
        "        }\n",
        "        @.later \"Later\" => unit\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.expression().body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 2);
    let AttachedChoiceItem::If(outer) = &choice_body.items()[0] else {
        panic!("if/else-if/else must remain one Choice item");
    };
    assert_eq!(outer.branches().len(), 2);
    assert!(matches!(
        outer.else_body(),
        Some(AttachedRequiredChoiceBody::Present(body))
            if body.items().len() == 1
    ));
    assert!(matches!(
        choice_body.items()[1],
        AttachedChoiceItem::CompactArm(_)
    ));
    assert!(!choice.expression().has_recovery());
}

#[test]
fn choice_required_body_suffix_preserves_record_and_block_expression_heads() {
    let snapshot = attach(concat!(
        "flow choice_suffixes {\n",
        "    choice {\n",
        "        option Route { key: \"main\" } { label = \"Route\" }\n",
        "        option { label = \"Missing\" }\n",
        "        for route in Routes { active: true } {\n",
        "            @.route \"Route\" => unit\n",
        "        }\n",
        "        if { allowed() } {\n",
        "            @.yes \"Yes\" => unit\n",
        "        } else {\n",
        "            @.no \"No\" => unit\n",
        "        }\n",
        "        match { current() } {\n",
        "            _ => @.fallback \"Fallback\" => unit\n",
        "        }\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.expression().body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 5);

    let AttachedChoiceItem::Option(record_id) = &choice_body.items()[0] else {
        panic!("first item must remain a full option");
    };
    let RequiredStatementExpressionNode::Expression(record_id_expression) = record_id.id() else {
        panic!("record-valued option ID must remain an expression");
    };
    assert_eq!(
        record_id_expression.syntax().source_text(),
        "Route { key: \"main\" }"
    );
    assert!(matches!(
        record_id.body(),
        AttachedRequiredChoiceOptionBody::Present(_)
    ));

    let AttachedChoiceItem::Option(missing_id) = &choice_body.items()[1] else {
        panic!("second item must remain a full option");
    };
    assert!(matches!(
        missing_id.id(),
        RequiredStatementExpressionNode::Missing(_)
    ));
    assert!(matches!(
        missing_id.body(),
        AttachedRequiredChoiceOptionBody::Present(_)
    ));

    let AttachedChoiceItem::For(loop_item) = &choice_body.items()[2] else {
        panic!("third item must remain Choice For");
    };
    let RequiredStatementExpressionNode::Expression(source) = loop_item.source() else {
        panic!("record-valued Choice For source must remain an expression");
    };
    assert_eq!(source.syntax().source_text(), "Routes { active: true }");
    assert!(matches!(
        loop_item.body(),
        AttachedRequiredChoiceBody::Present(_)
    ));

    let AttachedChoiceItem::If(branch) = &choice_body.items()[3] else {
        panic!("fourth item must remain Choice If");
    };
    let RequiredStatementExpressionNode::Expression(condition) = branch.branches()[0].condition()
    else {
        panic!("block-valued Choice condition must remain an expression");
    };
    assert_eq!(condition.syntax().source_text(), "{ allowed() }");

    let AttachedChoiceItem::Match(match_item) = &choice_body.items()[4] else {
        panic!("fifth item must remain Choice Match");
    };
    let RequiredStatementExpressionNode::Expression(scrutinee) = match_item.scrutinee() else {
        panic!("block-valued Choice scrutinee must remain an expression");
    };
    assert_eq!(scrutinee.syntax().source_text(), "{ current() }");
}

#[test]
fn typed_candidate_pattern_colons_are_not_misclassified_as_suite_bodies() {
    let snapshot = attach(concat!(
        "flow choice_typed_patterns {\n",
        "    choice:\n",
        "        for missing_for: Route in routes\n",
        "        option missing_option: Route in routes\n",
        "        for present: Route in routes:\n",
        "            @.nested \"Nested\" => unit\n",
        "        @.later \"Later\" => unit\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.expression().body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 4);

    let AttachedChoiceItem::For(missing_for) = &choice_body.items()[0] else {
        panic!("first item must remain Choice For");
    };
    assert_eq!(
        missing_for.pattern().syntax().source_text(),
        "missing_for: Route"
    );
    assert!(matches!(
        missing_for.body(),
        AttachedRequiredChoiceBody::Missing(_)
    ));

    let AttachedChoiceItem::OptionFor(missing_option) = &choice_body.items()[1] else {
        panic!("second item must remain Choice option-for");
    };
    assert_eq!(
        missing_option.pattern().syntax().source_text(),
        "missing_option: Route"
    );
    assert!(matches!(
        missing_option.body(),
        AttachedRequiredChoiceOptionBody::Missing(_)
    ));

    let AttachedChoiceItem::For(present) = &choice_body.items()[2] else {
        panic!("third item must remain Choice For");
    };
    assert_eq!(present.pattern().syntax().source_text(), "present: Route");
    assert!(matches!(
        present.body(),
        AttachedRequiredChoiceBody::Present(body) if body.items().len() == 1
    ));
    assert!(matches!(
        choice_body.items()[3],
        AttachedChoiceItem::CompactArm(_)
    ));
}

#[test]
fn choice_plan_required_body_suffix_preserves_typed_head_payloads() {
    let snapshot = attach(concat!(
        "flow choice_plan_suffixes {\n",
        "    choice { @.only \"Only\" => unit } with {\n",
        "        timeout { compute_duration() } { continue }\n",
        "        timeout { continue }\n",
        "        cancel on Trigger { kind: .Back } { continue }\n",
        "        on select Selection { value } { continue }\n",
        "    }\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    let plan = choice
        .expression()
        .plan()
        .expect("Choice plan must be present");
    let AttachedRequiredChoicePlanBody::Present(plan_body) = plan.body() else {
        panic!("Choice plan body must remain present");
    };
    assert_eq!(plan_body.items().len(), 4);

    let AttachedChoicePlanItem::Timeout(block_duration) = &plan_body.items()[0] else {
        panic!("first item must remain timeout");
    };
    let RequiredStatementExpressionNode::Expression(duration) = block_duration.duration() else {
        panic!("two-brace timeout must retain its block duration");
    };
    assert_eq!(duration.syntax().source_text(), "{ compute_duration() }");
    assert!(matches!(
        block_duration.body(),
        AttachedRequiredNestedThreadFlowBody::Present(_)
    ));

    let AttachedChoicePlanItem::Timeout(missing_duration) = &plan_body.items()[1] else {
        panic!("second item must remain timeout");
    };
    assert!(matches!(
        missing_duration.duration(),
        RequiredStatementExpressionNode::Missing(_)
    ));
    assert!(matches!(
        missing_duration.body(),
        AttachedRequiredNestedThreadFlowBody::Present(_)
    ));

    let AttachedChoicePlanItem::Cancel(cancel) = &plan_body.items()[2] else {
        panic!("third item must remain cancel-on");
    };
    assert_eq!(
        cancel.trigger().syntax().source_text(),
        "Trigger { kind: .Back }"
    );
    assert!(matches!(
        cancel.body(),
        AttachedRequiredNestedThreadFlowBody::Present(_)
    ));

    let AttachedChoicePlanItem::OnSelect(on_select) = &plan_body.items()[3] else {
        panic!("fourth item must remain on-select");
    };
    assert_eq!(
        on_select.pattern().syntax().source_text(),
        "Selection { value }"
    );
    assert!(matches!(
        on_select.body(),
        AttachedRequiredNestedThreadFlowBody::Present(_)
    ));
}

#[test]
fn empty_nested_option_reports_suite_recovery_without_losing_its_sibling() {
    let snapshot = attach(concat!(
        "flow empty_option {\n",
        "    choice:\n",
        "        option @.empty:\n",
        "        option @.ready:\n",
        "            label = \"Ready\"\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(choice_body.items().len(), 2);
    let AttachedChoiceItem::Option(empty) = &choice_body.items()[0] else {
        panic!("first item must remain an option");
    };
    assert!(matches!(
        empty.body(),
        AttachedRequiredChoiceOptionBody::Present(body)
            if matches!(body.source(), AttachedChoiceSuiteSource::Indented { .. })
                && body.fields().is_empty()
                && body.recovery().len() == 1
    ));
    assert!(matches!(
        choice_body.items()[1],
        AttachedChoiceItem::Option(_)
    ));
    assert!(choice.has_recovery());
}

#[test]
fn leading_block_comment_uses_the_first_content_column_for_item_indent() {
    let snapshot = attach(concat!(
        "flow comment_prefix {\n",
        "    choice:\n",
        "        /* note */ option @.yes:\n",
        "                    label = \"Yes\"\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("fixture must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    let AttachedRequiredChoiceBody::Present(choice_body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert!(matches!(
        choice_body.items(),
        [AttachedChoiceItem::Option(option)]
            if matches!(option.body(), AttachedRequiredChoiceOptionBody::Present(body)
                if body.fields().len() == 1)
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn body_indented_with_is_recovery_and_never_becomes_the_choice_plan() {
    let snapshot = attach(concat!(
        "flow nested_with {\n",
        "    choice:\n",
        "        @.yes \"Yes\" => unit\n",
        "        with:\n",
        "            window = @choice_window.nested\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    assert!(choice.plan().is_none());
    assert!(matches!(
        choice.body(),
        AttachedRequiredChoiceBody::Present(body)
            if body.items().len() == 2
                && matches!(body.items()[1], AttachedChoiceItem::Recovered(_))
    ));
    assert_eq!(body.items()[1].kind(), SyntaxKind::IncludeStatement);
    assert!(choice.has_recovery());
}

#[test]
fn misindented_with_after_a_missing_body_is_not_attached_as_the_choice_plan() {
    let snapshot = attach(concat!(
        "flow missing_body_nested_with {\n",
        "    choice @choice.only\n",
        "        with:\n",
        "            window = @choice_window.nested\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    assert!(matches!(
        choice.body(),
        AttachedRequiredChoiceBody::Missing(node) if node.range().is_empty()
    ));
    assert!(choice.plan().is_none());
    assert!(
        body.items()
            .iter()
            .skip(1)
            .any(|item| item.syntax().source_text().contains("with:")),
        "misindented `with:` must remain visible to the outer body owner"
    );
    assert!(matches!(
        body.items().last(),
        Some(AttachedThreadFlowItem::Include(_))
    ));
}

#[test]
fn indented_choice_semicolons_progress_without_creating_extra_suite_items() {
    let snapshot = attach(concat!(
        "flow semicolon_choice {\n",
        "    choice:\n",
        "        @.yes \"Yes\" => unit;\n",
        "        @.later \"Later\" => unit\n",
        "    with:\n",
        "        window = @choice_window.main;\n",
        "        focus = @actor.hero\n",
        "    include @flow.next\n",
        "}\n",
    ));
    assert_eq!(
        flow(&snapshot)
            .syntax()
            .rowan()
            .descendants()
            .filter(|node| { node.kind() == rowan::SyntaxKind(SyntaxKind::IndentedSuite as u16) })
            .count(),
        2,
        "Choice body and lifecycle plan must each own one structural suite"
    );
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap().expression().clone();
    assert!(matches!(
        choice.body(),
        AttachedRequiredChoiceBody::Present(body) if body.items().len() == 2
    ));
    let plan = choice
        .plan()
        .expect("same-indent plan must remain attached");
    assert!(matches!(
        plan.body(),
        AttachedRequiredChoicePlanBody::Present(body) if body.items().len() == 2
    ));
    assert!(matches!(
        body.items()[1],
        AttachedThreadFlowItem::Include(_)
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn dense_same_line_indented_choice_items_share_one_monotonic_indent_scan() {
    const ITEM_COUNT: usize = 512;
    let mut source = String::from("flow dense_choice {\n    choice:\n        ");
    for ordinal in 0..ITEM_COUNT {
        if ordinal != 0 {
            source.push_str("; ");
        }
        write!(&mut source, "@.item_{ordinal} \"Item {ordinal}\" => unit")
            .expect("writing to a String cannot fail");
    }
    source.push_str("\n    include @flow.next\n}\n");

    let snapshot = attach(&source);
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    assert!(matches!(
        choice.expression().body(),
        AttachedRequiredChoiceBody::Present(body) if body.items().len() == ITEM_COUNT
    ));
    assert!(!choice.expression().has_recovery());
}

#[test]
fn multiline_choice_item_accepts_a_same_line_semicolon_sibling_at_owner_indent() {
    let snapshot = attach(concat!(
        "flow multiline_semicolon_choice {\n",
        "    choice:\n",
        "        if allowed {\n",
        "            @.nested \"Nested\" => unit\n",
        "        }; @.later \"Later\" => unit\n",
        "    include @flow.next\n",
        "}\n",
    ));
    let declaration = flow(&snapshot).semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture requires a Flow body");
    };
    assert_eq!(body.items().len(), 2);
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first item must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    assert!(matches!(
        choice.expression().body(),
        AttachedRequiredChoiceBody::Present(body)
            if matches!(body.items(), [AttachedChoiceItem::If(_), AttachedChoiceItem::CompactArm(_)])
    ));
    assert!(!choice.expression().has_recovery());
}

#[test]
fn flow_attachment_keeps_rejected_defaults_out_of_the_callable_default_owner() {
    let source = "flow configured(value: Int = make_value()) { return value }";
    let snapshot = attach(source);
    let declaration = flow(&snapshot).semantics().unwrap();
    let parameter = &declaration.signature().parameters().unwrap().parameters()[0];

    assert!(parameter.default().is_none());
    assert_eq!(parameter.recovery().len(), 1);
    let recovery = &parameter.recovery()[0];
    assert_eq!(
        recovery.range(),
        SourceRange::new(source.find('=').unwrap(), source.rfind(')').unwrap())
    );
    assert_eq!(declaration.signature().recovery().len(), 1);
    let AttachedFlowSignatureRecovery::UnexpectedHeaderNode {
        syntax: signature_recovery,
    } = &declaration.signature().recovery()[0]
    else {
        panic!("Flow default must remain unexpected signature recovery");
    };
    assert_eq!(recovery.id(), signature_recovery.id());
    assert!(parameter.has_recovery());
    assert!(declaration.signature().has_recovery());
    assert!(matches!(
        declaration.body(),
        AttachedRequiredFlowBody::Present(_)
    ));
}

#[test]
fn flow_attachment_exposes_the_second_group_only_as_typed_signature_recovery() {
    let source = "flow invalid(first: Int)(second: Int) -> Int {}";
    let snapshot = attach(source);
    let declaration = flow(&snapshot).semantics().unwrap();
    let signature = declaration.signature();

    assert_eq!(signature.parameters().unwrap().parameters().len(), 1);
    assert_eq!(signature.recovery().len(), 1);
    let AttachedFlowSignatureRecovery::SecondParameterGroup { syntax, group } =
        &signature.recovery()[0]
    else {
        panic!("second Flow group must use typed signature recovery");
    };
    let second_start = source.find("(second").unwrap();
    assert_eq!(syntax.range().start(), second_start);
    assert_eq!(group.range(), syntax.range());
    assert!(signature.has_recovery());
    assert!(matches!(
        signature.result(),
        AttachedFlowReturnSyntax::Authored(_)
    ));
    assert!(matches!(
        declaration.body(),
        AttachedRequiredFlowBody::Present(_)
    ));
}
