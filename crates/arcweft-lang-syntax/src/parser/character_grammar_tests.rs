use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
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
    parse_shadow_document(&document(source)).expect("Character grammar builds")
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
fn canonical_character_owns_typed_header_alias_body_and_expression() {
    let source = concat!(
        "/// Alice\n",
        "#[test.fixture]\n",
        "pub character @character.alice Alice as alice {\n",
        "    display_name = \"Alice\"\n",
        "}\n",
    );
    let built = parse(source);
    for kind in [
        SyntaxKind::CharacterDeclarationItem,
        SyntaxKind::DeclarationHeader,
        SyntaxKind::DeclarationPublicId,
        SyntaxKind::SurfaceAlias,
        SyntaxKind::CharacterBody,
        SyntaxKind::CharacterDisplayNameMember,
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
    assert!(!has_kind(&built, SyntaxKind::CharacterDisplayNameMember));
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
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.declaration.relative_id")
    );
}

#[test]
fn character_member_failures_keep_typed_members_and_related_evidence() {
    let source = concat!(
        "character Alice {\n",
        "    display_name = \"Alice\"\n",
        "    display_name = \"Other\"\n",
        "    voice = @res.voice\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::CharacterDisplayNameMember)
            .count(),
        2
    );
    assert!(has_kind(&built, SyntaxKind::ErrorDeclarationMember));
    let duplicate = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.character.duplicate_member")
        .expect("duplicate diagnostic");
    assert!(duplicate.related_range().is_some());
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.character.unknown_member")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn character_missing_name_and_alias_are_zero_width() {
    let source = "character Alice as {}\n";
    let built = parse(source);
    assert!(has_kind(&built, SyntaxKind::MissingName));
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.character.missing_alias")
        .expect("missing Character alias diagnostic");
    assert_eq!(diagnostic.range().start(), diagnostic.range().end());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_character_body_stops_before_the_following_view() {
    let source = concat!(
        "character Alice {\n",
        "    display_name = \"Alice\"\n",
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
