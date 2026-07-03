use arcweft_character::id::CharacterLookId;
use arcweft_character::manifest::{
    CharacterAssetPath, CharacterBlendMode, CharacterLook, CharacterManifest, CharacterPart,
    CharacterPartSelection, CharacterRect, CharacterVariant,
};
use arcweft_presentation::character::{
    CharacterRenderDiagnosticKind, CharacterRenderSpec, CharacterStageBounds,
};

fn manifest() -> CharacterManifest {
    CharacterManifest::from_json(include_str!(
        "fixtures/zundamon.awchar/character.awchar.json"
    ))
    .expect("fixture manifest")
}

#[test]
fn render_spec_preserves_source_canvas_anchor_across_look_switches() {
    let manifest = manifest();
    let normal = CharacterRenderSpec::from_manifest(
        &manifest,
        &CharacterLookId::try_new("normal").expect("look"),
    )
    .expect("normal spec");
    let smile = CharacterRenderSpec::from_manifest(
        &manifest,
        &CharacterLookId::try_new("smile").expect("look"),
    )
    .expect("smile spec");

    assert_eq!(
        normal.source_canvas_bounds(),
        CharacterStageBounds::from_canvas_anchor(manifest.canvas(), manifest.anchor())
    );
    assert_eq!(normal.source_canvas_bounds(), smile.source_canvas_bounds());
    assert_eq!(normal.source_canvas_bounds().x(), -48);
    assert_eq!(normal.source_canvas_bounds().y(), -128);
}

#[test]
fn render_spec_carries_source_psd_layer_names() {
    let manifest = manifest();
    let spec = CharacterRenderSpec::from_manifest(
        &manifest,
        &CharacterLookId::try_new("smile").expect("look"),
    )
    .expect("spec");

    let eyes = spec
        .layers()
        .iter()
        .find(|layer| layer.part().as_str() == "eyes")
        .expect("eyes layer");
    let source = eyes.source_layer().expect("source layer");
    assert_eq!(source.group(), "part:eyes");
    assert_eq!(source.layer(), "smile");
}

#[test]
fn render_spec_reports_unsupported_blend_and_clipping() {
    let mut manifest = manifest();
    let eyes_part = manifest
        .parts()
        .iter()
        .find(|part| part.id().as_str() == "eyes")
        .expect("eyes")
        .clone();
    let unsupported = CharacterPart::new(
        eyes_part.id().clone(),
        eyes_part.z(),
        vec![CharacterVariant::new(
            eyes_part.variants()[0].id().clone(),
            CharacterAssetPath::try_new("layers/diagnostic-eyes.png").expect("path"),
            CharacterRect::new(0, 0, 1, 1),
            255,
            CharacterBlendMode::Hue,
            true,
        )],
    );
    manifest = CharacterManifest::new(
        manifest.character().clone(),
        manifest.canvas(),
        manifest.anchor(),
        CharacterLookId::try_new("diagnostic").expect("look"),
        vec![unsupported],
        vec![CharacterLook::new(
            CharacterLookId::try_new("diagnostic").expect("look"),
            vec![CharacterPartSelection::new(
                eyes_part.id().clone(),
                eyes_part.variants()[0].id().clone(),
            )],
        )],
        manifest.source().cloned(),
    )
    .expect("diagnostic manifest");

    let spec =
        CharacterRenderSpec::from_manifest(&manifest, manifest.default_look()).expect("spec");
    let diagnostics = spec.diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind()
                == CharacterRenderDiagnosticKind::UnsupportedBlendMode)
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind()
                == CharacterRenderDiagnosticKind::UnsupportedClipping)
    );
}
