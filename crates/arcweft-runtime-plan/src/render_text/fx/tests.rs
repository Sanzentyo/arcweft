use arcweft_lang_hir::{
    lower::lower_to_hir,
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
    let hir = lower_to_hir(parsed.typed_tree()).expect("Fx fixture lowers");
    FxCatalog::try_from_module(&hir).expect("Fx catalog compiles")
}

#[test]
fn rich_text_fx_expands_static_text_layers_and_defaults() {
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
    let (name, layers) = catalog.expand_tag(tag).expect("Fx tag expands");
    assert_eq!(name, "emphasis");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].style.tag_name(), "strong");
    assert_eq!(layers[1].style.tag_name(), "color");
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
        .expand_tag(tag)
        .expect_err("runtime binding is not a closed RichText value");
    assert!(error.to_string().contains("state.color"));
}

#[test]
fn transform_fx_keeps_complete_identity_instead_of_matching_a_builtin_basename() {
    let catalog = catalog(
        r"
#[fx]
fn wave(amplitude: Length = 2px) -> Fx {
    Fx.transform(target = .glyph, amplitude = amplitude)
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
    let (_, layers) = catalog.expand_tag(tag).expect("Fx tag expands");

    assert_eq!(layers[0].selector.as_deref(), Some("crate::wave"));
}
