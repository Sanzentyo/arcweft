use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-signal").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source)).expect("Signal grammar builds")
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
        "pub signal @signal.current current: Watch<Ref<Flow>>\n",
        "signal events: Stream<GameEvent, EventError>\n",
        "signal sample: Sample<f32>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 3);
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
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 3);
    assert_eq!(count_kind(&built, SyntaxKind::SignalObservableType), 3);
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
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_body_signal_statement_never_enters_declaration_grammar() {
    let source = concat!(
        "flow @flow.main {\n",
        "    signal.set(@signal.current, next)\n",
        "    signal changed\n",
        "}\n",
        "signal changed: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    assert!(count_kind(&built, SyntaxKind::SignalStatement) >= 1);
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
    assert_eq!(count_kind(&built, SyntaxKind::ActionDeclarationItem), 1);
    assert_eq!(built.green().to_string(), source);
}
