use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn reactive_ui_style_sample_sidecars_define_css_and_arcweft_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let style_path = root.join("samples/reactive-ui-style/.arcweft/content/ui.style.json");
    let style: Value =
        serde_json::from_slice(&fs::read(style_path).expect("style sidecar")).expect("style json");

    assert_eq!(
        style["arcweft_sources"][0]["identity"]["file"]["path"],
        "styles/reactive-ui.arcwstyle"
    );
    assert_eq!(
        style["css_sources"][0]["identity"]["file"]["path"],
        "styles/reactive-ui.css"
    );
}

#[test]
fn reactive_ui_style_sample_sidecars_define_interaction_selectors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let style_path = root.join("samples/reactive-ui-style/.arcweft/content/ui.style.json");
    let style: Value =
        serde_json::from_slice(&fs::read(style_path).expect("style sidecar")).expect("style json");

    let interactions = style["rules"]
        .as_array()
        .expect("rules array")
        .iter()
        .flat_map(|rule| rule["selector"]["parts"].as_array().into_iter().flatten())
        .filter_map(|part| part["interaction"].as_str())
        .collect::<Vec<_>>();

    assert!(interactions.contains(&"hover"));
    assert!(interactions.contains(&"active"));
    assert!(interactions.contains(&"disabled"));
}
