use arcweft_lang_syntax::parser::parse_source;

const SAMPLES: &[(&str, &str)] = &[
    (
        "settings-menu",
        include_str!("../../../examples/ui-interaction-routing/runnable/settings-menu.arcw"),
    ),
    (
        "confirmation-dialog",
        include_str!("../../../examples/ui-interaction-routing/runnable/confirmation-dialog.arcw"),
    ),
    (
        "inventory-menu",
        include_str!("../../../examples/ui-interaction-routing/runnable/inventory-menu.arcw"),
    ),
    (
        "keyboard-focus-list",
        include_str!("../../../examples/ui-interaction-routing/runnable/keyboard-focus-list.arcw"),
    ),
    (
        "settings-panel-surface",
        include_str!(
            "../../../examples/ui-interaction-routing/component-surface/settings-panel.arcw"
        ),
    ),
    (
        "confirmation-surface",
        include_str!(
            "../../../examples/ui-interaction-routing/component-surface/confirmation-dialog.arcw"
        ),
    ),
    (
        "inventory-grid-surface",
        include_str!(
            "../../../examples/ui-interaction-routing/component-surface/inventory-grid.arcw"
        ),
    ),
    (
        "toolbar-surface",
        include_str!("../../../examples/ui-interaction-routing/component-surface/toolbar.arcw"),
    ),
];

#[test]
fn ui_interaction_samples_parse_without_recovery_errors() {
    for (name, source) in SAMPLES {
        let parsed = parse_source(*source);
        assert!(parsed.is_ok(), "{name} parse errors: {:?}", parsed.errors());
    }
}
