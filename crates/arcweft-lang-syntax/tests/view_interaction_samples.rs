const SAMPLES: &[(&str, &str)] = &[
    (
        "settings-menu",
        include_str!("../../../examples/view-interaction-routing/runnable/settings-menu.arcw"),
    ),
    (
        "confirmation-dialog",
        include_str!(
            "../../../examples/view-interaction-routing/runnable/confirmation-dialog.arcw"
        ),
    ),
    (
        "inventory-menu",
        include_str!("../../../examples/view-interaction-routing/runnable/inventory-menu.arcw"),
    ),
    (
        "keyboard-focus-list",
        include_str!(
            "../../../examples/view-interaction-routing/runnable/keyboard-focus-list.arcw"
        ),
    ),
    (
        "settings-panel-surface",
        include_str!("../../../examples/view-interaction-routing/view-surface/settings-panel.arcw"),
    ),
    (
        "confirmation-surface",
        include_str!(
            "../../../examples/view-interaction-routing/view-surface/confirmation-dialog.arcw"
        ),
    ),
    (
        "inventory-grid-surface",
        include_str!("../../../examples/view-interaction-routing/view-surface/inventory-grid.arcw"),
    ),
    (
        "toolbar-surface",
        include_str!("../../../examples/view-interaction-routing/view-surface/toolbar.arcw"),
    ),
];

#[test]
fn view_interaction_samples_parse_without_recovery_errors() {
    for (name, source) in SAMPLES {
        let parsed = parse_view_interaction_fixture(name, *source);
        assert!(
            parsed.errors().is_empty(),
            "{name} parse errors: {:?}",
            parsed.errors()
        );
    }
}

fn parse_view_interaction_fixture(
    logical_name: &str,
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(format!(
                "arcweft-test://syntax/view-interaction/{logical_name}"
            ))
            .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path(format!("{logical_name}.arcw")),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
