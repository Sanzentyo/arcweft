use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::*;
use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::SyntaxKind;
use crate::id_ref::{AuthoredIdRoot, SyntaxIdRefIssue};
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    try_attach(text).unwrap()
}

fn try_attach(text: &str) -> Result<Arc<SyntaxSnapshotData>, crate::attachment::AttachmentFailure> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/style-attachment-test").unwrap(),
            SourceName::path("style-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
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
}

fn style(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<StyleItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::StyleItem)
        .expect("Style declaration")
        .cast()
        .unwrap()
}

fn styles(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<StyleItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::StyleItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

fn assert_canonical_style_id(declaration: &AttachedStyleDeclaration) {
    let reference = declaration.id().reference().unwrap().value().unwrap();
    assert!(matches!(
        reference.root(),
        AuthoredIdRoot::FamilyRelative { family, parent_depth: 0 }
            if family.as_str() == "style"
    ));
    assert_eq!(
        reference
            .segments()
            .iter()
            .map(crate::id_ref::AuthoredIdSegment::as_str)
            .collect::<Vec<_>>(),
        ["theme", "dark"]
    );
}

fn assert_canonical_selector(source: &str, rule: &AttachedStyleRule) {
    let sequences = rule.selector().sequences();
    assert_eq!(sequences.len(), 3);
    assert!(sequences[0].relation().is_none());
    assert_eq!(
        sequences[1].relation().unwrap().value(),
        StyleSelectorRelation::Descendant
    );
    assert_eq!(
        sequences[2].relation().unwrap().value(),
        StyleSelectorRelation::Child
    );
    assert_eq!(
        sequences[0].element().unwrap().value().unwrap().as_str(),
        "Panel"
    );
    assert_eq!(
        sequences[1].element().unwrap().value().unwrap().as_str(),
        "Button"
    );
    assert_eq!(
        sequences[1]
            .part()
            .unwrap()
            .name()
            .value()
            .unwrap()
            .as_str(),
        "primary"
    );
    assert_eq!(
        sequences[1].predicates()[0]
            .name()
            .value()
            .unwrap()
            .as_str(),
        "hover"
    );
    assert_eq!(
        sequences[2]
            .part()
            .unwrap()
            .name()
            .value()
            .unwrap()
            .as_str(),
        "label"
    );
    let descendant_start = source.find("Panel Button").unwrap() + "Panel".len();
    assert_eq!(
        sequences[1].relation().unwrap().source_span().range(),
        SourceRange::new(descendant_start, descendant_start + 1)
    );
    let child_start = source.find(" > .label").unwrap() + 1;
    assert_eq!(
        sequences[2].relation().unwrap().source_span().range(),
        SourceRange::new(child_start, child_start + 1)
    );
    assert_eq!(
        sequences[1]
            .syntax()
            .syntax()
            .rowan()
            .parent()
            .unwrap()
            .kind(),
        rowan::SyntaxKind(SyntaxKind::StyleSelector as u16)
    );
}

fn assert_canonical_environment(environment: &AttachedStyleEnvironment) {
    let fields = environment
        .condition()
        .clauses()
        .iter()
        .map(|clause| clause.field().value().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        [
            StyleEnvironmentFieldKind::ColorScheme,
            StyleEnvironmentFieldKind::Contrast,
            StyleEnvironmentFieldKind::ReducedMotion,
            StyleEnvironmentFieldKind::TextScale,
            StyleEnvironmentFieldKind::TextScale,
            StyleEnvironmentFieldKind::TextScale,
        ]
    );
    let comparisons = environment
        .condition()
        .clauses()
        .iter()
        .map(|clause| clause.comparison().value().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        comparisons,
        [
            StyleEnvironmentComparisonKind::Equal,
            StyleEnvironmentComparisonKind::NotEqual,
            StyleEnvironmentComparisonKind::Less,
            StyleEnvironmentComparisonKind::LessOrEqual,
            StyleEnvironmentComparisonKind::Greater,
            StyleEnvironmentComparisonKind::GreaterOrEqual,
        ]
    );
    assert!(environment.condition().recoveries().is_empty());
}

#[test]
fn attaches_minimal_style() {
    let snapshot = attach("style theme { token color.text = white }\n");
    let declaration = style(&snapshot).semantics().unwrap();
    assert!(!declaration.has_recovery());
    assert_eq!(declaration.body().members().len(), 1);
}

#[test]
fn canonical_style_attachment_owns_ids_selectors_properties_and_environment_matrix() {
    let source = concat!(
        "style theme.dark {\n",
        "    token color.text: Color = white\n",
        "    Panel Button.primary:hover > .label:active {\n",
        "        background-color = color.text\n",
        "        append shadow-list = shadow\n",
        "    }\n",
        "    when environment(\n",
        "        color-scheme == dark,\n",
        "        contrast != high,\n",
        "        reduced-motion < motion,\n",
        "        text-scale <= 100%,\n",
        "        text-scale > 90%,\n",
        "        text-scale >= 80%\n",
        "    ) {\n",
        "        Panel { opacity = 1 }\n",
        "    }\n",
        "}\n",
    );
    let snapshot = attach(source);
    let declaration = style(&snapshot).semantics().unwrap();
    assert_canonical_style_id(&declaration);

    let [
        AttachedStyleMember::Token(token),
        AttachedStyleMember::Rule(rule),
        AttachedStyleMember::Environment(environment),
    ] = declaration.body().members()
    else {
        panic!("canonical source order")
    };
    assert_eq!(token.name().value().unwrap().as_str(), "color.text");
    assert_eq!(token.name().dotted_component_count(), 2);
    assert_eq!(token.id().shape().segment_count(), 2);
    assert!(matches!(
        token.id().value().unwrap().root(),
        AuthoredIdRoot::Relative { parent_depth: 0 }
    ));
    assert!(token.type_annotation().is_some());
    assert_canonical_selector(source, rule);

    let properties = rule.body().declarations();
    assert_eq!(properties.len(), 2);
    assert_eq!(
        properties[0].name().value().unwrap().as_str(),
        "background-color"
    );
    assert_eq!(properties[0].operation(), StylePropertyOperation::Replace);
    assert_eq!(properties[1].operation(), StylePropertyOperation::Append);
    assert!(properties[1].append_keyword().is_some());

    assert_canonical_environment(environment);
    assert!(!declaration.has_recovery());
}

#[test]
fn style_id_forms_and_recovered_id_shapes_remain_parser_owned() {
    let snapshot = attach(concat!(
        "style plain {}\n",
        "style dotted.name {}\n",
        "style @.relative {}\n",
        "style broken. {}\n",
        "style {}\n",
    ));
    let declarations = styles(&snapshot)
        .into_iter()
        .map(|syntax| syntax.semantics().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(declarations[0].id().form(), Some(StyleIdForm::Bare));
    assert_eq!(declarations[1].id().form(), Some(StyleIdForm::Bare));
    assert_eq!(declarations[2].id().form(), Some(StyleIdForm::Explicit));
    assert_eq!(
        declarations[1]
            .id()
            .reference()
            .unwrap()
            .shape()
            .segment_count(),
        2
    );
    assert!(matches!(
        declarations[2].id().reference().unwrap().value().unwrap().root(),
        AuthoredIdRoot::FamilyRelative { family, parent_depth: 0 }
            if family.as_str() == "style"
    ));
    assert_eq!(
        declarations[3]
            .id()
            .reference()
            .unwrap()
            .shape()
            .segment_count(),
        2
    );
    assert!(matches!(
        declarations[3].id().reference().unwrap().value(),
        Err(SyntaxIdRefIssue::InvalidSegment { ordinal: 1 })
    ));
    assert_eq!(
        declarations[4]
            .id()
            .reference()
            .unwrap()
            .shape()
            .segment_count(),
        0
    );
    assert!(matches!(
        declarations[4].id().reference().unwrap().value(),
        Err(SyntaxIdRefIssue::MissingSuffix)
    ));
}

#[test]
fn style_token_ids_are_bound_from_the_attached_typed_name() {
    let snapshot = attach(concat!(
        "style tokens {\n",
        "    token clean.name = 1\n",
        "    token broken. = 2\n",
        "    token = 3\n",
        "}\n",
    ));
    let declaration = style(&snapshot).semantics().unwrap();
    let tokens = declaration
        .body()
        .members()
        .iter()
        .map(|member| match member {
            AttachedStyleMember::Token(token) => token.as_ref(),
            _ => panic!("token-only fixture"),
        })
        .collect::<Vec<_>>();

    let clean = tokens[0].id().value().unwrap();
    assert!(matches!(
        clean.root(),
        AuthoredIdRoot::Relative { parent_depth: 0 }
    ));
    assert_eq!(tokens[0].id().shape().segment_count(), 2);
    assert_eq!(
        clean
            .segments()
            .iter()
            .map(crate::id_ref::AuthoredIdSegment::as_str)
            .collect::<Vec<_>>(),
        ["clean", "name"]
    );
    assert!(matches!(
        tokens[1].id().value(),
        Err(SyntaxIdRefIssue::InvalidSegment { ordinal: 1 })
    ));
    assert_eq!(tokens[1].id().shape().segment_count(), 2);
    assert!(matches!(
        tokens[2].id().value(),
        Err(SyntaxIdRefIssue::MissingSuffix)
    ));
    assert_eq!(tokens[2].id().shape().segment_count(), 0);
}

#[test]
fn aliases_and_environment_list_errors_remain_typed_recovery() {
    let snapshot = attach(concat!(
        "style recover {\n",
        "    token color. = white\n",
        "    Button {\n",
        "        opacity += 1\n",
        "        size -= 1\n",
        "        color white\n",
        "        background-color =\n",
        "    }\n",
        "    when environment(, color-scheme == dark,) {}\n",
        "    when environment() {}\n",
        "    when environment(text_scale == 1, platform != desktop, contrast = high) {}\n",
        "}\n",
    ));
    let declaration = style(&snapshot).semantics().unwrap();
    let members = declaration.body().members();
    let AttachedStyleMember::Token(token) = &members[0] else {
        panic!("recovered token")
    };
    assert_eq!(token.id().shape().segment_count(), 2);
    assert!(token.id().value().is_err());

    let AttachedStyleMember::Rule(rule) = &members[1] else {
        panic!("recovered rule")
    };
    let properties = rule.body().declarations();
    assert_eq!(properties[0].operation(), StylePropertyOperation::Replace);
    assert_eq!(properties[1].operation(), StylePropertyOperation::Replace);
    assert_eq!(
        properties[0].assignment().state(),
        AttachedStyleAssignmentState::Unsupported
    );
    assert_eq!(
        properties[1].assignment().state(),
        AttachedStyleAssignmentState::Unsupported
    );
    assert_eq!(
        properties[2].assignment().state(),
        AttachedStyleAssignmentState::Missing
    );
    assert!(properties[3].value().has_recovery());

    let environment_recoveries = members[2..4]
        .iter()
        .map(|member| match member {
            AttachedStyleMember::Environment(environment) => environment
                .condition()
                .recoveries()
                .iter()
                .map(AttachedStyleEnvironmentConditionRecovery::issue)
                .collect::<Vec<_>>(),
            _ => panic!("environment recovery"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        environment_recoveries[0],
        [
            StyleEnvironmentConditionIssue::EmptyClause,
            StyleEnvironmentConditionIssue::TrailingComma,
        ]
    );
    assert_eq!(
        environment_recoveries[1],
        [StyleEnvironmentConditionIssue::EmptyCondition]
    );

    let AttachedStyleMember::Environment(environment) = &members[4] else {
        panic!("unsupported environment rows")
    };
    assert_eq!(environment.condition().clauses().len(), 3);
    assert!(
        environment
            .condition()
            .clauses()
            .iter()
            .all(AttachedStyleEnvironmentClause::has_recovery)
    );
    assert!(
        environment.condition().clauses()[0]
            .field()
            .value()
            .is_none()
    );
    assert!(
        environment.condition().clauses()[1]
            .field()
            .value()
            .is_none()
    );
    assert!(
        environment.condition().clauses()[2]
            .comparison()
            .value()
            .is_none()
    );
    assert!(declaration.has_recovery());
}

#[test]
fn recovery_rows_attach_independently() {
    for source in [
        "style recover { token color. = white }\n",
        "style recover { Button { opacity += 1 } }\n",
        "style recover { Button { size -= 1 } }\n",
        "style recover { Button { color white } }\n",
        "style recover { Button { background-color = } }\n",
        "style recover { when environment(, color-scheme == dark,) {} }\n",
        "style recover { when environment() {} }\n",
        "style recover { when environment(text_scale == 1) {} }\n",
        "style recover { when environment(platform != desktop) {} }\n",
        "style recover { when environment(contrast = high) {} }\n",
    ] {
        assert!(try_attach(source).is_ok(), "failed attachment: {source}");
    }
}

#[test]
fn missing_style_body_and_selector_are_owned_recovery() {
    let missing_body = attach("style theme\n");
    let declaration = style(&missing_body).semantics().unwrap();
    assert!(matches!(declaration.body(), AttachedStyleBody::Missing(_)));
    assert_eq!(
        declaration.body().source_span().range().start(),
        "style theme\n".len()
    );

    let missing_selector = attach("style theme { { color = white } }\n");
    let declaration = style(&missing_selector).semantics().unwrap();
    let [AttachedStyleMember::Rule(rule)] = declaration.body().members() else {
        panic!("missing selector still owns one rule")
    };
    assert!(rule.selector().missing().is_some());
    assert!(rule.selector().has_recovery());
}
