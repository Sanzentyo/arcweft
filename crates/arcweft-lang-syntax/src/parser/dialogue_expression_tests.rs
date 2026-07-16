use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

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
