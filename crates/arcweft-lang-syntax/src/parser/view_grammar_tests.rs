use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

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

#[test]
fn canonical_view_owns_fixed_signature_exports_fragment_and_typed_values() {
    let source = concat!(
        "pub view @view.MainDialogue MainDialogue(dialogue: DialogueView) {\n",
        "    export part panel as dialogue_panel\n",
        "    Panel {\n",
        "        Text(dialogue.character.display_name)\n",
        "        RichText(dialogue.content)\n",
        "    }.part(panel)\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::Parameter), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ViewExportDeclaration), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ViewFragment), 1);
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
