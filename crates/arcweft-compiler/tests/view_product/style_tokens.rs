use super::*;
use arcweft_presentation::appearance::{
    PresentationEnvironment, PresentationEnvironmentValues, SystemColor,
};
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

#[test]
fn source_token_references_font_stacks_and_palette_values_reach_the_real_resolver() {
    let source = r#"
style First {
  token metric.alias: Length = token(metric.ring)
  token metric.ring: Length = 2px
  token font.stack: FontFamilyList = ["Yu Gothic", "Noto Sans JP", system_font(.Ui)]
  token color.ink: Color = system_color(.CanvasText)
  TextField {
    font-family = token(font.stack)
    color = token(color.ink)
    border-width = token(metric.alias)
  }
}
style Other { token font.stack: FontFamilyList = [system_font(.Serif)] }
view Main() { Text("styles") }
"#;
    let compiled = project_view_fixture(source, "arcweft-test://style-source-token-resolution")
        .compile()
        .unwrap();
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
    let first = ViewStyleSheetId::try_new("style.First").unwrap();
    let other = ViewStyleSheetId::try_new("style.Other").unwrap();
    let font = ViewStyleTokenId::try_new("font.stack").unwrap();
    assert_ne!(
        decoded
            .program
            .resolve_token(&first, &font)
            .unwrap()
            .value(),
        decoded
            .program
            .resolve_token(&other, &font)
            .unwrap()
            .value()
    );
    assert_eq!(
        decoded
            .program
            .resolve_token(&first, &ViewStyleTokenId::try_new("color.ink").unwrap())
            .unwrap()
            .value(),
        &ViewSpecifiedValue::Color {
            value: ViewColorValue::System {
                role: SystemColor::CanvasText
            }
        }
    );
    let mount = arcweft_view::ViewMountId::from_raw(19);
    let key = ViewStyleNodeKey::new(mount, Vec::new(), 0);
    let node = ViewStyleNodeFacts::new(Some(arcweft_view::ViewStyleTargetKind::Element(
        arcweft_view::ViewElementKind::TextField,
    )));
    let applications = [ViewStyleApplication::new(
        ViewStyleApplicationTarget::Named { sheet: first },
        ViewStyleScopeId::new(1),
        0,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    )];
    let environment =
        PresentationEnvironment::initial(PresentationEnvironmentValues::ENGINE_DEFAULT);
    let context = ViewStyleResolveContext {
        node_key: &key,
        node: &node,
        ancestors: &[],
        applications: &applications,
        parent: None,
        parent_node_key: None,
        inherited_axes: ViewInheritedBoxAxes::for_host_seed(
            mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
        ),
        axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
        environment: &environment,
        revisions: ViewStyleRevisionSet::default(),
        trace: ViewStyleTraceMode::Off,
    };
    let mut resolver = ViewStyleResolver::new(ViewStyleResolverLimits::default());
    let result = resolver.resolve(&decoded.program, &context).unwrap();
    assert_eq!(
        result.computed().value(ViewPropertyKind::FontFamily),
        Some(&ViewSpecifiedValue::FontFamilyList {
            value: ViewFontFamilyList::new(vec![
                ViewFontFamily::Named("Yu Gothic".into()),
                ViewFontFamily::Named("Noto Sans JP".into()),
                ViewFontFamily::System(ViewSystemFontFamily::Ui)
            ])
            .unwrap()
        })
    );
    assert_eq!(
        result.computed().value(ViewPropertyKind::BorderWidth),
        Some(&ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(2000)
        })
    );
    assert_eq!(
        result.computed().value(ViewPropertyKind::Color),
        Some(&ViewSpecifiedValue::Color {
            value: ViewColorValue::System {
                role: SystemColor::CanvasText
            }
        })
    );
}

#[test]
fn source_style_construction_rejects_wrong_scopes_types_arity_and_token_graphs() {
    let control = project_view_fixture(
        "style A {\n token good: Length = 1px\n Text { width = token(good) }\n}\n",
        "arcweft-test://style-source-rejection-control",
    )
    .compile()
    .expect("valid control reaches the original sheet");
    assert_eq!(
        control
            .view_product()
            .product()
            .style()
            .unwrap()
            .resource()
            .program
            .sheet(&ViewStyleSheetId::try_new("style.A").unwrap())
            .unwrap()
            .rules()
            .len(),
        1
    );
    for source in [
        "style A { token ink: Color = rgba(1,2,3,255); Text { width = token(ink) } }",
        "style A {\n token ink: Color = rgba(1,2,3,255)\n}\nstyle B {\n Text { color = token(ink) }\n}\n",
        "style A { Text { color = token(missing) } }",
        "style A { Text { color = system_color(.Ui) } }",
        "style A { Text { color = system_color(.CanvasText, .Canvas) } }",
        "style A { token f: FontFamilyList = [system_font(.CanvasText)] }",
        "style A { token f: FontFamilyList = [] }",
        "style A { token f: FontFamilyList = [\"  \t\"] }",
        "style A { token a: Length = token(b); token b: Length = token(a) }",
        "style A { token same: Length = 1px; token same: Length = 2px }",
        "fn escaped() -> String { system_font(.Ui) }",
    ] {
        assert!(
            project_view_fixture(source, "arcweft-test://style-source-rejection")
                .compile()
                .is_err(),
            "{source}"
        );
    }
    let mut source = String::from("style A {\n");
    for index in 0..=ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH {
        source.push_str(&format!("token t{index}: Length = token(t{})\n", index + 1));
    }
    source.push_str(&format!(
        "token t{}: Length = 1px\n}}",
        ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH + 1
    ));
    assert!(
        project_view_fixture(&source, "arcweft-test://style-source-token-depth")
            .compile()
            .is_err()
    );
}
