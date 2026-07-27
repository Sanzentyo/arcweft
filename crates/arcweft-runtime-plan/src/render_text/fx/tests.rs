use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    syntax::{
        ast::dialogue::{DialogueTagKind, DialogueToken},
        parser::parse_source,
        text::parse_dialogue_text,
    },
};

use super::FxCatalog;

fn catalog(source: &str) -> FxCatalog {
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree())
        .expect("Fx fixture lowers");
    FxCatalog::try_from_module(&hir).expect("Fx catalog compiles")
}

#[test]
fn rich_text_fx_retains_typed_application_and_defaults() {
    let catalog = catalog(
        r##"
#[fx]
fn emphasis(accent: Color = rgb("#ffd060")) -> Fx {
    Fx.text(weight = .strong, color = accent)
}
"##,
    );
    let content = parse_dialogue_text("[fx emphasis()]warning[/fx]");
    let tag = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Fx => Some(tag),
            _ => None,
        })
        .expect("Fx tag");
    let (name, application) = catalog.bind_tag(tag, 3).expect("Fx tag binds");
    assert_eq!(name, "emphasis");
    assert_eq!(application.definition().to_string(), "crate::emphasis");
    assert_eq!(application.authored_ordinal(), 3);
    assert_eq!(application.parameters().len(), 1);
}

#[test]
fn rich_text_fx_uses_the_selected_package_identity() {
    let source = r##"
#[fx]
fn emphasis(accent: Color = rgb("#ffd060")) -> Fx {
    Fx.text(weight = .strong, color = accent)
}
"##;
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree())
        .expect("Fx fixture lowers");
    let catalog = FxCatalog::try_from_module_for_package(&hir, "opening-game")
        .expect("package-scoped Fx catalog compiles");
    let content = parse_dialogue_text("[fx emphasis()]warning[/fx]");
    let tag = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Fx => Some(tag),
            _ => None,
        })
        .expect("Fx tag");

    let (_, application) = catalog.bind_tag(tag, 0).expect("Fx tag binds");

    assert_eq!(
        application.definition().to_string(),
        "opening-game::emphasis"
    );
    assert_eq!(
        catalog
            .definitions
            .get("emphasis")
            .expect("compiled definition")
            .id(),
        application.definition()
    );
}

#[test]
fn rich_text_fx_rejects_open_runtime_binding() {
    let catalog = catalog(
        r"
#[fx]
fn emphasis(accent: Color) -> Fx {
    Fx.text(color = accent)
}
",
    );
    let content = parse_dialogue_text("[fx emphasis(accent=state.color)]warning[/fx]");
    let tag = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Fx => Some(tag),
            _ => None,
        })
        .expect("Fx tag");
    let error = catalog
        .bind_tag(tag, 0)
        .expect_err("runtime binding is not a closed RichText value");
    assert!(error.to_string().contains("state.color"));
}

#[test]
fn transform_fx_keeps_sampler_graph_identity_instead_of_a_legacy_label() {
    let catalog = catalog(
        r"
#[fx]
fn wave(amplitude: Length = 2px) -> Fx {
    Fx.transform(
        target = .glyph,
        sample = |ctx| Transform2D { translate_y: amplitude },
    )
}
",
    );
    let content = parse_dialogue_text("[fx wave()]warning[/fx]");
    let tag = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Fx => Some(tag),
            _ => None,
        })
        .expect("Fx tag");
    let (_, application) = catalog.bind_tag(tag, 0).expect("Fx tag binds");

    assert_eq!(application.definition().to_string(), "crate::wave");
    let definition = catalog
        .definitions
        .get("wave")
        .expect("compiled definition remains in catalog");
    let [arcweft_presentation::fx::FxNode::Transform { properties, .. }] =
        definition.graph().nodes()
    else {
        panic!("wave keeps one typed transform node");
    };
    assert!(properties.iter().any(|property| {
        property.name() == "sampler"
            && matches!(
                property.value(),
                arcweft_presentation::fx::FxStaticValue::Sampler(_)
            )
    }));
}
