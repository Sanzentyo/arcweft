use super::{compilation_state, compile_project, removed_role_dialogue_project};
use crate::project::CompiledProject;
use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

fn compile_source(source: &str) -> CompiledProject {
    let (project, context) = removed_role_dialogue_project(source);
    let (mut session, parsed) = compilation_state(&project);
    compile_project(&mut session, &project, &parsed, &context).unwrap_or_else(|error| {
        let hir_diagnostics = error.compilation_lease().map(|lease| {
            lease
                .tooling_lease()
                .modules()
                .iter()
                .map(|module| format!("{:?}", module.hir().diagnostics()))
                .collect::<Vec<_>>()
        });
        panic!("static View property compiles: {source}\n{error:?}\nHIR: {hir_diagnostics:?}")
    })
}

fn view_source(body: &str, styles: &str) -> String {
    format!(
        "{styles}\npub character alice {{ display = \"Alice\" }}\nflow main() {{ alice[ready[p]]; }}\nview Main() {{ {body} }}\n"
    )
}

fn main_body(compiled: &CompiledProject) -> &[ViewProgramInstruction] {
    let program = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource();
    let definition = program
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.Main")
        .unwrap();
    let body = &definition.body;
    &program.instructions[body.start_instruction as usize..body.end_instruction as usize]
}

#[test]
fn static_view_geometry_modifiers_compile_with_exact_lengths() {
    let source = view_source("Text(\"a\").x(1px).y(2px).width(3px).height(4px)", "");
    let compiled = compile_source(&source);
    let [ViewProgramInstruction::EmitText { styles, .. }] = main_body(&compiled) else {
        panic!("one text node")
    };
    assert_eq!(styles.len(), 4);
}

#[test]
fn static_view_part_modifier_keeps_checked_local_name() {
    let source = view_source("Text(\"a\").part(caption)", "");
    let compiled = compile_source(&source);
    let [
        ViewProgramInstruction::EmitText {
            part: Some(part), ..
        },
    ] = main_body(&compiled)
    else {
        panic!("one named text node")
    };
    assert_eq!(part.as_str(), "caption");
}

#[test]
fn static_view_constructor_part_keeps_the_same_checked_name() {
    let source = view_source("Panel(part=panel) { Text(\"a\") }", "");
    let compiled = compile_source(&source);
    let [
        ViewProgramInstruction::OpenElement {
            part: Some(part), ..
        },
        ViewProgramInstruction::BeginScope,
        ViewProgramInstruction::EmitText { part: None, .. },
        ViewProgramInstruction::EndScope,
        ViewProgramInstruction::CloseElement,
    ] = main_body(&compiled)
    else {
        panic!("part belongs to the container")
    };
    assert_eq!(part.as_str(), "panel");
}

#[test]
fn static_view_style_reference_keeps_original_declaration() {
    use arcweft_view::style::ViewStyleApplicationTarget;
    let source = view_source(
        "Text(\"a\").style(@style.Theme)",
        "pub style Theme { Text { color = rgba(1,2,3,255) } }",
    );
    let compiled = compile_source(&source);
    let [ViewProgramInstruction::EmitText { styles, .. }] = main_body(&compiled) else {
        panic!("one styled text node")
    };
    let [ViewStyleApplicationTarget::Named { sheet }] = styles.as_slice() else {
        panic!("typed Style identity")
    };
    assert_eq!(sheet.as_str(), "style.Theme");
    assert!(
        compiled
            .view_product()
            .product()
            .style()
            .unwrap()
            .resource()
            .program
            .sheet(sheet)
            .is_some()
    );
}

#[test]
fn static_view_style_aliases_and_explicit_ids_keep_original_sheet_identity() {
    use arcweft_view::style::ViewStyleApplicationTarget;
    for (styles, reference, expected) in [
        ("pub style Theme {}", "Theme", "style.Theme"),
        (
            "use crate.Theme as Accent\npub style Theme {}",
            "Accent",
            "style.Theme",
        ),
        (
            "pub style @style.custom.identity {}",
            "@style.custom.identity",
            "style.custom.identity",
        ),
    ] {
        let compiled = compile_source(&view_source(
            &format!("Text(\"a\").x(1px).style({reference}).y(2px)"),
            styles,
        ));
        let [ViewProgramInstruction::EmitText { styles, .. }] = main_body(&compiled) else {
            panic!("one styled text node")
        };
        let [
            ViewStyleApplicationTarget::Inline { .. },
            ViewStyleApplicationTarget::Named { sheet },
            ViewStyleApplicationTarget::Inline { .. },
        ] = styles.as_slice()
        else {
            panic!("authored application order survives catalog composition")
        };
        assert_eq!(sheet.as_str(), expected);
    }
}

#[test]
fn static_view_container_modifiers_only_style_the_returned_root() {
    let compiled = compile_source(&view_source(
        "Panel { Text(\"a\").x(2px) }.x(1px).width(40px).part(panel)",
        "",
    ));
    let [
        ViewProgramInstruction::OpenElement {
            styles: root,
            part: Some(part),
            ..
        },
        ViewProgramInstruction::BeginScope,
        ViewProgramInstruction::EmitText {
            styles: child,
            part: None,
            ..
        },
        ViewProgramInstruction::EndScope,
        ViewProgramInstruction::CloseElement,
    ] = main_body(&compiled)
    else {
        panic!("a styled parent and an independently styled child")
    };
    assert_eq!(part.as_str(), "panel");
    assert_eq!(root.len(), 2);
    assert_eq!(child.len(), 1);
    assert!(!root.contains(&child[0]));
}

#[test]
fn static_view_branch_modifiers_preserve_each_returned_node() {
    for body in [
        "(if flag { Text(\"a\") } else { Panel { Text(\"b\") } }).width(4px)",
        "(match flag { true => Text(\"a\"), false => Panel { Text(\"b\") } }).width(4px)",
    ] {
        let source = view_source(body, "").replace("view Main()", "view Main(flag: bool)");
        let compiled = compile_source(&source);
        let styles = main_body(&compiled)
            .iter()
            .filter_map(|instruction| match instruction {
                ViewProgramInstruction::OpenElement { styles, .. }
                | ViewProgramInstruction::EmitText { styles, .. } => Some(styles.len()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(styles, [1, 1, 0], "{body}");
    }
}

#[test]
fn static_view_properties_resolve_through_the_product_catalog_and_runtime_cascade() {
    use arcweft_presentation::appearance::{
        ColorScheme, ContrastPreference, PresentationColor, PresentationEnvironment,
        PresentationEnvironmentValues, TextScaleMilli,
    };
    use arcweft_view::{ViewMountId, style::*};
    let compiled = compile_source(&view_source(
        "Text(\"a\").part(caption).x(1px).style(@style.Theme).x(9px).y(2px).width(3px).height(4px)",
        "pub style Theme {\n.caption { color = rgba(1,2,3,255) }\n.caption:hover { color = rgba(4,5,6,255) }\n}",
    ));
    let [ViewProgramInstruction::EmitText { styles, part, .. }] = main_body(&compiled) else {
        panic!("one styled text node")
    };
    let scope = ViewStyleScopeId::new(1);
    let applications = styles
        .iter()
        .enumerate()
        .map(|(order, target)| {
            ViewStyleApplication::new(
                target.clone(),
                scope,
                1,
                u32::try_from(order).unwrap(),
                ViewStyleBoundaryFacts::SAME_VIEW,
            )
        })
        .collect::<Vec<_>>();
    let key = ViewStyleNodeKey::new(ViewMountId::from_raw(1), vec![], 0);
    let environment = PresentationEnvironment::initial(PresentationEnvironmentValues::new(
        ColorScheme::Light,
        ContrastPreference::Standard,
        false,
        TextScaleMilli::ONE,
    ));
    let program = &compiled
        .view_product()
        .product()
        .style()
        .unwrap()
        .resource()
        .program;
    for hovered in [false, true] {
        let interactions = if hovered {
            ViewInteractionStateSet::default().with(ViewInteractionSelector::Hovered)
        } else {
            ViewInteractionStateSet::default()
        };
        let node = ViewStyleNodeFacts::new(Some(ViewStyleTargetKind::Text))
            .with_parts(part.clone(), None)
            .with_interactions(interactions)
            .with_active_scopes(vec![scope]);
        let result = ViewStyleResolver::default()
            .resolve(
                program,
                &ViewStyleResolveContext {
                    node_key: &key,
                    node: &node,
                    ancestors: &[],
                    applications: &applications,
                    parent: None,
                    parent_node_key: None,
                    inherited_axes: ViewInheritedBoxAxes::for_host_seed(
                        key.mount(),
                        ViewBoxAxisSeedGeneration::INITIAL,
                        ViewBoxAxisHostSeed::Default,
                    ),
                    axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
                    environment: &environment,
                    revisions: ViewStyleRevisionSet::default(),
                    trace: ViewStyleTraceMode::Off,
                },
            )
            .unwrap();
        for (property, milli) in [
            (ViewPropertyKind::Left, 9_000),
            (ViewPropertyKind::Top, 2_000),
            (ViewPropertyKind::Width, 3_000),
            (ViewPropertyKind::Height, 4_000),
        ] {
            assert_eq!(
                result.computed().property(property).unwrap().value(),
                &ViewSpecifiedValue::Length {
                    value: ViewLengthMilli::new(milli)
                }
            );
        }
        let color = if hovered {
            PresentationColor::rgba(4, 5, 6, 255)
        } else {
            PresentationColor::rgba(1, 2, 3, 255)
        };
        assert_eq!(
            result
                .computed()
                .property(ViewPropertyKind::Color)
                .unwrap()
                .value(),
            &ViewSpecifiedValue::Color {
                value: ViewColorValue::Literal { color }
            }
        );
    }
}

#[test]
fn static_view_property_arguments_reject_wrong_types_units_and_duplicate_parts() {
    for body in [
        "Text(\"a\").x(1pt)",
        "Text(\"a\").width(1em)",
        "Text(\"a\").height(3)",
        "Text(\"a\").style(@character.alice)",
        "Text(\"a\").style(@style.missing)",
        "Text(\"a\").part(first).part(second)",
        "Panel(part=first).part(second)",
    ] {
        let source = view_source(body, "");
        let (project, context) = removed_role_dialogue_project(&source);
        let (mut session, parsed) = compilation_state(&project);
        assert!(
            compile_project(&mut session, &project, &parsed, &context).is_err(),
            "{body}"
        );
    }
}

#[test]
fn static_view_part_names_keep_dotted_identity_and_inclusive_limits() {
    for name in [
        "caption.body".to_owned(),
        "a".repeat(255),
        "界".repeat(85),
        vec!["p"; 32].join("."),
    ] {
        for body in [
            format!("Text(\"a\").part({name})"),
            format!("Panel(part={name})"),
        ] {
            let compiled = compile_source(&view_source(&body, ""));
            assert_eq!(main_body(&compiled)[0].part().unwrap().as_str(), name);
        }
    }
}

#[test]
fn static_view_part_names_reject_over_limit_and_non_name_inputs() {
    for name in [
        "a".repeat(256),
        "界".repeat(86),
        vec!["p"; 33].join("."),
        "\"caption\"".to_owned(),
        "2".to_owned(),
        "caption + other".to_owned(),
    ] {
        for body in [
            format!("Text(\"a\").part({name})"),
            format!("Panel(part={name})"),
        ] {
            let source = view_source(&body, "");
            let (project, context) = removed_role_dialogue_project(&source);
            let (mut session, parsed) = compilation_state(&project);
            assert!(
                compile_project(&mut session, &project, &parsed, &context).is_err(),
                "{body}"
            );
        }
    }
}
