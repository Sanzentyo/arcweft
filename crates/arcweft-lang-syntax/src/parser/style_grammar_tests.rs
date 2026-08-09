use std::fmt::Write;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_document;
use crate::grammar::build::{GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

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
        "pub style theme {\n",
        "    token color.text: Color = rgba(255, 255, 255, 255)\n",
        "    Button.primary:hover > .label {\n",
        "        color = color.text\n",
        "        append opacity = 0.2\n",
        "    }\n",
        "    when environment(text-scale >= 100%, contrast == high) {\n",
        "        Button { color = color.text }\n",
        "    }\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default())
        .expect("shadow grammar builds");
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default())
        .expect("shadow grammar builds");
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default())
        .expect("shadow grammar builds");
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

#[test]
fn style_members_share_the_declaration_member_limit_exactly() {
    let exact = style_with_tokens(SyntaxLimit::DeclarationMembers.maximum());
    let built = parse_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact Style declaration-member limit builds");
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::StyleTokenDeclaration),
        SyntaxLimit::DeclarationMembers.maximum()
    );

    let one_over = style_with_tokens(SyntaxLimit::DeclarationMembers.maximum() + 1);
    assert!(matches!(
        parse_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
}

#[test]
fn mixed_style_members_share_one_aggregate_declaration_member_budget() {
    let maximum = SyntaxLimit::DeclarationMembers.maximum();
    assert_eq!(maximum, 1_024);
    let exact = style_with_mixed_members(maximum);
    let built = parse_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("mixed Style aggregate at 1,024 members builds");
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::StyleTokenDeclaration),
        maximum - 9
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.style.environment_trailing_comma" })
    );

    let one_over = style_with_mixed_members(maximum + 1);
    assert!(matches!(
        parse_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
}

#[test]
fn style_environment_nesting_accepts_exact_limit_and_rejects_one_over() {
    let exact = nested_style_environments(SyntaxLimit::StyleNestingDepth.maximum());
    let built = parse_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact Style environment nesting limit builds");
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::StyleEnvironmentBlock),
        SyntaxLimit::StyleNestingDepth.maximum()
    );

    let one_over = nested_style_environments(SyntaxLimit::StyleNestingDepth.maximum() + 1);
    assert!(matches!(
        parse_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::StyleNestingDepth
        ))
    ));
}

fn style_with_tokens(count: usize) -> String {
    let mut source = String::from("style many {\n");
    for ordinal in 0..count {
        writeln!(source, "token token_{ordinal} = {ordinal}").expect("String writes cannot fail");
    }
    source.push_str("}\n");
    source
}

fn style_with_mixed_members(total_charge: usize) -> String {
    // Top-level rule: rule + sequence + predicate + declaration = 4.
    // Environment: block + clause + nested rule + sequence + declaration = 5.
    // Its trailing-comma recovery is deliberately outside the aggregate.
    const NON_TOKEN_CHARGE: usize = 9;
    let token_count = total_charge
        .checked_sub(NON_TOKEN_CHARGE)
        .expect("mixed fixture requires its fixed rule/environment members");
    let mut source = style_with_tokens(token_count);
    source.truncate(source.len() - "}\n".len());
    source.push_str("Panel:hover { opacity = 1 }\n");
    source.push_str("when environment(color-scheme == dark,) {\n");
    source.push_str("Panel { opacity = 1 }\n");
    source.push_str("}\n}\n");
    source
}

fn nested_style_environments(depth: usize) -> String {
    let mut source = String::from("style nested {\n");
    for _ in 0..depth {
        source.push_str("when environment(color-scheme == dark) {\n");
    }
    for _ in 0..depth {
        source.push_str("}\n");
    }
    source.push_str("}\n");
    source
}
