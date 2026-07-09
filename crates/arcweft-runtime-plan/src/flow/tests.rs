use super::*;
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::value::{RuntimeFieldValue, RuntimeSeq, runtime_sequence_from_literal_values};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    InlineFailurePolicy, RichTextCascadeLayer, RichTextColor, RichTextSettingSource, RichTextStyle,
};

#[test]
fn optimizer_rewrites_local_record_field_to_ordinal_projection() {
    let rows = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "score".to_owned(),
                value: RuntimeValue::i64(1),
            },
            RuntimeFieldValue {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(true),
            },
        ]),
        RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "score".to_owned(),
                value: RuntimeValue::i64(2),
            },
            RuntimeFieldValue {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(false),
            },
        ]),
    ]);
    assert!(matches!(
        rows,
        RuntimeValue::Seq(RuntimeSeq::RecordColumns(_))
    ));
    let mut ops = vec![
        FlowOp::Let {
            pattern: RuntimePattern::Ident("rows".to_owned()),
            expr: RuntimeExpr::Value(rows),
        },
        FlowOp::ReturnExpr(RuntimeExpr::Sum {
            source: Box::new(RuntimeExpr::Field {
                target: Box::new(RuntimeExpr::Local("rows".to_owned())),
                field: "score".to_owned(),
            }),
        }),
    ];
    let mut stats = RuntimePlanLowerStats::default();

    optimize_flow_ops(&mut ops, &mut stats);

    assert!(matches!(
        &ops[1],
        FlowOp::ReturnExpr(RuntimeExpr::Sum { source })
            if matches!(
                source.as_ref(),
                RuntimeExpr::ProjectRecord { ordinal: 0, target }
                    if matches!(target.as_ref(), RuntimeExpr::Local(name) if name == "rows")
            )
    ));
}

#[test]
fn value_position_view_handle_lowers_to_create_cleanup_and_release_cancel() {
    let parsed = parse_source(
        r#"
view ModernPanel() {
  Panel {
    Text("Ready")
  }
}

flow main {
  let panel = view(@view:.ModernPanel)
  panel.unmount()
  panel.release()
  return "done"
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let ops = &plan.flows[0].ops;

    assert!(matches!(
        &ops[0],
        FlowOp::Effect(LineEffectRequest::Call(call))
            if call.callee == "presentation.handle.create"
                && call.args.iter().any(|arg| arg == "handle = @handle.main.panel")
                && call.args.iter().any(|arg| arg == "kind = \"view\"")
    ));
    assert!(matches!(
        &ops[1],
        FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
            if key == "handle.main.panel"
                && call.callee == "presentation.handle.dispose"
    ));
    assert!(matches!(
        &ops[2],
        FlowOp::Let { pattern: RuntimePattern::Ident(name), expr: RuntimeExpr::Value(RuntimeValue::String(value)) }
            if name == "panel" && value == "handle.main.panel"
    ));
    assert!(matches!(
        &ops[3],
        FlowOp::Effect(LineEffectRequest::Call(call))
            if call.callee == "presentation.handle.unmount"
                && call.args == ["handle = @handle.main.panel"]
    ));
    assert!(matches!(
        &ops[4],
        FlowOp::Effect(LineEffectRequest::Call(call))
            if call.callee == "presentation.handle.release"
                && call.args == ["handle = @handle.main.panel"]
    ));
    assert!(matches!(
        &ops[5],
        FlowOp::CancelCleanup { key } if key == "handle.main.panel"
    ));
}

#[test]
fn value_position_image_handle_lowers_lifecycle_methods_and_cleanup_cancel() {
    let parsed = parse_source(
        r#"
pub asset bg.card {
  kind = image
  file = "bg/card.png"
}

pub image card {
  asset = @asset:.bg.card
  target = @target.card
  x = 0px
  y = 0px
  width = 10px
  height = 10px
  visible = true
}

flow main {
  let sprite = image(@image.card, visible = false, depth = 42)
  sprite.show()
  sprite.hide()
  sprite.destroy()
  return "done"
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let ops = &plan.flows[0].ops;

    assert!(
        matches!(
            &ops[0],
            FlowOp::Effect(LineEffectRequest::Call(call))
                if call.callee == "presentation.handle.create"
                    && call.args.iter().any(|arg| arg == "handle = @handle.main.sprite")
                    && call.args.iter().any(|arg| arg == "kind = \"image\"")
                    && call.args.iter().any(|arg| arg == "resource = @image.card")
                    && call.args.iter().any(|arg| arg == "visible = false")
                    && call.args.iter().any(|arg| arg == "depth = 42")
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[1],
            FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
                if key == "handle.main.sprite"
                    && call.callee == "presentation.handle.dispose"
        ),
        "{ops:#?}"
    );
    assert!(matches!(
        &ops[2],
        FlowOp::Let { pattern: RuntimePattern::Ident(name), expr: RuntimeExpr::Value(RuntimeValue::String(value)) }
            if name == "sprite" && value == "handle.main.sprite"
    ));
    for (index, callee) in [
        (3, "presentation.handle.show"),
        (4, "presentation.handle.hide"),
        (5, "presentation.handle.destroy"),
    ] {
        assert!(
            matches!(
                &ops[index],
                FlowOp::Effect(LineEffectRequest::Call(call))
                    if call.callee == callee
                        && call.args == ["handle = @handle.main.sprite"]
            ),
            "{ops:#?}"
        );
    }
    assert!(matches!(
        &ops[6],
        FlowOp::CancelCleanup { key } if key == "handle.main.sprite"
    ));
    assert!(matches!(&ops[7], FlowOp::ReturnExpr(_)));
}

#[test]
fn value_position_overlay_handle_lowers_pop_to_dispose_and_cleanup_cancel() {
    let parsed = parse_source(
        r#"
view MenuOverlay() {
  Panel {
    Text("Menu")
  }
}

flow main {
  let overlay_handle = overlay(@view:.MenuOverlay, layer = @layer.overlay)
  overlay_handle.pop()
  return "done"
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let ops = &plan.flows[0].ops;

    assert!(
        matches!(
            &ops[0],
            FlowOp::Effect(LineEffectRequest::Call(call))
                if call.callee == "presentation.handle.create"
                    && call.args.iter().any(|arg| arg == "handle = @handle.main.overlay_handle")
                    && call.args.iter().any(|arg| arg == "kind = \"overlay\"")
                    && call.args.iter().any(|arg| arg == "resource = @view.MenuOverlay")
                    && call.args.iter().any(|arg| arg == "layer = @layer.overlay")
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[1],
            FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
                if key == "handle.main.overlay_handle"
                    && call.callee == "presentation.handle.dispose"
        ),
        "{ops:#?}"
    );
    assert!(matches!(
        &ops[3],
        FlowOp::Effect(LineEffectRequest::Call(call))
            if call.callee == "presentation.handle.dispose"
                && call.args == ["handle = @handle.main.overlay_handle"]
    ));
    assert!(matches!(
        &ops[4],
        FlowOp::CancelCleanup { key } if key == "handle.main.overlay_handle"
    ));
}

#[test]
fn explicit_view_and_image_mount_exprs_lower_to_scoped_handle_create() {
    let parsed = parse_source(
        r#"
pub asset bg.card {
  kind = image
  file = "bg/card.png"
}

pub image card {
  asset = @asset:.bg.card
  target = @target.card
  x = 0px
  y = 0px
  width = 10px
  height = 10px
  visible = true
}

view ModernPanel() {
  Panel {
    Text("Ready")
  }
}

flow main {
  image(@image.card, depth = -1000)
  view(@view:.ModernPanel)
  return "done"
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let ops = &plan.flows[0].ops;

    assert!(
        matches!(
            &ops[0],
            FlowOp::Effect(LineEffectRequest::Call(call))
                if call.callee == "presentation.handle.create"
                    && call.args.iter().any(|arg| arg == "handle = @handle.main.mount.image.image.card")
                    && call.args.iter().any(|arg| arg == "kind = \"image\"")
                    && call.args.iter().any(|arg| arg == "resource = @image.card")
                    && call.args.iter().any(|arg| arg == "depth = -1000")
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[1],
            FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
                if key == "handle.main.mount.image.image.card"
                    && call.callee == "presentation.handle.dispose"
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[2],
            FlowOp::Effect(LineEffectRequest::Call(call))
                if call.callee == "presentation.handle.create"
                    && call.args.iter().any(|arg| arg == "handle = @handle.main.mount.view.view.ModernPanel")
                    && call.args.iter().any(|arg| arg == "kind = \"view\"")
                    && call.args.iter().any(|arg| arg == "resource = @view.ModernPanel")
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[3],
            FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
                if key == "handle.main.mount.view.view.ModernPanel"
                    && call.callee == "presentation.handle.dispose"
        ),
        "{ops:#?}"
    );
    assert!(matches!(&ops[4], FlowOp::ReturnExpr(_)));
}

#[test]
fn explicit_menu_and_overlay_mount_exprs_lower_to_scoped_handle_create() {
    let parsed = parse_source(
        r#"
view ModernPanel() {
  Panel {
    Text("Ready")
  }
}

flow main {
  menu(@view:.ModernPanel, layer = @layer.menu)
  overlay(@view:.ModernPanel, layer = @layer.overlay)
  return "done"
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let ops = &plan.flows[0].ops;

    assert!(
        matches!(
            &ops[0],
            FlowOp::Effect(LineEffectRequest::Call(call))
                if call.callee == "presentation.handle.create"
                    && call.args.iter().any(|arg| arg == "handle = @handle.main.mount.menu.view.ModernPanel")
                    && call.args.iter().any(|arg| arg == "kind = \"menu\"")
                    && call.args.iter().any(|arg| arg == "resource = @view.ModernPanel")
                    && call.args.iter().any(|arg| arg == "layer = @layer.menu")
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[1],
            FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
                if key == "handle.main.mount.menu.view.ModernPanel"
                    && call.callee == "presentation.handle.dispose"
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[2],
            FlowOp::Effect(LineEffectRequest::Call(call))
                if call.callee == "presentation.handle.create"
                    && call.args.iter().any(|arg| arg == "handle = @handle.main.mount.overlay.view.ModernPanel")
                    && call.args.iter().any(|arg| arg == "kind = \"overlay\"")
                    && call.args.iter().any(|arg| arg == "resource = @view.ModernPanel")
                    && call.args.iter().any(|arg| arg == "layer = @layer.overlay")
        ),
        "{ops:#?}"
    );
    assert!(
        matches!(
            &ops[3],
            FlowOp::RegisterCleanup { key, effect: LineEffectRequest::Call(call) }
                if key == "handle.main.mount.overlay.view.ModernPanel"
                    && call.callee == "presentation.handle.dispose"
        ),
        "{ops:#?}"
    );
    assert!(matches!(&ops[4], FlowOp::ReturnExpr(_)));
}

#[test]
fn runtime_plan_options_select_dialogue_defaults_profile() {
    let parsed = parse_source(
        r##"
pub dialogue defaults @dialogue.defaults {
    text_color = rgb("#101112")
}

pub dialogue defaults @dialogue:.defaults.mobile {
    text_color = rgb("#202122")
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
"##,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let report = lower_runtime_plan_with_stats_and_options(
        &hir,
        &RuntimePlanLowerOptions::default().with_dialogue_defaults("dialogue.defaults.mobile"),
    )
    .expect("runtime plan lowers with selected dialogue defaults");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display spec");

    assert_eq!(
        spec.base_styles,
        vec![RichTextStyle::Color {
            value: RichTextColor::Rgb {
                red: 32,
                green: 33,
                blue: 34
            }
        }]
    );
}

#[test]
fn speaker_preset_styles_join_dialogue_cascade() {
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    text_color = rgb("#101112")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
    }
}

flow @flow.main main {
    let alice_side = alice(rich_text=rich_text_style(text=text_style(color=rgb("#303132"))), inline_error=InlineFailure.fallback("preset"))
    let alice_worried = alice_side(rich_text=rich_text_style(text=text_style(color=rgb("#404142"))))

    alice_worried(text_color=rgb("#505152")): Hello #[missing][p]
}
"##;
    let preset_value = r##"rich_text_style(text=text_style(color=rgb("#404142")))"##;
    let preset_value_start = source
        .find(preset_value)
        .expect("preset value is present in fixture");
    let preset_value_end = preset_value_start + preset_value.len();
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let report = lower_runtime_plan_with_stats(&hir).expect("runtime plan lowers");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display spec");

    assert_eq!(
        spec.base_styles,
        vec![
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 16,
                    green: 17,
                    blue: 18,
                }
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 32,
                    green: 33,
                    blue: 34,
                }
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 48,
                    green: 49,
                    blue: 50,
                }
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 64,
                    green: 65,
                    blue: 66,
                }
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 80,
                    green: 81,
                    blue: 82,
                }
            },
        ]
    );
    assert!(matches!(
        spec.default_inline_failure_policy,
        Some(InlineFailurePolicy::Fallback { .. })
    ));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::SpeakerPreset
            && contribution.path == "rich_text.text.color"
            && contribution.value == "rgb(\"#404142\")"
            && contribution.active
            && contribution.style_index == Some(3)
            && matches!(
                &contribution.source,
                RichTextSettingSource::SourceFile {
                    range: Some(range),
                    ..
                } if range.start == preset_value_start && range.end == preset_value_end
            )
    }));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::SpeakerPreset
            && contribution.path == "rich_text.text.color"
            && contribution.value == "rgb(\"#303132\")"
            && !contribution.active
            && contribution.shadowed_by.is_some()
    }));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::LineOptions
            && contribution.path == "text_color"
            && contribution.value == "rgb(\"#505152\")"
            && contribution.active
            && contribution.style_index == Some(4)
    }));
}

#[test]
fn textbox_theme_styles_join_dialogue_cascade() {
    let parsed = parse_source(
        r##"
pub dialogue defaults @dialogue.defaults {
    window = @textbox.phone_message
}

pub textbox @textbox.phone_message PhoneMessageBox {
    rich_text {
        text {
            color = rgb("#303132")
        }
    }
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice(rich_text=rich_text_style(text=text_style(color=rgb("#404142")))): Hello[p]
}
"##,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let report = lower_runtime_plan_with_stats(&hir).expect("runtime plan lowers");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display spec");

    assert_eq!(spec.window.as_deref(), Some("textbox.phone_message"));
    assert!(spec.base_styles.iter().any(|style| {
        matches!(
            style,
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 48,
                    green: 49,
                    blue: 50,
                }
            }
        )
    }));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::TextBoxTheme
            && contribution.path == "rich_text.text.color"
            && contribution.value == "rgb(\"#303132\")"
            && !contribution.active
            && contribution.shadowed_by.is_some()
    }));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::LineOptions
            && contribution.path == "rich_text.text.color"
            && contribution.value == "rgb(\"#404142\")"
            && contribution.active
    }));
}

#[test]
fn runtime_plan_reports_ambiguous_dialogue_defaults_profiles() {
    let parsed = parse_source(
        r##"
pub dialogue defaults @dialogue.defaults.debug {
    text_color = rgb("#101112")
}

pub dialogue defaults @dialogue.defaults.mobile {
    text_color = rgb("#202122")
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
"##,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let errors = lower_runtime_plan_with_stats(&hir)
        .expect_err("ambiguous dialogue defaults should fail runtime lowering");

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("multiple dialogue defaults profiles")),
        "{errors:#?}"
    );
}

#[test]
fn runtime_plan_reports_missing_selected_dialogue_defaults_profile() {
    let parsed = parse_source(
        r##"
pub dialogue defaults @dialogue.defaults {
    text_color = rgb("#101112")
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
"##,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let errors = lower_runtime_plan_with_stats_and_options(
        &hir,
        &RuntimePlanLowerOptions::default().with_dialogue_defaults("dialogue.defaults.mobile"),
    )
    .expect_err("missing selected dialogue defaults should fail runtime lowering");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("dialogue.defaults.mobile")),
        "{errors:#?}"
    );
}

#[test]
fn agent_controller_plan_lowers_expect_and_deny_to_evaluated_tasks() {
    let parsed = parse_source(
        r#"
#[agent(version = 1)]
agent @agent.assertions assertions()
effects {}
{
    let accepted = true
    let denied = false
    expect(accepted, message = "accepted should be true")
    deny(denied)
}
"#,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("agent fixture lowers");
    let agent = hir.agents().first().expect("agent exists");
    let report = lower_agent_controller_plan_with_stats(&hir, agent)
        .expect("agent assertions lower to runtime plan");
    let assertion_tasks = report.plan.flows[0]
        .ops
        .iter()
        .filter_map(|op| match op {
            FlowOp::Await {
                binding: None,
                target,
                ..
            } if target.request.capability.0 == "agent" => {
                Some((target.request.operation.as_str(), target.request.args.len()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        assertion_tasks.contains(&("expect", 2)),
        "{assertion_tasks:?}"
    );
    assert!(
        assertion_tasks.contains(&("deny", 1)),
        "{assertion_tasks:?}"
    );
}
