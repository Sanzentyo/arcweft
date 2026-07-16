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
fn every_current_top_level_item_family_has_one_lossless_root() {
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
        "entry start {}\n",
        "extern capability audio {}\n",
        "extern mod native\n",
        "dialogue defaults {}\n",
        "test @test.smoke scenario {}\n",
        "bench @bench.speed {}\n",
        "source data {}\n",
        "style theme {}\n",
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
            SyntaxKind::EntryDeclarationItem,
            SyntaxKind::ExternCapabilityItem,
            SyntaxKind::ExternModuleItem,
            SyntaxKind::DialogueDefaultsItem,
            SyntaxKind::TestItem,
            SyntaxKind::BenchItem,
            SyntaxKind::SourceItem,
            SyntaxKind::StyleItem,
            SyntaxKind::TopLevelFlowItem,
            SyntaxKind::ErrorItem,
        ]
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
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
            | SyntaxKind::EntryDeclarationItem
            | SyntaxKind::ExternCapabilityItem
            | SyntaxKind::ExternModuleItem
            | SyntaxKind::DialogueDefaultsItem
            | SyntaxKind::TestItem
            | SyntaxKind::BenchItem
            | SyntaxKind::SourceItem
            | SyntaxKind::StyleItem
            | SyntaxKind::TopLevelFlowItem
            | SyntaxKind::ErrorItem
    )
}
