use super::*;
use arcweft_view::style::*;

#[test]
fn unannotated_style_tokens_infer_all_native_families_from_grammar_and_typed_uses() {
    let source = r#"
style Inferred {
  token color.not_color = 9px
  token alias = token(name = color.not_color)
  token amount = 0.5
  token gain = 0.5
  token weight = 500
  token depth = 2
  token axes = .HorizontalLtr
  token flag = true
  token angle = 0.125turn
  token ink = rgba(alpha = 255, blue = 3, red = 1, green = 2)
  token fonts = ["Yu Gothic", system_font(family = .Ui)]
  token face = .Normal
  token display = .Flex
  token position = .Absolute
  token overflow = .Hidden
  token direction = .Column
  token wrap = .Wrap
  token align = .Center
  token corners = radii(1px, 2px, 3px, 4px)
  token shadows = [shadow(color = token(ink), blur = token(alias))]
  token filters = [blur(token(alias)), brightness(token(gain)), opacity(token(amount))]
  token clip = rounded_rect(token(alias))
  token mask = mask(token(image))
  token blend = .Normal
  token motion = [transition(property = "opacity", duration = 120ms)]
  token image = resource("image.background")
  token empty_shadows = []
  token clip_none = .None
  token mask_none = .None
  Panel {
    opacity = token(amount)
    scale = token(gain)
    font-weight = token(weight)
    z-index = token(depth)
    font-style = token(face)
    blend-mode = token(blend)
    box-shadow = token(empty_shadows)
    clip = token(clip_none)
    mask = token(mask_none)
  }
}
style Other { token amount = rgba(9, 8, 7, 255) }
"#;
    let compiled = project_view_fixture(source, "arcweft-test://style-token-inference")
        .compile()
        .expect("grammar and actual use constraints infer complete native kinds");
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
    let sheet = decoded
        .program
        .sheet(&ViewStyleSheetId::try_new("style.Inferred").unwrap())
        .unwrap();
    let cases = [
        ("axes", ViewStyleValueKind::BoxAxes),
        ("flag", ViewStyleValueKind::Bool),
        ("depth", ViewStyleValueKind::Integer),
        ("amount", ViewStyleValueKind::Ratio),
        ("gain", ViewStyleValueKind::Scalar),
        ("color.not_color", ViewStyleValueKind::Length),
        ("angle", ViewStyleValueKind::Angle),
        ("ink", ViewStyleValueKind::Color),
        ("fonts", ViewStyleValueKind::FontFamilyList),
        ("weight", ViewStyleValueKind::FontWeight),
        ("face", ViewStyleValueKind::FontStyle),
        ("display", ViewStyleValueKind::Display),
        ("position", ViewStyleValueKind::Position),
        ("overflow", ViewStyleValueKind::Overflow),
        ("direction", ViewStyleValueKind::FlexDirection),
        ("wrap", ViewStyleValueKind::FlexWrap),
        ("align", ViewStyleValueKind::Alignment),
        ("corners", ViewStyleValueKind::BorderRadii),
        ("shadows", ViewStyleValueKind::ShadowList),
        ("filters", ViewStyleValueKind::FilterList),
        ("clip", ViewStyleValueKind::Clip),
        ("mask", ViewStyleValueKind::Mask),
        ("blend", ViewStyleValueKind::BlendMode),
        ("motion", ViewStyleValueKind::Transition),
        ("image", ViewStyleValueKind::Resource),
    ];
    assert_eq!(
        cases
            .iter()
            .map(|(_, kind)| *kind)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        ViewStyleValueKind::ALL.len()
    );
    for (name, expected) in cases {
        let token = sheet
            .token(&ViewStyleTokenId::try_new(name).unwrap())
            .unwrap();
        assert_eq!(token.value_kind(), expected, "{name}");
        let range = &decoded.source_map_refs[token.source().value() as usize];
        let declaration = &source[range.start_byte() as usize..range.end_byte() as usize];
        assert!(declaration.starts_with("token "));
        assert!(declaration.contains(name));
    }
    assert_eq!(
        sheet
            .token(&ViewStyleTokenId::try_new("amount").unwrap())
            .unwrap()
            .value(),
        &ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(500).unwrap()
        }
    );
    assert_eq!(
        sheet
            .token(&ViewStyleTokenId::try_new("gain").unwrap())
            .unwrap()
            .value(),
        &ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(500)
        }
    );
    assert_eq!(
        sheet
            .token(&ViewStyleTokenId::try_new("alias").unwrap())
            .unwrap()
            .value(),
        &ViewSpecifiedValue::Token {
            token: ViewStyleTokenId::try_new("color.not_color").unwrap(),
            value_kind: ViewStyleValueKind::Length
        }
    );
    assert_eq!(
        decoded
            .program
            .sheet(&ViewStyleSheetId::try_new("style.Other").unwrap())
            .unwrap()
            .token(&ViewStyleTokenId::try_new("amount").unwrap())
            .unwrap()
            .value_kind(),
        ViewStyleValueKind::Color
    );
}

#[test]
fn unannotated_style_aliases_propagate_the_use_type_across_the_native_maximum_chain() {
    let count = ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH;
    let tokens = (0..count)
        .map(|index| {
            let value = if index + 1 == count {
                "0.5".to_owned()
            } else {
                format!("token(t{})", index + 1)
            };
            format!("token t{index} = {value}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("style Inferred {{\n{tokens}\n Panel {{ opacity = token(t0) }}\n}}\n");
    let compiled = project_view_fixture(&source, "arcweft-test://style-inference-forward-chain")
        .compile()
        .unwrap();
    let program = &compiled
        .view_product()
        .product()
        .style()
        .unwrap()
        .resource()
        .program;
    let sheet = program
        .sheet(&ViewStyleSheetId::try_new("style.Inferred").unwrap())
        .unwrap();
    assert_eq!(sheet.tokens().len(), count);
    assert!(
        sheet
            .tokens()
            .iter()
            .all(|token| token.value_kind() == ViewStyleValueKind::Ratio)
    );
    assert_eq!(
        sheet
            .token(&ViewStyleTokenId::try_new(format!("t{}", count - 1)).unwrap())
            .unwrap()
            .value(),
        &ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(500).unwrap()
        }
    );
}

#[test]
fn unannotated_style_tokens_reject_ambiguity_conflicts_invalid_literals_and_cross_sheet_names() {
    for source in [
        "style Bad {\n token unused = 1\n}\n",
        "style Bad {\n token unused = 0.5\n}\n",
        "style Bad {\n token unused = .Normal\n}\n",
        "style Bad {\n token unused = []\n}\n",
        "style Bad {\n token value = 0.5\n Panel { opacity = token(value); scale = token(value) }\n}\n",
        "style Bad {\n token value = .None\n Panel { clip = token(value); mask = token(value) }\n}\n",
        "style Bad {\n token a = token(b)\n token b = token(a)\n}\n",
        "style Bad {\n token value = 0.0001px\n}\n",
        "style Bad {\n token value = 1i32\n}\n",
        "style Bad {\n token fonts = []\n Text { font-family = token(fonts) }\n}\n",
        "style Other { token value = 1px }\nstyle Bad { Panel { width = token(value) } }\n",
    ] {
        let error = project_view_fixture(source, "arcweft-test://style-inference-rejection")
            .compile()
            .unwrap_err();
        assert_eq!(error.stage(), "type-check", "{source}: {error:?}");
    }
}
