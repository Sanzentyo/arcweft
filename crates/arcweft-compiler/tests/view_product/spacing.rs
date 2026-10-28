use super::*;
use arcweft_presentation::appearance::{PresentationEnvironment, PresentationEnvironmentValues};
use arcweft_view::{
    ViewElementKind, ViewMountId,
    geometry::{ViewGeometrySize, ViewOuterSize, flow_intrinsic_size},
    style::*,
};

fn spacing_source(element: &str, value: &str) -> String {
    format!(
        "entry cli @entry.main {{ goto @flow.main }}\n\
         flow main() -> String {{ return \"done\" }}\n\
         view Main() {{ {element}(spacing = {value}) {{ Text(\"first\"); Text(\"second\") }} }}\n"
    )
}

#[test]
fn integer_pixel_spacing_reaches_canonical_style_and_physical_child_layout() {
    for (element, property, flow) in [
        (
            ViewElementKind::Row,
            ViewPropertyKind::ColumnGap,
            ViewPhysicalFlow::Row,
        ),
        (
            ViewElementKind::Column,
            ViewPropertyKind::RowGap,
            ViewPhysicalFlow::Column,
        ),
    ] {
        for pixels in [0, 16, 32] {
            let source = spacing_source(element.source_name(), &pixels.to_string());
            let compiled = project_view_fixture_with_entry(&source, "arcweft-test://view-spacing")
                .compile()
                .expect("typed spacing reaches the View product");
            let product = compiled.view_product().product();
            let program = product.program().unwrap().resource();
            let style = product.style().unwrap().resource();
            let decoded =
                arcweft_bundle::resource_codec::view::ViewStyleResource::decode_canonical_section(
                    &style.encode_canonical_section().unwrap(),
                )
                .unwrap();
            assert_eq!(&decoded, style);
            let main = program
                .definitions
                .iter()
                .find(|view| view.public_id.as_str() == "view.Main")
                .unwrap();
            let ViewProgramInstruction::OpenElement { styles, .. } =
                &program.instructions[main.body.start_instruction as usize]
            else {
                panic!("authored container instruction");
            };
            let [ViewStyleApplicationTarget::Inline { patch }] = styles.as_slice() else {
                panic!("one authoritative inline style patch");
            };
            let declarations = decoded.inline_patch(*patch).unwrap().declarations();
            assert_eq!(declarations.len(), 1);
            assert_eq!(declarations[0].property(), property);
            assert_eq!(
                declarations[0].value(),
                &ViewSpecifiedValue::Length {
                    value: ViewLengthMilli::new(pixels * 1_000)
                }
            );
            let range = &decoded.source_map_refs[declarations[0].source().value() as usize];
            assert_eq!(
                &source[range.start_byte() as usize..range.end_byte() as usize],
                pixels.to_string()
            );

            let mount = ViewMountId::from_raw(7);
            let key = ViewStyleNodeKey::new(mount, Vec::new(), main.body.start_instruction);
            let node =
                ViewStyleNodeFacts::new(Some(arcweft_view::ViewStyleTargetKind::Element(element)));
            let applications = [ViewStyleApplication::new(
                styles[0].clone(),
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
            let computed = resolver
                .resolve(&decoded.program, &context)
                .unwrap()
                .computed()
                .clone();
            let container = computed.physical_container(&key, element).unwrap().unwrap();
            assert_eq!(container.flow, flow);
            assert_eq!(
                container.row_gap.value(),
                if flow == ViewPhysicalFlow::Column {
                    pixels * 1_000
                } else {
                    0
                }
            );
            assert_eq!(
                container.column_gap.value(),
                if flow == ViewPhysicalFlow::Row {
                    pixels * 1_000
                } else {
                    0
                }
            );
            let children = [
                ViewOuterSize {
                    width_milli: 20_000,
                    height_milli: 10_000,
                },
                ViewOuterSize {
                    width_milli: 30_000,
                    height_milli: 15_000,
                },
            ];
            assert_eq!(
                flow_intrinsic_size(&key, container, &[]).unwrap(),
                ViewGeometrySize::new(0, 0)
            );
            assert_eq!(
                flow_intrinsic_size(&key, container, &children[..1]).unwrap(),
                ViewGeometrySize::new(20_000, 10_000)
            );
            assert_eq!(
                flow_intrinsic_size(&key, container, &children).unwrap(),
                if flow == ViewPhysicalFlow::Row {
                    ViewGeometrySize::new(u32::try_from(50_000 + pixels * 1_000).unwrap(), 15_000)
                } else {
                    ViewGeometrySize::new(30_000, u32::try_from(25_000 + pixels * 1_000).unwrap())
                }
            );
            assert!(
                resolver
                    .resolve(&decoded.program, &context)
                    .unwrap()
                    .cache_hit()
            );
        }
    }
}

#[test]
fn spacing_conversion_has_exact_nonnegative_fixed_point_bounds() {
    for element in ["Row", "Column"] {
        let accepted = spacing_source(element, "2147483");
        project_view_fixture_with_entry(&accepted, "arcweft-test://spacing-exact-bound")
            .compile()
            .expect("largest representable integer-pixel spacing");
        for value in ["-1", "2147484", "2147483647"] {
            let source = spacing_source(element, value);
            let error =
                project_view_fixture_with_entry(&source, "arcweft-test://spacing-invalid-bound")
                    .compile()
                    .unwrap_err();
            assert_eq!(error.stage(), "view-lower", "{value}: {error:?}");
        }
    }
}

#[test]
fn spacing_uses_the_typed_scalar_and_physical_flow_admission() {
    for element in ViewElementKind::ALL {
        if matches!(element, ViewElementKind::Row | ViewElementKind::Column) {
            continue;
        }
        let source = format!("view Main() {{ {}(spacing = 1) }}", element.source_name());
        assert!(
            project_view_fixture(&source, "arcweft-test://spacing-no-flow")
                .compile()
                .is_err(),
            "{element:?}"
        );
    }
    for body in [
        "Row(spacing = true)",
        "Row(spacing = 1px)",
        "Row(spacing = 1i64)",
        "Row(spacing = 1, spacing = 2)",
        "Row(spacing = value)",
    ] {
        let source = format!("view Main(value: i32 = 1) {{ {body} }}");
        assert!(
            project_view_fixture(&source, "arcweft-test://spacing-scalar-admission")
                .compile()
                .is_err(),
            "{body}"
        );
    }
}
