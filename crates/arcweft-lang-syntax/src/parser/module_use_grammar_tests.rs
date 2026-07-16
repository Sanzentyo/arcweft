use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/module-use-shadow").unwrap(),
        SourceName::path("module-use-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn green_kind_count(node: &rowan::GreenNodeData, kind: SyntaxKind) -> usize {
    usize::from(node.kind() == rowan::SyntaxKind(kind as u16))
        + node
            .children()
            .map(|child| match child {
                rowan::NodeOrToken::Node(child) => green_kind_count(child, kind),
                rowan::NodeOrToken::Token(_) => 0,
            })
            .sum::<usize>()
}

#[test]
fn module_and_use_families_emit_paths_groups_names_aliases_and_globs_losslessly() {
    let source = concat!(
        "mod crate.game.story\n",
        "pub use self.characters.{alice, bob as narrator}\n",
        "use super.common.route_gate as gate\n",
        "use crate.game.prelude.*\n",
        "fn next() {}\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ModuleDeclaration)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::UseDeclaration)
            .count(),
        3
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::Path)
            .count(),
        4
    );
    assert!(kinds.contains(&SyntaxKind::Visibility));
    assert_eq!(
        green_kind_count(built.green(), SyntaxKind::DelimitedGroup),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::NameReference)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::NameDefinition)
            .count(),
        3
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorNode));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_group_close_synchronizes_before_the_following_declaration() {
    let source = "use crate.game.{Hero, Villain\nproof next() = ()\n";
    let next_start = source.find("proof next").unwrap();
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::UseDeclaration)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.use.missing_group_close"
            && diagnostic.range().start() == next_start
            && diagnostic.range().end() == next_start
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_module_path_does_not_consume_the_following_use() {
    let source = "mod\nuse self.characters.alice\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ModuleDeclaration)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::UseDeclaration)
            .count(),
        1
    );
    assert!(kinds.contains(&SyntaxKind::MissingName));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.module.missing_path")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_alias_name_is_typed_without_losing_the_import() {
    let source = "use crate.game.View as\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::Path));
    assert!(kinds.contains(&SyntaxKind::MissingName));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.use.missing_alias")
    );
    assert_eq!(built.green().to_string(), source);
}
