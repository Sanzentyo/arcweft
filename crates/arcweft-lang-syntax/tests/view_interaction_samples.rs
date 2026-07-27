use arcweft_lang_syntax::parser::parse_source;

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
        let parsed = parse_source(*source);
        assert!(
            parsed.errors().is_empty(),
            "{name} parse errors: {:?}",
            parsed.errors()
        );
    }
}
