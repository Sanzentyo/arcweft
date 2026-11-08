use super::*;
use arcweft_view::style::*;

#[test]
fn sheet_local_token_declarations_keep_checked_values_and_original_source_through_codec() {
    let source = r#"
style First {
  token color.accent: Color = rgba(10, 20, 30, 255)
  token metric.gap: Length = -1.25px
  Panel { color = rgba(1, 2, 3, 255) }
}
style Other {
  token color.accent: Color = rgba(90, 80, 70, 255)
}
view Main() { Text("tokens") }
"#;
    let compiled = project_view_fixture(source, "arcweft-test://style-token-publication")
        .compile()
        .expect("checked token declarations publish");
    let resource = compiled
        .view_product()
        .product()
        .style()
        .unwrap()
        .resource();
    let decoded =
        arcweft_bundle::resource_codec::view::ViewStyleResource::decode_canonical_section(
            &resource.encode_canonical_section().unwrap(),
        )
        .unwrap();
    assert_eq!(&decoded, resource);
    let first = decoded
        .program
        .sheet(&ViewStyleSheetId::try_new("style.First").unwrap())
        .unwrap();
    let other = decoded
        .program
        .sheet(&ViewStyleSheetId::try_new("style.Other").unwrap())
        .unwrap();
    assert_eq!(first.tokens().len(), 2);
    assert_eq!(other.tokens().len(), 1);
    assert_eq!(first.tokens()[0].id(), other.tokens()[0].id());
    assert_eq!(first.tokens()[0].id().public_id().as_str(), "color.accent");
    assert_ne!(first.tokens()[0].value(), other.tokens()[0].value());
    assert_eq!(first.tokens()[0].value_kind(), ViewStyleValueKind::Color);
    assert_eq!(first.tokens()[1].value_kind(), ViewStyleValueKind::Length);
    assert_eq!(
        first.tokens()[1].value(),
        &ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(-1250)
        }
    );
    for (token, authored) in [
        (
            &first.tokens()[0],
            "token color.accent: Color = rgba(10, 20, 30, 255)",
        ),
        (&first.tokens()[1], "token metric.gap: Length = -1.25px"),
        (
            &other.tokens()[0],
            "token color.accent: Color = rgba(90, 80, 70, 255)",
        ),
    ] {
        let range = &decoded.source_map_refs[token.source().value() as usize];
        assert_eq!(
            &source[range.start_byte() as usize..range.end_byte() as usize],
            authored
        );
    }
}
