use std::collections::BTreeMap;

use arcweft_dialogue::rich_text::{RichTextTagFamily, canonical_tag_name, inferred_tag_family};
use arcweft_lang_hir::syntax::ast::dialogue::{DialogueTag, DialogueToken};
use arcweft_render_text::{
    DialogueHostEvent, FxTarget, InlineFailurePolicy, Milli, RichTextAngle, RichTextControl,
    RichTextInlineDirection, RichTextJlreqStrictness, RichTextLayout, RichTextNode,
    RichTextObjectProxy, RichTextPresentationStyle, RichTextRubyPosition, RichTextStyle,
    RichTextTransform, RichTextTransformOrigin, RichTextVec2, RichTextVerticalLatinMode,
    RichTextWritingMode, parse_milli_token, parse_z_index_token,
};

use crate::{errors::RuntimePlanLowerError, labels::expr_label};

use super::attrs::{
    angle_from_attrs, milli_attr, param_from_value, parse_attrs, parse_typed_attrs,
    transform_angle_attr, trim_quotes, truthy_attr,
};
use super::defaults::TextProxyTypeDefaults;
use super::inline_failure::{inline_failure_policy, inline_fallback_source_label};

pub(crate) fn lower_dialogue_token_parts(
    token: &DialogueToken,
    default_inline_failure_policy: Option<&InlineFailurePolicy>,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    Ok(match token {
        DialogueToken::Text(text) => vec![RichTextNode::Text { text: text.clone() }],
        DialogueToken::Raw(text) => {
            vec![RichTextNode::Control {
                control: RichTextControl::Raw { text: text.clone() },
            }]
        }
        DialogueToken::Tag(tag) => return lower_tag(tag, text_proxies),
        DialogueToken::InferredTag(tag) => return lower_inferred_tag(tag, text_proxies),
        DialogueToken::Mark(mark) => {
            vec![RichTextNode::Control {
                control: RichTextControl::Mark {
                    name: mark.name().to_owned(),
                },
            }]
        }
        DialogueToken::EndTag(end) => {
            vec![RichTextNode::StyleEnd {
                name: canonical_tag_name(end.name()).to_owned(),
            }]
        }
        DialogueToken::InferredEndTag => {
            vec![RichTextNode::StyleEnd {
                name: "/".to_owned(),
            }]
        }
        DialogueToken::Expr(expr) => {
            vec![RichTextNode::Interpolation {
                expr: expr_label(expr.expr()),
                fallback_source: inline_fallback_source_label(expr.expr()),
                on_error: inline_failure_policy(expr.expr(), default_inline_failure_policy),
            }]
        }
        DialogueToken::Ruby { base, ruby } => {
            vec![RichTextNode::Ruby {
                base: base.clone(),
                ruby: ruby.clone(),
            }]
        }
        DialogueToken::Escape(ch) => vec![RichTextNode::Text {
            text: ch.to_string(),
        }],
    })
}

fn lower_tag(
    tag: &DialogueTag,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    Ok(match tag.name() {
        "p" => vec![RichTextNode::Control {
            control: RichTextControl::Page,
        }],
        "l" => vec![RichTextNode::Control {
            control: RichTextControl::LineWait,
        }],
        "r" | "br" => vec![RichTextNode::Control {
            control: RichTextControl::HardBreak,
        }],
        "w" => vec![RichTextNode::Control {
            control: RichTextControl::TimedWait {
                duration_millis: tag
                    .wait_duration()
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                    .millis(),
            },
        }],
        "clear" | "er" | "cm" => vec![RichTextNode::Control {
            control: RichTextControl::Clear,
        }],
        "reset" => vec![RichTextNode::Control {
            control: RichTextControl::Reset,
        }],
        "speed" => vec![RichTextNode::StyleStart {
            style: RichTextStyle::Speed {
                value: tag
                    .reveal_speed()
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                    .canonical_cps(),
            },
        }],
        "em" | "strong" | "color" | "font" | "size" | "i" | "italic" | "oblique" | "slant" => {
            vec![RichTextNode::StyleStart {
                style: RichTextStyle::from_tag(tag.name(), tag.attrs()),
            }]
        }
        "style" => lower_style_tag(tag),
        "layout" => lower_layout_tag(tag),
        "transform" => return lower_transform_tag(tag),
        "object" => lower_object_tag(tag, text_proxies),
        "effect" | "fx" => return lower_effect_tag(tag),
        "voice" => host_event(DialogueHostEvent::Voice {
            attrs: tag.attrs().to_owned(),
        }),
        "face" => host_event(DialogueHostEvent::Face {
            attrs: tag.attrs().to_owned(),
        }),
        "pose" => host_event(DialogueHostEvent::Pose {
            attrs: tag.attrs().to_owned(),
        }),
        "show" => host_event(DialogueHostEvent::Show {
            attrs: tag.attrs().to_owned(),
        }),
        "hide" => host_event(DialogueHostEvent::Hide {
            attrs: tag.attrs().to_owned(),
        }),
        "move" => host_event(DialogueHostEvent::Move {
            attrs: tag.attrs().to_owned(),
        }),
        "scale" => host_event(DialogueHostEvent::Scale {
            attrs: tag.attrs().to_owned(),
        }),
        "rotate" => host_event(DialogueHostEvent::Rotate {
            attrs: tag.attrs().to_owned(),
        }),
        "anim" => host_event(DialogueHostEvent::Anim {
            attrs: tag.attrs().to_owned(),
        }),
        "shake" => host_event(DialogueHostEvent::Shake {
            attrs: tag.attrs().to_owned(),
        }),
        "at" => host_event(DialogueHostEvent::TimedCue {
            attrs: tag.attrs().to_owned(),
        }),
        "call" => host_event(DialogueHostEvent::Call {
            attrs: tag.attrs().to_owned(),
        }),
        "signal" => host_event(DialogueHostEvent::Signal {
            attrs: tag.attrs().to_owned(),
        }),
        "if" | "else" | "endif" => host_event(DialogueHostEvent::Conditional {
            name: tag.name().to_owned(),
            attrs: tag.attrs().to_owned(),
        }),
        name => vec![RichTextNode::Control {
            control: RichTextControl::Unknown {
                name: name.to_owned(),
                attrs: tag.attrs().to_owned(),
            },
        }],
    })
}

fn lower_inferred_tag(
    tag: &DialogueTag,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    let selector = tag.name().trim_start_matches('.');
    if inferred_text_proxy_type(selector, tag.attrs(), text_proxies) {
        return Ok(lower_object_selector(selector, tag.attrs(), text_proxies));
    }
    Ok(match inferred_tag_family(selector, tag.attrs()) {
        Some(RichTextTagFamily::Style) => lower_style_selector(selector, tag.attrs()),
        Some(RichTextTagFamily::Layout) => lower_layout_selector(selector, tag.attrs()),
        Some(RichTextTagFamily::Transform) => {
            return lower_transform_selector(selector, tag.attrs());
        }
        Some(RichTextTagFamily::Effect) => return lower_effect_selector(selector, tag.attrs()),
        Some(RichTextTagFamily::Marker) | None => {
            vec![RichTextNode::Control {
                control: RichTextControl::Mark {
                    name: tag.name().to_owned(),
                },
            }]
        }
    })
}

pub(crate) fn inferred_text_proxy_type(
    selector: &str,
    attrs: &str,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> bool {
    let attrs = parse_attrs(attrs);
    object_proxy_type_name_attr(&attrs)
        .is_some_and(|type_name| text_proxies.contains_key(&type_name))
        || text_proxies.contains_key(selector)
}

fn lower_style_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_style_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_style_selector(selector: &str, attrs: &str) -> Vec<RichTextNode> {
    let style = match selector {
        "italic" | "i" => RichTextStyle::Italic {
            attrs: attrs.to_owned(),
        },
        "oblique" | "slant" => RichTextStyle::Oblique {
            angle: angle_from_attrs(attrs, "deg").unwrap_or_else(|| RichTextAngle {
                degrees: parse_milli_token(attrs),
            }),
            raw: attrs.to_owned(),
        },
        "opacity" | "alpha" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: Some(parse_milli_token(&style_scalar_attr(attrs, "opacity"))),
                layer: None,
                params: BTreeMap::new(),
                z_index: None,
            },
        },
        "layer" | "object_layer" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: None,
                layer: style_layer_attr(attrs),
                params: BTreeMap::new(),
                z_index: None,
            },
        },
        "meta" | "metadata" | "data" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: None,
                layer: None,
                params: parse_typed_attrs(attrs)
                    .into_iter()
                    .map(|(key, value)| (key, param_from_value(&value)))
                    .collect(),
                z_index: None,
            },
        },
        "z" | "z_index" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: None,
                layer: None,
                params: BTreeMap::new(),
                z_index: parse_z_index_token(&style_scalar_attr(attrs, "z_index")),
            },
        },
        _ => RichTextStyle::Unknown {
            name: "style".to_owned(),
            attrs: attrs.to_owned(),
        },
    };
    vec![RichTextNode::StyleStart { style }]
}

fn style_scalar_attr(attrs: &str, preferred: &str) -> String {
    let parsed = parse_attrs(attrs);
    parsed
        .get(preferred)
        .or_else(|| match preferred {
            "opacity" => parsed.get("alpha"),
            "z_index" => parsed.get("z"),
            "layer" => parsed.get("object_layer"),
            _ => None,
        })
        .or_else(|| parsed.get("value"))
        .or_else(|| parsed.get("amount"))
        .map_or_else(|| attrs.to_owned(), ToOwned::to_owned)
}

fn style_layer_attr(attrs: &str) -> Option<String> {
    let value = style_scalar_attr(attrs, "layer");
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn lower_layout_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_layout_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_layout_selector(selector: &str, attrs: &str) -> Vec<RichTextNode> {
    vec![RichTextNode::StyleStart {
        style: RichTextStyle::Layout {
            layout: layout_from_selector(selector, attrs),
        },
    }]
}

fn lower_transform_tag(tag: &DialogueTag) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_transform_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_transform_selector(
    selector: &str,
    attrs: &str,
) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    Ok(vec![RichTextNode::StyleStart {
        style: RichTextStyle::Transform {
            transform: transform_from_selector(selector, attrs)?,
        },
    }])
}

fn lower_effect_tag(tag: &DialogueTag) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_effect_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_effect_selector(
    selector: &str,
    attrs: &str,
) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
    if effect_selector_is_host_event(attrs) {
        return Ok(host_event(DialogueHostEvent::Effect {
            id: host_event_effect_id(selector, attrs),
            attrs: attrs.trim().to_owned(),
        }));
    }
    Err(RuntimePlanLowerError::new(format!(
        "visual rich-text effect `{selector}` reached descriptor lowering instead of typed Fx expansion"
    )))
}

fn effect_selector_is_host_event(attrs: &str) -> bool {
    parse_attrs(attrs)
        .get("phase")
        .is_some_and(|phase| phase == "host_event")
}

fn host_event_effect_id(selector: &str, attrs: &str) -> String {
    let attrs = parse_attrs(attrs);
    match selector {
        "host" => attrs
            .get("id")
            .or_else(|| attrs.get("effect"))
            .or_else(|| attrs.get("name"))
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| selector.to_owned()),
        "shader" => attrs
            .get("id")
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| selector.to_owned()),
        _ => selector.to_owned(),
    }
}

fn lower_object_tag(
    tag: &DialogueTag,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_object_selector(selector.trim_start_matches('.'), attrs, text_proxies)
}

fn lower_object_selector(
    selector: &str,
    attrs: &str,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    vec![RichTextNode::StyleStart {
        style: RichTextStyle::Object {
            proxy: object_proxy_from_selector(selector, attrs, text_proxies),
        },
    }]
}

fn object_proxy_from_selector(
    selector: &str,
    attrs: &str,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> RichTextObjectProxy {
    let typed_attrs = parse_typed_attrs(attrs);
    let attrs = parse_attrs(attrs);
    let explicit_type_name = object_proxy_type_name_attr(&attrs);
    let defaults = explicit_type_name
        .as_deref()
        .and_then(|type_name| text_proxies.get(type_name))
        .or_else(|| text_proxies.get(selector));
    let id = if selector.is_empty() {
        attrs
            .get("id")
            .or_else(|| attrs.get("name"))
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "text_object".to_owned())
    } else {
        selector.to_owned()
    };
    let mut params = defaults.map_or_else(BTreeMap::new, |defaults| defaults.params.clone());
    params.extend(
        typed_attrs
            .iter()
            .filter(|(key, _)| !is_object_proxy_metadata_attr(key))
            .map(|(key, value)| (key.clone(), param_from_value(value))),
    );
    RichTextObjectProxy {
        id,
        declaration: defaults.map(|defaults| defaults.declaration.clone()),
        type_name: explicit_type_name
            .or_else(|| defaults.map(|defaults| defaults.type_name.clone())),
        role: attrs
            .get("role")
            .or_else(|| attrs.get("kind"))
            .map(|value| trim_quotes(value).to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| defaults.and_then(|defaults| defaults.role.clone())),
        layer: attrs
            .get("layer")
            .or_else(|| attrs.get("object_layer"))
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| defaults.and_then(|defaults| defaults.layer.clone())),
        depth: attrs
            .get("depth")
            .or_else(|| attrs.get("z"))
            .or_else(|| attrs.get("z_index"))
            .map(|value| parse_milli_token(value))
            .or_else(|| defaults.and_then(|defaults| defaults.depth)),
        hit_test: attrs
            .get("hit")
            .or_else(|| attrs.get("hit_test"))
            .map(|value| truthy_attr(value))
            .or_else(|| defaults.and_then(|defaults| defaults.default_hit))
            .unwrap_or(false),
        params,
    }
}

fn object_proxy_type_name_attr(attrs: &BTreeMap<String, String>) -> Option<String> {
    attrs
        .get("type")
        .or_else(|| attrs.get("struct"))
        .or_else(|| attrs.get("proxy"))
        .map(|value| trim_quotes(value).to_owned())
        .filter(|value| !value.is_empty())
}

fn is_object_proxy_metadata_attr(key: &str) -> bool {
    matches!(
        key,
        "id" | "name"
            | "kind"
            | "type"
            | "struct"
            | "proxy"
            | "role"
            | "layer"
            | "object_layer"
            | "depth"
            | "z"
            | "z_index"
            | "hit"
            | "hit_test"
    )
}

pub(crate) fn split_selector_attrs(attrs: &str) -> (&str, &str) {
    let attrs = attrs.trim();
    let mut parts = attrs.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    if first.starts_with('.') {
        (first, parts.next().unwrap_or_default().trim())
    } else {
        ("", attrs)
    }
}

fn layout_from_selector(selector: &str, attrs: &str) -> RichTextLayout {
    let attrs = parse_attrs(attrs);
    let ruby_selector = matches!(
        selector,
        "ruby_over" | "ruby_under" | "ruby_inter_character"
    );
    RichTextLayout {
        writing_mode: match selector {
            "vertical_rl" | "vertical" => RichTextWritingMode::VerticalRl,
            "vertical_lr" => RichTextWritingMode::VerticalLr,
            "horizontal_tb" => RichTextWritingMode::HorizontalTb,
            _ => match attrs.get("mode").map(String::as_str) {
                Some("vertical_rl" | "vertical" | "rl") => RichTextWritingMode::VerticalRl,
                Some("vertical_lr" | "lr") => RichTextWritingMode::VerticalLr,
                _ => RichTextWritingMode::HorizontalTb,
            },
        },
        direction: match attrs
            .get("dir")
            .or_else(|| attrs.get("value"))
            .map(String::as_str)
        {
            Some("ltr") => RichTextInlineDirection::Ltr,
            Some("rtl") => RichTextInlineDirection::Rtl,
            _ => RichTextInlineDirection::Auto,
        },
        vertical_latin: match attrs.get("latin").map(String::as_str) {
            Some("upright") => RichTextVerticalLatinMode::Upright,
            Some("sideways") => RichTextVerticalLatinMode::Sideways,
            _ => RichTextVerticalLatinMode::Mixed,
        },
        ruby_position: match selector {
            "ruby_over" => RichTextRubyPosition::Over,
            "ruby_under" => RichTextRubyPosition::Under,
            "ruby_inter_character" => RichTextRubyPosition::InterCharacter,
            _ => RichTextRubyPosition::Auto,
        },
        jlreq_strictness: jlreq_strictness_attr(&attrs),
        column_gap: column_gap_attr(&attrs, ruby_selector).unwrap_or(Milli(8000)),
        ruby_font_size: ruby_milli_attr(&attrs, ruby_selector, "ruby_size", "size"),
        ruby_gap: ruby_milli_attr(&attrs, ruby_selector, "ruby_gap", "gap"),
        ruby_overhang: attrs
            .get("ruby_overhang")
            .or_else(|| attrs.get("overhang"))
            .map(|value| parse_milli_token(value)),
        ruby_collision_gap: attrs
            .get("ruby_collision_gap")
            .or_else(|| attrs.get("collision_gap"))
            .map(|value| parse_milli_token(value)),
    }
}

fn column_gap_attr(attrs: &BTreeMap<String, String>, ruby_selector: bool) -> Option<Milli> {
    attrs
        .get("column_gap")
        .or_else(|| {
            if ruby_selector {
                None
            } else {
                attrs.get("gap")
            }
        })
        .map(|value| parse_milli_token(value))
}

fn ruby_milli_attr(
    attrs: &BTreeMap<String, String>,
    ruby_selector: bool,
    explicit_name: &str,
    ruby_selector_short_name: &str,
) -> Option<Milli> {
    attrs
        .get(explicit_name)
        .or_else(|| {
            if ruby_selector {
                attrs.get(ruby_selector_short_name)
            } else {
                None
            }
        })
        .map(|value| parse_milli_token(value))
}

fn jlreq_strictness_attr(attrs: &BTreeMap<String, String>) -> RichTextJlreqStrictness {
    match attrs
        .get("jlreq")
        .or_else(|| attrs.get("strictness"))
        .or_else(|| attrs.get("kinsoku"))
        .map(String::as_str)
    {
        Some("loose") => RichTextJlreqStrictness::Loose,
        Some("normal") => RichTextJlreqStrictness::Normal,
        Some("strict") => RichTextJlreqStrictness::Strict,
        _ => RichTextJlreqStrictness::Auto,
    }
}

fn transform_from_selector(
    selector: &str,
    attrs: &str,
) -> Result<RichTextTransform, RuntimePlanLowerError> {
    let raw_attrs = attrs;
    let attrs = parse_attrs(attrs);
    let mut transform = RichTextTransform::default();
    match selector {
        "offset" | "pos" => {
            transform.translate = RichTextVec2::new(
                milli_attr(&attrs, "x").unwrap_or_default(),
                milli_attr(&attrs, "y").unwrap_or_default(),
            );
        }
        "rotate" => {
            transform.rotate = transform_angle_attr(&attrs, raw_attrs).unwrap_or_default();
            transform.origin = RichTextTransformOrigin::Center;
        }
        "scale" => {
            transform.scale = RichTextVec2::new(
                milli_attr(&attrs, "x").unwrap_or(Milli::ONE),
                milli_attr(&attrs, "y").unwrap_or(Milli::ONE),
            );
            transform.origin = RichTextTransformOrigin::Center;
        }
        "skew" => {
            transform.skew = RichTextVec2::new(
                milli_attr(&attrs, "x").unwrap_or_default(),
                milli_attr(&attrs, "y").unwrap_or_default(),
            );
        }
        _ => {}
    }
    transform.target = target_attr(&attrs)?;
    if let Some(origin) = transform_origin_attr(&attrs)? {
        transform.origin = origin;
    }
    Ok(transform)
}

fn target_attr(attrs: &BTreeMap<String, String>) -> Result<FxTarget, RuntimePlanLowerError> {
    match attrs.get("target").map(String::as_str) {
        None | Some("content") => Ok(FxTarget::Content),
        Some("node") => Ok(FxTarget::Node),
        Some("background") => Ok(FxTarget::Background),
        Some("line") => Ok(FxTarget::Line),
        Some("glyph") => Ok(FxTarget::Glyph),
        Some("viewport") => Ok(FxTarget::Viewport),
        Some(target) => Err(RuntimePlanLowerError::new(format!(
            "unknown rich-text transform target `{target}`; expected node, content, background, line, glyph, or viewport"
        ))),
    }
}

fn transform_origin_attr(
    attrs: &BTreeMap<String, String>,
) -> Result<Option<RichTextTransformOrigin>, RuntimePlanLowerError> {
    let Some(origin) = attrs.get("origin").map(String::as_str) else {
        return Ok(None);
    };
    match origin {
        "baseline_start" | "start" => Ok(Some(RichTextTransformOrigin::BaselineStart)),
        "baseline_center" => Ok(Some(RichTextTransformOrigin::BaselineCenter)),
        "center" => Ok(Some(RichTextTransformOrigin::Center)),
        "glyph_center" | "glyph" => Ok(Some(RichTextTransformOrigin::GlyphCenter)),
        origin => Err(RuntimePlanLowerError::new(format!(
            "unknown rich-text transform origin `{origin}`"
        ))),
    }
}

fn host_event(event: DialogueHostEvent) -> Vec<RichTextNode> {
    vec![RichTextNode::HostEvent { event }]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::target_attr;

    #[test]
    fn transform_target_values_outside_the_current_set_fail() {
        let attrs = BTreeMap::from([("target".to_owned(), "elsewhere".to_owned())]);
        let error = target_attr(&attrs).expect_err("unknown target value must diagnose");
        assert!(
            error
                .to_string()
                .contains("unknown rich-text transform target")
        );
    }
}
