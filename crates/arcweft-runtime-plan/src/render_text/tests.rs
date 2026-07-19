use super::*;
use arcweft_core::plan::RuntimeLineId;
use arcweft_dialogue::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    DialogueHostEvent, FxTarget, Milli, RichTextCascadeLayer, RichTextColor, RichTextControl,
    RichTextFontFamily, RichTextJlreqStrictness, RichTextLayout, RichTextNode, RichTextParam,
    RichTextRubyPosition, RichTextSettingSource, RichTextSourceRange, RichTextStyle,
    RichTextStyleContribution, RichTextTransformOrigin, RichTextWritingMode, RuntimeLineContext,
};

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn lower_dialogue_display_with_module_fx(
    line: RuntimeLineId,
    dialogue: &arcweft_lang_hir::model::HirDialogue,
    defaults: &DialogueDisplayDefaults,
    module: &arcweft_lang_hir::model::HirModule,
) -> arcweft_render_text::LineDisplaySpec {
    let fx = FxCatalog::try_from_module(module).expect("test Fx inventory compiles");
    lower_dialogue_display_with_speaker_presets_and_fx(line, dialogue, defaults, &[], &fx)
        .expect("test dialogue fixture has valid typed Fx")
}

#[test]
fn lowers_full_tag_families_to_render_text_nodes() {
    let parsed = parse_source(
        r##"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice(style=text_style(font=serif, color="#f7e8ff"), inline_error=InlineFailure.fallback("?")): Hello #[player] |[夢](ゆめ)[r][font monospace][em:quiet][voice auto][face smile][at 0.2s call=flash][signal .seen][p]
}
"##,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let spec = lower_dialogue_display(line_id("say.opening.001"), dialogue, &defaults);

    assert_eq!(spec.view.as_str(), "std.view.dialogue");
    assert_eq!(
        spec.base_styles,
        vec![
            RichTextStyle::Font {
                family: RichTextFontFamily::Serif
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 247,
                    green: 232,
                    blue: 255
                }
            }
        ]
    );
    assert_eq!(
        spec.default_inline_failure_policy,
        Some(InlineFailurePolicy::Fallback {
            fallback: InlineFallback::Text {
                text: "?".to_owned(),
                style: FallbackStylePolicy::Plain
            }
        })
    );
    assert!(spec.content.nodes.iter().any(|node| {
        matches!(
            node,
            RichTextNode::Interpolation {
                expr,
                fallback_source,
                on_error: InlineFailurePolicy::Fallback {
                    fallback: InlineFallback::Text { text, .. }
                },
            } if expr == "player"
                && fallback_source == "player"
                && text == "?"
        )
    }));
    assert!(spec.content.nodes.iter().any(|node| {
        matches!(
            node,
            RichTextNode::Ruby { base, ruby } if base == "夢" && ruby == "ゆめ"
        )
    }));
    assert_has_host_event(&spec.content.nodes, |event| {
        matches!(event, DialogueHostEvent::Voice { .. })
    });
    assert_has_host_event(
        &spec.content.nodes,
        |event| matches!(event, DialogueHostEvent::TimedCue { attrs } if attrs == "0.2s call=flash"),
    );
    assert_has_host_event(&spec.content.nodes, |event| {
        matches!(event, DialogueHostEvent::Signal { .. })
    });
    assert!(spec.content.nodes.iter().any(|node| {
        matches!(
            node,
            RichTextNode::StyleStart {
                style: RichTextStyle::Font {
                    family: RichTextFontFamily::Monospace
                }
            }
        )
    }));
}

#[test]
fn canonical_scalar_tags_match_short_and_direct_dialogue_styles() {
    let lower = |content: &str| {
        let source = format!(
            "character @character.alice Alice as alice {{}}\n\nflow @flow.main main {{\n    alice: {content}\n}}\n"
        );
        let parsed = parse_source(&source);
        let hir = lower_to_hir(parsed.typed_tree()).expect("scalar tag fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");
        lower_dialogue_display(
            line_id("say.scalar.tags"),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        )
        .content
        .nodes
    };

    let short_and_direct =
        lower("[color #a8b5ff:夜][font \"Yu Gothic\"]字[/font][size 36]大[/size][p]");
    let canonical = lower(
        "[color value=\"#a8b5ff\"]夜[/color][font value=\"Yu Gothic\"]字[/font][size value=36]大[/size][p]",
    );

    assert_eq!(short_and_direct, canonical);
    assert!(canonical.iter().any(|node| matches!(
        node,
        RichTextNode::StyleStart {
            style: RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 168,
                    green: 181,
                    blue: 255
                }
            }
        }
    )));
    assert!(canonical.iter().any(|node| matches!(
        node,
        RichTextNode::StyleStart {
            style: RichTextStyle::Font {
                family: RichTextFontFamily::Named { name }
            }
        } if name == "Yu Gothic"
    )));
    assert!(canonical.iter().any(|node| matches!(
        node,
        RichTextNode::StyleStart {
            style: RichTextStyle::Size {
                points: Some(36),
                raw
            }
        } if raw == "36"
    )));
}

#[test]
fn lowers_typed_dialogue_wait_and_rejects_invalid_duration() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[w 0.5s][speed fast]B[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("valid wait fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let spec = lower_dialogue_display(line_id("say.wait.valid"), dialogue, &defaults);

    assert!(spec.content.nodes.iter().any(|node| matches!(
        node,
        RichTextNode::Control {
            control: RichTextControl::TimedWait {
                duration_millis: 500
            }
        }
    )));
    assert!(spec.content.nodes.iter().any(|node| matches!(
        node,
        RichTextNode::StyleStart {
            style: RichTextStyle::Speed { value }
        } if value == "56"
    )));

    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[w]B[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree())
        .expect("invalid wait remains available to lowering diagnostics");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let error = lower_dialogue_display_with_speaker_presets(
        line_id("say.wait.invalid"),
        dialogue,
        &defaults,
        &[],
    )
    .expect_err("invalid wait must not become a zero-duration control");

    assert!(error.message().contains("requires a duration"));

    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[speed warp]B[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree())
        .expect("invalid speed remains available to lowering diagnostics");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let error = lower_dialogue_display_with_speaker_presets(
        line_id("say.speed.invalid"),
        dialogue,
        &defaults,
        &[],
    )
    .expect_err("invalid speed must not become a default-rate style");

    assert!(error.message().contains("not a supported name"));
}

fn assert_has_host_event(nodes: &[RichTextNode], predicate: impl Fn(&DialogueHostEvent) -> bool) {
    assert!(nodes.iter().any(|node| match node {
        RichTextNode::HostEvent { event } => predicate(event),
        _ => false,
    }));
}

#[test]
fn inferred_dot_builtin_without_attrs_lowers_to_typed_fx_application() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.sparkle]BC[/]D[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.001"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
        &hir,
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let effect_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .expect("effect text run");
    let application = effect_run
        .presentation
        .fx
        .first()
        .expect("typed Fx presentation");

    assert_eq!(application.definition().package(), "arcweft.builtin");
    assert!(
        application
            .definition()
            .function()
            .starts_with("rich_text.sparkle.")
    );
    assert!(application.parameters().is_empty());
    let plain_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "D")
        })
        .expect("plain text run after inferred close");
    assert!(plain_run.presentation.fx.is_empty());
}

#[test]
fn host_event_phase_effect_lowers_to_typed_host_event() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[effect .wave phase=host_event amp=4px target=glyph]BC[/effect][.host id=sparkle phase=host_event channel=debug]DE[/][p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.host_event.effect"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");

    assert_eq!(frame.text, "ABCDE");
    assert!(frame.host_events.iter().any(|event| {
        matches!(
            event,
            DialogueHostEvent::Effect { id, attrs }
                if id == "wave"
                    && attrs.contains("phase=host_event")
                    && attrs.contains("amp=4px")
        )
    }));
    assert!(frame.host_events.iter().any(|event| {
        matches!(
            event,
            DialogueHostEvent::Effect { id, attrs }
                if id == "sparkle"
                    && attrs.contains("phase=host_event")
                    && attrs.contains("channel=debug")
        )
    }));
    assert!(
        frame
            .display_map
            .text_runs
            .iter()
            .all(|run| run.presentation.fx.is_empty()),
        "host_event phase effects should not become visual presentation effects: {:#?}",
        frame.display_map.text_runs
    );
}

#[test]
fn hard_break_before_styled_interpolation_preserves_value_run() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Captured[r][effect .wave amp=5px target=content phase=glyph_transform][color #ff4050][strong][em][size 38]#[brief][/size][/em][/strong][/color][/effect]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.styled_interpolation_after_break"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
        &hir,
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::new(vec![
            arcweft_core::value::RuntimeBinding {
                name: "brief".to_owned(),
                value: arcweft_core::value::RuntimeValue::String("Idea42".to_owned()),
            },
        ]))
        .expect("rich text frame resolves");

    assert_eq!(frame.text, "Captured\nIdea42");
    let brief_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "Idea42")
        })
        .expect("brief interpolation run");
    assert!(brief_run.presentation.italic);
    assert_eq!(brief_run.presentation.fx.len(), 1);
    assert!(
        brief_run.presentation.fx[0]
            .definition()
            .function()
            .starts_with("rich_text.wave.")
    );
    assert!(
        brief_run
            .styles
            .iter()
            .any(|style| matches!(style, RichTextStyle::Strong { .. }))
    );
}

#[test]
fn explicit_object_tag_lowers_text_proxy_metadata_to_presentation() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[object .hotspot type=KeywordHit role=keyword depth=4 hit=true channel=choice]BC[/object]D[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.object.proxy"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let object_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .expect("object proxy text run");
    let proxy = object_run
        .presentation
        .object_proxies
        .first()
        .expect("object proxy presentation");

    assert_eq!(proxy.id, "hotspot");
    assert!(proxy.declaration.is_none());
    assert_eq!(proxy.type_name.as_deref(), Some("KeywordHit"));
    assert_eq!(proxy.role.as_deref(), Some("keyword"));
    assert_eq!(proxy.depth, Some(Milli(4000)));
    assert!(proxy.hit_test);
    assert_eq!(
        proxy.params.get("channel"),
        Some(&RichTextParam::Raw {
            value: "choice".to_owned()
        })
    );
    assert!(
        !proxy.params.contains_key("type") && !proxy.params.contains_key("depth"),
        "proxy metadata keys should not be forwarded as custom params"
    );
    let plain_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "D")
        })
        .expect("plain text run after object close");
    assert!(plain_run.presentation.object_proxies.is_empty());
}

#[test]
fn text_proxy_struct_attribute_supplies_object_proxy_defaults() {
    let parsed = parse_source(
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[object .hotspot type=KeywordHit]BC[/object]D[p]
}
"#,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.object.proxy.defaults"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let proxy = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .and_then(|run| run.presentation.object_proxies.first())
        .expect("object proxy presentation");

    assert_eq!(proxy.id, "hotspot");
    assert_eq!(
        proxy.declaration.as_ref().map(|declaration| (
            declaration.struct_name.as_str(),
            declaration.attribute.as_str()
        )),
        Some(("KeywordHit", "text_proxy"))
    );
    assert_eq!(proxy.type_name.as_deref(), Some("KeywordHit"));
    assert_eq!(proxy.role.as_deref(), Some("keyword"));
    assert_eq!(proxy.depth, Some(Milli(4000)));
    assert!(proxy.hit_test);
    assert_eq!(
        proxy.params.get("channel"),
        Some(&RichTextParam::Raw {
            value: "choice".to_owned()
        })
    );
}

#[test]
fn nested_text_proxy_struct_attributes_accumulate_with_inline_overrides() {
    let parsed = parse_source(
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=false, depth=2, layer=view)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[object .hotspot type=KeywordHit channel=inventory][object .hover type=HoverHit depth=7 hit=true layer=hud tone=alert]BC[/object][/object]D[p]
}
"#,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.object.proxy.nested"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let object_proxies = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .map(|run| run.presentation.object_proxies.as_slice())
        .expect("nested object proxy text run");
    let [keyword, hover] = object_proxies else {
        panic!("nested object run should carry two proxies: {object_proxies:?}");
    };

    assert_eq!(keyword.id, "hotspot");
    assert_eq!(
        keyword.declaration.as_ref().map(|declaration| (
            declaration.struct_name.as_str(),
            declaration.attribute.as_str()
        )),
        Some(("KeywordHit", "text_proxy"))
    );
    assert_eq!(keyword.type_name.as_deref(), Some("KeywordHit"));
    assert_eq!(keyword.role.as_deref(), Some("keyword"));
    assert_eq!(keyword.depth, Some(Milli(4000)));
    assert!(keyword.hit_test);
    assert_eq!(
        keyword.params.get("channel"),
        Some(&RichTextParam::Raw {
            value: "inventory".to_owned()
        })
    );

    assert_eq!(hover.id, "hover");
    assert_eq!(
        hover.declaration.as_ref().map(|declaration| (
            declaration.struct_name.as_str(),
            declaration.attribute.as_str()
        )),
        Some(("HoverHit", "text_proxy"))
    );
    assert_eq!(hover.type_name.as_deref(), Some("HoverHit"));
    assert_eq!(hover.role.as_deref(), Some("hover"));
    assert_eq!(hover.layer.as_deref(), Some("hud"));
    assert_eq!(hover.depth, Some(Milli(7000)));
    assert!(hover.hit_test);
    assert_eq!(
        hover.params.get("tone"),
        Some(&RichTextParam::Raw {
            value: "alert".to_owned()
        })
    );
    assert!(
        !hover.params.contains_key("type")
            && !hover.params.contains_key("hit")
            && !hover.params.contains_key("layer"),
        "proxy metadata keys should not be forwarded as custom params"
    );
}

#[test]
fn inferred_text_proxy_struct_shorthand_lowers_to_object_proxy() {
    let parsed = parse_source(
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=false, depth=2, layer=view)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.hotspot type=KeywordHit channel=inventory][.HoverHit depth=7 hit=true tone=alert]BC[/][/][.sparkle amp=2px]FX[/][p]
}
"#,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.object.proxy.inferred"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
        &hir,
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let object_proxies = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .map(|run| run.presentation.object_proxies.as_slice())
        .expect("inferred object proxy text run");
    let [keyword, hover] = object_proxies else {
        panic!("inferred proxy run should carry two proxies: {object_proxies:?}");
    };

    assert_eq!(keyword.id, "hotspot");
    assert_eq!(
        keyword.declaration.as_ref().map(|declaration| (
            declaration.struct_name.as_str(),
            declaration.attribute.as_str()
        )),
        Some(("KeywordHit", "text_proxy"))
    );
    assert_eq!(keyword.type_name.as_deref(), Some("KeywordHit"));
    assert_eq!(keyword.role.as_deref(), Some("keyword"));
    assert_eq!(keyword.depth, Some(Milli(4000)));
    assert!(keyword.hit_test);
    assert_eq!(
        keyword.params.get("channel"),
        Some(&RichTextParam::Raw {
            value: "inventory".to_owned()
        })
    );

    assert_eq!(hover.id, "HoverHit");
    assert_eq!(
        hover.declaration.as_ref().map(|declaration| (
            declaration.struct_name.as_str(),
            declaration.attribute.as_str()
        )),
        Some(("HoverHit", "text_proxy"))
    );
    assert_eq!(hover.type_name.as_deref(), Some("HoverHit"));
    assert_eq!(hover.role.as_deref(), Some("hover"));
    assert_eq!(hover.layer.as_deref(), Some("view"));
    assert_eq!(hover.depth, Some(Milli(7000)));
    assert!(hover.hit_test);
    assert_eq!(
        hover.params.get("tone"),
        Some(&RichTextParam::Raw {
            value: "alert".to_owned()
        })
    );

    assert_run_has_fx_without_object_proxy(&frame, "FX", "sparkle");
}

fn assert_run_has_fx_without_object_proxy(
    frame: &arcweft_render_text::LineDisplayFrame,
    text: &str,
    effect_id: &str,
) {
    let effect_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|run_text| run_text == text)
        })
        .expect("typed Fx run remains distinct from object proxies");
    assert!(effect_run.presentation.object_proxies.is_empty());
    assert!(effect_run.presentation.fx.iter().any(|application| {
        application
            .definition()
            .function()
            .starts_with(&format!("rich_text.{effect_id}."))
    }));
}

#[test]
fn rich_text_proxy_struct_attribute_supplies_object_proxy_defaults() {
    let parsed = parse_source(
        r#"
#[rich_text_proxy(kind="quest", default_hit=true, depth=6, layer=hud, channel=quest)]
pub struct QuestHit {
    channel: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.QuestHit state=active]BC[/]D[p]
}
"#,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.object.proxy.rich_text_attribute"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let proxy = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .and_then(|run| run.presentation.object_proxies.first())
        .expect("rich_text_proxy presentation");

    assert_eq!(proxy.id, "QuestHit");
    assert_eq!(
        proxy.declaration.as_ref().map(|declaration| (
            declaration.struct_name.as_str(),
            declaration.attribute.as_str()
        )),
        Some(("QuestHit", "rich_text_proxy"))
    );
    assert_eq!(proxy.type_name.as_deref(), Some("QuestHit"));
    assert_eq!(proxy.role.as_deref(), Some("quest"));
    assert_eq!(proxy.layer.as_deref(), Some("hud"));
    assert_eq!(proxy.depth, Some(Milli(6000)));
    assert!(proxy.hit_test);
    assert_eq!(
        proxy.params.get("channel"),
        Some(&RichTextParam::Raw {
            value: "quest".to_owned()
        })
    );
    assert_eq!(
        proxy.params.get("state"),
        Some(&RichTextParam::Raw {
            value: "active".to_owned()
        })
    );
}

#[test]
fn presentation_scalar_style_sets_opacity_and_z_index() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.layer hud][.z_index 7][.opacity 0.5][.meta role=caption hover=true weight=2]BC[/][/][/][/]D[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.presentation.scalar"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .expect("presentation scalar run");

    assert_eq!(run.presentation.z_index, 7);
    assert_eq!(run.presentation.opacity, Some(Milli(500)));
    assert_eq!(run.presentation.layer.as_deref(), Some("hud"));
    assert_eq!(
        run.presentation.params.get("role"),
        Some(&RichTextParam::Raw {
            value: "caption".to_owned()
        })
    );
    assert_eq!(
        run.presentation.params.get("hover"),
        Some(&RichTextParam::Bool { value: true })
    );
    assert_eq!(
        run.presentation.params.get("weight"),
        Some(&RichTextParam::Int { value: 2 })
    );
}

#[test]
fn builtin_and_unknown_shorthand_retain_exact_fx_identity() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.sparkle amp=2px target=glyph]BC[/]D[p]
    alice: X[effect .nudge amount=3px]YZ[/effect]Q[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let dialogues = hir
        .flows()
        .first()
        .expect("flow exists")
        .body()
        .iter()
        .filter_map(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .collect::<Vec<_>>();

    let inferred = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.host.inferred"),
        dialogues[0],
        &defaults,
        &hir,
    )
    .resolve_frame(&RuntimeLineContext::default())
    .expect("inferred host frame resolves");
    let inferred_effect = inferred
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            inferred
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .and_then(|run| run.presentation.fx.first())
        .expect("inferred built-in Fx");

    assert_eq!(inferred_effect.definition().package(), "arcweft.builtin");
    assert!(
        inferred_effect
            .definition()
            .function()
            .starts_with("rich_text.sparkle.")
    );

    let explicit = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.host.explicit"),
        dialogues[1],
        &defaults,
        &hir,
    )
    .resolve_frame(&RuntimeLineContext::default())
    .expect("explicit host frame resolves");
    let explicit_effect = explicit
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            explicit
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "YZ")
        })
        .and_then(|run| run.presentation.fx.first())
        .expect("unknown effect retains a typed application");

    assert_eq!(explicit_effect.definition().package(), "arcweft.builtin");
    assert!(
        explicit_effect
            .definition()
            .function()
            .starts_with("rich_text.nudge.")
    );
    assert_ne!(inferred_effect.definition(), explicit_effect.definition());
}

#[test]
fn explicit_effect_selector_end_tag_closes_effect_span() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[effect .shake amp=2px]BC[/shake]D[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let spec = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.effect.end"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
        &hir,
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let plain_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "D")
        })
        .expect("plain text run after explicit selector end");

    assert!(
        plain_run.presentation.fx.is_empty(),
        "explicit selector close leaked Fx into the following run: {plain_run:#?}; document: {:#?}",
        spec.content
    );
}

#[test]
fn rotate_transform_selector_accepts_named_and_positional_angles() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.rotate angle=8deg origin=baseline_start target=glyph]BC[/]D[transform .rotate 10deg origin=glyph_center target=content]EF[/transform]G[p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let spec = lower_dialogue_display(
        line_id("say.rich_text.transform.rotate"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");

    let named_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "BC")
        })
        .expect("named rotate run");
    let positional_run = frame
        .display_map
        .text_runs
        .iter()
        .find(|run| {
            frame
                .text
                .get(run.range.start..run.range.end)
                .is_some_and(|text| text == "EF")
        })
        .expect("positional rotate run");

    let named_transform = named_run
        .presentation
        .transform
        .as_ref()
        .expect("named rotate transform");
    let positional_transform = positional_run
        .presentation
        .transform
        .as_ref()
        .expect("positional rotate transform");

    assert_eq!(named_transform.rotate.degrees, Milli(8000));
    assert_eq!(
        named_transform.origin,
        RichTextTransformOrigin::BaselineStart
    );
    assert_eq!(named_transform.target, FxTarget::Glyph);
    assert_eq!(positional_transform.rotate.degrees, Milli(10000));
    assert_eq!(
        positional_transform.origin,
        RichTextTransformOrigin::GlyphCenter
    );
    assert_eq!(positional_transform.target, FxTarget::Content);
}

#[test]
fn rich_text_defaults_and_line_options_lower_to_ruby_layout() {
    let source = r"
pub dialogue defaults {
    rich_text {
        ruby {
            size = 14px
            gap = 2px
        }
    }
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice(rich_text=rich_text_style(ruby=ruby_style(size=11px, gap=1px))): |[夢](ゆめ)[p]
}
";
    let default_ruby_size_start = source.find("14px").expect("default ruby size literal");
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let spec = lower_dialogue_display(
        line_id("say.rich_text.defaults"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );

    assert!(spec.base_styles.iter().any(|style| {
        matches!(
            style,
            RichTextStyle::Layout {
                layout: RichTextLayout {
                    ruby_font_size: Some(Milli(14000)),
                    ..
                }
            }
        )
    }));
    assert!(spec.base_styles.iter().any(|style| {
        matches!(
            style,
            RichTextStyle::Layout {
                layout: RichTextLayout {
                    ruby_gap: Some(Milli(2000)),
                    ..
                }
            }
        )
    }));
    assert!(spec.base_styles.iter().any(|style| {
        matches!(
            style,
            RichTextStyle::Layout {
                layout: RichTextLayout {
                    ruby_font_size: Some(Milli(11000)),
                    ruby_gap: Some(Milli(1000)),
                    ..
                }
            }
        )
    }));
    assert!(
        spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::DialogueDefaults
                && contribution.path == "rich_text.ruby.size"
                && contribution.value == "14px"
                && !contribution.active
                && contribution.style_index == Some(0)
                && contribution_source_range(contribution)
                    == Some((default_ruby_size_start, default_ruby_size_start + 4))
        }),
        "{:#?}",
        spec.style_contributions
    );
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::LineOptions
            && contribution.path == "rich_text.ruby.size"
            && contribution.value == "11px"
            && contribution.active
            && contribution.style_index == Some(2)
            && matches!(
                contribution.source,
                RichTextSettingSource::SourceFile {
                    range: Some(RichTextSourceRange { .. }),
                    ..
                }
            )
    }));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::DialogueDefaults
            && contribution.path == "rich_text.ruby.size"
            && !contribution.active
            && contribution.shadowed_by.is_some()
    }));
}

#[test]
fn dialogue_display_uses_canonical_defaults_profile_when_multiple_exist() {
    let parsed = parse_source(
        r##"
pub dialogue defaults @dialogue.debug {
    text_color = rgb("#ff0000")
}

pub dialogue defaults {
    text_color = rgb("#101112")
}

pub dialogue defaults @dialogue.mobile {
    text_color = rgb("#00ff00")
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
"##,
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let spec = lower_dialogue_display(
        line_id("say.rich_text.defaults.profile"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );

    assert_eq!(
        spec.base_styles,
        vec![RichTextStyle::Color {
            value: RichTextColor::Rgb {
                red: 16,
                green: 17,
                blue: 18
            }
        }]
    );
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::DialogueDefaults
            && contribution.path == "text_color"
            && contribution.value == "rgb(\"#101112\")"
            && contribution.active
    }));
    assert!(!spec.style_contributions.iter().any(|contribution| {
        contribution.value == "rgb(\"#ff0000\")" || contribution.value == "rgb(\"#00ff00\")"
    }));
}

#[test]
fn inferred_layout_selector_lowers_jlreq_strictness_preset() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=strict]天地。「人[/][p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.002"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let run = frame.display_map.text_runs.first().expect("text run");
    let layout = run.presentation.layout.as_ref().expect("layout");

    assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);
    assert_eq!(layout.jlreq_strictness, RichTextJlreqStrictness::Strict);
}

#[test]
fn inferred_layout_selector_lowers_ruby_under_position() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.ruby_under]|[夢](ゆめ)[/][p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.003"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let ruby = frame
        .display_map
        .ruby_annotations
        .first()
        .expect("ruby annotation");
    let layout = ruby.presentation.layout.as_ref().expect("ruby layout");

    assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);
    assert_eq!(layout.ruby_position, RichTextRubyPosition::Under);
}

#[test]
fn ruby_layout_selector_lowers_typography_attrs() {
    let parsed = parse_source(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.ruby_over ruby_size=11px ruby_gap=1px ruby_overhang=4px ruby_collision_gap=3px]|[夢](ゆめ)[/][p]
}
",
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.004"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("rich text frame resolves");
    let ruby = frame
        .display_map
        .ruby_annotations
        .first()
        .expect("ruby annotation");
    let layout = ruby.presentation.layout.as_ref().expect("ruby layout");

    assert_eq!(layout.ruby_position, RichTextRubyPosition::Over);
    assert_eq!(layout.ruby_font_size, Some(Milli(11000)));
    assert_eq!(layout.ruby_gap, Some(Milli(1000)));
    assert_eq!(layout.ruby_overhang, Some(Milli(4000)));
    assert_eq!(layout.ruby_collision_gap, Some(Milli(3000)));
    assert_eq!(layout.column_gap, Milli(8000));
}

#[test]
fn inline_rich_text_span_contributes_cascade_provenance() {
    let source = r"
pub dialogue defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.ruby_over ruby_size=11px]|[夢](ゆめ)[/][p]
}
";
    let inline_size_start = source.find("11px").expect("inline ruby size literal");
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(
        line_id("say.rich_text.inline"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
    );

    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::InlineSpan
            && contribution.path == "rich_text.ruby.size"
            && contribution.value == "11px"
            && contribution.active
            && contribution_source_range(contribution)
                == Some((inline_size_start, inline_size_start + 4))
    }));
    assert!(spec.style_contributions.iter().any(|contribution| {
        contribution.layer == RichTextCascadeLayer::DialogueDefaults
            && contribution.path == "rich_text.ruby.size"
            && contribution.value == "14px"
            && !contribution.active
            && contribution.shadowed_by.is_some()
    }));
}

#[test]
fn multiline_inline_span_provenance_projects_lf_and_crlf_ranges() {
    let source_lf = "character @character.alice Alice as alice {}\n\nflow @flow.main main {\n    alice:\n        Intro\n        [.ruby_over ruby_size=11px]|[夢](ゆめ)[/][p]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let inline_size_start = source.find("11px").expect("inline ruby size literal");
        let parsed = parse_source(&source);
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            line_id("say.rich_text.multiline_inline"),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        assert!(spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::InlineSpan
                && contribution.path == "rich_text.ruby.size"
                && contribution.value == "11px"
                && contribution_source_range(contribution)
                    == Some((inline_size_start, inline_size_start + 4))
        }));
    }
}

#[test]
fn inline_fx_contribution_preserves_typed_definition_and_authored_range() {
    let source = r#"character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.sparkle seed="contains ] safely" amp=2px]text[/][p]
}
"#;
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");
    let spec = lower_dialogue_display_with_module_fx(
        line_id("say.rich_text.quoted_effect"),
        dialogue,
        &DialogueDisplayDefaults::from_module(&hir),
        &hir,
    );

    let attrs_start = source.find("seed=").expect("effect attributes");
    let attrs_end = source.find("]text").expect("effect opener end");
    let definition = spec
        .style_contributions
        .iter()
        .find(|contribution| {
            contribution.layer == RichTextCascadeLayer::InlineSpan
                && contribution
                    .path
                    .starts_with("rich_text.fx.arcweft.builtin::rich_text.sparkle.")
        })
        .expect("typed definition contribution");
    assert_eq!(
        definition.path.trim_start_matches("rich_text.fx."),
        definition.value
    );
    assert_eq!(
        contribution_source_range(definition),
        Some((attrs_start, attrs_end))
    );
}

#[test]
fn dialogue_display_inherits_global_and_character_style_defaults() {
    let source = r##"
pub dialogue defaults {
    font = serif
    text_color = rgb("#101112")
    inline_error = InlineFailure.fallback("global")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
        inline_error = InlineFailure.discard
    }
}

flow @flow.main main {
    @<character.alice>.say(color=rgb("#303132"))[Hello #[missing][p]]
}
"##;
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(line_id("say.opening.002"), dialogue, &defaults);

    assert_eq!(
        spec.base_styles,
        vec![
            RichTextStyle::Font {
                family: RichTextFontFamily::Serif
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 16,
                    green: 17,
                    blue: 18
                }
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 32,
                    green: 33,
                    blue: 34
                }
            },
            RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 48,
                    green: 49,
                    blue: 50
                }
            }
        ]
    );
    assert_eq!(
        spec.default_inline_failure_policy,
        Some(InlineFailurePolicy::Discard)
    );
    let character_text_color = spec
        .style_contributions
        .iter()
        .find(|contribution| {
            contribution.layer == RichTextCascadeLayer::CharacterDialogueStyle
                && contribution.path == "text_color"
                && contribution.value == "rgb(\"#202122\")"
        })
        .expect("character text color contribution");
    let RichTextSettingSource::SourceFile {
        range: Some(range), ..
    } = &character_text_color.source
    else {
        panic!("character contribution should preserve its source range");
    };
    assert_eq!(source[range.start..range.end].trim(), "rgb(\"#202122\")");
    assert!(spec.content.nodes.iter().any(|node| {
        matches!(
            node,
            RichTextNode::Interpolation {
                expr,
                on_error: InlineFailurePolicy::Discard,
                ..
            } if expr == "missing"
        )
    }));
}

#[test]
fn dialogue_display_uses_character_display_label_for_speaker() {
    let source = r#"
pub character concierge {
    display = "Arcweft Concierge"
}

flow @flow.main main {
    concierge: Welcome back.
}
"#;
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
    let defaults = DialogueDisplayDefaults::from_module(&hir);
    let dialogue = hir
        .flows()
        .first()
        .and_then(|flow| flow.body().first())
        .and_then(|item| match item {
            arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
            _ => None,
        })
        .expect("dialogue item");

    let spec = lower_dialogue_display(line_id("say.opening.display_label"), dialogue, &defaults);
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");

    assert_eq!(spec.callee, "concierge");
    assert_eq!(spec.speaker_label.as_deref(), Some("Arcweft Concierge"));
    assert_eq!(frame.speaker_label.as_deref(), Some("Arcweft Concierge"));
}

fn contribution_source_range(contribution: &RichTextStyleContribution) -> Option<(usize, usize)> {
    match &contribution.source {
        RichTextSettingSource::SourceFile {
            range: Some(range), ..
        } => Some((range.start, range.end)),
        _ => None,
    }
}
