use super::*;
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::value::{RuntimeFieldValue, RuntimeSeq, runtime_sequence_from_literal_values};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision, InlineFailurePolicy};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    RichTextCascadeLayer, RichTextColor, RichTextSettingSource, RichTextStyle,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewId, ViewProgramId, ViewStyleSheetId};

fn test_dialogue_revision() -> DialogueProfileRevision {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-plan-flow-test").expect("document ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("test document");
    let sources =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("test source revision");
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.runtime-plan-flow-test").expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn admitted_options(profile: DialoguePresentationProfile) -> AdmittedRuntimePlanLowerOptions {
    RuntimePlanLowerOptions::default().with_dialogue_profile(profile, test_dialogue_revision())
}

fn lower_runtime_plan(module: &HirModule) -> Result<RuntimePlan, Vec<RuntimePlanLowerError>> {
    super::lower_runtime_plan(
        module,
        &admitted_options(DialoguePresentationProfile::engine_default()),
    )
}

fn lower_runtime_plan_with_stats(
    module: &HirModule,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    super::lower_runtime_plan_with_stats(
        module,
        &admitted_options(DialoguePresentationProfile::engine_default()),
    )
}

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

    optimizer::optimize_flow_ops(&mut ops, &mut stats);

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
fn admitted_dialogue_profile_propagates_typed_owner_style_policy_and_revision() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let profile = DialoguePresentationProfile::new(
        ViewId::try_new("view.MobileDialogue").expect("View ID"),
        Some(ViewStyleSheetId::try_new("style.dialogue.mobile").expect("Style ID")),
        InlineFailurePolicy::Discard,
    );
    let revision = test_dialogue_revision();
    let options =
        RuntimePlanLowerOptions::default().with_dialogue_profile(profile, revision.clone());
    let report = super::lower_runtime_plan_with_stats(&hir, &options)
        .expect("runtime plan lowers with admitted dialogue profile");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display spec");

    assert_eq!(spec.view.as_str(), "view.MobileDialogue");
    assert_eq!(
        spec.profile_style.as_ref().map(ViewStyleSheetId::as_str),
        Some("style.dialogue.mobile")
    );
    assert_eq!(spec.inline_failure, InlineFailurePolicy::Discard);
    assert_eq!(spec.dialogue_revision, revision);
    assert_eq!(report.line_display_catalog.dialogue_revision(), &revision);
}

#[test]
fn speaker_preset_styles_join_dialogue_cascade() {
    let source = r##"
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
        spec.inline_failure,
        InlineFailurePolicy::Fallback { .. }
    ));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::SpeakerPreset
            && contribution.path == "rich_text.text.color"
            && contribution.value == "rgb(\"#404142\")"
            && contribution.active
            && contribution.style_index == Some(2)
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
            && contribution.style_index == Some(3)
    }));
}
