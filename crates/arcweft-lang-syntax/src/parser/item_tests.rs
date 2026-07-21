use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:item-families").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

#[test]
fn every_current_top_level_declaration_family_has_one_lossless_root() {
    let source = concat!(
        "mod story\n",
        "use story::Thing\n",
        "flow opening {}\n",
        "fn value() {}\n",
        "predicate current() = true\n",
        "proof verify() {}\n",
        "trait Render {}\n",
        "impl Render for Game {}\n",
        "enum Mood {}\n",
        "struct Point {}\n",
        "type Count = Int\n",
        "res actor: Character {}\n",
        "character Alice {}\n",
        "view Main() {}\n",
        "action Ping()\n",
        "activity Game {}\n",
        "signal Current: Watch<Int>\n",
        "metric gauge Frame: f32 {}\n",
        "layer World: world_2d {}\n",
        "entry cli @entry.cli.main { goto @flow.main }\n",
        "extern capability audio {}\n",
        "extern mod native\n",
        "test @test.smoke scenario {}\n",
        "bench @bench.speed {}\n",
        "source data {}\n",
        "style theme {}\n",
        "asset bg_room {}\n",
        "let top = true\n",
        "???\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|kind| is_item_kind(*kind))
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
            SyntaxKind::ErrorItem,
            SyntaxKind::TestItem,
            SyntaxKind::BenchItem,
            SyntaxKind::ErrorItem,
            SyntaxKind::StyleItem,
            SyntaxKind::ErrorItem,
            SyntaxKind::ErrorItem,
            SyntaxKind::ErrorItem,
        ]
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.item.expected_declaration")
            .count(),
        5,
        "{:?}",
        built.diagnostics()
    );
    assert_eq!(built.green().to_string(), source);

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

const fn is_item_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ModuleDeclaration
            | SyntaxKind::UseDeclaration
            | SyntaxKind::FlowItem
            | SyntaxKind::FunctionItem
            | SyntaxKind::PredicateItem
            | SyntaxKind::ProofItem
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem
            | SyntaxKind::EnumItem
            | SyntaxKind::StructItem
            | SyntaxKind::TypeAliasItem
            | SyntaxKind::ResourceDeclarationItem
            | SyntaxKind::CharacterDeclarationItem
            | SyntaxKind::ViewDeclarationItem
            | SyntaxKind::ActionDeclarationItem
            | SyntaxKind::ActivityDeclarationItem
            | SyntaxKind::SignalDeclarationItem
            | SyntaxKind::MetricDeclarationItem
            | SyntaxKind::LayerDeclarationItem
            | SyntaxKind::EntryDeclarationItem
            | SyntaxKind::ExternCapabilityItem
            | SyntaxKind::TestItem
            | SyntaxKind::BenchItem
            | SyntaxKind::StyleItem
            | SyntaxKind::ErrorItem
    )
}
