use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::types::TypeRefNodePath;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-signal").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source), crate::parser::ParseOptions::default())
        .expect("Signal grammar builds")
}

fn count_kind(built: &GrammarBuild, kind: SyntaxKind) -> usize {
    built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|actual| *actual == kind)
        .count()
}

#[test]
fn canonical_signal_rows_own_closed_typed_observable_shapes() {
    let source = concat!(
        "pub signal current: Watch<Ref<Flow>>\n",
        "signal events: Stream<GameEvent, EventError>\n",
        "signal sample: Sample<f32>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 3);
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 3);
    assert_eq!(count_kind(&built, SyntaxKind::SignalObservableType), 3);
    assert_eq!(count_kind(&built, SyntaxKind::GenericApplicationType), 4);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn signal_defers_observable_shape_validation_but_rejects_source_policy_tails() {
    let source = concat!(
        "signal count: Counter<u64>\n",
        "signal broken: Stream<Event>\n",
        "signal hosted: Watch<State> = host.current\n",
        "signal plain: State = host.plain\n",
        "signal qualified: game::state::Watch<State> policy runtime\n",
        "signal malformed: Stream<Event = host.events\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 6);
    assert_eq!(count_kind(&built, SyntaxKind::SignalObservableType), 6);
    assert!(
        !built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.signal.invalid_observable_type")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.signal.initializer_not_allowed")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.type.invalid")
    );
    let recovery_path = built
        .index()
        .entries()
        .iter()
        .find(|entry| {
            entry.kind() == SyntaxKind::ErrorNode && entry.role() == SyntaxRole::Recovery(0)
        })
        .expect("forbidden initializer recovery")
        .path()
        .elements();
    let initializer = built
        .index()
        .entries()
        .iter()
        .find(|entry| entry.kind().is_expression() && entry.role() == SyntaxRole::Initializer)
        .expect("typed initializer expression retained under recovery");
    assert!(
        initializer.path().elements().starts_with(recovery_path),
        "initializer expression must remain a child of the Signal recovery node"
    );
    let root_types = built
        .index()
        .entries()
        .iter()
        .filter_map(UnattachedGrammarEntry::type_projection)
        .filter(|projection| projection.path() == &TypeRefNodePath::root())
        .map(|projection| {
            let range = projection.authored().root_source().whole();
            &source[range.start()..range.end()]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        root_types,
        [
            "Counter<u64>",
            "Stream<Event>",
            "Watch<State>",
            "State",
            "game::state::Watch<State>",
            "Stream<Event",
        ],
        "observable Type nodes stop before forbidden declaration tails"
    );
    assert_eq!(count_kind(&built, SyntaxKind::ErrorType), 1);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_body_signal_statement_never_enters_declaration_grammar() {
    let source = concat!(
        "flow main {\n",
        "    signal.set(@signal.current, next)\n",
        "    signal changed <- true\n",
        "}\n",
        "signal changed: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::SignalStatement), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ExpressionStatement), 1);
    assert_eq!(count_kind(&built, SyntaxKind::CallExpression), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn signal_statement_and_postfix_expression_heads_use_structured_lookahead() {
    let source = concat!(
        "flow main {\n",
        "    signal(@signal.current, next)\n",
        "    signal::set(@signal.current, next)\n",
        "    signal[@signal.current]\n",
        "    signal changed <- true\n",
        "    signal changed => false\n",
        "}\n",
    );
    let built = parse(source);

    assert_eq!(count_kind(&built, SyntaxKind::SignalStatement), 2);
    assert_eq!(count_kind(&built, SyntaxKind::ExpressionStatement), 3);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.statement.missing_signal_arrow")
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn signal_missing_colon_and_type_are_zero_width_typed_recovery() {
    let source = concat!(
        "signal NoColon Watch<bool>\n",
        "signal NoType:\n",
        "action Tail()\n",
    );
    let built = parse(source);
    for code in ["syntax.signal.missing_colon", "syntax.signal.missing_type"] {
        let diagnostic = built
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == code)
            .unwrap_or_else(|| panic!("missing {code}: {:?}", built.diagnostics()));
        assert_eq!(diagnostic.range().start(), diagnostic.range().end());
    }
    assert!(count_kind(&built, SyntaxKind::MissingType) >= 1);
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 2);
    assert_eq!(count_kind(&built, SyntaxKind::ActionDeclarationItem), 1);
    assert_eq!(built.green().to_string(), source);
}
