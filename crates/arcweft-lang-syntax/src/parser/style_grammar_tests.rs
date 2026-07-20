use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/style-shadow").expect("fixed document ID is valid"),
        SourceName::path("style-shadow.arcw"),
        text,
    )
    .expect("test text is a valid source document")
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn style_shadow_grammar_is_lossless_and_owns_typed_members() {
    let source = concat!(
        "/// Main authored theme.\n",
        "#[preview]\n",
        "pub style @style.theme {\n",
        "    token color.text: Color = rgba(255, 255, 255, 255)\n",
        "    Button.primary:hover > .label {\n",
        "        color = color.text\n",
        "        opacity += 0.2\n",
        "    }\n",
        "    when environment(text_scale >= 100%, contrast = high) {\n",
        "        Button { color = color.text }\n",
        "    }\n",
        "}\n",
    );
    let built = parse_shadow_document(&document(source)).expect("shadow grammar builds");
    let entries = built.index().entries();

    for expected in [
        SyntaxKind::StyleItem,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::Visibility,
        SyntaxKind::NameDefinition,
        SyntaxKind::StyleBody,
        SyntaxKind::StyleTokenDeclaration,
        SyntaxKind::StyleRule,
        SyntaxKind::StyleSelector,
        SyntaxKind::StyleSelectorSequence,
        SyntaxKind::StylePropertyDeclaration,
        SyntaxKind::StyleEnvironmentBlock,
        SyntaxKind::StyleEnvironmentCondition,
        SyntaxKind::StyleEnvironmentClause,
        SyntaxKind::CallExpression,
        SyntaxKind::PathType,
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind() == expected),
            "missing {expected:?}: kinds={:?}, diagnostics={:?}",
            entries
                .iter()
                .map(UnattachedGrammarEntry::kind)
                .collect::<Vec<_>>(),
            built.diagnostics(),
        );
    }
    assert_eq!(kind_count(entries, SyntaxKind::StyleRule), 2);
    assert_eq!(kind_count(entries, SyntaxKind::StylePropertyDeclaration), 3);
    assert_eq!(kind_count(entries, SyntaxKind::StyleEnvironmentClause), 2);
    let authored_members = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind(),
                SyntaxKind::StyleTokenDeclaration
                    | SyntaxKind::StyleRule
                    | SyntaxKind::StyleEnvironmentBlock
            )
        })
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();
    assert_eq!(
        authored_members,
        [
            SyntaxKind::StyleTokenDeclaration,
            SyntaxKind::StyleRule,
            SyntaxKind::StyleEnvironmentBlock,
            SyntaxKind::StyleRule,
        ]
    );
    let member_paths = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind(),
                SyntaxKind::StyleTokenDeclaration
                    | SyntaxKind::StyleRule
                    | SyntaxKind::StyleEnvironmentBlock
            )
        })
        .map(|entry| entry.path().elements())
        .collect::<Vec<_>>();
    assert!(
        member_paths.windows(2).all(|paths| paths[0] != paths[1]),
        "identity-bearing Style members must retain distinct event paths: {member_paths:?}"
    );
    assert!(
        entries
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::StyleRule)
            .all(|entry| entry.role() != SyntaxRole::Recovery(0))
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_style_member_recovers_before_later_member_and_declaration() {
    let source = concat!(
        "style theme {\n",
        "    Button { color color.text }\n",
        "    Button { opacity = 1 }\n",
        "}\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).expect("shadow grammar builds");
    let codes = built
        .diagnostics()
        .iter()
        .map(PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::StyleRule),
        2
    );
    assert_eq!(
        kind_count(
            built.index().entries(),
            SyntaxKind::StylePropertyDeclaration
        ),
        2
    );
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::ProofItem),
        1
    );
    assert!(
        codes.contains(&"syntax.style.property_initializer"),
        "{codes:?}"
    );
    let malformed = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.style.property_initializer")
        .expect("malformed property diagnostic is present");
    let value_start = source
        .find("color.text")
        .expect("fixture contains the malformed property value");
    assert_eq!(malformed.range().start(), value_start);
    assert_eq!(malformed.range().end(), value_start);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_style_close_preserves_the_following_declaration() {
    let source = concat!(
        "style theme {\n",
        "    Button { color = color.text }\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).expect("shadow grammar builds");
    let codes = built
        .diagnostics()
        .iter()
        .map(PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::StyleItem),
        1
    );
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::ProofItem),
        1
    );
    assert!(
        codes.contains(&"syntax.style.missing_rule_close")
            || codes.contains(&"syntax.style.missing_body_close"),
        "{codes:?}"
    );
    assert_eq!(built.green().to_string(), source);
}
