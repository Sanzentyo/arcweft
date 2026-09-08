use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-character").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_document(&document(source), crate::parser::ParseOptions::default())
        .expect("Character grammar builds")
}

fn nth_source_range(source: &str, fragment: &str, occurrence: usize) -> SourceRange {
    let start = source
        .match_indices(fragment)
        .nth(occurrence)
        .map(|(start, _)| start)
        .expect("fixture occurrence");
    SourceRange::new(start, start + fragment.len())
}

fn has_kind(built: &GrammarBuild, kind: SyntaxKind) -> bool {
    built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .any(|actual| actual == kind)
}

#[test]
fn canonical_character_owns_typed_header_body_and_expression() {
    let source = concat!(
        "/// Alice\n",
        "#[verify.fixture]\n",
        "pub character @character.alice {\n",
        "    display = \"Alice\"\n",
        "}\n",
    );
    let built = parse(source);
    for kind in [
        SyntaxKind::CharacterDeclarationItem,
        SyntaxKind::DeclarationHeader,
        SyntaxKind::DeclarationPublicId,
        SyntaxKind::CharacterBody,
        SyntaxKind::CharacterDisplayMember,
        SyntaxKind::LiteralExpression,
    ] {
        assert!(has_kind(&built, kind), "missing {kind:?}");
    }
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn empty_character_body_is_typed_and_clean() {
    let source = "character Alice {}\n";
    let built = parse(source);
    assert!(has_kind(&built, SyntaxKind::CharacterBody));
    assert!(!has_kind(&built, SyntaxKind::CharacterDisplayMember));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn character_identity_errors_are_typed_and_do_not_consume_the_next_item() {
    let source = concat!(
        "character @view.alice Alice {}\n",
        "character @.bob Bob {}\n",
        "action Continue()\n",
    );
    let built = parse(source);
    assert!(has_kind(&built, SyntaxKind::WrongFamilyReference));
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::CharacterDeclarationItem)
            .count(),
        2
    );
    assert!(has_kind(&built, SyntaxKind::ActionDeclarationItem));
    let wrong = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.declaration.wrong_family_id")
        .expect("wrong-family diagnostic");
    assert_eq!(
        wrong.range(),
        SourceRange::new(
            source.find("@view.alice").unwrap(),
            source.find("@view.alice").unwrap() + "@view.alice".len(),
        )
    );
    assert!(wrong.related_range().is_some());
    assert!(
        !built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.declaration.relative_id")
    );
}

#[test]
fn character_member_failures_keep_typed_members_and_related_evidence() {
    let source = concat!(
        "character Alice {\n",
        "    display = \"Alice\"\n",
        "    display = \"Other\"\n",
        "    voice = @res.voice\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::CharacterDisplayMember)
            .count(),
        2
    );
    assert!(has_kind(&built, SyntaxKind::ErrorDeclarationMember));
    let duplicate = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.character.duplicate_member")
        .expect("duplicate diagnostic");
    assert_eq!(duplicate.range(), nth_source_range(source, "display", 1));
    assert_eq!(
        duplicate.related_range(),
        Some(nth_source_range(source, "display", 0))
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.character.unknown_member")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn removed_character_header_components_use_ordinary_recovery() {
    let source = "character @character.alice Alice as alice {}\n";
    let built = parse(source);
    assert!(has_kind(&built, SyntaxKind::ErrorNode));
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.declaration.unexpected_header")
        .expect("ordinary unexpected-header diagnostic");
    assert!(!diagnostic.range().is_empty());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_character_body_stops_before_the_following_view() {
    let source = concat!(
        "character Alice {\n",
        "    display = \"Alice\"\n",
        "view Next() { Panel {} }\n",
    );
    let built = parse(source);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.declaration.missing_close")
    );
    assert!(has_kind(&built, SyntaxKind::CharacterDeclarationItem));
    assert!(has_kind(&built, SyntaxKind::ViewDeclarationItem));
    assert_eq!(built.green().to_string(), source);
}
