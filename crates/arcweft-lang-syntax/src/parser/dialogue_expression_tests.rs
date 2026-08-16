use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_document;
use crate::expressions::{
    ExpressionProjection, SyntaxBuiltinRichTextTag, SyntaxDialogueApplicationForm,
    SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection,
    SyntaxRichTextArgumentProjection, SyntaxRichTextTagIdentity,
};
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
fn thread_flow_colon_dialogue_forms_are_typed_and_lossless() {
    for source in [
        "flow opening {\n    alice: Hello.[p]\n}\n",
        "flow opening {\n    alice:\n        Hello.[p]\n}\n",
    ] {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        let applications = built
            .index()
            .entries()
            .iter()
            .filter_map(UnattachedGrammarEntry::expression_projection)
            .filter_map(|projection| match projection.projection() {
                ExpressionProjection::DialogueContentApplication(application) => Some(application),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(applications.len(), 1, "{source}: {applications:?}");
        assert_eq!(
            applications[0].form(),
            &SyntaxDialogueApplicationForm::Colon
        );
        assert!(matches!(
            applications[0].content(),
            SyntaxDialogueContentProjection::Present(_)
        ));
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert!(
            !built
                .index()
                .entries()
                .iter()
                .any(|entry| entry.kind() == SyntaxKind::ErrorExpression)
        );
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn flow_postfix_brackets_select_distinct_typed_lossless_owners() {
    let source = concat!(
        "flow opening {\n",
        "    let handles = alice()[本文です。[p]]\n",
        "    let direct = alice[おはよう。[p]]\n",
        "    let selected = rows[0]\n",
        "    let named = rows[index]\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
        SyntaxKind::PostfixBracketExpression,
        SyntaxKind::DialogueContentApplicationExpression,
        SyntaxKind::CallExpression,
        SyntaxKind::PostfixBracketPayload,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::PostfixBracketExpression)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::DialogueContentApplicationExpression)
            .count(),
        2
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn dot_selector_and_empty_close_are_typed_inference_owned_by_dialogue_grammar() {
    let source = "flow opening {\n    let line = alice[[.shake]effect[/][p]]\n}\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let projection = built
        .index()
        .entries()
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::DialogueContentApplicationExpression)
        .and_then(UnattachedGrammarEntry::expression_projection)
        .expect("dialogue application retains its typed projection");
    let ExpressionProjection::DialogueContentApplication(application) = projection.projection()
    else {
        panic!("selected dialogue application projection");
    };
    let SyntaxDialogueContentProjection::Present(content) = application.content() else {
        panic!("dialogue application retains content");
    };

    assert!(matches!(
        content.nodes().first(),
        Some(SyntaxDialogueNodeProjection::InferredStartTag { tag: 0 })
    ));
    assert!(matches!(
        content.nodes().get(2),
        Some(SyntaxDialogueNodeProjection::InferredEndTag(end))
            if end.is_inferred() && end.issue().is_none()
    ));
    assert_eq!(content.tags()[0].paired_end_node(), Some(2));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
}

#[test]
fn unclosed_dialogue_content_recovers_before_the_next_item() {
    let source = concat!(
        "flow broken {\n",
        "    let handles = alice()[unfinished\n",
        "}\n",
        "proof next() = true\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::PostfixBracketExpression));
    assert!(kinds.contains(&SyntaxKind::ProofItem));
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.expression.missing_postfix_bracket_close"
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unterminated_rich_text_quote_recovers_before_following_tags() {
    let source = concat!(
        "flow opening {\n",
        "    let line = alice[本文。",
        "[effect .wave note=\"unfinished][.sparkle]next[/]]\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
        let source =
            format!("flow opening {{\n    let line = alice[本文。{payload}[.sparkle]]\n}}\n");
        let built =
            parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
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
    let source = format!("flow opening {{\n    let line = alice[本文。{tags}]\n}}\n");
    let built = parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();

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
        "[effect {}][/effect]",
        (0..MAX_RICH_TEXT_TAG_ARGUMENTS)
            .map(|index| format!("k{index}=v"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let content = one_tag.repeat(MAX_RICH_TEXT_CONTENT_ARGUMENTS / MAX_RICH_TEXT_TAG_ARGUMENTS + 3);
    let source = format!("flow opening {{\n    let line = alice[本文。{content}]\n}}\n");
    let built = parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();

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
fn per_tag_argument_one_over_recovers_as_text_without_tag_or_argument_identity() {
    let arguments = (0..=MAX_RICH_TEXT_TAG_ARGUMENTS)
        .map(|index| format!("k{index}=v"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!(
        "flow opening {{\n    let line = alice[本文。[effect {arguments}]text[/effect]]\n}}\n"
    );
    let built = parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();

    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.rich_text.attribute.too_many")
            .count(),
        1
    );
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextTag)
            .count(),
        0
    );
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry.kind(),
                    SyntaxKind::RichTextNamedArgument | SyntaxKind::RichTextPositionalArgument
                )
            })
            .count(),
        0
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn explicit_mark_selector_retains_one_typed_positional_argument() {
    let source = concat!(
        "flow opening {\n",
        "    let line = alice[本文。[mark .checkpoint]]\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    let projection = built
        .index()
        .entries()
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::DialogueContentApplicationExpression)
        .and_then(UnattachedGrammarEntry::expression_projection)
        .expect("mark remains inside one typed Dialogue application");
    let ExpressionProjection::DialogueContentApplication(application) = projection.projection()
    else {
        panic!("selected Dialogue application projection");
    };
    let SyntaxDialogueContentProjection::Present(content) = application.content() else {
        panic!("mark Dialogue application retains content");
    };
    let [tag] = content.tags() else {
        panic!("one explicit marker tag");
    };
    assert!(matches!(
        tag.identity(),
        SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Marker)
    ));
    assert!(matches!(
        tag.arguments(),
        [SyntaxRichTextArgumentProjection::Positional { value }]
            if value.decoded() == ".checkpoint"
    ));

    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextTag)
            .count(),
        1
    );
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextPositionalArgument)
            .count(),
        1
    );
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::RichTextArgumentPayload)
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn overlong_rich_text_body_is_opaque_to_inner_tag_identity() {
    let oversized = format!("[{}[p]]", "a".repeat(MAX_RICH_TEXT_TAG_BODY_BYTES + 1));
    let source =
        format!("flow opening {{\n    let line = alice[本文。{oversized}[.sparkle]]\n}}\n");
    let built = parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();

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
