use arcweft_lang_hir::{
    lower::lower_to_hir,
    model::HirTopLevelDecl,
    style::{HirStyleBody, HirStyleSyntax},
};
use arcweft_lang_syntax::{expr::Expr, parser::parse_source};

#[test]
fn named_style_lowers_to_hir_owned_selector_and_expression_nodes() {
    let parsed = parse_source(
        r"pub style controls {
    token metric.radius: Length = 12px
    Button:hover { border-radius = token(metric.radius) }
}
",
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("style lowers");
    let style = hir
        .declarations()
        .iter()
        .find_map(|declaration| match declaration {
            HirTopLevelDecl::Style(style) => Some(style),
            _ => None,
        })
        .expect("HIR style");
    let HirStyleBody::Arcweft(sheet) = style.body() else {
        panic!("expected native style body");
    };
    assert_eq!(sheet.tokens()[0].public_id(), "metric.radius");
    assert_eq!(
        sheet.rules()[0].selector().sequences()[0]
            .element()
            .expect("element")
            .text(),
        "Button"
    );
    assert!(matches!(
        sheet.rules()[0].declarations()[0].value().expr(),
        Expr::Call { .. }
    ));
}

#[test]
fn lowering_extracts_inline_native_and_css_patches_in_source_order() {
    let parsed = parse_source(
        r#"pub view Example() {
    Button("OK")
        .style { opacity = 900milli }
        .style(.Css) { outline-width: 2px; }
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("View lowers");
    assert_eq!(hir.style_patches().len(), 2);
    assert_eq!(hir.style_patches()[0].ordinal(), 0);
    assert_eq!(hir.style_patches()[0].syntax(), HirStyleSyntax::Arcweft);
    assert_eq!(hir.style_patches()[0].declarations().len(), 1);
    assert_eq!(hir.style_patches()[1].ordinal(), 1);
    assert_eq!(hir.style_patches()[1].syntax(), HirStyleSyntax::Css);
    assert!(
        hir.style_patches()[1]
            .css_source()
            .is_some_and(|source| source.source().contains("outline-width"))
    );
}
