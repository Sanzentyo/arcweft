use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;
use crate::text::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS, MAX_RICH_TEXT_TAG_ARGUMENTS,
    MAX_RICH_TEXT_TAG_BODY_BYTES,
};

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:dialogue-expression").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

#[test]
fn flow_dialogue_context_distinguishes_content_from_indexing() {
    let source = concat!(
        "flow @flow.opening opening {\n",
        "    let handles = alice.say()[本文です。[p]]\n",
        "    let direct = alice[おはよう。[p]]\n",
        "    let selected = rows[0]\n",
        "    let named = rows[index]\n",
        "}\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::FlowBody,
        SyntaxKind::Block,
        SyntaxKind::LetStatement,
        SyntaxKind::DialogueCallExpression,
        SyntaxKind::CallExpression,
        SyntaxKind::IndexExpression,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::DialogueCallExpression)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::IndexExpression)
            .count(),
        2
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_dialogue_content_recovers_before_the_next_item() {
    let source = concat!(
        "flow broken {\n",
        "    let handles = alice.say()[unfinished\n",
        "}\n",
        "proof next() = true\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::DialogueCallExpression));
    assert!(kinds.contains(&SyntaxKind::ProofItem));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.expression.missing_dialogue_close")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unterminated_rich_text_quote_recovers_before_following_tags() {
    let source = concat!(
        "flow @flow.opening opening {\n",
        "    let line = alice[本文。",
        "[effect .wave note=\"unfinished][.sparkle]next[/]]\n",
        "}\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();
    let rich_text_kinds = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind(),
                SyntaxKind::RichTextTag
                    | SyntaxKind::RichTextEndTag
                    | SyntaxKind::RichTextInvalidArgument
                    | SyntaxKind::RichTextInvalidArgumentIssue
            )
        })
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        rich_text_kinds,
        [
            SyntaxKind::RichTextTag,
            SyntaxKind::RichTextInvalidArgument,
            SyntaxKind::RichTextInvalidArgumentIssue,
            SyntaxKind::RichTextTag,
            SyntaxKind::RichTextEndTag,
        ]
    );
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.rich_text.attribute.unterminated_quote")
        .expect("unterminated RichText quote diagnostic");
    assert_eq!(&source[diagnostic.range().as_range()], r#""unfinished]"#);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn dedicated_rich_text_payloads_share_unterminated_quote_recovery() {
    for payload in [
        r#"[fx call(note="unfinished]"#,
        r#"[call target(note="unfinished]"#,
        r#"[! target(note="unfinished]"#,
        r#"[if predicate("unfinished]"#,
    ] {
        let source = format!(
            "flow @flow.opening opening {{\n    let line = alice[本文。{payload}[.sparkle]]\n}}\n"
        );
        let built = parse_shadow_document(&document(&source)).unwrap();
        let diagnostics = built
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == "syntax.rich_text.attribute.unterminated_quote"
            })
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 1, "{payload}: {:?}", built.diagnostics());
        assert!(
            source[diagnostics[0].range().as_range()].ends_with(']'),
            "{payload}"
        );
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::RichTextTag)
                .count(),
            2,
            "{payload}"
        );
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn private_rich_text_grammar_stops_at_the_content_tag_limit() {
    let tags = "[p]".repeat(MAX_RICH_TEXT_CONTENT_TAGS + 3);
    let source = format!("flow @flow.opening opening {{\n    let line = alice[本文。{tags}]\n}}\n");
    let built = parse_shadow_document(&document(&source)).unwrap();

    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextTag)
            .count(),
        MAX_RICH_TEXT_CONTENT_TAGS
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.rich_text.content.tag_limit")
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn private_rich_text_grammar_reports_the_content_argument_limit_once() {
    let one_tag = format!(
        "[effect {}]",
        (0..MAX_RICH_TEXT_TAG_ARGUMENTS)
            .map(|index| format!("k{index}=v"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let content = one_tag.repeat(MAX_RICH_TEXT_CONTENT_ARGUMENTS / MAX_RICH_TEXT_TAG_ARGUMENTS + 3);
    let source =
        format!("flow @flow.opening opening {{\n    let line = alice[本文。{content}]\n}}\n");
    let built = parse_shadow_document(&document(&source)).unwrap();

    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| { diagnostic.code() == "syntax.rich_text.content.argument_limit" })
            .count(),
        1
    );
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextNamedArgument)
            .count(),
        MAX_RICH_TEXT_CONTENT_ARGUMENTS
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn mark_payload_does_not_invent_rich_text_arguments() {
    let source = concat!(
        "flow @flow.opening opening {\n",
        "    let line = alice[本文。[mark checkpoint]]\n",
        "}\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();

    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextTag)
            .count(),
        1
    );
    assert!(!built.index().entries().iter().any(|entry| matches!(
        entry.kind(),
        SyntaxKind::RichTextPositionalArgument | SyntaxKind::RichTextArgumentPayload
    )));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn overlong_rich_text_body_is_opaque_to_inner_tag_identity() {
    let oversized = format!("[{}[p]]", "a".repeat(MAX_RICH_TEXT_TAG_BODY_BYTES + 1));
    let source = format!(
        "flow @flow.opening opening {{\n    let line = alice[本文。{oversized}[.sparkle]]\n}}\n"
    );
    let built = parse_shadow_document(&document(&source)).unwrap();

    let tags = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::RichTextTag)
        .collect::<Vec<_>>();
    assert_eq!(tags.len(), 1);
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.rich_text.tag.body_too_long")
        .expect("overlong RichText body diagnostic");
    assert!(diagnostic.range().start() < diagnostic.range().end());
    assert_eq!(built.green().to_string(), source);
}
