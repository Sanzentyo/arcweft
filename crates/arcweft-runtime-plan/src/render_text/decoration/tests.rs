use arcweft_lang_hir::{decoration::DecorationExpansionLimits, lower::lower_to_hir};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    Milli, RichTextCascadeLayer, RichTextColor, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextNode, RichTextParam, RichTextSettingSource, RichTextStyle, RuntimeLineContext,
};

use crate::flow::{RuntimePlanLowerReport, lower_runtime_plan_with_stats};

use super::DecorationCatalog;

const MODULE_PREFIX: &str = r"
character @character.alice Alice as alice {}
";

fn lower(source: &str) -> Result<RuntimePlanLowerReport, String> {
    let parsed = parse_source(source);
    if !parsed.errors().is_empty() {
        let mut messages = Vec::new();
        for error in parsed.errors() {
            messages.push(error.message());
        }
        return Err(messages.join(" | "));
    }
    let hir = lower_to_hir(parsed.typed_tree()).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join(" | ")
    })?;
    lower_runtime_plan_with_stats(&hir).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join(" | ")
    })
}

fn source(declarations: &str, dialogue: &str) -> String {
    format!("{MODULE_PREFIX}\n{declarations}\nflow @flow.main main {{\n    alice: {dialogue}\n}}\n")
}

fn validate_catalog_with_limits(
    declarations: &str,
    limits: DecorationExpansionLimits,
) -> Result<(), String> {
    let parsed = parse_source(source(declarations, "plain[p]"));
    if !parsed.errors().is_empty() {
        return Err(parsed
            .errors()
            .iter()
            .map(arcweft_lang_syntax::parser::recovery::ParseError::message)
            .collect::<Vec<_>>()
            .join(" | "));
    }
    let hir = lower_to_hir(parsed.typed_tree()).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join(" | ")
    })?;
    DecorationCatalog::try_from_module_with_limits(&hir, limits)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn assert_warning_effect_params(effect: &RichTextEffectDescriptor) {
    for (name, expected) in [
        ("amp", RichTextParam::Milli { value: Milli(2000) }),
        (
            "seed",
            RichTextParam::Text {
                value: "warning glow".to_owned(),
            },
        ),
        ("speed", RichTextParam::Int { value: 2 }),
        (
            "dir",
            RichTextParam::Raw {
                value: "0,1".to_owned(),
            },
        ),
        (
            "registry",
            RichTextParam::Raw {
                value: "source-shader".to_owned(),
            },
        ),
        (
            "label",
            RichTextParam::Text {
                value: "custom glow".to_owned(),
            },
        ),
        (
            "numeric",
            RichTextParam::Text {
                value: "2".to_owned(),
            },
        ),
        (
            "truth",
            RichTextParam::Text {
                value: "true".to_owned(),
            },
        ),
    ] {
        assert_eq!(effect.params.get(name), Some(&expected), "parameter {name}");
    }
}

#[test]
fn decoration_expands_defaults_overrides_and_custom_rest_to_existing_styles() {
    let report = lower(&source(
        r##"
decoration warning(
    accent = "#ff4050",
    amplitude = 2px,
    seed = "warning glow",
    ...effect_args,
) {
    strong()
    color(value=accent)
    effect(.wave, amp=amplitude, seed=seed, effect_args...)
}
"##,
        r##"[decorate .warning accent="#ffd060" speed=2 dir=0,1 registry=source-shader label="custom glow" numeric="2" truth="true"]important[/decorate][p]"##,
    ))
    .expect("decoration lowers");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display");
    assert!(
        matches!(
            spec.content.nodes.as_slice(),
            [
                RichTextNode::StyleStart {
                    style: RichTextStyle::Strong { .. }
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::Color {
                        value: RichTextColor::Rgb {
                            red: 255,
                            green: 208,
                            blue: 96
                        }
                    }
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::Effect { .. }
                },
                RichTextNode::Text { .. },
                RichTextNode::StyleEnd { name: effect },
                RichTextNode::StyleEnd { name: color },
                RichTextNode::StyleEnd { name: strong },
                RichTextNode::Control { .. }
            ] if effect == "effect" && color == "color" && strong == "strong"
        ),
        "{:#?}",
        spec.content.nodes
    );
    let RichTextNode::StyleStart {
        style: RichTextStyle::Effect { effect },
    } = &spec.content.nodes[2]
    else {
        panic!("expanded effect style");
    };
    assert_warning_effect_params(effect);

    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("expanded frame resolves");
    assert_eq!(frame.text, "important");
    assert_eq!(frame.display_map.text_runs[0].styles.len(), 3);
}

#[test]
fn nested_decorations_flatten_recursively_and_forward_rest_explicitly() {
    let report = lower(&source(
        r##"
decoration motion(amplitude = 1px, ...custom) {
    effect(.wave, amp=amplitude, custom...)
}

decoration warning(accent = "#ff4050", ...custom) {
    strong()
    color(value=accent)
    decorate(.motion, custom...)
}
"##,
        r"[decorate .warning amplitude=3px speed=2]nested[/decorate][p]",
    ))
    .expect("nested decoration lowers");
    let nodes = &report.line_display_catalog.lines()[0].content.nodes;
    assert!(matches!(
        &nodes[..3],
        [
            RichTextNode::StyleStart {
                style: RichTextStyle::Strong { .. }
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Color { .. }
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect { .. }
            }
        ]
    ));
    let RichTextNode::StyleStart {
        style: RichTextStyle::Effect { effect },
    } = &nodes[2]
    else {
        panic!("nested effect");
    };
    assert_eq!(
        effect.params.get("amp"),
        Some(&RichTextParam::Milli { value: Milli(3000) })
    );
    assert_eq!(
        effect.params.get("speed"),
        Some(&RichTextParam::Int { value: 2 })
    );
}

#[test]
fn declaration_cycles_report_the_complete_chain_even_when_unused() {
    let error = lower(&source(
        r"
decoration a() { decorate(.b) }
decoration b() { decorate(.c) }
decoration c() { decorate(.a) }
",
        "plain[p]",
    ))
    .expect_err("cycle is rejected");
    assert!(error.contains("a -> b -> c -> a"), "{error}");
}

#[test]
fn invocation_contract_rejects_missing_duplicate_unknown_and_positional_arguments() {
    let declarations = r"
decoration required(value) { color(value=value) }
";
    for (dialogue, expected) in [
        (
            "[decorate .required]x[/decorate][p]",
            "missing required argument `value`",
        ),
        (
            "[decorate .required value=red value=blue]x[/decorate][p]",
            "duplicate argument `value`",
        ),
        (
            "[decorate .required value=red extra=1]x[/decorate][p]",
            "has no argument named `extra`",
        ),
        (
            "[decorate .required stray value=red]x[/decorate][p]",
            "named arguments only",
        ),
        (
            r#"[decorate ".required" value=red]x[/decorate][p]"#,
            "must use unquoted `.name` syntax",
        ),
    ] {
        let error = lower(&source(declarations, dialogue)).expect_err("invalid invocation");
        assert!(
            error.contains(expected),
            "expected `{expected}` in `{error}`"
        );
    }
}

#[test]
fn empty_and_scalar_builders_reject_rest_spread_regardless_of_bag_contents() {
    for (layer, expected) in [
        ("strong(custom...)", "does not accept arguments"),
        ("color(custom...)", "requires exactly one `value` argument"),
    ] {
        let declaration = format!("decoration invalid(...custom) {{ {layer} }}");
        for dialogue in ["plain[p]", "[decorate .invalid extra=1]x[/decorate][p]"] {
            let error = lower(&source(&declaration, dialogue))
                .expect_err("rest spread is invalid for this builder shape");
            assert!(
                error.contains(expected),
                "expected `{expected}` for `{layer}` with `{dialogue}` in `{error}`"
            );
        }
    }
}

#[test]
fn visual_builder_rejects_repeated_rest_spread_even_when_the_bag_is_empty() {
    let error = lower(&source(
        "decoration invalid(...custom) { effect(.wave, custom..., custom...) }",
        "plain[p]",
    ))
    .expect_err("a visual builder may spread a rest bag only once");
    assert!(
        error.contains("builder `effect` may spread its rest parameter at most once"),
        "{error}"
    );
}

#[test]
fn decoration_values_reject_runtime_expressions_and_unknown_body_parameters() {
    let call_error = lower(&source(
        r"decoration wave(amount = 1) { effect(.wave, amp=amount) }",
        "[decorate .wave amount=compute(2)]x[/decorate][p]",
    ))
    .expect_err("runtime call rejected");
    assert!(call_error.contains("runtime expression"), "{call_error}");

    let body_error = lower(&source(
        r"decoration wave(amount = 1) { effect(.wave, amp=amout) }",
        "plain[p]",
    ))
    .expect_err("unknown body parameter rejected eagerly");
    assert!(
        body_error.contains("unknown decoration parameter `amout`"),
        "{body_error}"
    );

    let character_error = lower(&source(
        r"decoration wave(seed) { effect(.wave, seed=seed) }",
        r#"[decorate .wave seed="x"c]x[/decorate][p]"#,
    ))
    .expect_err("character literals are not decoration constants");
    assert!(
        character_error.contains("must be a closed literal, selector, or raw identifier token"),
        "{character_error}"
    );
}

#[test]
fn decoration_body_rejects_non_visual_and_host_event_layers() {
    let control_error = lower(&source(
        r"decoration invalid() { speed(value=20) }",
        "plain[p]",
    ))
    .expect_err("speed rejected");
    assert!(
        control_error.contains("unsupported decoration layer builder `speed`"),
        "{control_error}"
    );

    let host_error = lower(&source(
        r#"decoration invalid() { effect(.wave, phase="host_event") }"#,
        "plain[p]",
    ))
    .expect_err("host event rejected");
    assert!(host_error.contains("phase=host_event"), "{host_error}");
}

#[test]
fn decoration_close_is_atomic_but_inferred_close_can_end_an_inner_style() {
    let declaration = r"decoration warning() { strong() }";
    lower(&source(
        declaration,
        "[decorate .warning][.italic]x[/][/decorate][p]",
    ))
    .expect("inner inferred style closes before decoration");

    let crossing = lower(&source(
        declaration,
        "[decorate .warning][strong]x[/decorate][/strong][p]",
    ))
    .expect_err("crossing close rejected");
    assert!(crossing.contains("crosses a rich-text span"), "{crossing}");

    let internal = lower(&source(
        declaration,
        "[decorate .warning]x[/strong][/decorate][p]",
    ))
    .expect_err("internal layer close rejected");
    assert!(
        internal.contains("cannot close an internal layer"),
        "{internal}"
    );

    let unclosed = lower(&source(declaration, "[decorate .warning]x[p]"))
        .expect_err("unclosed decoration rejected");
    assert!(
        unclosed.contains("unclosed rich-text decoration"),
        "{unclosed}"
    );
}

#[test]
fn speed_modifier_inside_decoration_does_not_become_an_open_span() {
    let report = lower(&source(
        "decoration warning() { strong() }",
        "[decorate .warning][speed 24]x[/decorate][p]",
    ))
    .expect("point speed modifier does not block the decoration close");

    let nodes = &report.line_display_catalog.lines()[0].content.nodes;
    assert!(nodes.iter().any(|node| matches!(
        node,
        RichTextNode::StyleStart {
            style: RichTextStyle::Speed { value }
        } if value == "24"
    )));
    assert!(nodes.iter().any(|node| matches!(
        node,
        RichTextNode::StyleEnd { name } if name == "strong"
    )));
}

#[test]
fn quoted_custom_value_containing_host_event_text_remains_a_visual_span() {
    let declarations = "decoration warning() { strong() }";
    for dialogue in [
        r#"[decorate .warning][effect .wave note="not phase=host_event"]x[/effect][/decorate][p]"#,
        r#"[decorate .warning][.wave note="not phase=host_event"]x[/][/decorate][p]"#,
    ] {
        let report = lower(&source(declarations, dialogue))
            .expect("quoted custom text does not change the effect phase");
        assert!(
            report.line_display_catalog.lines()[0]
                .content
                .nodes
                .iter()
                .any(|node| matches!(node, RichTextNode::StyleStart {
                style: RichTextStyle::Effect { effect }
            } if effect.phase == RichTextEffectPhase::GlyphTransform))
        );
    }
}

#[test]
fn nested_rest_forwarding_requires_a_target_rest_parameter() {
    let error = lower(&source(
        r"
decoration leaf(value = 1) { effect(.wave, amp=value) }
decoration outer(...custom) { decorate(.leaf, custom...) }
",
        "plain[p]",
    ))
    .expect_err("target without rest rejected");
    assert!(error.contains("declares no rest parameter"), "{error}");
}

#[test]
fn nested_decoration_rejects_repeated_rest_spread_even_when_the_bag_is_empty() {
    let error = lower(&source(
        r"
decoration leaf(...custom) { effect(.wave, custom...) }
decoration outer(...custom) { decorate(.leaf, custom..., custom...) }
",
        "plain[p]",
    ))
    .expect_err("a rest bag may be forwarded only once");
    assert!(
        error.contains("may spread its rest parameter at most once"),
        "{error}"
    );
}

#[test]
fn decoration_composition_depth_is_bounded_during_eager_validation() {
    let declarations = (0..4)
        .map(|index| {
            if index == 3 {
                format!("decoration d{index}() {{ strong() }}")
            } else {
                format!("decoration d{index}() {{ decorate(.d{}) }}", index + 1)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let error = validate_catalog_with_limits(
        &declarations,
        DecorationExpansionLimits {
            max_depth: 3,
            max_visits: 100,
            max_layers: 100,
        },
    )
    .expect_err("four-level chain exceeds a depth budget of three");
    assert!(error.contains("maximum nesting depth of 3"), "{error}");
    assert!(error.contains("entering `.d3`"), "{error}");
}

#[test]
fn exponential_decoration_dag_is_bounded_by_visits_and_expanded_layers() {
    let declarations = r"
decoration d0() {
    decorate(.d1)
    decorate(.d1)
}
decoration d1() {
    decorate(.d2)
    decorate(.d2)
}
decoration d2() { strong() }
";
    let visit_error = validate_catalog_with_limits(
        declarations,
        DecorationExpansionLimits {
            max_depth: 64,
            max_visits: 6,
            max_layers: 100,
        },
    )
    .expect_err("repeated DAG traversal exceeds the visit budget");
    assert!(
        visit_error.contains("maximum declaration visits of 6"),
        "{visit_error}"
    );

    let layer_error = validate_catalog_with_limits(
        declarations,
        DecorationExpansionLimits {
            max_depth: 64,
            max_visits: 100,
            max_layers: 3,
        },
    )
    .expect_err("four concrete styles exceed the expanded-layer budget");
    assert!(
        layer_error.contains("maximum expanded style layers of 3"),
        "{layer_error}"
    );
}

#[test]
fn closed_decoration_builder_families_reject_unknown_selectors() {
    for (layer, selector) in [
        ("style(.italci)", ".italci"),
        ("layout(.vertcial_rl)", ".vertcial_rl"),
        ("transform(.offest)", ".offest"),
    ] {
        let error = lower(&source(
            &format!("decoration invalid() {{ {layer} }}"),
            "plain[p]",
        ))
        .expect_err("closed selector typo is rejected");
        assert!(error.contains("does not support selector"), "{error}");
        assert!(error.contains(selector), "{error}");
    }

    lower(&source(
        "decoration custom() { effect(.registry_owned) }",
        "[decorate .custom]open selector[/decorate][p]",
    ))
    .expect("effect selectors remain registry-extensible");
}

#[test]
fn invocation_argument_names_must_be_identifiers_but_support_unicode() {
    let declarations = "decoration custom(...params) { effect(.wave, params...) }";
    for argument in ["=1", "1bad=1", "bad-name=1"] {
        let error = lower(&source(
            declarations,
            &format!("[decorate .custom {argument}]x[/decorate][p]"),
        ))
        .expect_err("invalid invocation argument name is rejected");
        assert!(error.contains("canonical identifier"), "{error}");
    }

    let report = lower(&source(
        declarations,
        "[decorate .custom 速度=2]x[/decorate][p]",
    ))
    .expect("Unicode identifiers use the canonical lexer predicate");
    let RichTextNode::StyleStart {
        style: RichTextStyle::Effect { effect },
    } = &report.line_display_catalog.lines()[0].content.nodes[0]
    else {
        panic!("expanded effect style");
    };
    assert_eq!(
        effect.params.get("速度"),
        Some(&RichTextParam::Int { value: 2 })
    );
}

#[test]
fn signed_duration_defaults_are_closed_decoration_values() {
    let report = lower(&source(
        "decoration delayed(delay = -500ms) { effect(.wave, delay=delay) }",
        "[decorate .delayed]x[/decorate][p]",
    ))
    .expect("negative duration remains a deterministic authored token");
    let RichTextNode::StyleStart {
        style: RichTextStyle::Effect { effect },
    } = &report.line_display_catalog.lines()[0].content.nodes[0]
    else {
        panic!("expanded effect style");
    };
    assert_eq!(
        effect.params.get("delay"),
        Some(&RichTextParam::Raw {
            value: "-500ms".to_owned()
        })
    );
}

#[test]
fn unbound_required_parameter_is_not_interpreted_as_its_identifier() {
    let declaration = "decoration d(host_event) { effect(.wave, phase=host_event) }";
    let report = lower(&source(
        declaration,
        "[decorate .d host_event=glyph_transform]x[/decorate][p]",
    ))
    .expect("the caller binds the required phase before renderer validation");
    let RichTextNode::StyleStart {
        style: RichTextStyle::Effect { effect },
    } = &report.line_display_catalog.lines()[0].content.nodes[0]
    else {
        panic!("expanded effect style");
    };
    assert_eq!(effect.phase, RichTextEffectPhase::GlyphTransform);

    let invocation_error = lower(&source(
        declaration,
        "[decorate .d host_event=host_event]x[/decorate][p]",
    ))
    .expect_err("an authored host-event binding remains forbidden");
    assert!(
        invocation_error.contains("phase=host_event"),
        "{invocation_error}"
    );

    let default_error = lower(&source(
        "decoration d(phase = host_event) { effect(.wave, phase=phase) }",
        "plain[p]",
    ))
    .expect_err("an authored host-event default remains forbidden");
    assert!(
        default_error.contains("phase=host_event"),
        "{default_error}"
    );

    let sibling_error = lower(&source(
        r#"decoration d(amount) { effect(.wave, phase="host_event", amp=amount) }"#,
        "plain[p]",
    ))
    .expect_err("an unbound sibling must not suppress known-value validation");
    assert!(
        sibling_error.contains("phase=host_event"),
        "{sibling_error}"
    );
}

#[test]
fn decoration_layers_emit_direct_equivalent_inline_color_contributions() {
    let declaration = r##"decoration accent(value = "#a8b5ff") { color(value=value) }"##;

    let default_source = source(declaration, "[decorate .accent]default[/decorate][p]");
    let default_report = lower(&default_source).expect("default decoration lowers");
    let default_color = default_report.line_display_catalog.lines()[0]
        .style_contributions
        .iter()
        .find(|contribution| {
            contribution.layer == RichTextCascadeLayer::InlineSpan
                && contribution.path == "rich_text.text.color"
        })
        .expect("expanded default color contribution");
    assert_eq!(default_color.value, "#a8b5ff");
    let selector_start = default_source
        .find(".accent")
        .expect("decoration invocation selector");
    assert_eq!(
        contribution_range(default_color),
        Some((selector_start, selector_start + ".accent".len()))
    );

    let override_source = source(
        declaration,
        r##"[decorate .accent value="#ff4050"]decorated[/decorate][color value="#ff4050"]direct[/color][p]"##,
    );
    let override_report = lower(&override_source).expect("override and direct color lower");
    let mut colors = override_report.line_display_catalog.lines()[0]
        .style_contributions
        .iter()
        .filter(|contribution| {
            contribution.layer == RichTextCascadeLayer::InlineSpan
                && contribution.path == "rich_text.text.color"
        })
        .collect::<Vec<_>>();
    assert_eq!(colors.len(), 2, "contributions: {colors:#?}");
    assert!(
        colors
            .iter()
            .all(|contribution| contribution.value == "#ff4050")
    );

    let mut expected_ranges = override_source
        .match_indices(r##""#ff4050""##)
        .map(|(start, value)| (start, start + value.len()))
        .collect::<Vec<_>>();
    let mut actual_ranges = colors
        .drain(..)
        .filter_map(contribution_range)
        .collect::<Vec<_>>();
    expected_ranges.sort_unstable();
    actual_ranges.sort_unstable();
    assert_eq!(actual_ranges, expected_ranges);

    let ordered_source = source(
        declaration,
        r##"[decorate .accent value="#111111"]decorated[/decorate][color value="#222222"]direct[/color][p]"##,
    );
    let ordered_report = lower(&ordered_source).expect("ordered inline styles lower");
    let ordered_colors = ordered_report.line_display_catalog.lines()[0]
        .style_contributions
        .iter()
        .filter(|contribution| {
            contribution.layer == RichTextCascadeLayer::InlineSpan
                && contribution.path == "rich_text.text.color"
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_colors.len(),
        2,
        "contributions: {ordered_colors:#?}"
    );
    assert_eq!(ordered_colors[0].value, "#111111");
    assert!(!ordered_colors[0].active);
    assert_eq!(ordered_colors[1].value, "#222222");
    assert!(ordered_colors[1].active);
}

fn contribution_range(
    contribution: &arcweft_render_text::RichTextStyleContribution,
) -> Option<(usize, usize)> {
    match &contribution.source {
        RichTextSettingSource::SourceFile {
            range: Some(range), ..
        } => Some((range.start, range.end)),
        RichTextSettingSource::SourceFile { range: None, .. }
        | RichTextSettingSource::EngineDefault { .. } => None,
    }
}
