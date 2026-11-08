use super::*;
use arcweft_presentation::appearance::SystemColor;
use arcweft_view::style::*;

fn compiled_sheet(source: &str) -> ViewStyleSheet {
    let compiled = project_view_fixture(source, "arcweft-test://style-composites")
        .compile()
        .expect("typed composite Style source");
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
        .sheet(&ViewStyleSheetId::try_new("style.Composite").unwrap())
        .unwrap();
    for token in sheet.tokens() {
        let range = &decoded.source_map_refs[token.source().value() as usize];
        let declaration = &source[range.start_byte() as usize..range.end_byte() as usize];
        assert!(declaration.starts_with("token "));
        assert!(declaration.contains(token.id().public_id().as_str()));
    }
    sheet.clone()
}

fn token_value<'a>(sheet: &'a ViewStyleSheet, name: &str) -> &'a ViewSpecifiedValue {
    sheet
        .token(&ViewStyleTokenId::try_new(name).unwrap())
        .unwrap()
        .value()
}

fn radii(values: [i32; 4]) -> ViewBorderRadii {
    ViewBorderRadii {
        top_left: ViewLengthMilli::new(values[0]),
        top_right: ViewLengthMilli::new(values[1]),
        bottom_right: ViewLengthMilli::new(values[2]),
        bottom_left: ViewLengthMilli::new(values[3]),
    }
}

#[test]
fn native_style_composites_preserve_order_defaults_exact_units_and_symbolic_colors_through_codec() {
    let sheet = compiled_sheet(
        r#"
style Composite {
  token corners: BorderRadii = radii(bottom_left = 4px, top_right = 2px, top_left = 1px, bottom_right = 3px)
  token uniform: BorderRadii = -1.25px
  token shadows: ShadowList = [
    shadow(color = system_color(.CanvasText), y = 18px, blur = 42px),
    inset_shadow(0px, 1px, 0px, 0px, rgba(255, 255, 255, 46)),
  ]
  token filters: FilterList = [brightness(1.25), contrast(2), opacity(94%), blur(10px)]
  token clipping: Clip = rounded_rect(radii(1px, 2px, 3px, 4px))
  token masking: Mask = mask(resource("image.mask"))
  token image: Resource = resource(id = "image.background")
  token transitions: Transition = [
    transition("opacity", 1.25s, delay = 40ms),
    transition(duration = 4294967295ms, property = "border-radius"),
  ]
  token no_shadows: ShadowList = []
  token no_filters: FilterList = []
  token no_transitions: Transition = []
}
"#,
    );
    assert_eq!(sheet.tokens().len(), 11);
    assert_eq!(
        token_value(&sheet, "corners"),
        &ViewSpecifiedValue::BorderRadii {
            value: radii([1000, 2000, 3000, 4000])
        }
    );
    assert_eq!(
        token_value(&sheet, "uniform"),
        &ViewSpecifiedValue::BorderRadii {
            value: radii([-1250; 4])
        }
    );
    assert_eq!(
        token_value(&sheet, "shadows"),
        &ViewSpecifiedValue::ShadowList {
            value: vec![
                ViewShadow {
                    x: ViewLengthMilli::new(0),
                    y: ViewLengthMilli::new(18000),
                    blur: ViewLengthMilli::new(42000),
                    spread: ViewLengthMilli::new(0),
                    color: ViewColorValue::System {
                        role: SystemColor::CanvasText
                    },
                    inset: false
                },
                ViewShadow {
                    x: ViewLengthMilli::new(0),
                    y: ViewLengthMilli::new(1000),
                    blur: ViewLengthMilli::new(0),
                    spread: ViewLengthMilli::new(0),
                    color: ViewColorValue::Literal {
                        color: arcweft_presentation::appearance::PresentationColor::rgba(
                            255, 255, 255, 46
                        )
                    },
                    inset: true
                },
            ]
        }
    );
    assert_eq!(
        token_value(&sheet, "filters"),
        &ViewSpecifiedValue::FilterList {
            value: vec![
                ViewFilter::Brightness {
                    amount: ViewScalarMilli::new(1250)
                },
                ViewFilter::Contrast {
                    amount: ViewScalarMilli::new(2000)
                },
                ViewFilter::Opacity {
                    amount: ViewRatioMilli::new(940).unwrap()
                },
                ViewFilter::Blur {
                    radius: ViewLengthMilli::new(10000)
                },
            ]
        }
    );
    assert_eq!(
        token_value(&sheet, "clipping"),
        &ViewSpecifiedValue::Clip {
            value: ViewClip::RoundedRect(radii([1000, 2000, 3000, 4000]))
        }
    );
    assert_eq!(
        token_value(&sheet, "masking"),
        &ViewSpecifiedValue::Mask {
            value: ViewMask::Resource(PublicId::try_new("image.mask").unwrap())
        }
    );
    assert_eq!(
        token_value(&sheet, "image"),
        &ViewSpecifiedValue::Resource {
            value: PublicId::try_new("image.background").unwrap()
        }
    );
    assert_eq!(
        token_value(&sheet, "transitions"),
        &ViewSpecifiedValue::Transition {
            value: vec![
                ViewStyleTransition::new(ViewPropertyKind::Opacity, 1250, 40).unwrap(),
                ViewStyleTransition::new(ViewPropertyKind::BorderRadius, u32::MAX, 0).unwrap(),
            ]
        }
    );
    assert_eq!(
        token_value(&sheet, "no_shadows"),
        &ViewSpecifiedValue::ShadowList { value: vec![] }
    );
    assert_eq!(
        token_value(&sheet, "no_filters"),
        &ViewSpecifiedValue::FilterList { value: vec![] }
    );
    assert_eq!(
        token_value(&sheet, "no_transitions"),
        &ViewSpecifiedValue::Transition { value: vec![] }
    );
}

#[test]
fn native_style_composite_operands_resolve_forward_tokens_and_uniform_radii_in_their_sheet() {
    let sheet = compiled_sheet(
        r#"
style Composite {
  token shadows: ShadowList = [shadow(color = token(ink), blur = token(alias))]
  token filters: FilterList = [blur(token(alias))]
  token uniform: BorderRadii = token(alias)
  token clipping: Clip = rounded_rect(token(alias))
  token masking: Mask = mask(token(image))
  token alias: Length = token(radius)
  token radius: Length = 9px
  token ink: Color = system_color(.CanvasText)
  token image: Resource = resource("image.mask")
  Panel { border-radius = token(alias) }
}
style Other { token radius: Length = 200px }
"#,
    );
    assert_eq!(
        token_value(&sheet, "alias"),
        &ViewSpecifiedValue::Token {
            token: ViewStyleTokenId::try_new("radius").unwrap(),
            value_kind: ViewStyleValueKind::Length
        }
    );
    assert_eq!(
        token_value(&sheet, "uniform"),
        &ViewSpecifiedValue::BorderRadii {
            value: radii([9000; 4])
        }
    );
    assert_eq!(
        token_value(&sheet, "clipping"),
        &ViewSpecifiedValue::Clip {
            value: ViewClip::RoundedRect(radii([9000; 4]))
        }
    );
    let ViewSpecifiedValue::ShadowList { value } = token_value(&sheet, "shadows") else {
        panic!("shadow list");
    };
    assert_eq!(value[0].blur.value(), 9000);
    assert_eq!(
        value[0].color,
        ViewColorValue::System {
            role: SystemColor::CanvasText
        }
    );
    assert_eq!(
        token_value(&sheet, "filters"),
        &ViewSpecifiedValue::FilterList {
            value: vec![ViewFilter::Blur {
                radius: ViewLengthMilli::new(9000)
            }]
        }
    );
    assert_eq!(
        token_value(&sheet, "masking"),
        &ViewSpecifiedValue::Mask {
            value: ViewMask::Resource(PublicId::try_new("image.mask").unwrap())
        }
    );
    assert_eq!(
        sheet.rules()[0].declarations()[0].value(),
        &ViewSpecifiedValue::Token {
            token: ViewStyleTokenId::try_new("alias").unwrap(),
            value_kind: ViewStyleValueKind::Length,
        }
    );
    let sheet_id = sheet.id().clone();
    let program = ViewStyleProgram::try_new(vec![sheet], vec![]).unwrap();
    let mount = arcweft_view::ViewMountId::from_raw(23);
    let key = ViewStyleNodeKey::new(mount, vec![], 0);
    let node = ViewStyleNodeFacts::new(Some(
        arcweft_view::ViewStyleTargetKind::from_source_name("Panel").unwrap(),
    ));
    let applications = [ViewStyleApplication::new(
        ViewStyleApplicationTarget::Named { sheet: sheet_id },
        ViewStyleScopeId::new(1),
        0,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    )];
    let environment = arcweft_presentation::appearance::PresentationEnvironment::initial(
        arcweft_presentation::appearance::PresentationEnvironmentValues::ENGINE_DEFAULT,
    );
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
    let resolved = resolver.resolve(&program, &context).unwrap();
    assert_eq!(
        resolved.computed().value(ViewPropertyKind::BorderRadius),
        Some(&ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(9000)
        })
    );
}

#[test]
fn native_style_composite_source_rejects_invalid_binding_domains_precision_and_transition_properties()
 {
    for (kind, values) in [
        (
            "BorderRadii",
            &[
                "radii(1px)",
                "radii(1px, 2px, 3px, 4px, 5px)",
                "radii(1px, 2px, 3px, true)",
            ][..],
        ),
        (
            "ShadowList",
            &[
                "[shadow()]",
                "[shadow(0px, x = 1px, color = rgba(0, 0, 0, 255))]",
                "[shadow(color = rgba(0, 0, 0, 255), missing = 1px)]",
                "[blur(1px)]",
            ][..],
        ),
        (
            "FilterList",
            &[
                "[blur(1)]",
                "[opacity(101%)]",
                "[brightness(-1)]",
                "[brightness(4294967.296)]",
                "[contrast(1, 2)]",
            ][..],
        ),
        (
            "Clip",
            &["rounded_rect(true)", "rounded_rect::<i64>(1px)"][..],
        ),
        ("Mask", &["mask(\"image.mask\")"][..]),
        (
            "Resource",
            &["resource(\"bad identity\")", "resource(1)"][..],
        ),
        (
            "Transition",
            &[
                "[transition(\"display\", 100ms)]",
                "[transition(\"Opacity\", 100ms)]",
                "[transition(\"opacity\", 0.5ms)]",
                "[transition(\"opacity\", 4294967296ms)]",
                "[transition(\"opacity\", -1ms)]",
                "[transition(\"opacity\", 1)]",
                "[transition(\"opacity\", 1ms, delay = 1us)]",
            ][..],
        ),
    ] {
        for value in values {
            let source = format!("style Invalid {{\n token value: {kind} = {value}\n}}\n");
            let error = project_view_fixture(&source, "arcweft-test://style-composite-rejection")
                .compile()
                .expect_err(value);
            assert_eq!(error.stage(), "type-check", "{kind} = {value}: {error:?}");
        }
    }
}

#[test]
fn native_style_composite_duplicate_named_arguments_are_rejected_at_hir_readiness() {
    let source = "style Invalid {\n token value: ShadowList = [shadow(color = rgba(0, 0, 0, 255), x = 1px, x = 2px)]\n}\n";
    let error = project_view_fixture(source, "arcweft-test://style-duplicate-named-argument")
        .compile()
        .unwrap_err();
    assert_eq!(error.stage(), "readiness");
}

#[test]
fn native_style_composite_token_constants_enforce_cycles_and_the_native_depth_bound() {
    for depth in [
        ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH,
        ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH + 1,
    ] {
        let tokens = (0..depth)
            .map(|index| {
                let value = if index + 1 == depth {
                    "1px".to_owned()
                } else {
                    format!("token(t{})", index + 1)
                };
                format!("token t{index}: Length = {value}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!(
            "style Composite {{\n token filters: FilterList = [blur(token(t0))]\n{tokens}\n}}\n"
        );
        let result =
            project_view_fixture(&source, "arcweft-test://style-composite-depth").compile();
        assert_eq!(
            result.is_ok(),
            depth == ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH,
            "depth {depth}: {result:?}"
        );
    }
    let source = "style Composite {\n token filters: FilterList = [blur(token(a))]\n token a: Length = token(b)\n token b: Length = token(a)\n}\n";
    let error = project_view_fixture(source, "arcweft-test://style-composite-cycle")
        .compile()
        .unwrap_err();
    assert_eq!(error.stage(), "type-check");
}
