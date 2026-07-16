use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lsp::{
    features::style_environment as lsp_environment,
    positions::{LineIndex, PositionEncoding},
};
use arcweft_presentation::appearance::PresentationEnvironmentField;
use arcweft_tooling::model::TextEdit;
use arcweft_tooling::style_environment::{
    StyleEnvironmentCodeAction, StyleEnvironmentCodeActionKind, StyleEnvironmentCompletionInput,
    StyleEnvironmentCompletionSite, StyleEnvironmentHover, StyleEnvironmentHoverSubject,
    StyleEnvironmentIntrinsicTarget, StyleEnvironmentNavigationResult,
    StyleEnvironmentSemanticKind, StyleEnvironmentSemanticSpan, complete_style_environment,
};
use lsp_types::{CompletionTextEdit, HoverContents, Position, Uri};

#[test]
fn lsp_adapters_project_tooling_results_without_recomputing() {
    let source = "when environment(color-scheme == dark) {}\n";
    let index = LineIndex::new(source, PositionEncoding::Utf16);
    let replace = TextRange::new(17, 29);
    let tooling = complete_style_environment(StyleEnvironmentCompletionInput {
        site: StyleEnvironmentCompletionSite::Value {
            field: PresentationEnvironmentField::ColorScheme,
        },
        replace,
    });
    let completions = lsp_environment::completion_items(&tooling, &index);
    assert_eq!(
        completions
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        tooling.iter().map(|item| item.label).collect::<Vec<_>>()
    );
    let Some(CompletionTextEdit::Edit(edit)) = &completions[0].text_edit else {
        panic!("completion edit")
    };
    assert_eq!(
        edit.range.start,
        index.position_from_byte_offset(replace.start())
    );
    assert_eq!(
        edit.range.end,
        index.position_from_byte_offset(replace.end())
    );

    let tooling_hover = StyleEnvironmentHover {
        range: replace,
        subject: StyleEnvironmentHoverSubject::Value(PresentationEnvironmentField::ColorScheme),
        markdown: "checked markdown".to_owned(),
    };
    let hover = lsp_environment::hover(&tooling_hover, &index);
    assert!(matches!(
        hover.contents,
        HoverContents::Markup(ref markup) if markup.value == "checked markdown"
    ));
    assert_eq!(hover.range.unwrap().start, Position::new(0, 17));

    let semantic = lsp_environment::semantic_spans(
        &[StyleEnvironmentSemanticSpan {
            range: replace,
            kind: StyleEnvironmentSemanticKind::EnumValue,
        }],
        &index,
    );
    assert_eq!(semantic[0].token_type, 4);

    let uri = "file:///style.arcw".parse::<Uri>().unwrap();
    let actions = lsp_environment::code_actions(
        &[StyleEnvironmentCodeAction {
            kind: StyleEnvironmentCodeActionKind::ReplaceWithEquality,
            title: "Replace with equality comparison",
            edits: Box::new([TextEdit {
                start: 30,
                end: 32,
                replacement: "==".to_owned(),
            }]),
            diagnostics: Box::new([]),
        }],
        &uri,
        &index,
    );
    assert_eq!(
        actions[0]
            .edit
            .as_ref()
            .and_then(|edit| edit.changes.as_ref())
            .and_then(|changes| changes.values().next())
            .and_then(|edits| edits.first())
            .map(|edit| edit.new_text.as_str()),
        Some("==")
    );

    let navigation = lsp_environment::navigation(
        StyleEnvironmentNavigationResult {
            origin: replace,
            target: StyleEnvironmentIntrinsicTarget::Dark,
        },
        &index,
    );
    assert_eq!(navigation.target, StyleEnvironmentIntrinsicTarget::Dark);
    assert_eq!(navigation.origin.start, Position::new(0, 17));
}
