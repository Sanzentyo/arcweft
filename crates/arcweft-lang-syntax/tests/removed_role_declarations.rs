use arcweft_lang_syntax::ast::items::Item;
use arcweft_lang_syntax::parser::{
    FragmentKind, ParseCompletion, ParseOptions, ParsedFragmentKind, parse_document,
    parse_document_with_source, parse_fragment, parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

const REMOVED_DECLARATIONS: [&str; 3] = [
    "state GameState {\n    value: i32\n}\n",
    "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
    "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
];

fn assert_rejected(parsed: &arcweft_lang_syntax::source::ParsedSource, source: &str) {
    assert!(
        !parsed.errors().is_empty(),
        "removed declaration unexpectedly parsed as current source: {source}"
    );
    assert!(
        parsed
            .typed_tree()
            .items()
            .iter()
            .all(|item| matches!(item, Item::Raw(_))),
        "removed declaration recovery must not produce an executable typed item: {source}\n{:#?}",
        parsed.typed_tree().items()
    );
}

#[test]
fn removed_role_declarations_are_rejected_by_the_current_grammar() {
    for source in REMOVED_DECLARATIONS {
        assert_rejected(&parse_source(source), source);
        assert_rejected(&parse_document(source, ParseOptions::default()), source);

        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("arcweft-test://removed-role/{}", source.len()))
                    .expect("test source ID"),
                SourceName::path("removed.arcw"),
                source,
            )
            .expect("test source document"),
        );
        assert_rejected(
            &parse_document_with_source(document, ParseOptions::default()),
            source,
        );

        let fragment = parse_fragment(source, FragmentKind::Items, ParseOptions::default());
        assert_eq!(fragment.completion(), &ParseCompletion::Invalid);
        assert!(!fragment.errors().is_empty());
        let Some(ParsedFragmentKind::Items(items)) = fragment.kind() else {
            panic!("item fragment must retain non-executable recovery nodes");
        };
        assert!(items.iter().all(|item| matches!(item, Item::Raw(_))));
    }
}

#[test]
fn ordinary_document_entrypoints_share_one_parse_result() {
    let source = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.main {
    controller = smoke
}
";
    let source_parse = parse_source(source);
    let document_parse = parse_document(source, ParseOptions::default());

    assert_eq!(source_parse.errors(), document_parse.errors());
    assert_eq!(source_parse.typed_tree(), document_parse.typed_tree());
}
