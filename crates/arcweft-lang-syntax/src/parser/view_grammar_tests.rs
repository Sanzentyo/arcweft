use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-view").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source)).expect("View grammar builds")
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

fn source_range(source: &str, fragment: &str) -> SourceRange {
    let start = source.find(fragment).expect("fixture fragment");
    SourceRange::new(start, start + fragment.len())
}

#[test]
fn canonical_view_owns_fixed_signature_exports_fragment_and_typed_values() {
    let source = concat!(
        "pub view @view.MainDialogue MainDialogue(dialogue: DialogueView) {\n",
        "    export part panel as dialogue_panel\n",
        "    Panel {\n",
        "        Text(dialogue.character.display_name)\n",
        "        RichText(dialogue.content)\n",
        "        Style { opacity = 0.5 }\n",
        "    }.part(panel)\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::Parameter), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ViewExportDeclaration), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ViewFragment), 1);
    assert!(count_kind(&built, SyntaxKind::RecordExpression) >= 1);
    assert!(count_kind(&built, SyntaxKind::CallExpression) >= 1);
    assert_eq!(count_kind(&built, SyntaxKind::ErrorExpression), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn view_defaults_are_typed_but_destructuring_and_header_extensions_are_rejected() {
    let source = concat!(
        "view Broken((left, right): Pair, count: u32 = 1) -> View {\n",
        "    Panel {}\n",
        "}\n",
    );
    let built = parse(source);
    assert!(count_kind(&built, SyntaxKind::TuplePattern) >= 1);
    assert!(count_kind(&built, SyntaxKind::LiteralExpression) >= 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.view.invalid_parameter")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.view.return_not_allowed")
    );
}

#[test]
fn misplaced_export_is_typed_and_following_sibling_is_preserved() {
    let source = concat!(
        "view Broken() {\n",
        "    Panel {}\n",
        "    export part panel as public_panel\n",
        "}\n",
        "signal ready: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ViewExportDeclaration), 1);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.view.misplaced_export")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn view_missing_parameters_and_where_clause_recover_without_losing_the_body() {
    let source = concat!(
        "view Missing { Panel {} }\n",
        "view Constrained() where T: View { Panel {} }\n",
    );
    let built = parse(source);
    for code in [
        "syntax.view.missing_parameters",
        "syntax.declaration.unexpected_header",
    ] {
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == code),
            "missing {code}: {:?}",
            built.diagnostics()
        );
    }
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationItem), 2);
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationBody), 2);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn rest_parameter_is_retained_as_typed_invalid_parameter() {
    let source = "view Rest(..values: Items) { Panel {} }\n";
    let built = parse(source);
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.view.invalid_parameter")
        .expect("rest parameter diagnostic");
    assert_eq!(diagnostic.range(), source_range(source, "..values"));
    assert_eq!(count_kind(&built, SyntaxKind::Parameter), 1);
    assert!(count_kind(&built, SyntaxKind::RestPattern) >= 1);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_export_retains_typed_export_recovery_children() {
    let source = concat!(
        "view Broken() {\n",
        "    export local_part\n",
        "    Panel {}\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ViewExportDeclaration), 1);
    assert!(count_kind(&built, SyntaxKind::MissingName) >= 1);
    for code in [
        "syntax.view.export_missing_part",
        "syntax.view.export_missing_as",
        "syntax.view.export_missing_public",
    ] {
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == code),
            "missing {code}: {:?}",
            built.diagnostics()
        );
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unknown_view_line_uses_typed_expression_recovery() {
    let source = concat!(
        "view Broken() {\n",
        "    unexpected raw view words\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationItem), 1);
    assert!(count_kind(&built, SyntaxKind::ErrorExpression) >= 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.view.invalid_value"),
        "{:?}",
        built.diagnostics()
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_nested_view_value_stops_before_the_next_declaration() {
    let source = concat!(
        "view Broken() {\n",
        "    Panel {\n",
        "        Text(\"content\")\n",
        "signal ready: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    assert!(built.missing_tokens().len() >= 2);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.expression.missing_record_close")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.view.missing_body_close")
    );
    assert_eq!(built.green().to_string(), source);
}
