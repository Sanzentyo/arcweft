use std::collections::HashMap;
use std::fmt::Write;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AstNode, AttachedLayerEntry, AttachedLayerExpression, AttachedLayerKind, AttachedLayerPolicy,
    AttachedLayerReference, LayerDeclarationItemKind,
};
use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::id_ref::AuthoredIdRoot;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/layer-attachment-test").unwrap(),
            SourceName::path("layer-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(157).unwrap());
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

fn layers(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<LayerDeclarationItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::LayerDeclarationItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

#[test]
fn layer_attachment_owns_closed_kind_members_references_and_expressions() {
    let snapshot = attach(concat!(
        "/// Dialogue surface\n",
        "#[test.fixture]\n",
        "pub layer dialogue_ui: dialogue {\n",
        "    parent = @layer.root\n",
        "    phase = dialogue\n",
        "    z = 100\n",
        "    visible = true\n",
        "    transform = Transform.identity()\n",
        "    input = hit_test\n",
        "    hit_test = view_tree\n",
        "    capture = none\n",
        "    accessibility = container\n",
        "    view = @<view.MainDialogue>\n",
        "}\n",
    ));
    let declaration = layers(&snapshot)[0].semantics().unwrap();
    assert!(matches!(declaration.kind(), AttachedLayerKind::Dialogue(_)));
    assert_eq!(
        declaration.prefix().documentation().unwrap().markdown(),
        "Dialogue surface"
    );
    assert_eq!(declaration.prefix().attributes().len(), 1);
    assert!(declaration.prefix().visibility().is_some());
    assert!(!declaration.colon().is_missing());
    assert!(!declaration.has_recovery());

    let entries = declaration.body().entries();
    assert_eq!(entries.len(), 10);
    let AttachedLayerEntry::Parent(parent) = &entries[0] else {
        panic!("first member must be parent");
    };
    let reference = parent.value().reference().unwrap().value().unwrap();
    assert!(matches!(reference.root(), AuthoredIdRoot::Absolute { .. }));
    assert_eq!(reference.segments()[0].as_str(), "layer");
    assert_eq!(parent.source_ordinal(), 0);
    assert!(!parent.state().is_duplicate());

    assert!(matches!(
        &entries[1],
        AttachedLayerEntry::Phase(member)
            if matches!(member.value(), AttachedLayerPolicy::PhaseDialogue(_))
    ));
    let AttachedLayerEntry::Z(z) = &entries[2] else {
        panic!("third member must be z");
    };
    assert!(matches!(z.value(), AttachedLayerExpression::Authored(_)));
    assert_eq!(
        z.value().expression().unwrap().snapshot_id(),
        declaration.syntax().snapshot_id()
    );
    assert!(matches!(
        &entries[5],
        AttachedLayerEntry::Input(member)
            if matches!(member.value(), AttachedLayerPolicy::InputHitTest(_))
    ));
    assert!(matches!(
        &entries[6],
        AttachedLayerEntry::HitTest(member)
            if matches!(member.value(), AttachedLayerPolicy::HitTestViewTree(_))
    ));
    assert!(matches!(
        &entries[7],
        AttachedLayerEntry::Capture(member)
            if matches!(member.value(), AttachedLayerPolicy::CaptureNone(_))
    ));
    assert!(matches!(
        &entries[8],
        AttachedLayerEntry::Accessibility(member)
            if matches!(member.value(), AttachedLayerPolicy::AccessibilityContainer(_))
    ));
    let AttachedLayerEntry::View(view) = &entries[9] else {
        panic!("last member must be view");
    };
    let reference = view.value().reference().unwrap().value().unwrap();
    assert!(matches!(
        reference.root(),
        AuthoredIdRoot::Absolute { delimited: true }
    ));
    assert_eq!(reference.segments()[0].as_str(), "view");
}

#[test]
fn layer_attachment_covers_every_authored_kind() {
    let mut source = String::new();
    for (index, kind) in [
        "background",
        "world_2d",
        "character",
        "effects",
        "dialogue",
        "game_view",
        "html_view",
        "activity",
        "modal",
        "overlay",
        "debug",
        "agent",
        "offscreen",
        "custom",
    ]
    .into_iter()
    .enumerate()
    {
        writeln!(source, "layer Layer{index}: {kind} {{}}").expect("String writes are infallible");
    }
    let snapshot = attach(&source);
    let declarations = layers(&snapshot)
        .iter()
        .map(AstNode::<LayerDeclarationItemKind>::semantics)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(matches!(
        declarations[0].kind(),
        AttachedLayerKind::Background(_)
    ));
    assert!(matches!(
        declarations[1].kind(),
        AttachedLayerKind::World2d(_)
    ));
    assert!(matches!(
        declarations[2].kind(),
        AttachedLayerKind::Character(_)
    ));
    assert!(matches!(
        declarations[3].kind(),
        AttachedLayerKind::Effects(_)
    ));
    assert!(matches!(
        declarations[4].kind(),
        AttachedLayerKind::Dialogue(_)
    ));
    assert!(matches!(
        declarations[5].kind(),
        AttachedLayerKind::GameView(_)
    ));
    assert!(matches!(
        declarations[6].kind(),
        AttachedLayerKind::HtmlView(_)
    ));
    assert!(matches!(
        declarations[7].kind(),
        AttachedLayerKind::Activity(_)
    ));
    assert!(matches!(
        declarations[8].kind(),
        AttachedLayerKind::Modal(_)
    ));
    assert!(matches!(
        declarations[9].kind(),
        AttachedLayerKind::Overlay(_)
    ));
    assert!(matches!(
        declarations[10].kind(),
        AttachedLayerKind::Debug(_)
    ));
    assert!(matches!(
        declarations[11].kind(),
        AttachedLayerKind::Agent(_)
    ));
    assert!(matches!(
        declarations[12].kind(),
        AttachedLayerKind::Offscreen(_)
    ));
    assert!(matches!(
        declarations[13].kind(),
        AttachedLayerKind::Custom(_)
    ));
    assert!(
        declarations
            .iter()
            .all(|declaration| !declaration.has_recovery())
    );
}

#[test]
fn layer_attachment_covers_every_closed_policy_value() {
    let fixtures = [
        ("phase", "background", 0),
        ("phase", "world", 1),
        ("phase", "characters", 2),
        ("phase", "effects", 3),
        ("phase", "dialogue", 4),
        ("phase", "game_view", 5),
        ("phase", "html_view", 6),
        ("phase", "modal", 7),
        ("phase", "debug", 8),
        ("phase", "agent_overlay", 9),
        ("input", "ignore", 10),
        ("input", "pass_through", 11),
        ("input", "hit_test", 12),
        ("input", "modal", 13),
        ("input", "capture", 14),
        ("hit_test", "none", 15),
        ("hit_test", "bounds", 16),
        ("hit_test", "view_tree", 17),
        ("hit_test", "object_id_mask", 18),
        ("capture", "none", 19),
        ("capture", "color", 20),
        ("capture", "object_id", 21),
        ("capture", "mask", 22),
        ("capture", "all", 23),
        ("accessibility", "hidden", 24),
        ("accessibility", "exposed", 25),
        ("accessibility", "container", 26),
    ];
    for (member, value, expected) in fixtures {
        let snapshot = attach(&format!("layer Policy: custom {{ {member} = {value} }}\n"));
        let declaration = layers(&snapshot)[0].semantics().unwrap();
        let policy = match &declaration.body().entries()[0] {
            AttachedLayerEntry::Phase(member)
            | AttachedLayerEntry::Input(member)
            | AttachedLayerEntry::HitTest(member)
            | AttachedLayerEntry::Capture(member)
            | AttachedLayerEntry::Accessibility(member) => member.value(),
            entry => panic!("expected policy member, got {entry:?}"),
        };
        assert_eq!(policy_ordinal(policy), expected, "{member} = {value}");
        assert!(!policy.has_recovery());
    }
}

#[test]
fn layer_attachment_retains_typed_recovery_without_source_rediscovery() {
    let snapshot = attach(concat!(
        "layer Broken root {\n",
        "    phase = impossible extra\n",
        "    z\n",
        "    z = 2\n",
        "    parent = @view.parent\n",
        "    view = @<activity.game>\n",
        "    unknown = true\n",
        "}\n",
    ));
    let declaration = layers(&snapshot)[0].semantics().unwrap();
    assert!(declaration.colon().is_missing());
    assert!(matches!(declaration.kind(), AttachedLayerKind::Unknown(_)));
    assert!(declaration.has_recovery());
    let entries = declaration.body().entries();
    assert_eq!(entries.len(), 6);
    let AttachedLayerEntry::Phase(phase) = &entries[0] else {
        panic!("first member must be phase");
    };
    assert!(matches!(phase.value(), AttachedLayerPolicy::Invalid(_)));
    assert!(phase.trailing_recovery().is_some());
    let AttachedLayerEntry::Z(missing) = &entries[1] else {
        panic!("second member must be z");
    };
    assert!(missing.assignment().is_missing());
    assert!(matches!(
        missing.value(),
        AttachedLayerExpression::Missing(_)
    ));
    let AttachedLayerEntry::Z(duplicate) = &entries[2] else {
        panic!("third member must be duplicate z");
    };
    assert!(duplicate.state().is_duplicate());
    assert!(matches!(
        &entries[3],
        AttachedLayerEntry::Parent(member)
            if matches!(member.value(), AttachedLayerReference::WrongFamily { .. })
    ));
    assert!(matches!(
        &entries[4],
        AttachedLayerEntry::View(member)
            if matches!(member.value(), AttachedLayerReference::WrongFamily { .. })
    ));
    assert!(matches!(entries[5], AttachedLayerEntry::Recovery { .. }));
}

#[test]
fn family_relative_reference_keeps_typed_root_for_semantic_resolution() {
    let snapshot = attach(concat!(
        "layer Relative: custom {\n",
        "    parent = @view:.parent\n",
        "    view = @activity:.game\n",
        "}\n",
    ));
    let declaration = layers(&snapshot)[0].semantics().unwrap();
    assert!(!declaration.has_recovery());
    for entry in declaration.body().entries() {
        let reference = match entry {
            AttachedLayerEntry::Parent(member) | AttachedLayerEntry::View(member) => {
                assert!(matches!(
                    member.value(),
                    AttachedLayerReference::Retained { .. }
                ));
                member.value().reference().unwrap().value().unwrap()
            }
            _ => panic!("fixture contains references only"),
        };
        assert!(matches!(
            reference.root(),
            AuthoredIdRoot::FamilyRelative { .. }
        ));
    }
}

#[test]
fn recovered_authored_expression_marks_the_layer_declaration() {
    let snapshot = attach("layer Recovered: custom { z = left + }\n");
    let declaration = layers(&snapshot)[0].semantics().unwrap();
    let AttachedLayerEntry::Z(member) = &declaration.body().entries()[0] else {
        panic!("fixture must contain z");
    };
    assert!(member.value().has_recovery());
    assert!(declaration.has_recovery());
}

fn policy_ordinal(policy: &AttachedLayerPolicy) -> u8 {
    match policy {
        AttachedLayerPolicy::PhaseBackground(_) => 0,
        AttachedLayerPolicy::PhaseWorld(_) => 1,
        AttachedLayerPolicy::PhaseCharacters(_) => 2,
        AttachedLayerPolicy::PhaseEffects(_) => 3,
        AttachedLayerPolicy::PhaseDialogue(_) => 4,
        AttachedLayerPolicy::PhaseGameView(_) => 5,
        AttachedLayerPolicy::PhaseHtmlView(_) => 6,
        AttachedLayerPolicy::PhaseModal(_) => 7,
        AttachedLayerPolicy::PhaseDebug(_) => 8,
        AttachedLayerPolicy::PhaseAgentOverlay(_) => 9,
        AttachedLayerPolicy::InputIgnore(_) => 10,
        AttachedLayerPolicy::InputPassThrough(_) => 11,
        AttachedLayerPolicy::InputHitTest(_) => 12,
        AttachedLayerPolicy::InputModal(_) => 13,
        AttachedLayerPolicy::InputCapture(_) => 14,
        AttachedLayerPolicy::HitTestNone(_) => 15,
        AttachedLayerPolicy::HitTestBounds(_) => 16,
        AttachedLayerPolicy::HitTestViewTree(_) => 17,
        AttachedLayerPolicy::HitTestObjectIdMask(_) => 18,
        AttachedLayerPolicy::CaptureNone(_) => 19,
        AttachedLayerPolicy::CaptureColor(_) => 20,
        AttachedLayerPolicy::CaptureObjectId(_) => 21,
        AttachedLayerPolicy::CaptureMask(_) => 22,
        AttachedLayerPolicy::CaptureAll(_) => 23,
        AttachedLayerPolicy::AccessibilityHidden(_) => 24,
        AttachedLayerPolicy::AccessibilityExposed(_) => 25,
        AttachedLayerPolicy::AccessibilityContainer(_) => 26,
        AttachedLayerPolicy::Invalid(_) | AttachedLayerPolicy::Missing(_) => u8::MAX,
    }
}
