use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_document;
use crate::attachment::source_file::{AttachedDelimiterState, AttachedPathRoot};
use crate::attachment::{
    AttachedAttributeValue, AttachedOuterAttributeForm, AttachedOuterAttributeIssue,
    ProofTrustSyntax, TypedItemNode,
};
use crate::expressions::{SyntaxCallArgumentListTerminator, SyntaxCallArgumentProjection};
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::SyntaxDatabase;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:item-families").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn attached_first_item(source: &str) -> TypedItemNode {
    let attached_document = Arc::new(document(source));
    let snapshot = SourceSnapshotId::initial(attached_document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(
            snapshot,
            attached_document,
            crate::parser::ParseOptions::default(),
        )
        .unwrap();
    let mut items = parsed.items().unwrap();
    assert_eq!(items.len(), 1, "fixture must retain one source item");
    items.remove(0)
}

#[test]
fn outer_attributes_own_dotted_paths_and_shared_ordinary_arguments_without_call_callees() {
    let source = concat!(
        "#[generated]\n",
        "#[derive(Clone, Debug)]\n",
        "#[verify.trusted(reason = \"external\")]\n",
        "#[link(Flow, @flow.main..., level = .soft)]\n",
        "proof attributes() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::CallExpression)
            .count(),
        0,
        "attribute paths must not manufacture ordinary Call expression owners"
    );
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::SelectExpression)
            .count(),
        0,
        "dotted attribute paths must not manufacture Select expression owners"
    );

    let item = attached_first_item(source);
    let TypedItemNode::Proof(proof) = &item else {
        panic!("fixture item must be a Proof")
    };
    let proof = proof.semantics().unwrap();
    let Some(ProofTrustSyntax::Trusted { reason, .. }) = proof.trust() else {
        panic!("Proof-specific trust must consume verify.trusted")
    };
    assert_eq!(reason.as_str(), "external");
    let prefix = proof.prefix();
    let [generated, derive, link] = prefix.attributes() else {
        panic!("three generic attached attributes");
    };
    for attribute in prefix.attributes() {
        assert!(matches!(
            attribute.path().root(),
            AttachedPathRoot::ImplicitCrate
        ));
        assert_eq!(attribute.issue(), None);
        assert_eq!(attribute.recovery(), None);
    }
    assert_eq!(
        generated
            .path()
            .segments()
            .iter()
            .map(super::super::attachment::source_file::AttachedPathSegment::source_text)
            .collect::<Vec<_>>(),
        ["generated"]
    );
    assert!(matches!(
        generated.form(),
        AttachedOuterAttributeForm::Marker
    ));

    assert_eq!(
        derive
            .path()
            .segments()
            .iter()
            .map(super::super::attachment::source_file::AttachedPathSegment::source_text)
            .collect::<Vec<_>>(),
        ["derive"]
    );
    assert!(derive.arguments().iter().all(|argument| matches!(
        argument.projection(),
        SyntaxCallArgumentProjection::Positional { .. }
    )));
    assert_eq!(derive.arguments().len(), 2);

    let [flow, spread, level] = link.arguments() else {
        panic!("mixed ordinary argument family");
    };
    assert!(matches!(
        flow.projection(),
        SyntaxCallArgumentProjection::Positional { .. }
    ));
    assert!(matches!(
        spread.projection(),
        SyntaxCallArgumentProjection::Spread { .. }
    ));
    assert!(matches!(
        level.projection(),
        SyntaxCallArgumentProjection::Named { name, .. }
            if name.as_ref().is_ok_and(|name| name.as_str() == "level")
    ));
}

#[test]
fn removed_attribute_shapes_are_one_generic_recovery_without_expression_owners() {
    let source = concat!(
        "#[foo::bar]\n",
        "#[foo<T>()]\n",
        "#[foo { || true }]\n",
        "#[1 + 2]\n",
        "proof invalid_attributes() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    assert!(built.has_recovery());
    assert_eq!(built.green().to_string(), source);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.attribute.invalid_shape")
            .count(),
        4
    );
    for forbidden in [
        SyntaxKind::CallExpression,
        SyntaxKind::SelectExpression,
        SyntaxKind::ClosureExpression,
        SyntaxKind::GenericApplicationType,
    ] {
        assert!(
            built
                .index()
                .entries()
                .iter()
                .all(|entry| entry.kind() != forbidden),
            "invalid attributes must not manufacture {forbidden:?}"
        );
    }

    let item = attached_first_item(source);
    let prefix = item.attached_prefix().unwrap();
    let [qualified, typed, callback, non_path] = prefix.attributes() else {
        panic!("four recovered attributes");
    };
    for attribute in [qualified, typed, callback] {
        assert_eq!(
            attribute.issue(),
            Some(AttachedOuterAttributeIssue::InvalidShape)
        );
        assert!(attribute.recovery().is_some());
        assert_eq!(
            attribute
                .path()
                .segments()
                .iter()
                .map(super::super::attachment::source_file::AttachedPathSegment::source_text)
                .collect::<Vec<_>>(),
            ["foo"]
        );
    }
    assert_eq!(
        non_path.issue(),
        Some(AttachedOuterAttributeIssue::MissingPath)
    );
    assert!(non_path.path().segments().is_empty());
    assert!(non_path.path().missing_name().is_some());
    assert!(non_path.recovery().is_some());
}

#[test]
fn outer_attribute_delimiter_and_missing_value_recovery_remain_typed() {
    let source = concat!(
        "#[foo(name =)]\n",
        "#[bar(value]\n",
        "#[baz(value)\n",
        "proof recovered_attributes() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    assert!(built.has_recovery());
    assert_eq!(built.green().to_string(), source);

    let item = attached_first_item(source);
    let prefix = item.attached_prefix().unwrap();
    let [missing_value, missing_call_close, missing_attribute_close] = prefix.attributes() else {
        panic!("three recovered attributes");
    };
    let [argument] = missing_value.arguments() else {
        panic!("one missing value argument");
    };
    assert!(matches!(
        argument.value(),
        AttachedAttributeValue::Missing(_)
    ));
    assert_eq!(missing_value.issue(), None);
    assert_eq!(
        missing_value.form().terminator(),
        Some(SyntaxCallArgumentListTerminator::Closed)
    );
    assert_eq!(
        missing_call_close.form().terminator(),
        Some(SyntaxCallArgumentListTerminator::RecoveredMissing)
    );
    assert_eq!(missing_call_close.issue(), None);
    assert!(matches!(
        missing_attribute_close.close_state(),
        AttachedDelimiterState::Missing(_)
    ));
    assert_eq!(
        missing_attribute_close.form().terminator(),
        Some(SyntaxCallArgumentListTerminator::Closed)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the declaration-family inventory is one closed lossless-root matrix"
)]
fn every_current_top_level_declaration_family_has_one_lossless_root() {
    let cases = [
        (SyntaxKind::ModuleDeclaration, "mod story\n"),
        (SyntaxKind::UseDeclaration, "use story::Thing\n"),
        (SyntaxKind::FlowItem, "flow opening {}\n"),
        (SyntaxKind::FunctionItem, "fn value() {}\n"),
        (SyntaxKind::PredicateItem, "predicate current() = true\n"),
        (SyntaxKind::ProofItem, "proof verify() {}\n"),
        (SyntaxKind::TraitItem, "trait Render {}\n"),
        (SyntaxKind::ImplItem, "impl Render for Game {}\n"),
        (SyntaxKind::EnumItem, "enum Mood {}\n"),
        (SyntaxKind::StructItem, "struct Point {}\n"),
        (SyntaxKind::TypeAliasItem, "type Count = Int\n"),
        (
            SyntaxKind::ResourceDeclarationItem,
            "res actor: Character {}\n",
        ),
        (SyntaxKind::CharacterDeclarationItem, "character Alice {}\n"),
        (SyntaxKind::ViewDeclarationItem, "view Main() {}\n"),
        (SyntaxKind::ActionDeclarationItem, "action Ping()\n"),
        (SyntaxKind::ActivityDeclarationItem, "activity Game {}\n"),
        (
            SyntaxKind::SignalDeclarationItem,
            "signal Current: Watch<Int>\n",
        ),
        (
            SyntaxKind::MetricDeclarationItem,
            "metric gauge Frame: f32 {}\n",
        ),
        (
            SyntaxKind::LayerDeclarationItem,
            "layer World: world_2d {}\n",
        ),
        (
            SyntaxKind::EntryDeclarationItem,
            "entry cli @entry.cli.main { goto @flow.main }\n",
        ),
        (
            SyntaxKind::ExternCapabilityItem,
            "extern capability audio {}\n",
        ),
        (SyntaxKind::TestItem, "test @test.smoke scenario {}\n"),
        (SyntaxKind::BenchItem, "bench @bench.speed {}\n"),
        (SyntaxKind::StyleItem, "style theme {}\n"),
        (SyntaxKind::ErrorItem, "???\n"),
    ];
    let source = cases.iter().map(|(_, source)| *source).collect::<String>();
    let built = parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|kind| kind.is_item())
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        [
            SyntaxKind::ModuleDeclaration,
            SyntaxKind::UseDeclaration,
            SyntaxKind::FlowItem,
            SyntaxKind::FunctionItem,
            SyntaxKind::PredicateItem,
            SyntaxKind::ProofItem,
            SyntaxKind::TraitItem,
            SyntaxKind::ImplItem,
            SyntaxKind::EnumItem,
            SyntaxKind::StructItem,
            SyntaxKind::TypeAliasItem,
            SyntaxKind::ResourceDeclarationItem,
            SyntaxKind::CharacterDeclarationItem,
            SyntaxKind::ViewDeclarationItem,
            SyntaxKind::ActionDeclarationItem,
            SyntaxKind::ActivityDeclarationItem,
            SyntaxKind::SignalDeclarationItem,
            SyntaxKind::MetricDeclarationItem,
            SyntaxKind::LayerDeclarationItem,
            SyntaxKind::EntryDeclarationItem,
            SyntaxKind::ExternCapabilityItem,
            SyntaxKind::TestItem,
            SyntaxKind::BenchItem,
            SyntaxKind::StyleItem,
            SyntaxKind::ErrorItem,
        ]
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.parse")
            .count(),
        1,
        "{:?}",
        built.diagnostics()
    );
    assert_eq!(built.green().to_string(), source);

    for (expected, source) in cases {
        let attached_document = Arc::new(document(source));
        let snapshot = SourceSnapshotId::initial(attached_document.display_name().clone());
        let mut database = SyntaxDatabase::try_new().unwrap();
        let parsed = database
            .parse_initial(
                snapshot,
                attached_document,
                crate::parser::ParseOptions::default(),
            )
            .unwrap_or_else(|error| panic!("{expected:?} attachment failed: {error:?}"));
        assert_eq!(
            parsed
                .items()
                .unwrap()
                .into_iter()
                .map(|item| item.kind())
                .collect::<Vec<_>>(),
            [expected]
        );
    }

    let attached_document = Arc::new(document(&source));
    let snapshot = SourceSnapshotId::initial(attached_document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(
            snapshot,
            attached_document,
            crate::parser::ParseOptions::default(),
        )
        .unwrap();
    let items = parsed.items().unwrap();
    assert!(matches!(
        items.as_slice(),
        [
            TypedItemNode::Module(_),
            TypedItemNode::Use(_),
            TypedItemNode::Flow(_),
            TypedItemNode::Function(_),
            TypedItemNode::Predicate(_),
            TypedItemNode::Proof(_),
            TypedItemNode::Trait(_),
            TypedItemNode::Impl(_),
            TypedItemNode::Enum(_),
            TypedItemNode::Struct(_),
            TypedItemNode::TypeAlias(_),
            TypedItemNode::Resource(_),
            TypedItemNode::Character(_),
            TypedItemNode::View(_),
            TypedItemNode::Action(_),
            TypedItemNode::Activity(_),
            TypedItemNode::Signal(_),
            TypedItemNode::Metric(_),
            TypedItemNode::Layer(_),
            TypedItemNode::Entry(_),
            TypedItemNode::ExternCapability(_),
            TypedItemNode::Test(_),
            TypedItemNode::Bench(_),
            TypedItemNode::Style(_),
            TypedItemNode::Error(_),
        ]
    ));

    for expected in [
        SyntaxKind::StyleBody,
        SyntaxKind::OpenBraceNode,
        SyntaxKind::CloseBraceNode,
    ] {
        assert!(
            built
                .index()
                .entries()
                .iter()
                .any(|entry| entry.kind() == expected),
            "style item must be structurally dispatched as {expected:?}"
        );
    }
}

#[test]
fn removed_top_level_shapes_are_ordinary_error_items() {
    let source = concat!(
        "extern rust mod native from crate \"native\" {}\n",
        "asset bg_room {}\n",
        "let top = true\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|kind| kind.is_item())
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        [
            SyntaxKind::ErrorItem,
            SyntaxKind::ErrorItem,
            SyntaxKind::ErrorItem,
        ]
    );
    assert!(built.has_recovery());
    assert_eq!(built.green().to_string(), source);
}
