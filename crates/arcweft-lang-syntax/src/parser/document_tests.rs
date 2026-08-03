use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{DocumentLexer, SyntaxKind, parse_shadow_document};

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/shadow-document").unwrap(),
        SourceName::path("shadow-document.arcw"),
        text,
    )
    .unwrap()
}

#[test]
fn one_pass_lexer_classifies_current_token_families_losslessly() {
    let source = "proof π<'a>(c: Char = \"界\"c) = r##\"x\r\ny\"## // note\r\n@actor.hero";
    let document = document(source);
    let tokens = DocumentLexer::new(source).lex();
    let rebuilt = tokens
        .iter()
        .map(|token| &source[token.range.as_range()])
        .collect::<String>();
    assert_eq!(rebuilt, source);
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == SyntaxKind::KeywordToken)
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == SyntaxKind::LifetimeToken)
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == SyntaxKind::CharacterToken)
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == SyntaxKind::RawStringToken)
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == SyntaxKind::EntityReferenceToken)
    );
    let built = parse_shadow_document(&document, crate::parser::ParseOptions::default()).unwrap();
    assert_eq!(built.green().to_string(), source);
    let stats = built.stats();
    assert_eq!(stats.accepted_source_bytes(), source.len());
    assert_eq!(stats.lexer_tokens(), tokens.len());
    assert_eq!(stats.grammar_events(), built.events().len());
}

#[test]
fn block_comments_split_newlines_without_losing_comment_state() {
    let source = "/** doc\r\nstill */\n/* ordinary */";
    let tokens = DocumentLexer::new(source).lex();
    assert_eq!(
        tokens
            .iter()
            .map(|token| &source[token.range.as_range()])
            .collect::<String>(),
        source
    );
    assert_eq!(tokens[0].kind, SyntaxKind::DocCommentToken);
    assert_eq!(tokens[1].kind, SyntaxKind::NewlineToken);
    assert_eq!(tokens[2].kind, SyntaxKind::DocCommentToken);
    assert_eq!(tokens[3].kind, SyntaxKind::NewlineToken);
    assert_eq!(tokens[4].kind, SyntaxKind::CommentToken);
}

#[test]
fn terminal_entity_reference_colon_remains_a_suite_token() {
    let source = "@choice:.menu @choice.menu:";
    let tokens = DocumentLexer::new(source).lex();
    let significant = tokens
        .iter()
        .filter(|token| token.kind != SyntaxKind::WhitespaceToken)
        .map(|token| (token.kind, &source[token.range.as_range()]))
        .collect::<Vec<_>>();
    assert_eq!(
        significant,
        [
            (SyntaxKind::EntityReferenceToken, "@choice:.menu"),
            (SyntaxKind::EntityReferenceToken, "@choice.menu"),
            (SyntaxKind::PunctuationToken, ":"),
        ]
    );
}

#[test]
fn numeric_ranges_raw_strings_and_character_escapes_keep_exact_boundaries() {
    let source = "1..2 3.14 6.02e-23 0xff_u8 r###\"x\"##y\"### \"界\"c \"\\u{754c}\"c 'life";
    let tokens = DocumentLexer::new(source).lex();
    let significant = tokens
        .iter()
        .filter(|token| token.kind != SyntaxKind::WhitespaceToken)
        .map(|token| (token.kind, &source[token.range.as_range()]))
        .collect::<Vec<_>>();
    assert_eq!(
        significant,
        [
            (SyntaxKind::NumberToken, "1"),
            (SyntaxKind::PunctuationToken, ".."),
            (SyntaxKind::NumberToken, "2"),
            (SyntaxKind::NumberToken, "3.14"),
            (SyntaxKind::NumberToken, "6.02e-23"),
            (SyntaxKind::NumberToken, "0xff_u8"),
            (SyntaxKind::RawStringToken, "r###\"x\"##y\"###"),
            (SyntaxKind::CharacterToken, "\"界\"c"),
            (SyntaxKind::CharacterToken, "\"\\u{754c}\"c"),
            (SyntaxKind::LifetimeToken, "'life"),
        ]
    );
}

#[test]
fn shadow_root_keeps_non_declarations_as_generic_errors() {
    let source = concat!(
        "pub predicate positive(x: Int) = x > 0\n",
        "proof unit() {}\n",
        "pub(crate) fn value() -> Int { 1 }\n",
        "let shown = true\n",
        "???\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(crate::grammar::build::UnattachedGrammarEntry::kind)
        .filter(|kind| {
            matches!(
                kind,
                SyntaxKind::SourceFile
                    | SyntaxKind::PredicateItem
                    | SyntaxKind::ProofItem
                    | SyntaxKind::FunctionItem
                    | SyntaxKind::ErrorItem
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            SyntaxKind::SourceFile,
            SyntaxKind::PredicateItem,
            SyntaxKind::ProofItem,
            SyntaxKind::FunctionItem,
            SyntaxKind::ErrorItem,
            SyntaxKind::ErrorItem,
        ]
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unterminated_non_dialogue_string_retains_typed_literal_recovery() {
    let source = "fn bad() { let values = [\"unfinished] }\n";
    let tokens = DocumentLexer::new(source).lex();
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == SyntaxKind::UnterminatedStringToken)
    );

    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    assert!(
        built
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::LiteralExpression)
    );
    assert!(built.has_recovery());
    assert_eq!(built.green().to_string(), source);
}
